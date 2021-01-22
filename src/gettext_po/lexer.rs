// See <https://github.com/autotools-mirror/gettext/blob/6c9cff1221f2cbf585fbee6f86ff047c8ede5286/gettext-tools/src/po-lex.c>
// For testing the behavior of GNU gettext the following Python program can be
// used (launch with the environment variable `USECPO` set to `1`):
//
//     from translate.storage.po import pofile
//     import sys
//     POFile(sys.stdin.buffer).serialize(sys.stdout.buffer)
//
// Needless to say, it requires installation of <https://github.com/translate/translate>
// (also see <https://github.com/translate/translate/blob/88d13bea244b1894a4bedf67ba5b8b65cc29d3b0/translate/storage/po.py>).

use std::borrow::Cow;
use std::iter;
use std::str;

#[derive(Debug)]
pub struct Token<'src> {
  pub start_pos: CharPos,
  pub end_pos: CharPos,
  pub type_: TokenType<'src>,
}

#[derive(Debug, Clone, Copy)]
pub struct CharPos {
  pub index: usize,
  pub line: usize,
  pub column: usize,
}

#[derive(Debug)]
pub enum TokenType<'src> {
  PreviousMarker,
  Newline,
  Domain,
  Msgctxt,
  Msgid,
  Msgstr,
  Comment(CommentType, &'src str),
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
pub struct ParsingError {
  pub pos: CharPos,
  pub message: String,
}

#[derive(Debug)]
pub struct Lexer<'src> {
  src: &'src str,
  src_iter: iter::Peekable<str::CharIndices<'src>>,
  has_error: bool,
  token_start_pos: CharPos,
  current_pos: CharPos,
  newline_char_reached: bool,
  next_char_index: usize,
}

impl<'src> Lexer<'src> {
  pub fn lex(src: &'src str) -> Self {
    let current_pos = CharPos { index: 0, line: 0, column: 0 };
    Self {
      src,
      src_iter: src.char_indices().peekable(),
      has_error: false,
      token_start_pos: current_pos,
      current_pos,
      newline_char_reached: true,
      next_char_index: 0,
    }
  }

  fn next_char(&mut self) -> Option<char> {
    let (chr_pos, chr) = self.src_iter.next()?;
    self.current_pos.index = chr_pos;
    self.next_char_index = chr_pos + chr.len_utf8();
    if self.newline_char_reached {
      self.current_pos.column = 1;
      self.current_pos.line += 1;
    } else {
      self.current_pos.column += 1;
    }
    self.newline_char_reached = chr == '\n';
    Some(chr)
  }

  fn peek_char(&mut self) -> Option<char> {
    let &(_chr_pos, chr) = self.src_iter.peek()?;
    Some(chr)
  }

  fn begin_token(&mut self) { self.token_start_pos = self.current_pos; }
  fn end_token(&self, type_: TokenType<'src>) -> Token<'src> {
    Token { start_pos: self.token_start_pos, end_pos: self.current_pos, type_ }
  }

  fn emit_error(&mut self, message: String) -> ParsingError {
    self.has_error = true;
    ParsingError { pos: self.current_pos, message }
  }
}

impl<'src> Iterator for Lexer<'src> {
  type Item = Result<Token<'src>, ParsingError>;

  fn next(&mut self) -> Option<Self::Item> {
    assert!(!self.has_error);

    macro_rules! emit_error {
      ($($arg:tt)*) => {
        {
          return Some(Err(self.emit_error(format!($($arg)*))));
        }
      }
    }

    while self.peek_char().map_or(false, |c| {
      // NOTE: is_ascii_whitespace doesn't match \v (which GNU gettext
      // considers whitespace) and \n needs to be emitted as a token
      matches!(c, '\t' | /* \v */ '\x0B' | /* \f */ '\x0C' | '\r' | ' ')
    }) {
      self.next_char();
    }

    let c = self.next_char()?;
    self.begin_token();
    let token_type = match c {
      '\n' => TokenType::Newline,

      '#' => match self.peek_char() {
        Some('~') => emit_error!("obsolete entries are unsupported"),
        Some('|') => {
          self.next_char();
          TokenType::PreviousMarker
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
          TokenType::Comment(comment_type, text)
        }
      },

      '\"' => {
        let text_start_index = self.next_char_index;
        let mut literal_text_start_index = text_start_index;
        let mut text_buf: Option<String> = None;

        loop {
          let c = match self.peek_char() {
            None | Some('\n') => emit_error!("unterminated string"),
            Some(c) => c,
          };
          self.next_char();
          match c {
            '\"' => break,

            '\\' => {
              let literal_text = &self.src[literal_text_start_index..self.current_pos.index];
              let c = match self.peek_char() {
                None | Some('\n') => emit_error!("expected a character to escape"),
                Some(c) => c,
              };
              self.next_char();

              let text_buf =
                text_buf.get_or_insert_with(|| String::with_capacity(literal_text.len() + 1));
              text_buf.push_str(literal_text);
              text_buf.push(match c {
                'n' => '\n',
                't' => '\t',
                'b' => '\x08',
                'r' => '\r',
                'f' => '\x0C',
                'v' => '\x0B',
                'a' => '\x07',
                '\\' | '\"' => c,
                _ => emit_error!("unknown escaped character {:?}", c),
              });
              literal_text_start_index = self.next_char_index;
            }
            _ => {}
          }
        }

        let last_literal_text = &self.src[literal_text_start_index..self.current_pos.index];
        TokenType::String(match text_buf {
          Some(mut text_buf) => {
            text_buf.push_str(last_literal_text);
            Cow::Owned(text_buf)
          }
          None => Cow::Borrowed(last_literal_text),
        })
      }

      _ if c.is_ascii_alphabetic() || c == '_' => {
        while self.peek_char().map_or(false, |c| c.is_ascii_alphanumeric() || c == '_') {
          self.next_char();
        }
        let keyword = &self.src[self.token_start_pos.index..self.next_char_index];
        match keyword {
          "domain" => TokenType::Domain,
          "msgctxt" => TokenType::Msgctxt,
          "msgid" => TokenType::Msgid,
          "msgstr" => TokenType::Msgstr,
          "msgid_plural" | "msgstr_plural" => emit_error!(
            "keyword {:?} is unsupported because plurals were unneeded and thus are unsupported",
            keyword,
          ),
          _ => emit_error!("unknown keyword {:?}", keyword),
        }
      }

      _ => emit_error!("unexpected character {:?}", c),
    };

    Some(Ok(self.end_token(token_type)))
  }
}
