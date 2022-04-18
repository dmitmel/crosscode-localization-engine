pub mod json;
pub mod parsing;
pub mod serde;

use crate::impl_prelude::*;

use indexmap::IndexMap;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::fs;
use std::io;
use std::marker::PhantomData;
use std::path::{Component as PathComponent, Path, PathBuf};
use std::rc::{Rc, Weak as RcWeak};
use std::sync::{Arc, Weak as ArcWeak};
use std::time::SystemTime;
use uuid::Uuid;

/// The One UUID generator, wrapped into a function for refactorability if
/// needed to switch to another UUID version.
#[inline(always)]
pub fn new_uuid() -> Uuid { Uuid::new_v4() }

pub const UUID_BYTE_LEN: usize = 16;
pub const COMPACT_UUID_BYTE_LEN: usize = 26;

/// The encoder/decoder of compact UUIDs is a domain-optimized variant of
/// <https://github.com/andreasots/base32/blob/d901ddebf9254d5730a64ca7286df47f0fd78bdb/src/lib.rs>.
pub type CompactUuid = [u8; COMPACT_UUID_BYTE_LEN];

/// <https://tools.ietf.org/html/rfc4648>
static BASE32_ALPHABET: [u8; 32] = *b"abcdefghijklmnopqrstuvwxyz234567";
/// <https://tools.ietf.org/html/rfc4648>
#[rustfmt::skip]
static BASE32_INV_ALPHABET: [i8; 1 << 8] = [
  //   1   2   3   4   5   6   7   8   9   A   B   C   D   E   F
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 0
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 1
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 2
  -1, -1, 26, 27, 28, 29, 30, 31, -1, -1, -1, -1, -1,  0, -1, -1, // 3
  -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, // 4
  15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1, // 5
  -1,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, // 6
  15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1, // 7
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 8
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // 9
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // A
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // B
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // C
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // D
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // E
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, // F
];
const BASE32_INPUT_BLOCK_SIZE: usize = 5;
const BASE32_ENCODED_BLOCK_SIZE: usize = 8;

/// This function should compile down to assembly without branching or panics.
/// Optimized variant of <https://github.com/andreasots/base32/blob/d901ddebf9254d5730a64ca7286df47f0fd78bdb/src/lib.rs#L12-L53>.
pub fn encode_compact_uuid(uuid: &Uuid) -> CompactUuid {
  let input: &[u8; UUID_BYTE_LEN] = uuid.as_bytes();
  let mut input_padded = [0u8; BASE32_INPUT_BLOCK_SIZE * 4];
  input_padded[..UUID_BYTE_LEN].copy_from_slice(input);

  let mut out_buffer = [0u8; BASE32_ENCODED_BLOCK_SIZE * 4];
  let mut out_current: &mut [u8] = &mut out_buffer;
  for block in input_padded.chunks(BASE32_INPUT_BLOCK_SIZE) {
    let alphabet = BASE32_ALPHABET;
    #[rustfmt::skip]
    let encoded_block: [u8; BASE32_ENCODED_BLOCK_SIZE] = [
      alphabet[( (block[0] & 0xF8) >> 3)                             as usize],
      alphabet[(((block[0] & 0x07) << 2) | ((block[1] & 0xC0) >> 6)) as usize],
      alphabet[( (block[1] & 0x3E) >> 1)                             as usize],
      alphabet[(((block[1] & 0x01) << 4) | ((block[2] & 0xF0) >> 4)) as usize],
      alphabet[(((block[2] & 0x0F) << 1) |  (block[3]         >> 7)) as usize],
      alphabet[( (block[3] & 0x7C) >> 2)                             as usize],
      alphabet[(((block[3] & 0x03) << 3) | ((block[4] & 0xE0) >> 5)) as usize],
      alphabet[ ( block[4] & 0x1F      )                             as usize],
    ];
    out_current[..BASE32_ENCODED_BLOCK_SIZE].copy_from_slice(&encoded_block);
    out_current = &mut out_current[BASE32_ENCODED_BLOCK_SIZE..];
  }

  let mut encoded: CompactUuid = [0; COMPACT_UUID_BYTE_LEN];
  encoded.copy_from_slice(&out_buffer[..COMPACT_UUID_BYTE_LEN]);
  encoded
}

