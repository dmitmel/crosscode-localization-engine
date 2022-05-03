from __future__ import annotations

import argparse
import sys
from argparse import ArgumentError as ArgumentError
from argparse import Namespace as ArgumentNamespace
from typing import TYPE_CHECKING, Any, Callable, Iterable, NoReturn, Sequence, TypeVar

from .utils import nop, unreachable

nop(ArgumentError, ArgumentNamespace)

_T = TypeVar("_T")


class ArgumentParser(argparse.ArgumentParser):
  exit_on_error: bool = False

  def exit(self, status: int = 0, message: str | None = None) -> NoReturn:
    if self.exit_on_error:
      super().exit(status, message)
      unreachable()
    else:
      if message:
        self._print_message(message, sys.stderr)
      raise ArgumentParserExit(status)

  def error(self, message: str) -> NoReturn:
    if self.exit_on_error:
      super().error(message)
      unreachable()
    else:
      raise argparse.ArgumentError(None, message)


class ArgumentParserExit(Exception):

  def __init__(self, code: int) -> None:
    super().__init__(f"code = {code}")
    self.code = code


if sys.version_info < (3, 9) or TYPE_CHECKING:
  # Copied from: <https://github.com/python/cpython/blob/v3.10.4/Lib/argparse.py#L862-L900>
  # Also see: <https://thisdataguy.com/2017/07/03/no-options-with-argparse-and-python/>
  class FlagAction(argparse.Action):

    def __init__(
      self,
      option_strings: Sequence[str],
      dest: str,
      default: _T | str | None = None,
      type: Callable[[str], _T] | argparse.FileType | None = None,
      choices: Iterable[_T] | None = None,
      required: bool = False,
      help: str | None = None,
      metavar: str | tuple[str, ...] | None = None,
    ) -> None:
      new_option_strings: list[str] = []
      for opt in option_strings:
        new_option_strings.append(opt)
        if opt.startswith("--"):
          opt = "--no-" + opt[2:]
          new_option_strings.append(opt)

      if help is not None and default is not None and default is not argparse.SUPPRESS:
        help += " (default: %(default)s)"

      super().__init__(
        option_strings=new_option_strings,
        dest=dest,
        nargs=0,
        default=default,
        type=type,
        choices=choices,
        required=required,
        help=help,
        metavar=metavar
      )

    def __call__(
      self,
      parser: argparse.ArgumentParser,
      namespace: argparse.Namespace,
      values: str | Sequence[Any] | None,
      option_string: str | None = None,
    ) -> None:
      if option_string in self.option_strings:
        assert option_string is not None
        setattr(namespace, self.dest, not option_string.startswith("--no-"))

    # format_usage has been added only in Py 3.9, no need to implement it

else:

  from argparse import BooleanOptionalAction as FlagAction
  nop(FlagAction)
