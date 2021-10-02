use serde_json::ser::{CharEscape, Formatter};
use std::borrow::Cow;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub use serde_json::{Map, Value};

#[allow(missing_debug_implementations)]
pub enum ValueEntriesIter<'a> {
  Array { iter: std::slice::Iter<'a, Value>, counter: usize },
  Object { iter: serde_json::map::Iter<'a> },
}

impl<'a> ValueEntriesIter<'a> {
  pub fn new(value: &'a Value) -> Option<Self> {
    Some(match value {
      Value::Array(vec) => Self::Array { iter: vec.iter(), counter: 0 },
      Value::Object(map) => Self::Object { iter: map.iter() },
      _ => return None,
    })
  }
}

impl<'a> Iterator for ValueEntriesIter<'a> {
  type Item = (Cow<'a, str>, &'a Value);

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Array { iter, counter, .. } => {
        let (k, v) = (counter.to_string(), iter.next()?);
        *counter += 1;
        Some((Cow::Owned(k), v))
      }
      Self::Object { iter, .. } => {
        let (k, v) = iter.next()?;
        Some((Cow::Borrowed(k), v))
      }
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    match self {
      Self::Array { iter, .. } => iter.size_hint(),
      Self::Object { iter, .. } => iter.size_hint(),
    }
  }

  fn count(self) -> usize {
    match self {
      Self::Array { iter, .. } => iter.count(),
      Self::Object { iter, .. } => iter.count(),
    }
  }
}

pub fn read_file<'a, T: serde::Deserialize<'a>>(
  path: &Path,
  out_bytes: &'a mut Vec<u8>,
) -> io::Result<T> {
  *out_bytes = fs::read(path)?;
  let value = serde_json::from_slice(out_bytes)?;
  Ok(value)
}

pub fn write_file<T: serde::Serialize>(
  path: &Path,
  value: &T,
  config: UltimateFormatterConfig,
) -> io::Result<()> {
  let mut writer = io::BufWriter::new(fs::File::create(path)?);
  let mut serializer =
    serde_json::Serializer::with_formatter(&mut writer, UltimateFormatter::new(config));
  value.serialize(&mut serializer)?;
  writer.write_all(b"\n")?;
  writer.flush()?;
  Ok(())
}

/// Copied from <https://github.com/serde-rs/json/blob/9b64e0b17ca73e7fbecace37758ff19bc35dea05/src/ser.rs#L2066-L2075>.
pub fn format_escaped_str<W, F>(writer: &mut W, formatter: &mut F, value: &str) -> io::Result<()>
where
  W: ?Sized + Write,
  F: ?Sized + Formatter,
{
  formatter.begin_string(writer)?;
  format_escaped_str_contents(writer, formatter, value)?;
  formatter.end_string(writer)?;
  Ok(())
}

/// Copied from <https://github.com/serde-rs/json/blob/9b64e0b17ca73e7fbecace37758ff19bc35dea05/src/ser.rs#L2077-L2143>.
pub fn format_escaped_str_contents<W, F>(
  writer: &mut W,
  formatter: &mut F,
  value: &str,
) -> io::Result<()>
where
  W: ?Sized + Write,
  F: ?Sized + Formatter,
{
  let bytes = value.as_bytes();

  let mut start = 0;

  for (i, &byte) in bytes.iter().enumerate() {
    let escape = ESCAPE[byte as usize];
    if escape == 0 {
      continue;
    }

    if start < i {
      formatter.write_string_fragment(writer, &value[start..i])?;
    }

    let char_escape = match escape {
      BB => CharEscape::Backspace,
      TT => CharEscape::Tab,
      NN => CharEscape::LineFeed,
      FF => CharEscape::FormFeed,
      RR => CharEscape::CarriageReturn,
      QU => CharEscape::Quote,
      BS => CharEscape::ReverseSolidus,
      UU => CharEscape::AsciiControl(byte),
      _ => unreachable!(),
    };
    formatter.write_char_escape(writer, char_escape)?;

    start = i + 1;
  }

  if start != bytes.len() {
    formatter.write_string_fragment(writer, &value[start..])?;
  }

  return Ok(());

  const BB: u8 = b'b'; // \x08
  const TT: u8 = b't'; // \x09
  const NN: u8 = b'n'; // \x0A
  const FF: u8 = b'f'; // \x0C
  const RR: u8 = b'r'; // \x0D
  const QU: u8 = b'"'; // \x22
  const BS: u8 = b'\\'; // \x5C
  const UU: u8 = b'u'; // \x00...\x1F except the ones above
  const __: u8 = 0;

  /// See <https://github.com/serde-rs/json/blob/9b64e0b17ca73e7fbecace37758ff19bc35dea05/src/ser.rs#L2123-L2125>.
  static ESCAPE: [u8; 1 << 8] = [
    //   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
    UU, UU, UU, UU, UU, UU, UU, UU, BB, TT, NN, UU, FF, RR, UU, UU, // 0
    UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, UU, // 1
    __, __, QU, __, __, __, __, __, __, __, __, __, __, __, __, __, // 2
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 3
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 4
    __, __, __, __, __, __, __, __, __, __, __, __, BS, __, __, __, // 5
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 6
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 7
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 8
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // 9
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // A
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // B
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // C
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // D
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // E
    __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, __, // F
  ];
}

