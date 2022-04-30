from __future__ import annotations

import argparse
import configparser
import contextlib
import functools
import io
import json
import os.path
import subprocess
import sys
import time
import traceback
import urllib.parse
from datetime import datetime, timezone
from multiprocessing.pool import ThreadPool
from pathlib import Path
from tarfile import TarFile
from types import TracebackType
from typing import (
  IO, TYPE_CHECKING, Any, Callable, Generator, Iterable, Mapping, NoReturn, TypedDict, TypeVar,
  cast, overload
)

try:
  from tqdm import tqdm
except ImportError:
  tqdm = None

from . import BINARY_NAME, gettext_po, utils
from .archives import ArchiveAdapter, TarGzArchiveAdapter, ZipArchiveAdapter
from .cli import ArgumentError, ArgumentNamespace, ArgumentParser, ArgumentParserExit
from .http_client import HTTPClient, HTTPRequest, HTTPResponse

_T = TypeVar("_T")
_UNSET: Any = object()


def run_main() -> NoReturn:
  bin_name = BINARY_NAME
  try:
    bin_name = os.path.basename(sys.argv[0])
    _Main().main(sys.argv[1:], bin_name)
  except ArgumentError as err:
    parser: ArgumentParser = getattr(err, "parser")
    parser.exit_on_error = True
    parser.error(str(err))
  except ArgumentParserExit as err:
    sys.exit(err.code)
  except Exception:
    print(f"{bin_name}: error:\n\n{traceback.format_exc()}", file=sys.stderr)
    sys.exit(1)
  else:
    sys.exit(0)


def main(args: list[str], bin_name: str = BINARY_NAME) -> None:
  return _Main().main(args, bin_name)


