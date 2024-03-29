# Pretty much a 1-to-1 port of the Rust gettext PO parser. See all of the
# details and insights in comments in those files.

from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import List, NoReturn, TypeVar


class ParsingError(Exception):

  def __init__(self, message: str, pos: int) -> None:
    super().__init__(message)
    self.message: str = message
    self.pos: int = pos


@dataclass()
class Token:
  start_pos: int = field(init=False, default=0)
  end_pos: int = field(init=False, default=0)


@dataclass()
class TokenComment(Token):
  comment_type: CommentType
  text: str


class CommentType(Enum):
  Translator = 1
  Automatic = 2
  Reference = 3
  Flags = 4


@dataclass()
class TokenMsgctxt(Token):
  is_previous: bool


@dataclass()
class TokenMsgid(Token):
  is_previous: bool


@dataclass()
class TokenMsgstr(Token):
  pass


@dataclass()
class TokenString(Token):
  is_previous: bool
  text: str


@dataclass()
class ParsedMessage:
  translator_comments: List[str] = field(default_factory=list)
  automatic_comments: List[str] = field(default_factory=list)
  reference_comments: List[str] = field(default_factory=list)
  flags_comments: List[str] = field(default_factory=list)
  prev_msgctxt: List[str] = field(default_factory=list)
  prev_msgid: List[str] = field(default_factory=list)
  msgctxt: List[str] = field(default_factory=list)
  msgid: List[str] = field(default_factory=list)
  msgstr: List[str] = field(default_factory=list)


class Lexer:

  __slots__ = (
    "src", "done", "token_start_pos", "current_pos", "next_char_index", "is_previous_entry"
  )

  def __init__(self, src: str) -> None:
    self.src: str = src
    self.done: bool = False
    self.token_start_pos: int = 0
    self.current_pos: int = 0
    self.next_char_index: int = 0
    self.is_previous_entry: bool = False

  def next_char(self) -> str | None:
    i = self.next_char_index
    try:
      c = self.src[i]
    except IndexError:
      self.done = True
      return None
    self.current_pos = i
    self.next_char_index = i + 1
    if c == "\n":
      self.reset_current_line_flags()
    return c

  def peek_char(self) -> str | None:
    try:
      return self.src[self.next_char_index]
    except IndexError:
      return None

  def begin_token(self) -> None:
    self.token_start_pos = self.current_pos

  _TokenT = TypeVar("_TokenT", bound=Token)

  def end_token(self, token: _TokenT) -> _TokenT:
    token.start_pos = self.token_start_pos
    token.end_pos = self.next_char_index
    return token

  def emit_error(self, message: str) -> NoReturn:
    self.done = True
    raise ParsingError(message, self.current_pos)

  def reset_current_line_flags(self) -> None:
    self.is_previous_entry = False

  def parse_next_token(self) -> Token | None:
    while not self.done:
      self.skip_whitespace()

      c = self.next_char()
      if c is None:
        return None
      self.begin_token()

      token: Token | None
      if c == "#":
        token = self.parse_comment()
      elif c == '"':
        token = self.parse_string()
      elif "a" <= c <= "z" or "A" <= c <= "Z" or c == "_":
        token = self.parse_keyword()
      else:
        return self.emit_error(f"unexpected chharacter {c!r}")

      if token is not None:
        return self.end_token(token)

    return None

  def skip_whitespace(self) -> None:
    while True:
      c = self.peek_char()
      if c is not None and c in " \n\r\t\v\f":
        self.next_char()
      else:
        break

  def parse_comment(self) -> TokenComment | None:
    comment_type = CommentType.Translator

    c = self.peek_char()
    if c == "~":
      self.next_char()
      return self.emit_error("obsolete entries are unsupported")
    elif c == "|":
      self.next_char()
      self.is_previous_entry = True
      return None
    elif c == ".":
      comment_type = CommentType.Automatic
    elif c == ":":
      comment_type = CommentType.Reference
    elif c == ",":
      comment_type = CommentType.Flags

    if comment_type != CommentType.Translator:
      self.next_char()
    text_start_index = self.next_char_index
    while True:
      c = self.peek_char()
      if c is None or c == "\n":
        break
      self.next_char()
    text = self.src[text_start_index:self.next_char_index]
    return TokenComment(comment_type=comment_type, text=text)

  def parse_string(self) -> TokenString | None:
    literal_text_start_index = self.next_char_index
    text_buf: List[str] = []

    CHARACTER_ESCAPES = {
      "\n": "",
      "n": "\n",
      "t": "\t",
      "b": "\b",
      "r": "\r",
      "f": "\f",
      "v": "\v",
      "a": "\a",
      "\\": "\\",
      '"': '"',
    }

    while True:
      c = self.peek_char()
      if c is None or c == "\n":
        return self.emit_error("unterminated string")
      self.next_char()

      if c == '"':
        break
      elif c == "\\":
        literal_text = self.src[literal_text_start_index:self.current_pos]
        c = self.peek_char()
        if c is None:
          return self.emit_error("expected a character to escape")
        self.next_char()

        unescaped_char = CHARACTER_ESCAPES.get(c)
        if unescaped_char is None:
          return self.emit_error(f"unknown escaped character: {c!r}")

        text_buf.append(literal_text)
        text_buf.append(unescaped_char)
        literal_text_start_index = self.next_char_index

    last_literal_text = self.src[literal_text_start_index:self.current_pos]
    text_buf.append(last_literal_text)
    return TokenString(is_previous=self.is_previous_entry, text="".join(text_buf))

  def parse_keyword(self) -> TokenMsgctxt | TokenMsgid | TokenMsgstr | None:
    while True:
      c = self.peek_char()
      if c is None:
        break
      if "a" <= c <= "z" or "A" <= c <= "Z" or c == "_":
        self.next_char()
      else:
        break
    keyword = self.src[self.token_start_pos:self.next_char_index]

    if keyword == "domain":
      self.emit_error(
        'the "domain" keyword is unsupported due to the lack of documentation about it'
      )
    elif keyword == "msgctxt":
      return TokenMsgctxt(is_previous=self.is_previous_entry)
    elif keyword == "msgid":
      return TokenMsgid(is_previous=self.is_previous_entry)
    elif keyword == "msgstr" and not self.is_previous_entry:
      return TokenMsgstr()
    elif keyword == "msgid_plural" or keyword == "msgstr_plural":
      return self.emit_error(
        f"keyword {keyword!r} is unsupported because plurals were unneeded and thus are unsupported"
      )
    else:
      return self.emit_error(f"unexpected keyword {keyword!r}")


