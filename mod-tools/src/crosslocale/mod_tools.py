from __future__ import annotations

import configparser
import os.path
import sys
import traceback
from pathlib import Path
from typing import List, NoReturn

from . import BINARY_NAME
from .cli import ArgumentError, ArgumentNamespace, ArgumentParser, ArgumentParserExit
from .http_client import HTTPClient


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


def main(args: List[str], bin_name: str = BINARY_NAME) -> None:
  return _Main().main(args, bin_name)


class _Main:

  def main(self, raw_args: List[str], bin_name: str) -> None:
    self.arg_parser: ArgumentParser = self.build_arg_parser(bin_name)

    try:
      self.cli_args: ArgumentNamespace = self.arg_parser.parse_args(raw_args)
    except (ArgumentError, ArgumentParserExit) as err:
      setattr(err, "parser", self.arg_parser)
      raise err

    self.project: Project = Project(self.cli_args.project)
    self.http_client: HTTPClient = HTTPClient(
      network_timeout=self.project.config.getint("project", "network_timeout"),
    )
    self.cli_args.command_fn()

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
    url = self.project.config.get("translation", "weblate_root_url")
    with self.http_client.request("GET", url) as (res, body):
      print(res, len(body.read()))

  def cmd_make_dist(self) -> None:
    raise NotImplementedError()


class Project:

  # NOTE: Field references are resolved lazily by ConfigParser, at the moment
  # when they are accessed. So, if the user changes a field like
  # `localize_me_commit`, the change will affect all other fields downstream
  # that are using `${localize_me_commit}`.
  DEFAULT_CONFIG: dict[str, dict[str, str]] = {
    "project": {
      "work_dir": "./mod-tools-work",
      "network_timeout": "60",
      "network_threads": "10",
    },
    "translation": {
      "target_game_version": "1.4.2-1",
      "scan_database_file": "scan-${target_game_version}.json",
      # "scan_database_url": (
      #   "https://raw.githubusercontent.com/dmitmel/crosslocale-scans/master/scan-${target_game_version}.json"
      # ),
      "localize_me_packs_dir": "./packs",
      "localize_me_mapping_file": "./packs-mapping.json",
      "weblate_root_url": "https://weblate.crosscode.ru",
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
        "1.3.3",
      "ultimate_ui_version":
        "1.1.0",
      "ultimate_ui_file":
        "ultimate-localized-ui-${ultimate_ui_version}.tgz",
      "ultimate_ui_url":
        "https://github.com/CCDirectLink/crosscode-ru/releases/download/v${ultimate_ui_cc_ru_version}/ultimate-localized-ui_v${ultimate_ui_version}.tgz",
    },
  }

  CONFIG_FILE_NAMES = ("crosslocale-mod-tools.ini", "crosslocale-mod-tools.local.ini")

  def __init__(self, root_dir: Path) -> None:
    print(f"Loading project from {str(root_dir)!r}")

    self.config = configparser.ConfigParser(
      interpolation=configparser.ExtendedInterpolation(),
      delimiters=("=",),
      comment_prefixes=(";", "#"),
    )
    self.config.optionxform = lambda optionstr: optionstr
    self.config.read_dict(self.DEFAULT_CONFIG)

    for i, filename in enumerate(self.CONFIG_FILE_NAMES):
      file_path = root_dir / filename
      print(f"Loading project config from {str(file_path)!r}")
      try:
        with open(file_path, "r") as file:
          self.config.read_file(file, file.name)
      except FileNotFoundError as err:
        if i == 0:
          raise
        else:
          print(str(err))
          continue

    for section in self.config:
      for k, v in self.config.items(section):
        print((k, v))