class _Main:

  def main(self, raw_args: list[str], bin_name: str) -> None:
    self.arg_parser: ArgumentParser = self.build_arg_parser(bin_name)

    try:
      self.cli_args: ArgumentNamespace = self.arg_parser.parse_args(raw_args)
    except (ArgumentError, ArgumentParserExit) as err:
      setattr(err, "parser", self.arg_parser)
      raise err

    self.tqdm: type[_tqdm_fallback] = _tqdm_fallback
    if tqdm is not None and self.cli_args.progress_bars:
      self.tqdm = cast("type[_tqdm_fallback]", tqdm)

    with self.wrap_print_for_tqdm():
      start_time = time.perf_counter()

      self.project: Project = Project(self.cli_args.project)
      for path in [
        self.project.work_dir,
        self.project.download_dir,
        self.project.components_dir,
        self.project.dist_archives_dir,
      ]:
        path.mkdir(exist_ok=True, parents=True)

      self.http_client: HTTPClient = HTTPClient(
        network_timeout=self.project.get_conf("project", "network_timeout", int, fallback=None),
        network_max_retries=self.project.get_conf(
          "project", "network_max_retries", int, fallback=None
        ),
        network_retry_wait=self.project.get_conf(
          "project", "network_retry_wait", int, fallback=None
        ),
      )

      self.weblate_client: WeblateClient = WeblateClient(
        http_client=self.http_client,
        root_url=self.project.get_conf("weblate", "root_url"),
        auth_token=self.project.get_conf("weblate", "auth_token", fallback=None),
        project_name=self.project.get_conf("weblate", "project"),
      )

      self.cli_args.command_fn()

      elapsed_time = time.perf_counter() - start_time
      print("Done in {:.2f}s".format(elapsed_time))

  def build_arg_parser(self, bin_name: str) -> ArgumentParser:
    parser = ArgumentParser(prog=bin_name, exit_on_error=False)

    parser.add_argument("--project", type=Path, default=Path.cwd())
    parser.add_argument("--progress-bars", action=argparse.BooleanOptionalAction, default=True)

    # Empty help strings are necessary for subparsers to show up in help.
    subparsers = parser.add_subparsers(required=True, metavar="COMMAND")

    subparser = subparsers.add_parser("download", help="")
    subparser.set_defaults(command_fn=self.cmd_download)

    subparser = subparsers.add_parser("make-dist", help="")
    subparser.set_defaults(command_fn=self.cmd_make_dist)

    return parser

  def cmd_download(self) -> None:

    local_data_repo_path = self.project.get_conf(
      "weblate", "local_data_repo_path", self.project.get_conf_path, fallback=None
    )
    components_to_compile: dict[str, Path]
    if local_data_repo_path is not None:
      print(f"Using the local data repository in {str(local_data_repo_path)!r}")
      project_locale = self.project.get_conf("translation", "locale")
      components_to_compile = {}
      for path in (local_data_repo_path / "po" / project_locale / "components").iterdir():
        if not path.is_dir() and path.name.endswith(Project.COMPONENT_FILE_EXT):
          component_id = path.name[:-len(Project.COMPONENT_FILE_EXT)]
          components_to_compile[component_id] = path

    else:
      with ComponentsState(self.project.components_state_file) as local_components_state:
        print("Downloading the list of components")
        project_locale = self.project.get_conf("weblate", "locale")
        remote_components_state = self.weblate_client.fetch_components_state(project_locale)
        for component_id in (
          self.project.get_conf("weblate", "components_exclude", self.project.get_conf_list)
        ):
          remote_components_state.pop(component_id, None)

        existing_component_files: set[Path] = set(
          path for path in self.project.components_dir.iterdir()
          if not path.is_dir() and path.name.endswith(Project.COMPONENT_FILE_EXT)
        )

        for component_id in list(local_components_state.data.keys()):
          component_path = self.project.path_for_component(component_id)
          if (
            component_path not in existing_component_files or
            component_id not in remote_components_state
          ):
            local_components_state.data.pop(component_id, None)
            local_components_state.dirty = True
          existing_component_files.discard(component_path)
        local_components_state.save()

        for component_path in existing_component_files:
          try:
            component_path.unlink()
          except OSError:
            pass

        components_to_fetch: set[str] = set()

        for component_id, remote_mtime in remote_components_state.items():
          should_fetch = False

          if component_id not in local_components_state.data:
            # We don't have this component downloaded, it has probably been
            # created.
            should_fetch = True
          else:
            # Notice that missing timestamps mean that the component has never
            # been modified so far.
            local_mtime = local_components_state.data[component_id]

            if remote_mtime is None and local_mtime is None:
              # We have downloaded an empty component and there are still no
              # changes affecting it.
              should_fetch = False
            elif remote_mtime is not None and local_mtime is None:
              # Got the first ever changes!
              should_fetch = True
            elif remote_mtime is None and local_mtime is not None:
              # We have already downloaded some changed version of the component,
              # but it has probably been reset to an empty state since.
              should_fetch = True
            elif remote_mtime is not None and local_mtime is not None:
              # The sane and normal code path.
              should_fetch = remote_mtime > local_mtime

          if should_fetch:
            components_to_fetch.add(component_id)

        if len(components_to_fetch) != 0:
          print(f"Downloading {len(components_to_fetch)} components from Weblate")

          def download_worker_callback(component_id: str) -> tuple[str, int]:
            downloaded_size = self.download_file(
              component_id,
              lambda: self.weblate_client.download_component(component_id, project_locale, "po"),
              lambda: self.project.path_for_component(component_id).open("wb"),
            )
            return component_id, downloaded_size

          with ThreadPool(self.project.get_conf("project", "network_threads", int)) as pool:
            with self.tqdm(
              miniters=1,
              leave=False,
              desc="Downloaded components",
              total=len(components_to_fetch)
            ) as total_progress:
              downloaded_count = 0
              total_downloaded_size = 0
              for component_id, downloaded_size in pool.imap_unordered(
                download_worker_callback, components_to_fetch
              ):
                downloaded_count += 1
                total_downloaded_size += downloaded_size
                total_progress.update()
                print(
                  f"Downloaded {downloaded_count}/{len(components_to_fetch)} {component_id} - net {self.format_bytes(downloaded_size)}, {self.format_bytes(total_downloaded_size)} total"
                )
                local_components_state.data[component_id] = remote_components_state[component_id]
                local_components_state.dirty = True
                local_components_state.save()

        else:
          print("Every component is up to date")

        components_to_compile = {
          component_id: self.project.path_for_component(component_id)
          for component_id in local_components_state.data.keys()
        }

    out_credits_file = self.project.get_conf(
      "weblate", "credits_file", self.project.get_conf_path, fallback=None
    )
    if out_credits_file is not None:
      print("Downloading contributor statistics from Weblate")
      credits_data = self.weblate_client.fetch_credits(project_locale)
      out_credits_file.parent.mkdir(parents=True, exist_ok=True)
      with out_credits_file.open("w") as file:
        write_json(file, credits_data)
        file.write("\n")
        file.flush()

    print(f"Parsing and compiling {len(components_to_compile)} components")
    with self.tqdm(
      miniters=1, leave=False, desc="Parsed components", total=len(components_to_compile)
    ) as total_progress:
      with self.tqdm(
        leave=False,
        unit="B",
        unit_scale=True,
        unit_divisor=1024,
      ) as progress:
        compiler = TrPackCompiler()
        for component_id, component_path in components_to_compile.items():
          print(f"Parsing {component_id}")
          progress.desc = component_id
          progress.refresh()
          with component_path.open("r") as reader:
            parser = gettext_po.Parser(reader.read())
            # Also calls progress.refresh()
            progress.reset(len(parser.lexer.src))
            while True:
              message: gettext_po.ParsedMessage | None = parser.parse_next_message()
              if message is None:
                break
              compiler.add_fragment(message)
              progress.update(parser.lexer.next_char_index - progress.n)
          total_progress.update()

    print(f"Writing {len(compiler.packs)} compiled translation packs")
    out_packs_dir = self.project.get_conf(
      "project", "localize_me_packs_dir", self.project.get_conf_path
    )
    with self.tqdm(miniters=1, leave=False, total=len(compiler.packs)) as progress:
      for rel_pack_path, pack in compiler.packs.items():
        pack_path = out_packs_dir.joinpath(rel_pack_path)
        pack_path.parent.mkdir(parents=True, exist_ok=True)
        with pack_path.open("w") as pack_file:
          write_json(pack_file, pack)
          pack_file.write("\n")
          pack_file.flush()
        progress.update()

    print("Writing the mapping file")
    out_mapping_file = self.project.get_conf(
      "project", "localize_me_mapping_file", self.project.get_conf_path
    )
    out_mapping_file.parent.mkdir(parents=True, exist_ok=True)
    with out_mapping_file.open("w") as file:
      write_json(file, compiler.packs_mapping)
      file.write("\n")
      file.flush()

  def cmd_make_dist(self) -> None:

    print("Downloading dependencies")

    def download_dependency(name: str) -> Path:
      url = self.project.get_conf("dependencies", f"{name}_url")
      filename = self.project.get_conf("dependencies", f"{name}_file")
      download_path = self.project.download_dir / filename
      if not download_path.exists():
        downloaded_size = self.download_file(
          name,
          lambda: self.http_client.request(HTTPRequest(url)),
          lambda: download_path.open("wb"),
        )
        print(f"{name} downloaded - net {self.format_bytes(downloaded_size)}")
      else:
        print(f"{name} already downloaded")
      return download_path

    localize_me_file = download_dependency("localize_me")
    ccloader_file = download_dependency("ccloader")
    ultimate_ui_file = download_dependency("ultimate_ui")

    print("Collecting metadata")

    with (self.project.root_dir / "ccmod.json").open("r") as manifest_file:
      manifest = json.load(manifest_file)
      mod_id: str = manifest["id"]
      mod_version: str = manifest["version"]

    mod_files: list[Path] = []
    for pattern in self.project.get_conf(
      "distributables", "mod_files_patterns", self.project.get_conf_list
    ):
      mod_files.extend(
        path.relative_to(self.project.root_dir) for path in self.project.root_dir.glob(pattern)
      )
    # Note that paths here are sorted as lists of their components and not as
    # strings, so the path separators will not be taken into account when
    # sorting.
    mod_files.sort()

    commiter_time = int(
      subprocess.run(
        ["git", "log", "--max-count=1", "--date=unix", "--pretty=format:%cd"],
        check=True,
        stdout=subprocess.PIPE,
        cwd=self.project.root_dir,
      ).stdout
    )

    print("Making packages")

    def archive_add_mod_files(archived_prefix: Path) -> None:
      print("Adding mod files")
      for file in mod_files:
        archive.add_real_file(
          str(self.project.root_dir / file),
          str(archived_prefix / file),
          recursive=False,
          mtime=commiter_time,
        )

    def archive_add_dependency(
      archived_prefix: Path, dependency_path: Path, strip_components: int
    ) -> None:
      print(f"Adding files from {dependency_path.name}")
      with TarFile.gzopen(dependency_path) as dependency_archive:
        for file_info in dependency_archive:
          archived_path = str(
            Path(
              archived_prefix,
              *Path(file_info.name).parts[strip_components:],
            )
          )
          if file_info.isreg():
            file_reader = dependency_archive.extractfile(file_info)
            assert file_reader is not None
            archive.add_file_entry(archived_path, file_reader.read(), mtime=file_info.mtime)
          elif file_info.issym():
            archive.add_symlink_entry(archived_path, file_info.linkname, mtime=file_info.mtime)
          elif file_info.isdir():
            # Directories are deliberately ignored because the previous setup
            # didn't put them into resulting archives, and their entries are
            # useless to us anyway.
            pass
          else:
            # Other file types (character devices, block devices and named
            # pipes) are UNIX-specific and can't be handled by Zip, but it's
            # not like they are used in dependencies anyway. Correction: well,
            # after checking APPNOTE.TXT section 4.5.7 I noticed that these
            # exotic file types may be supported, but it's not like any modding
            # projects use those.
            pass

    all_archive_adapters: list[type[ArchiveAdapter]] = [TarGzArchiveAdapter, ZipArchiveAdapter]
    for archive_cls in all_archive_adapters:

      archive_name = f"{mod_id}_v{mod_version}{archive_cls.DEFAULT_EXTENSION}"
      print(f"Making {archive_name}")
      with archive_cls.create(self.project.dist_archives_dir / archive_name) as archive:
        archive_add_mod_files(Path(mod_id))

      # TODO: Sort all files in quick install archives
      archive_name = f"{mod_id}_quick-install_v{mod_version}{archive_cls.DEFAULT_EXTENSION}"
      print(f"Making {archive_name}")
      with archive_cls.create(self.project.dist_archives_dir / archive_name) as archive:
        archive_add_mod_files(Path("assets", "mods", mod_id))
        archive_add_dependency(Path("assets", "mods", "Localize-me"), localize_me_file, 1)
        archive_add_dependency(Path("assets", "mods"), ultimate_ui_file, 0)
        archive_add_dependency(Path(), ccloader_file, 0)

  def download_file(
    self,
    progress_desc: str,
    request_thunk: Callable[[], HTTPResponse],
    output_file_thunk: Callable[[], IO[bytes]],
  ) -> int:
    with self.tqdm(
      miniters=1,
      leave=False,
      desc=progress_desc,
      unit="B",
      unit_scale=True,
      unit_divisor=1024,
    ) as progress:
      with request_thunk() as response:
        # Also calls progress.refresh()
        progress.reset(response.length)
        response_reader = self.http_client.get_response_content_reader(response)
        downloaded_size = 0

        orig_response_read = response.read

        @functools.wraps(orig_response_read)
        def wrapped_response_read(amt: int | None = None) -> bytes:
          nonlocal downloaded_size
          data = orig_response_read(amt)
          downloaded_size += len(data)
          progress.update(len(data))
          return data

        response.read = wrapped_response_read

        with output_file_thunk() as output_file:
          buf_size = 8 * 1024
          while True:
            buf = response_reader.read(buf_size)
            if len(buf) == 0:
              break
            output_file.write(buf)

        progress.refresh()
        return downloaded_size

  @contextlib.contextmanager
  def wrap_print_for_tqdm(self) -> Generator[None, None, None]:
    if tqdm is None:
      yield
      return

    old_print = __builtins__["print"]
    try:

      @functools.wraps(old_print)
      def new_print(*args: Any, **kwargs: Any) -> object:
        with self.tqdm.external_write_mode(kwargs.get("file", None)):
          return old_print(*args, **kwargs)

      __builtins__["print"] = new_print
      yield

    finally:
      __builtins__["print"] = old_print

  def format_bytes(self, n: float) -> str:
    return self.tqdm.format_sizeof(n, suffix="B", divisor=1024)