pub const DEFAULT_INDENT: &str = "  ";

#[derive(Debug, Clone)]
pub struct UltimateFormatterConfig {
  pub compact: bool,
  pub indent: Option<&'static str>,
  pub trailing_commas: bool,
}

impl UltimateFormatterConfig {
  pub const PRETTY: Self = Self {
    //
    compact: false,
    indent: Some(DEFAULT_INDENT),
    trailing_commas: false,
  };
  pub const COMPACT: Self = Self {
    //
    compact: true,
    indent: None,
    trailing_commas: false,
  };
}

impl Default for UltimateFormatterConfig {
  #[inline(always)]
  fn default() -> Self { Self::PRETTY }
}

/// Based on [`serde_json::ser::PrettyFormatter`].
#[derive(Debug)]
pub struct UltimateFormatter {
  current_indent: usize,
  has_value: bool,
  config: UltimateFormatterConfig,
}

impl UltimateFormatter {
  #[inline]
  pub fn new(config: UltimateFormatterConfig) -> Self {
    UltimateFormatter { current_indent: 0, has_value: false, config }
  }

  fn indent<W>(wr: &mut W, n: usize, s: &str) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    for _ in 0..n {
      wr.write_all(s.as_bytes())?;
    }
    Ok(())
  }
}

impl Formatter for UltimateFormatter {
  #[inline]
  fn begin_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.current_indent += 1;
    self.has_value = false;
    writer.write_all(b"[")
  }

  #[inline]
  fn end_array<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.current_indent -= 1;

    if self.config.trailing_commas {
      writer.write_all(b",")?;
    }

    if let Some(indent) = self.config.indent {
      if self.has_value {
        writer.write_all(b"\n")?;
        Self::indent(writer, self.current_indent, indent)?;
      }
    }

    writer.write_all(b"]")
  }

  #[inline]
  fn begin_array_value<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    if !first {
      writer.write_all(b",")?;
    }

    if let Some(indent) = self.config.indent {
      writer.write_all(b"\n")?;
      Self::indent(writer, self.current_indent, indent)?;
    }

    Ok(())
  }

  #[inline]
  fn end_array_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.has_value = true;
    Ok(())
  }

  #[inline]
  fn begin_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.current_indent += 1;
    self.has_value = false;
    writer.write_all(b"{")
  }

  #[inline]
  fn end_object<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.current_indent -= 1;

    if self.config.trailing_commas {
      writer.write_all(b",")?;
    }

    if let Some(indent) = self.config.indent {
      if self.has_value {
        writer.write_all(b"\n")?;
        Self::indent(writer, self.current_indent, indent)?;
      }
    }

    writer.write_all(b"}")
  }

  #[inline]
  fn begin_object_key<W>(&mut self, writer: &mut W, first: bool) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    if !first {
      writer.write_all(b",")?;
    }

    if let Some(indent) = self.config.indent {
      writer.write_all(b"\n")?;
      Self::indent(writer, self.current_indent, indent)?;
    }

    Ok(())
  }

  #[inline]
  fn begin_object_value<W>(&mut self, writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    writer.write_all(if self.config.compact { b":" } else { b": " })
  }

  #[inline]
  fn end_object_value<W>(&mut self, _writer: &mut W) -> io::Result<()>
  where
    W: ?Sized + Write,
  {
    self.has_value = true;
    Ok(())
  }
}
