// See:
// <https://github.com/autotools-mirror/gettext/blob/6c9cff1221f2cbf585fbee6f86ff047c8ede5286/gettext-tools/src/po-gram-gen.y>
// <https://github.com/autotools-mirror/gettext/blob/6c9cff1221f2cbf585fbee6f86ff047c8ede5286/gettext-tools/src/po-lex.c>
// <https://www.gnu.org/software/gettext/manual/gettext.html#PO-Files>
// <https://www.gnu.org/software/gettext/manual/gettext.html#Filling-in-the-Header-Entry>
// <https://www.gnu.org/software/gettext/manual/gettext.html#Invoking-the-msginit-Program>
// <https://github.com/izimobil/polib/blob/0ab9af63d227d30fb261c2dd496ee74f91844a86/polib.py>
// <https://github.com/translate/translate/blob/88d13bea244b1894a4bedf67ba5b8b65cc29d3b0/translate/storage/pypo.py>
// <https://github.com/translate/translate/blob/88d13bea244b1894a4bedf67ba5b8b65cc29d3b0/translate/storage/cpo.py>
// <https://docs.oasis-open.org/xliff/v1.2/xliff-profile-po-1.2-pr-02-20061016-DIFF.pdf>
//
// For testing the behavior of GNU gettext the following Python program can be
// used (launch with the environment variable `USECPO` set to `1`):
//
//     from translate.storage.po import pofile
//     import sys
//     POFile(sys.stdin.buffer).serialize(sys.stdout.buffer)
//
// Needless to say, it requires installation of <https://github.com/translate/translate>
// (also see <https://github.com/translate/translate/blob/88d13bea244b1894a4bedf67ba5b8b65cc29d3b0/translate/storage/po.py>).

pub mod lexer;
pub mod parser;

use crate::rc_string::RcString;
pub use lexer::Lexer;
pub use parser::{ParsedMessage, Parser};

use std::fmt;
use std::iter;
use std::str;

pub fn parse(src: &str) -> Parser { Parser::new(Lexer::new(src)) }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CharPos {
  pub index: usize,
  pub line: usize,
  pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParsingError {
  pub pos: CharPos,
  pub message: RcString,
}

impl ParsingError {
  #[inline(always)]
  pub fn nice_formatter<'error>(
    &'error self,
    filename: &'error str,
    src: &'error str,
  ) -> NiceParsingErrorFormatter<'error> {
    NiceParsingErrorFormatter { error: self, filename, src }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NiceParsingErrorFormatter<'error> {
  error: &'error ParsingError,
  filename: &'error str,
  src: &'error str,
}

impl<'error> fmt::Display for NiceParsingErrorFormatter<'error> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let pos = self.error.pos;
    writeln!(f, "Syntax error in {}:{}:{}", self.filename, pos.line, pos.column)?;
    if let Some(line_text) = CharPosIter::find_line(self.src, pos.line) {
      let line_number_str = format!("{}", pos.line);
      let line_number_margin = " ".repeat(line_number_str.len());
      writeln!(f, "{} | {}", line_number_str, line_text)?;
      // A whitespace is not added in the pointer line because column numbers
      // generally start at one, but there may be a situation when an error at
      // the column 0 makes sense.
      writeln!(f, "{} |{}^", line_number_margin, " ".repeat(pos.column))?;
      write!(f, "{} = {}", line_number_margin, self.error.message)?;
    } else {
      write!(f, "{}", self.error.message)?;
    }
    Ok(())
  }
}

#[derive(Debug, Clone)]
pub struct CharPosIter<'str> {
  iter: str::CharIndices<'str>,
  current_pos: CharPos,
  newline_char_reached: bool,
}

impl<'str> CharPosIter<'str> {
  #[inline(always)]
  pub fn as_str(&self) -> &'str str { self.iter.as_str() }
  #[inline(always)]
  pub fn current_pos(&self) -> CharPos { self.current_pos }

  pub fn new(string: &'str str) -> Self {
    Self {
      iter: string.char_indices(),
      current_pos: CharPos { index: 0, line: 0, column: 0 },
      newline_char_reached: true,
    }
  }

  pub fn find_line(string: &str, line_number: usize) -> Option<&str> {
    let mut found_the_line = false;
    let mut line_start_index: usize = 0;
    let mut line_end_index: usize = 0;

    for (pos, c) in CharPosIter::new(string) {
      if !found_the_line {
        // Phase 1: find the line
        if pos.line == line_number {
          line_start_index = pos.index;
          found_the_line = true;
        }
      } else {
        // Phase 2: find the end of the line
        if c == '\n' || pos.line != line_number {
          break;
        }
      }
      // During the Phase 1 this end index doesn't have much value, but once
      // the beginning of the line has been found, we must immediately start
      // recording the end index because the line might be 1 character long and
      // the last one in the file, in which case the Phase 2 is never reached.
      // And in all other cases the loop continues rolling and the check in the
      // Phase 2 branch is evaluated to stop right at the end of the line.
      line_end_index = pos.index + c.len_utf8();
    }

    if found_the_line {
      Some(&string[line_start_index..line_end_index])
    } else {
      None
    }
  }
}

impl<'str> Iterator for CharPosIter<'str> {
  type Item = (CharPos, char);

  fn next(&mut self) -> Option<Self::Item> {
    let (i, c) = self.iter.next()?;
    self.current_pos.index = i;
    if self.newline_char_reached {
      self.current_pos.column = 1;
      self.current_pos.line += 1;
    } else {
      self.current_pos.column += 1;
    }
    self.newline_char_reached = c == '\n';
    Some((self.current_pos, c))
  }

  #[inline(always)]
  fn count(self) -> usize { self.iter.count() }

  #[inline(always)]
  fn size_hint(&self) -> (usize, Option<usize>) { self.iter.size_hint() }
}

impl iter::FusedIterator for CharPosIter<'_> {
}
