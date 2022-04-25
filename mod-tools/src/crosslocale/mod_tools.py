from __future__ import annotations

import configparser
import contextlib
import functools
import io
import json
import os.path
import sys
import time
import traceback
import urllib.parse
from datetime import datetime, timezone
from multiprocessing.pool import ThreadPool
from pathlib import Path
from types import TracebackType
from typing import (
  IO, TYPE_CHECKING, Any, Callable, Generator, Iterable, Mapping, NoReturn, TypeAlias, TypedDict,
  TypeVar, cast, overload
)

try:
  from tqdm import tqdm
except ImportError:
  tqdm = None

from . import BINARY_NAME, gettext_po, utils
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
    with wrap_print_for_tqdm():

      self.arg_parser: ArgumentParser = self.build_arg_parser(bin_name)

      try:
        self.cli_args: ArgumentNamespace = self.arg_parser.parse_args(raw_args)
      except (ArgumentError, ArgumentParserExit) as err:
        setattr(err, "parser", self.arg_parser)
        raise err

      start_time = time.perf_counter()

      self.project: Project = Project(self.cli_args.project)
      for path in [self.project.work_dir, self.project.download_dir, self.project.components_dir]:
        path.mkdir(exist_ok=True, parents=True)

      self.http_client: HTTPClient = HTTPClient(
        network_timeout=self.project.get_conf("project", "network_timeout", int, fallback=None),
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

    # Empty help strings are necessary for subparsers to show up in help.
    subparsers = parser.add_subparsers(required=True, metavar="COMMAND")

    subparser = subparsers.add_parser("download", help="")
    subparser.set_defaults(command_fn=self.cmd_download)

    subparser = subparsers.add_parser("make-dist", help="")
    subparser.set_defaults(command_fn=self.cmd_make_dist)

    return parser

  def cmd_download(self) -> None:
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
        if path.name.endswith(Project.COMPONENT_FILE_EXT)
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

      if len(components_to_fetch) != 0:
        print(f"Downloading {len(components_to_fetch)} components from Weblate")
        with ThreadPool(self.project.get_conf("project", "network_threads", int)) as pool:

          def download_worker_callback(component_id: str) -> str:
            with maybe_tqdm(
              miniters=1,
              leave=False,
              desc=component_id,
              unit="B",
              unit_scale=True,
              unit_divisor=1024,
            ) as progress:
              with (
                self.weblate_client.download_component(component_id, project_locale, "po")
              ) as response:
                response_reader = self.http_client.get_response_content_reader(response)
                content_length = response.headers.get("content-length", None)
                # Also calls progress.refresh()
                progress.reset(int(content_length) if content_length is not None else None)

                with self.project.path_for_component(component_id).open("wb") as output_file:
                  buf_size = 8 * 1024
                  while True:
                    buf = response_reader.read(buf_size)
                    progress.update(len(buf))
                    if len(buf) == 0:
                      break
                    output_file.write(buf)

            return component_id

          with maybe_tqdm(
            miniters=1, leave=False, desc="Downloaded components", total=len(components_to_fetch)
          ) as total_progress:
            downloaded_count = 0
            for component_id in pool.imap_unordered(download_worker_callback, components_to_fetch):
              downloaded_count += 1
              total_progress.update()
              if total_progress.disable:
                print(f"Downloaded {downloaded_count}/{len(components_to_fetch)} {component_id}")
              local_components_state.data[component_id] = remote_components_state[component_id]
              local_components_state.dirty = True
              local_components_state.save()

      else:
        print("Every component is up to date")

      print(f"Parsing and compiling {len(local_components_state.data)} components")
      with maybe_tqdm(
        miniters=1, leave=False, desc="Parsed components", total=len(local_components_state.data)
      ) as total_progress:
        with maybe_tqdm(
          leave=False,
          unit="B",
          unit_scale=True,
          unit_divisor=1024,
        ) as progress:
          compiler = TrPackCompiler()
          for component_id in local_components_state.data.keys():
            if total_progress.disable:
              print(f"Parsing {component_id}")
            progress.desc = component_id
            progress.refresh()
            with self.project.path_for_component(component_id).open("r") as reader:
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
      with maybe_tqdm(miniters=1, leave=False, total=len(compiler.packs)) as progress:
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
    raise NotImplementedError()


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
    self.components_dir: Path = self.download_dir / "components"
    self.components_state_file: Path = self.download_dir / "components.json"

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
    return self.root_dir.joinpath(s)


class ComponentsState:
  if TYPE_CHECKING:
    Data: TypeAlias = dict[str, datetime | None]

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

  def make_request(self, url: str) -> HTTPResponse:
    req = HTTPRequest(url=urllib.parse.urljoin(self.root_url, url))
    if self.auth_token is not None:
      req.add_header("authorization", f"Token {self.auth_token}")
    return self.http_client.request(req)

  def fetch_components_state(self, locale: str) -> ComponentsState.Data:
    components: ComponentsState.Data = {}
    next_api_url: str
    next_api_url = f"/api/projects/{self.project_name}/statistics/{locale}/"
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

  def download_component(self, component_name: str, locale: str, format: str = "") -> HTTPResponse:
    return self.make_request(
      f"/download/{self.project_name}/{component_name}/{locale}/?format={format}"
    )

  def fetch_credits(self, locale: str) -> Any:
    with self.make_request(f"/api/projects/{self.project_name}/credits/") as response:
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
    self.disable = False
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


maybe_tqdm = cast("type[_tqdm_fallback]", tqdm) if tqdm is not None else _tqdm_fallback


@contextlib.contextmanager
def wrap_print_for_tqdm() -> Generator[None, None, None]:
  if tqdm is None:
    yield
    return

  old_print = __builtins__["print"]
  try:

    @functools.wraps(old_print)
    def new_print(*args: Any, **kwargs: Any) -> object:
      with maybe_tqdm.external_write_mode(kwargs.get("file", None)):
        return old_print(*args, **kwargs)

    __builtins__["print"] = new_print
    yield

  finally:
    __builtins__["print"] = old_print


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