class Project:

  PROJECT_TYPE_ID = "crosslocale//mod_tools//provisional"

  # NOTE: Field references are resolved lazily by ConfigParser, at the moment
  # when they are accessed. So, if the user changes a field like
  # `localize_me_commit`, the change will affect all other fields downstream
  # that are using `${localize_me_commit}`.
  DEFAULT_CONFIG: dict[str, dict[str, str]] = {
    "project": {
      "work_dir": "./crosslocale-work",
      "network_timeout": "60",
      "network_threads": "10",
      "network_max_retries": "3",
      "network_retry_wait": "1",
      "localize_me_packs_dir": "./packs",
      "localize_me_mapping_file": "./packs-mapping.json",
    },
    "translation": {
      # "original_locale": None,
      # "locale": None,
      "target_game_version": "1.4.2-1",
      "scan_database_file": "scan-${target_game_version}.json",
      # "scan_database_url": (
      #   "https://raw.githubusercontent.com/dmitmel/crosslocale-scans/master/scan-${target_game_version}.json"
      # ),
    },
    "weblate": {
      "root_url": "https://weblate.openkrosskod.org",
      # "auth_token": None,
      "project": "crosscode",
      # "original_locale": None,
      # "locale": None,
      "components_exclude": "glossary",
      # "credits_file": None,
      # "local_data_repo_path": None,
      # "use_local_data_repo": None,
    },
    "distributables": {
      # "mod_files_patterns": None,
    },
    "dependencies": {
      "localize_me_commit":
        "cd84932c815297c6777fafcf4e5fcfbc0d3d6cc3",
      "localize_me_file":
        "Localize-me-${localize_me_commit}.tgz",
      "localize_me_url":
        "https://github.com/L-Sherry/Localize-me/tarball/${localize_me_commit}",
      "ccloader_build":
        "20220208223224",
      "ccloader_file":
        "ccloader-${ccloader_build}.tgz",
      "ccloader_url":
        "https://stronghold.openkrosskod.org/~dmitmel/ccloader3/${ccloader_build}/ccloader_3.0.0-alpha_quick-install.tar.gz",
      "ultimate_ui_cc_ru_version":
        "1.3.4",
      "ultimate_ui_version":
        "1.2.0",
      "ultimate_ui_file":
        "ultimate-localized-ui-${ultimate_ui_version}.tgz",
      "ultimate_ui_url":
        "https://github.com/CCDirectLink/crosscode-ru/releases/download/v${ultimate_ui_cc_ru_version}/ultimate-localized-ui_v${ultimate_ui_version}.tgz",
    },
  }

  CONFIG_FILE_NAMES = ("crosslocale-mod-tools.ini", "crosslocale-mod-tools.local.ini")

  def __init__(self, root_dir: Path) -> None:
    print(f"Loading project from {str(root_dir)!r}")
    self.root_dir: Path = root_dir.resolve()

    self._config = configparser.ConfigParser(
      interpolation=configparser.ExtendedInterpolation(),
      delimiters=("=",),
      comment_prefixes=(";", "#"),
    )
    self._config.optionxform = lambda optionstr: optionstr
    self._config.read_dict(self.DEFAULT_CONFIG)

    for i, filename in enumerate(self.CONFIG_FILE_NAMES):
      file_path = self.root_dir / filename
      print(f"Loading project config from {str(file_path)!r}")
      try:
        file = file_path.open("r")
      except FileNotFoundError as err:
        if i == 0:
          raise
        else:
          print(str(err))
          continue
      with file:
        self._config.read_file(file, file.name)
      if self._config.get("project", "type", raw=True, fallback=None) != self.PROJECT_TYPE_ID:
        raise ValueError(f"expected project type to be {self.PROJECT_TYPE_ID!r}")

    self.work_dir: Path = self.root_dir / self.get_conf("project", "work_dir", Path)
    self.download_dir: Path = self.work_dir / "download"
    self.components_dir: Path = self.work_dir / "components"
    self.components_state_file: Path = self.work_dir / "components.json"
    self.dist_archives_dir: Path = self.work_dir / "dist"

  COMPONENT_FILE_EXT = ".po"

  def path_for_component(self, component: str) -> Path:
    return self.components_dir.joinpath(f"{component}{self.COMPONENT_FILE_EXT}")

  @overload
  def get_conf(self, section: str, option: str, *, raw: bool = ..., vars: Mapping[str, str] | None = ...) -> str:  # yapf: disable
    ...

  @overload
  def get_conf(self, section: str, option: str, *, raw: bool = ..., vars: Mapping[str, str] | None = ..., fallback: _T = ...) -> str | _T:  # yapf: disable
    ...

  @overload
  def get_conf(self, section: str, option: str, type_conv: Callable[[str], _T], *, raw: bool = ..., vars: Mapping[str, str] | None = ..., fallback: _T = ...) -> _T:  # yapf: disable
    ...

  def get_conf(
    self,
    section: str,
    option: str,
    type_conv: Callable[[str], _T] | None = None,
    *,
    raw: bool = False,
    vars: Mapping[str, str] | None = None,
    fallback: _T = _UNSET,
  ) -> str | _T:
    try:
      value = self._config.get(section, option, raw=raw, vars=vars)
    except (configparser.NoSectionError, configparser.NoOptionError):
      if fallback is _UNSET:
        raise
      return fallback
    try:
      return type_conv(value) if type_conv is not None else value
    except Exception as err:
      raise ValueError(
        f"Value of option {option!r} in section {section!r} is invalid: {value!r}"
      ) from err

  def get_conf_list(self, s: str) -> list[str]:
    return [x for x in s.splitlines() if len(x) > 0]

  def get_conf_path(self, s: str) -> Path:
    return self.root_dir.joinpath(s).resolve()

  def get_conf_bool(self, s: str) -> bool:
    config_any: Any = self._config
    return config_any._convert_to_boolean(s)


