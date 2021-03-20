use crate::rc_string::RcString;
use crate::utils::parsing::{CharPos, CharPosIter, ParsingError};

use std::borrow::Cow;
use std::iter;
use std::str;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'src> {
  pub start_pos: CharPos,
  pub end_pos: CharPos,
  pub type_: TokenType<'src>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenType<'src> {
  Comment(CommentType, Cow<'src, str>),
  PrevMsgctxt,
  PrevMsgid,
  PrevString(Cow<'src, str>),
  Msgctxt,
  Msgid,
  Msgstr,
  String(Cow<'src, str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommentType {
  Translator,
  Automatic,
  Reference,
  Flags,
}

#[derive(Debug)]
pub struct Lexer<'src> {
  src: &'src str,
  src_iter: iter::Peekable<CharPosIter<'src>>,
  done: bool,
  token_start_pos: CharPos,
  current_pos: CharPos,
  next_char_index: usize,
  is_previous_entry: bool,
}

impl<'src> Lexer<'src> {
  pub fn new(src: &'src str) -> Self {
    Self {
      src,
      src_iter: CharPosIter::new(src).peekable(),
      done: false,
      token_start_pos: CharPos::default(),
      current_pos: CharPos::default(),
      next_char_index: 0,
      is_previous_entry: false,
    }
  }

  fn next_char(&mut self) -> Option<char> {
    match self.src_iter.next() {
      Some((chr_pos, chr)) => {
        self.current_pos = chr_pos;
        self.next_char_index = chr_pos.byte_index + chr.len_utf8();
        // HACK: Honestly, this should be handled by the main match block.
        if chr == '\n' {
          self.reset_current_line_flags();
        }
        Some(chr)
      }
      None => {
        self.done = true;
        None
      }
    }
  }

  fn peek_char(&mut self) -> Option<char> {
    let &(_chr_pos, chr) = self.src_iter.peek()?;
    Some(chr)
  }

  fn begin_token(&mut self) { self.token_start_pos = self.current_pos; }
  fn end_token(&self, type_: TokenType<'src>) -> Token<'src> {
    Token {
      start_pos: self.token_start_pos,
      end_pos: CharPos {
        byte_index: self.next_char_index,
        char_index: self.current_pos.char_index + 1,
        column: self.current_pos.column + 1,
        line: self.current_pos.line,
      },
      type_,
    }
  }

  fn emit_error(&mut self, message: String) -> Result<!, ParsingError> {
    self.done = true;
    Err(ParsingError { pos: self.current_pos, message: RcString::from(message) })
  }

  fn reset_current_line_flags(&mut self) { self.is_previous_entry = false; }

  pub fn parse_next_token(&mut self) -> Result<Option<Token<'src>>, ParsingError> {
    while !self.done {
      self.skip_whitespace();

      // NOTE: This is the only place where quick return when no more tokens
      // are available is permitted, all other calls of `next_char` must handle
      // the EOF and emit an error or something like that.
      let c = match self.next_char() {
        Some(c) => c,
        None => return Ok(None),
      };
      self.begin_token();

      let token_type = match c {
        '#' => self.parse_comment()?,
        '\"' => self.parse_string()?,
        _ if c.is_ascii_alphabetic() || c == '_' => self.parse_keyword()?,
        _ => self.emit_error(format!("unexpected character {:?}", c))?,
      };

      if let Some(token_type) = token_type {
        return Ok(Some(self.end_token(token_type)));
      }
    }

    Ok(None)
  }

  fn skip_whitespace(&mut self) {
    while self.peek_char().map_or(false, |c| {
      // Note that is_ascii_whitespace doesn't match \v which GNU gettext
      // considers whitespace
      matches!(c, '\t' | '\n' | /* \v */ '\x0B' | /* \f */ '\x0C' | '\r' | ' ')
    }) {
      self.next_char();
    }
  }

  fn parse_comment(&mut self) -> Result<Option<TokenType<'src>>, ParsingError> {
    match self.peek_char() {
      Some('~') => {
        self.next_char();
        self.emit_error("obsolete entries are unsupported".to_owned())?;
      }

      Some('|') => {
        self.next_char();
        self.is_previous_entry = true;
        Ok(None)
      }

      marker_char => {
        let comment_type = match marker_char {
          Some('.') => CommentType::Automatic,
          Some(':') => CommentType::Reference,
          Some(',') => CommentType::Flags,
          _ => CommentType::Translator,
        };
        if comment_type != CommentType::Translator {
          self.next_char();
        }
        let text_start_index = self.next_char_index;
        while self.peek_char().map_or(false, |c| c != '\n') {
          self.next_char();
        }
        let text = &self.src[text_start_index..self.next_char_index];
        Ok(Some(TokenType::Comment(comment_type, Cow::Borrowed(text))))
      }
    }
  }

  fn parse_string(&mut self) -> Result<Option<TokenType<'src>>, ParsingError> {
    let text_start_index = self.next_char_index;
    let mut literal_text_start_index = text_start_index;
    let mut text_buf: Option<String> = None;

    loop {
      let c = match self.peek_char() {
        None | Some('\n') => self.emit_error("unterminated string".to_owned())?,
        Some(c) => c,
      };
      self.next_char();
      match c {
        '\"' => break,

        '\\' => {
          let literal_text = &self.src[literal_text_start_index..self.current_pos.byte_index];
          let c = match self.peek_char() {
            None => self.emit_error("expected a character to escape".to_owned())?,
            Some(c) => c,
          };
          self.next_char();

          let unescaped_char = match c {
            '\n' => None,
            _ => Some(match c {
              'n' => '\n',
              't' => '\t',
              'b' => '\x08',
              'r' => '\r',
              'f' => '\x0C',
              'v' => '\x0B',
              'a' => '\x07',
              '\\' | '\"' => c,
              // TODO: octal (optional), hex and unicode escape sequences
              _ => self.emit_error(format!("unknown escaped character {:?}", c))?,
            }),
          };

          let text_buf = text_buf.get_or_insert_with(|| {
            String::with_capacity(literal_text.len() + unescaped_char.map_or(0, char::len_utf8))
          });
          text_buf.push_str(literal_text);
          if let Some(unescaped_char) = unescaped_char {
            text_buf.push(unescaped_char);
          }
          literal_text_start_index = self.next_char_index;
        }

        _ => {}
      }
    }

    let last_literal_text = &self.src[literal_text_start_index..self.current_pos.byte_index];
    let text_cow = match text_buf {
      Some(mut text_buf) => {
        text_buf.push_str(last_literal_text);
        Cow::Owned(text_buf)
      }
      None => Cow::Borrowed(last_literal_text),
    };

    Ok(Some(match self.is_previous_entry {
      false => TokenType::String(text_cow),
      true => TokenType::PrevString(text_cow),
    }))
  }

  fn parse_keyword(&mut self) -> Result<Option<TokenType<'src>>, ParsingError> {
    while self.peek_char().map_or(false, |c| c.is_ascii_alphanumeric() || c == '_') {
      self.next_char();
    }
    let keyword = &self.src[self.token_start_pos.byte_index..self.next_char_index];

    Ok(Some(match (self.is_previous_entry, keyword) {
      (false, "domain") => self.emit_error(
        "the \"domain\" keyword is unsupported due to the lack of documentation about it"
          .to_owned(),
      )?,
      (true, "msgctxt") => TokenType::PrevMsgctxt,
      (false, "msgctxt") => TokenType::Msgctxt,
      (true, "msgid") => TokenType::PrevMsgid,
      (false, "msgid") => TokenType::Msgid,
      (false, "msgstr") => TokenType::Msgstr,
      (_, "msgid_plural") | (false, "msgstr_plural") => self.emit_error(format!(
        "keyword {:?} is unsupported because plurals were unneeded and thus are unsupported",
        keyword,
      ))?,
      _ => self.emit_error(format!("unexepected keyword {:?}", keyword))?,
    }))
  }
}

impl<'src> Iterator for Lexer<'src> {
  type Item = Result<Token<'src>, ParsingError>;

  fn next(&mut self) -> Option<Self::Item> {
    match self.parse_next_token() {
      Ok(Some(v)) => Some(Ok(v)),
      Ok(None) => None,
      Err(e) => Some(Err(e)),
    }
  }
}
