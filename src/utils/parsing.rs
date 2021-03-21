use crate::impl_prelude::*;
use crate::rc_string::RcString;

use std::fmt;
use std::iter;
use std::str;
use std::str::pattern::Pattern;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CharPos {
  pub byte_index: usize,
  pub char_index: usize,
  pub line: usize,
  pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    NiceParsingErrorFormatter { error: self, filename, src: Some(src) }
  }
}

impl fmt::Display for ParsingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(
      &NiceParsingErrorFormatter { error: self, filename: "<unknown>", src: None },
      f,
    )
  }
}

impl StdError for ParsingError {
}

#[derive(Debug)]
pub struct NiceParsingErrorFormatter<'error> {
  error: &'error ParsingError,
  filename: &'error str,
  src: Option<&'error str>,
}

impl<'error> fmt::Display for NiceParsingErrorFormatter<'error> {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let pos = self.error.pos;
    writeln!(f, "Syntax error in {}:{}:{}", self.filename, pos.line, pos.column)?;
    if let Some(line_text) = try { CharPosIter::find_line(self.src?, pos.line)? } {
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
      current_pos: CharPos::default(),
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
          line_start_index = pos.byte_index;
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
      line_end_index = pos.byte_index + c.len_utf8();
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
    self.current_pos.byte_index = i;
    self.current_pos.char_index += 1;
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

/// Taken from <https://users.rust-lang.org/t/how-to-find-a-substring-starting-at-a-specific-index/8299/2>.
pub fn find_start_at<'a>(slice: &'a str, at: usize, pat: impl Pattern<'a>) -> Option<usize> {
  slice.get(at..)?.find(pat).map(|i| at + i)
}
