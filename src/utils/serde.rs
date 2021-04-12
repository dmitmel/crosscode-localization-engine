// TODO: Apparently automatic borrowing via deserialization of Cow doesn't
// work. I need to implement the visitors myself.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::cell::{Ref, RefCell, RefMut};
use std::convert::TryFrom;
use std::str;
use uuid::Uuid;

pub const MULTILINE_STRING_WRAP_WIDTH: usize = 80;

#[derive(Debug)]
pub struct MultilineStringHelper;

impl MultilineStringHelper {
  pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
    T: AsRef<str>,
  {
    // TODO: find how to wrap text with preserving all whitespace, so that
    // original strings aren't corrupted.
    // let wrapper =
    //   textwrap::Wrapper::with_splitter(MULTILINE_STRING_WRAP_WIDTH, textwrap::NoHyphenation)
    //     .break_words(false);
    // let lines: Vec<Cow<str>> = super::LinesWithEndings::new(value.as_ref())
    //   .flat_map(|line| wrapper.wrap_iter(line))
    //   .collect();
    let lines: Vec<&str> = super::LinesWithEndings::new(value.as_ref()).collect();
    lines.serialize(serializer)
  }

  pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
  where
    D: Deserializer<'de>,
    T: From<String>,
  {
    let lines = Vec::<Cow<'de, str>>::deserialize(deserializer)?;
    Ok(super::fast_concat_cow(&lines).into())
  }
}

#[derive(Debug)]
pub struct MultilineStringHelperRefCell;

impl MultilineStringHelperRefCell {
  pub fn serialize<S, T>(value: &RefCell<T>, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
    T: AsRef<str>,
  {
    MultilineStringHelper::serialize(&*value.borrow(), serializer)
  }

  pub fn deserialize<'de, D, T>(deserializer: D) -> Result<RefCell<T>, D::Error>
  where
    D: Deserializer<'de>,
    T: From<String>,
  {
    Ok(RefCell::new(MultilineStringHelper::deserialize(deserializer)?))
  }
}

#[derive(Debug)]
pub struct RefHelper;

impl RefHelper {
  pub fn serialize<S, T>(value: &Ref<T>, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
    T: Serialize,
  {
    value.serialize(serializer)
  }
}

#[derive(Debug)]
pub struct RefMutHelper;

impl RefMutHelper {
  pub fn serialize<S, T>(value: &RefMut<T>, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
    T: Serialize,
  {
    value.serialize(serializer)
  }
}

#[derive(Debug)]
pub struct CompactUuidHelper;

// TODO: Handle direct (de)serialization (from) to byte arrays without
// encoding, like here:
// <https://github.com/uuid-rs/uuid/blob/34a4f1c65b63b29e828a078566bd3a7c5e042c76/src/serde_support.rs>.
impl CompactUuidHelper {
  pub fn serialize<S>(value: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let bytes = super::encode_compact_uuid(value);
    str::from_utf8(&bytes).unwrap().serialize(serializer)
  }

  pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
  where
    D: Deserializer<'de>,
  {
    let text = Cow::<'de, str>::deserialize(deserializer)?;
    match try {
      let bytes = <[u8; super::COMPACT_UUID_BYTE_LEN]>::try_from(text.as_bytes()).ok()?;
      super::decode_compact_uuid(&bytes)?
    } {
      Some(v) => Ok(v),
      None => Err(serde::de::Error::custom("compact UUID parsing failed")),
    }
  }
}