class Parser:

  __slots__ = ("lexer", "done", "peeked_token", "current_token")

  def __init__(self, src: str) -> None:
    self.lexer = Lexer(src)
    self.done: bool = False
    self.peeked_token: Token | None = None
    self.current_token: Token | None = None

  def next_token(self) -> Token | None:
    token = self.peeked_token
    if token is None:
      token = self.lexer.parse_next_token()
      if token is None:
        self.done = True
    self.peeked_token = None
    self.current_token = token
    return token

  def peek_token(self) -> Token | None:
    token = self.peeked_token
    if token is None:
      token = self.lexer.parse_next_token()
      self.peeked_token = token
      if token is None:
        self.done = True
    return self.peeked_token

  def emit_error(self, message: str) -> NoReturn:
    self.done = True
    raise ParsingError(
      message, self.current_token.start_pos if self.current_token is not None else 0
    )

  def emit_error_after(self, message: str) -> NoReturn:
    self.done = True
    raise ParsingError(
      message, self.current_token.end_pos if self.current_token is not None else 0
    )

  def parse_next_message(self) -> ParsedMessage | None:
    if self.done:
      return None

    message = ParsedMessage()

    self.parse_comments_block(message)

    has_prev_msgctxt = False
    token = self.peek_token()
    if isinstance(token, TokenMsgctxt) and token.is_previous:
      self.next_token()
      self.parse_string_list(message.prev_msgctxt, True)
      has_prev_msgctxt = True

    token = self.peek_token()
    if isinstance(token, TokenMsgid) and token.is_previous:
      self.next_token()
      self.parse_string_list(message.prev_msgid, True)
    elif has_prev_msgctxt:
      self.next_token()
      return self.emit_error("expected prev_msgid")

    has_msgctxt = False
    token = self.peek_token()
    if isinstance(token, TokenMsgctxt) and not token.is_previous:
      self.next_token()
      self.parse_string_list(message.msgctxt, False)
      has_msgctxt = True

    token = self.next_token()
    if isinstance(token, TokenMsgid) and not token.is_previous:
      self.parse_string_list(message.msgid, False)
    else:
      return self.emit_error("expected msgid" if has_msgctxt else "expected msgid or msgctxt")

    token = self.next_token()
    if isinstance(token, TokenMsgstr):
      self.parse_string_list(message.msgstr, False)
    else:
      return self.emit_error("expected msgstr")

    return message

  def parse_string_list(self, out: List[str], is_previous_entry: bool) -> None:
    found_any_strings = False
    while True:
      token = self.peek_token()
      if not isinstance(token, TokenString) or token.is_previous != is_previous_entry:
        break
      self.next_token()
      out.append(token.text)
      found_any_strings = True
    if not found_any_strings:
      self.emit_error_after(
        "expected one or more " + ("prev_strings" if is_previous_entry else "strings")
      )

  def parse_comments_block(self, out: ParsedMessage) -> None:
    while True:
      token = self.peek_token()
      if not isinstance(token, TokenComment):
        break
      self.next_token()
      if token.comment_type == CommentType.Translator:
        out.translator_comments.append(token.text)
      elif token.comment_type == CommentType.Automatic:
        out.automatic_comments.append(token.text)
      elif token.comment_type == CommentType.Reference:
        out.reference_comments.append(token.text)
      elif token.comment_type == CommentType.Flags:
        out.flags_comments.append(token.text)