class ComponentsState:
  if TYPE_CHECKING:
    Data = dict[str, datetime | None]

  def __init__(self, path: Path) -> None:
    self.file_path: Path = path
    self.file: io.TextIOWrapper | None = None
    self.data: ComponentsState.Data = {}
    self.dirty: bool = False

  def open(self, write: bool) -> io.TextIOWrapper:
    if self.file is None:
      self.file = self.file_path.open("a+" if write else "r+")
      self.file.seek(0)
    return self.file

  def close(self) -> None:
    if self.file is not None:
      self.file.close()
      self.file = None

  def load(self) -> None:
    self.data = {}
    try:
      file = self.open(write=False)
    except FileNotFoundError:
      return
    file.seek(0)
    json_data: dict[str, float | None] = json.load(file)
    for c_id, c_mtime in json_data.items():
      self.data[c_id] = (
        datetime.fromtimestamp(c_mtime, timezone.utc) if c_mtime is not None else None
      )

  def save(self, force: bool = False) -> None:
    if not (self.dirty or force):
      return
    file = self.open(write=True)
    file.seek(0)
    file.truncate()
    json_data: dict[str, float | None] = {}
    for c_id, c_mtime in self.data.items():
      json_data[c_id] = c_mtime.timestamp() if c_mtime is not None else None
    json.dump(json_data, file, ensure_ascii=False, indent=2)
    file.write("\n")
    file.flush()
    self.dirty = False

  def __enter__(self) -> ComponentsState:
    try:
      self.load()
    except BaseException:
      self.close()
      raise
    return self

  def __exit__(
    self,
    exc_type: type[BaseException] | None,
    exc_value: BaseException | None,
    exc_traceback: TracebackType | None,
  ) -> None:
    try:
      if exc_type is None:
        self.save()
    finally:
      self.close()


