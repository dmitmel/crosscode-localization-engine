from __future__ import annotations

import gzip
import urllib.parse
import urllib.request
from contextlib import contextmanager
from http.client import HTTPResponse
from io import BufferedIOBase
from typing import IO, Generator, Iterable, MutableMapping, Tuple

from . import __version__


# TODO: retry on failure
class HTTPClient:

  def __init__(self, network_timeout: int | None = None) -> None:
    self.opener: urllib.request.OpenerDirector = urllib.request.OpenerDirector()
    self.network_timeout: int | None = network_timeout

    handlers: list[urllib.request.BaseHandler] = [
      urllib.request.ProxyHandler(),
      urllib.request.UnknownHandler(),
      urllib.request.HTTPHandler(),
      urllib.request.HTTPDefaultErrorHandler(),
      urllib.request.HTTPRedirectHandler(),
      urllib.request.HTTPErrorProcessor(),
      urllib.request.HTTPSHandler(),
    ]
    for handler in handlers:
      self.opener.add_handler(handler)

    self.opener.addheaders = [
      (
        "user-agent",
        f"crosslocale-mod-tools/{__version__} Python-urllib/{getattr(urllib.request, '__version__')}"
      ),
      ("accept-encoding", "gzip"),
    ]

  @contextmanager
  def request(
    self,
    method: str,
    url: str,
    headers: MutableMapping[str, str] | None = None,
    body: bytes | IO[bytes] | Iterable[bytes] | None = None,
  ) -> Generator[Tuple[HTTPResponse, BufferedIOBase], None, None]:
    print("Requesting " + repr(f"{method} {url}"))
    req = urllib.request.Request(
      method=method, url=url, headers=headers if headers is not None else {}, data=body
    )

    res: object
    with self.opener.open(req, timeout=self.network_timeout) as res:
      if not isinstance(res, HTTPResponse):
        raise Exception("Expected an HTTPResponse")

      # <https://github.com/kurtmckee/feedparser/blob/727ee7f08f77d8f0a0f085ec3dfbc58e09f69a4b/feedparser/http.py#L166-L188>
      reader: BufferedIOBase = res
      content_encoding = res.getheader("content-encoding")
      if content_encoding == "gzip":
        reader = gzip.GzipFile(fileobj=res)

      yield res, reader

  def close(self) -> None:
    self.opener.close()
