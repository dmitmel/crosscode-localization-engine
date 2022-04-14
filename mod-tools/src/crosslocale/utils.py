from __future__ import annotations

from typing import NoReturn


def unreachable() -> NoReturn:
  raise Exception("unreachable")


def nop(*args: object, **kwargs: object) -> None:
  pass