class WeblateClient:

  def __init__(
    self, http_client: HTTPClient, root_url: str, auth_token: str | None, project_name: str
  ) -> None:
    self.http_client: HTTPClient = http_client
    self.root_url: str = root_url
    self.auth_token: str | None = auth_token
    self.project_name: str = project_name

  def make_url(self, path: str, **params: str | None) -> str:
    return urllib.parse.urlunsplit(
      urllib.parse.urlsplit(self.root_url)._replace(
        path=urllib.parse.quote(path),
        query=urllib.parse.urlencode({k: v for k, v in params.items() if v is not None}),
      )
    )

  def make_request(self, url: str) -> HTTPResponse:
    req = HTTPRequest(url)
    if self.auth_token is not None:
      req.add_header("authorization", f"Token {self.auth_token}")
    return self.http_client.request(req)

  def fetch_components_state(self, locale: str) -> ComponentsState.Data:
    components: ComponentsState.Data = {}
    next_api_url: str = self.make_url(f"/api/projects/{self.project_name}/statistics/{locale}/")
    while next_api_url is not None:
      with self.make_request(next_api_url) as response:
        api_response = json.load(self.http_client.get_response_content_reader(response))
        next_api_url = api_response["next"]
        for stats_obj in api_response["results"]:
          c_id = stats_obj["component"]
          mtime: datetime | None = None
          mtime_str: str | None = stats_obj["last_change"]
          if mtime_str is not None:
            mtime = datetime.fromisoformat(
              mtime_str[:-1] + "+00:00" if mtime_str.endswith("Z") else mtime_str
            )
          components[c_id] = mtime
    return components

  def download_component(
    self, component_name: str, locale: str, format: str | None = None
  ) -> HTTPResponse:
    return self.make_request(
      self.make_url(f"/download/{self.project_name}/{component_name}/{locale}/", format=format)
    )

  def fetch_credits(self, locale: str) -> Any:
    with self.make_request(
      self.make_url(f"/api/projects/{self.project_name}/credits/")
    ) as response:
      api_response = json.load(self.http_client.get_response_content_reader(response))
      entries: list[Any] = []
      for language_data in api_response:
        if language_data["code"] == locale:
          for author_data in language_data["authors"]:
            entry = {k: author_data.get(k) for k in ("full_name", "change_count")}
            entries.append(entry)
          break
      return entries


