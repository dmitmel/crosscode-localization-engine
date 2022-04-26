from __future__ import annotations

import gzip
import urllib.parse
import urllib.request
from http.client import HTTPResponse
from io import BufferedIOBase
from typing import Any, TypeAlias

from . import __version__

HTTPRequest: TypeAlias = urllib.request.Request


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

  def request(self, req: HTTPRequest) -> HTTPResponse:
    print("Requesting " + repr(f"{req.get_method()} {req.get_full_url()}"))
    res: Any = None
    try:
      res = self.opener.open(req, timeout=self.network_timeout)
      if not isinstance(res, HTTPResponse):
        raise Exception("Expected an HTTPResponse")
      return res
    except BaseException:
      if res is not None and hasattr(res, "close"):
        res.close()
      raise

  def close(self) -> None:
    self.opener.close()

  # <https://github.com/kurtmckee/feedparser/blob/727ee7f08f77d8f0a0f085ec3dfbc58e09f69a4b/feedparser/http.py#L166-L188>
  def get_response_content_reader(self, res: HTTPResponse) -> BufferedIOBase:
    content_encoding = res.headers.get("content-encoding")
    if content_encoding == "gzip":
      return gzip.GzipFile(fileobj=res)
    else:
      return res
