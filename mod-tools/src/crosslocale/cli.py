from __future__ import annotations

import argparse
import sys
from argparse import ArgumentError as ArgumentError
from argparse import Namespace as ArgumentNamespace
from typing import NoReturn

from .utils import nop, unreachable

nop(ArgumentError, ArgumentNamespace)


class ArgumentParser(argparse.ArgumentParser):
  exit_on_error: bool

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