class _tqdm_fallback:

  # Just copied from <https://github.com/tqdm/tqdm/blob/v4.64.0/tqdm/std.py#L258-L286>.
  @staticmethod
  def format_sizeof(num: float, suffix: str = "", divisor: float = 1000) -> str:
    for unit in ["", "k", "M", "G", "T", "P", "E", "Z"]:
      if abs(num) < 999.5:
        if abs(num) < 99.95:
          if abs(num) < 9.995:
            return "{0:1.2f}".format(num) + unit + suffix
          return "{0:2.1f}".format(num) + unit + suffix
        return "{0:3.0f}".format(num) + unit + suffix
      num /= divisor
    return "{0:3.1f}Y".format(num) + suffix

  @classmethod
  @contextlib.contextmanager
  def external_write_mode(
    cls,
    file: IO[str] = ...,
    nolock: bool = ...,
  ) -> Generator[None, None, None]:
    yield

  def __init__(
    self,
    desc: str | None = ...,
    total: float | None = ...,
    leave: bool | None = ...,
    miniters: float | None = ...,
    unit: str | None = ...,
    unit_scale: bool | float | None = ...,
    unit_divisor: float | None = ...,
  ) -> None:
    self.desc: str = desc if desc is not None else ""
    self.disable = True
    self.n: float = 0

  def __enter__(self) -> _tqdm_fallback:
    return self

  def __exit__(
    self,
    exc_type: type[BaseException] | None,
    exc_value: BaseException | None,
    exc_traceback: TracebackType | None,
  ) -> None:
    pass

  def update(self, n: float = ...) -> bool | None:
    pass

  def refresh(
    self, nolock: bool | None = ..., lock_args: Iterable[object] | None = ...
  ) -> bool | None:
    pass

  def reset(self, total: float | None) -> None:
    pass


