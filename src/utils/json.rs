use crate::impl_prelude::*;

use std::borrow::Cow;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub use serde_json::Value;

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
) -> AnyResult<T> {
  *out_bytes = fs::read(path)?;
  let value = serde_json::from_slice(out_bytes)?;
  Ok(value)
}

pub fn write_file<T: serde::Serialize>(path: &Path, value: &T) -> AnyResult<()> {
  let mut writer = io::BufWriter::new(fs::File::create(path)?);
  serde_json::to_writer_pretty(&mut writer, value)?;
  writer.write_all(b"\n")?;
  writer.flush()?;
  Ok(())
}