/// This function should compile down to assembly without branching or panics.
/// Optimized variant of <https://github.com/andreasots/base32/blob/d901ddebf9254d5730a64ca7286df47f0fd78bdb/src/lib.rs#L55-L101>.
pub fn decode_compact_uuid(compact_uuid: &CompactUuid) -> Result<Uuid, usize> {
  let encoded: &[u8; COMPACT_UUID_BYTE_LEN] = compact_uuid;
  let mut encoded_padded = [0u8; BASE32_ENCODED_BLOCK_SIZE * 4];
  encoded_padded[..COMPACT_UUID_BYTE_LEN].copy_from_slice(encoded);

  for (idx, byte) in encoded_padded[..COMPACT_UUID_BYTE_LEN].iter_mut().enumerate() {
    let value: i8 = BASE32_INV_ALPHABET[*byte as usize];
    if value < 0 {
      return Err(idx);
    }
    *byte = value as u8;
  }

  let mut out_buffer = [0u8; BASE32_INPUT_BLOCK_SIZE * 4];
  let mut out_current: &mut [u8] = &mut out_buffer;
  for block in encoded_padded.chunks(BASE32_ENCODED_BLOCK_SIZE) {
    #[rustfmt::skip]
    let decoded_block: [u8; BASE32_INPUT_BLOCK_SIZE] = [
      ((block[0] << 3) | (block[1] >> 2)                  ),
      ((block[1] << 6) | (block[2] << 1) | (block[3] >> 4)),
      ((block[3] << 4) | (block[4] >> 1)                  ),
      ((block[4] << 7) | (block[5] << 2) | (block[6] >> 3)),
      ((block[6] << 5) |  block[7]                        ),
    ];
    out_current[..BASE32_INPUT_BLOCK_SIZE].copy_from_slice(&decoded_block);
    out_current = &mut out_current[BASE32_INPUT_BLOCK_SIZE..];
  }

  let mut decoded: uuid::Bytes = [0; UUID_BYTE_LEN];
  decoded.copy_from_slice(&out_buffer[..UUID_BYTE_LEN]);
  Ok(Uuid::from_bytes(decoded))
}

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

pub trait RcExt<T: ?Sized>: private::Sealed {
  fn share_rc(&self) -> Rc<T>;
  fn share_rc_weak(&self) -> RcWeak<T>;
  fn rc_clone_inner(&self) -> T
  where
    T: Clone;
}

impl<T: ?Sized> RcExt<T> for Rc<T> {
  #[inline(always)]
  fn share_rc(&self) -> Rc<T> { Self::clone(self) }
  #[inline(always)]
  fn share_rc_weak(&self) -> RcWeak<T> { Self::downgrade(self) }
  #[inline(always)]
  fn rc_clone_inner(&self) -> T
  where
    T: Clone,
  {
    (**self).clone()
  }
}

pub trait ArcExt<T: ?Sized>: private::Sealed {
  fn share_rc(&self) -> Arc<T>;
  fn share_rc_weak(&self) -> ArcWeak<T>;
  fn rc_clone_inner(&self) -> T
  where
    T: Clone;
}

impl<T: ?Sized> ArcExt<T> for Arc<T> {
  #[inline(always)]
  fn share_rc(&self) -> Arc<T> { Self::clone(self) }
  #[inline(always)]
  fn share_rc_weak(&self) -> ArcWeak<T> { Self::downgrade(self) }
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
  fn share_rc_weak(&self) -> RcWeak<T> { Self::clone(self) }
}

pub trait ArcWeakExt<T: ?Sized>: private::Sealed {
  fn share_rc_weak(&self) -> ArcWeak<T>;
}