class TrPackEntry(TypedDict):
  orig: str
  text: str


if TYPE_CHECKING:
  TrPack = dict[str, TrPackEntry]
  TrPackMapping = dict[str, str]


class TrPackCompiler:

  def __init__(self) -> None:
    self.packs: dict[str, TrPack] = {}
    self.packs_mapping: TrPackMapping = {}

  def add_fragment(self, message: gettext_po.ParsedMessage) -> None:
    msgctxt = "".join(message.msgctxt)
    msgid = "".join(message.msgid)
    msgstr = "".join(message.msgstr)
    file_path, paths_sep, json_path = msgctxt.partition("//")
    if len(msgctxt) == 0 or len(msgid) == 0 or len(msgstr) == 0 or len(paths_sep) == 0:
      return

    file_path = utils.str_strip_prefix(file_path, "data/")
    self.packs_mapping[file_path] = file_path
    pack: TrPack | None = self.packs.get(file_path)
    if pack is None:
      pack = {}
      self.packs[file_path] = pack
    pack[f"{file_path}/{json_path}"] = TrPackEntry(orig=msgid, text=msgstr)


def write_json(file: IO[str], data: object, indent: int | None = None) -> None:
  json.dump(
    data,
    file,
    ensure_ascii=False,
    indent=indent,
    separators=(",", ":") if not indent else (", ", ": "),
  )
