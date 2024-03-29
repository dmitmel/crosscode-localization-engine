from __future__ import annotations

from typing import NoReturn, TypeVar

_T = TypeVar("_T")


def unreachable() -> NoReturn:
  raise Exception("unreachable")


def nop(*args: object, **kwargs: object) -> None:
  pass


def str_strip_prefix(s: str, prefix: str) -> str:
  return s[len(prefix):] if s.startswith(prefix) else s


def str_strip_suffix(s: str, suffix: str) -> str:
  return s[len(suffix):] if s.endswith(suffix) else s


def or_default(value: _T | None, default: _T) -> _T:
  return value if value is not None else default
