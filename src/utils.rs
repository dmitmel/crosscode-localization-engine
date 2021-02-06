pub mod json;
pub mod serde;

use crate::impl_prelude::*;

use std::borrow::Cow;
use std::convert::TryFrom;
use std::fs;
use std::io;
use std::path::Path;
use std::rc::{Rc, Weak as RcWeak};
use std::time::SystemTime;
use uuid::Uuid;

/// The One UUID generator, wrapped into a function for refactorability if
/// needed to switch to another UUID version.
#[inline(always)]
pub fn new_uuid() -> Uuid { Uuid::new_v4() }

pub type Timestamp = i64;

pub fn get_timestamp() -> Timestamp {
  Timestamp::try_from(SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs())
    .expect("hello, fellow programmers and CrossCode translators of the future!")
}

pub fn fast_concat(strings: &[&str]) -> String {
  let mut capacity = 0;
  for s in strings {
    capacity += s.len();
  }
  let mut result = String::with_capacity(capacity);
  for s in strings {
    result.push_str(s);
  }
  result
}

pub fn fast_concat_cow(strings: &[Cow<str>]) -> String {
  let mut capacity = 0;
  for s in strings {
    capacity += s.len();
  }
  let mut result = String::with_capacity(capacity);
  for s in strings {
    result.push_str(s);
  }
  result
}

// Type hints for the `try { ... }` blocks

#[inline(always)]
pub fn try_any_result_hint<T>(r: AnyResult<T>) -> AnyResult<T> { r }
#[inline(always)]
pub fn try_option_hint<T>(r: Option<T>) -> Option<T> { r }

pub trait RcExt<T: ?Sized>: private::Sealed {
  fn share_rc(&self) -> Rc<T>;
  fn share_rc_weak(&self) -> RcWeak<T>;
  fn rc_clone_inner(&self) -> T
  where
    T: Clone;
}

impl<T: ?Sized> RcExt<T> for Rc<T> {
  #[inline(always)]
  fn share_rc(&self) -> Rc<T> { Rc::clone(self) }
  #[inline(always)]
  fn share_rc_weak(&self) -> RcWeak<T> { Rc::downgrade(self) }
  #[inline(always)]
  fn rc_clone_inner(&self) -> T
  where
    T: Clone,
  {
    (**self).clone()
  }
}

pub trait RcWeakExt<T: ?Sized>: private::Sealed {
  fn share_rc_weak(&self) -> RcWeak<T>;
}

impl<T: ?Sized> RcWeakExt<T> for RcWeak<T> {
  #[inline(always)]
  fn share_rc_weak(&self) -> RcWeak<T> { RcWeak::clone(self) }
}

mod private {
  pub trait Sealed {}
  impl<T: ?Sized> Sealed for T {
  }
}

#[inline]
pub fn is_default<T: Default + PartialEq>(t: &T) -> bool { *t == T::default() }

/// Taken from <https://stackoverflow.com/a/40457615>.
#[derive(Debug)]
pub struct LinesWithEndings<'a> {
  text: &'a str,
}

impl<'a> LinesWithEndings<'a> {
  #[inline(always)]
  pub fn new(text: &'a str) -> LinesWithEndings<'a> { LinesWithEndings { text } }
  #[inline(always)]
  pub fn as_str(&self) -> &'a str { &self.text }
}

impl<'a> Iterator for LinesWithEndings<'a> {
  type Item = &'a str;
  fn next(&mut self) -> Option<Self::Item> {
    if self.text.is_empty() {
      return None;
    }
    #[allow(clippy::or_fun_call)]
    let split = self.text.find('\n').map(|i| i + 1).unwrap_or(self.text.len());
    let (line, rest) = self.text.split_at(split);
    self.text = rest;
    Some(line)
  }
}

pub fn create_dir_recursively(path: &Path) -> io::Result<()> {
  fs::DirBuilder::new().recursive(true).create(path)
}

pub fn split_filename_extension(filename: &str) -> (&str, Option<&str>) {
  if let Some(dot_index) = filename.rfind('.') {
    if dot_index > 0 {
      // Safe because `rfind` is guaranteed to return valid character indices.
      let stem = unsafe { filename.get_unchecked(..dot_index) };
      // Safe because in addition to above, byte length of the string "."
      // (which we have to skip and not include in the extension) encoded in
      // UTF-8 is exactly 1.
      let ext = unsafe { filename.get_unchecked(dot_index + 1..) };
      return (stem, Some(ext));
    }
  }
  (filename, None)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_split_filename_extension() {
    assert_eq!(split_filename_extension(""), ("", None));
    assert_eq!(split_filename_extension("name"), ("name", None));
    assert_eq!(split_filename_extension(".name"), (".name", None));
    assert_eq!(split_filename_extension("name."), ("name", Some("")));
    assert_eq!(split_filename_extension(".name."), (".name", Some("")));
    assert_eq!(split_filename_extension("name.ext"), ("name", Some("ext")));
    assert_eq!(split_filename_extension(".name.ext"), (".name", Some("ext")));
    assert_eq!(split_filename_extension("name.ext."), ("name.ext", Some("")));
    assert_eq!(split_filename_extension(".name.ext."), (".name.ext", Some("")));
    assert_eq!(split_filename_extension("name.ext1.ext2"), ("name.ext1", Some("ext2")));
    assert_eq!(split_filename_extension(".name.ext1.ext2"), (".name.ext1", Some("ext2")));
  }
}