impl<T: ?Sized> ArcWeakExt<T> for ArcWeak<T> {
  #[inline(always)]
  fn share_rc_weak(&self) -> ArcWeak<T> { Self::clone(self) }
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
  pub fn as_str(&self) -> &'a str { self.text }
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

/// See <https://github.com/rust-lang/rust/blob/1.55.0/library/std/src/fs.rs#L201-L207>
/// and <https://github.com/rust-lang/rust/commit/a990c76d84ccc5c285cbd533ea6020778fa18863>.
pub fn buffer_capacity_for_reading_file(file: &fs::File) -> usize {
  match file.metadata() {
    Ok(m) => m.len() as usize + 1,
    Err(_) => 0,
  }
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

pub trait IsEmpty {
  fn is_empty(&self) -> bool;
}

impl<T> IsEmpty for Vec<T> {
  #[inline(always)]
  fn is_empty(&self) -> bool { self.is_empty() }
}

impl<K, V> IsEmpty for HashMap<K, V> {
  #[inline(always)]
  fn is_empty(&self) -> bool { self.is_empty() }
}

impl<T> IsEmpty for HashSet<T> {
  #[inline(always)]
  fn is_empty(&self) -> bool { self.is_empty() }
}

impl<K, V> IsEmpty for IndexMap<K, V> {
  #[inline(always)]
  fn is_empty(&self) -> bool { self.is_empty() }
}

impl<T: IsEmpty> IsEmpty for RefCell<T> {
  #[inline(always)]
  fn is_empty(&self) -> bool { self.borrow().is_empty() }
}

impl<T: IsEmpty> IsEmpty for Rc<T> {
  #[inline(always)]
  fn is_empty(&self) -> bool { (**self).is_empty() }
}

/// Copied from <https://github.com/rust-lang/cargo/blob/4c27c96645e235d81f6c8dfff03ff9ebaf0ef71d/crates/cargo-util/src/paths.rs#L73-L106>.
pub fn normalize_path(path: &Path) -> PathBuf {
  let mut components = path.components().peekable();
  let mut ret = if let Some(c @ PathComponent::Prefix(..)) = components.peek().cloned() {
    components.next();
    PathBuf::from(c.as_os_str())
  } else {
    PathBuf::new()
  };
  for component in components {
    match component {
      PathComponent::Prefix(..) => unreachable!(),
      PathComponent::RootDir => {
        ret.push(component.as_os_str());
      }
      PathComponent::CurDir => {}
      PathComponent::ParentDir => {
        ret.pop();
      }
      PathComponent::Normal(c) => {
        ret.push(c);
      }
    }
  }
  ret
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

  #[test]
  fn test_compact_uuid() {
    for _ in 1..1000 {
      let initial_uuid = Uuid::new_v4();
      let mut current_uuid = initial_uuid;
      for _ in 1..10 {
        current_uuid = decode_compact_uuid(&encode_compact_uuid(&current_uuid)).unwrap();
      }
      assert_eq!(current_uuid, initial_uuid);
    }
  }
}

#[derive(Debug)]
pub struct StrategicalRegistry<A, R> {
  ids: Vec<&'static str>,
  map: HashMap<&'static str, fn(A) -> AnyResult<R>>,
  _phantom: PhantomData<(A, R)>,
}

impl<A, R> StrategicalRegistry<A, R> {
  pub fn new(declarations: &[StrategyDeclaration<A, R>]) -> Self {
    let mut ids = Vec::new();
    let mut map = HashMap::new();
    for decl in declarations {
      ids.push(decl.id);
      if map.insert(decl.id, decl.ctor).is_some() {
        panic!("Duplicate strategy was registered for: {:?}", decl.id);
      }
    }
    Self { ids, map, _phantom: PhantomData }
  }

  #[inline(always)]
  pub fn ids(&self) -> &[&'static str] { &self.ids }
  #[inline(always)]
  pub fn map(&self) -> &HashMap<&'static str, fn(A) -> AnyResult<R>> { &self.map }

  pub fn get_constructor(&self, id: &str) -> Option<&fn(A) -> AnyResult<R>> { self.map.get(id) }

  pub fn create(&self, id: &str, args: A) -> AnyResult<R> {
    let constructor =
      self.map.get(id).ok_or_else(|| format_err!("Strategy not found: {:?}", id))?;
    constructor(args)
  }
}

#[derive(Debug)]
pub struct StrategyDeclaration<A, R> {
  pub id: &'static str,
  pub ctor: fn(A) -> AnyResult<R>,
}
