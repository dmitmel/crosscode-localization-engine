pub mod json;

use crate::impl_prelude::*;

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

pub type Timestamp = u64;

pub fn get_timestamp() -> u64 {
  SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
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

// Type hints for the `try { ... }` blocks

#[inline(always)]
pub fn try_any_result_hint<T>(r: AnyResult<T>) -> AnyResult<T> { r }
#[inline(always)]
pub fn try_option_hint<T>(r: Option<T>) -> Option<T> { r }

pub trait ShareRc: private::Sealed {
  fn share_rc(&self) -> Self;
}

impl<T: ?Sized> ShareRc for Rc<T> {
  #[inline(always)]
  fn share_rc(&self) -> Self { Rc::clone(self) }
}

pub trait ShareRcWeak: private::Sealed {
  type Weak;
  fn share_rc_weak(&self) -> Self::Weak;
}

impl<T: ?Sized> ShareRcWeak for Rc<T> {
  type Weak = RcWeak<T>;
  #[inline(always)]
  fn share_rc_weak(&self) -> Self::Weak { Rc::downgrade(self) }
}

impl<T: ?Sized> ShareRcWeak for RcWeak<T> {
  type Weak = Self;
  #[inline(always)]
  fn share_rc_weak(&self) -> Self::Weak { RcWeak::clone(self) }
}

mod private {
  pub trait Sealed {}
  impl<T: ?Sized> Sealed for T {
  }
}

#[inline]
pub fn is_default<T: Default + PartialEq>(t: &T) -> bool { t == &T::default() }

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
