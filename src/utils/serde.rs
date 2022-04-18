// TODO: Apparently automatic borrowing via deserialization of Cow doesn't
// work. I need to implement the visitors myself.

use serde::de::{self, Deserialize, Deserializer};
use serde::ser::{Serialize, Serializer};
use std::borrow::Cow;
use std::cell::{Ref, RefCell, RefMut};
use std::convert::TryFrom;
use std::fmt;
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
    Ok(super::fast_concat(&lines).into())
  }
}

#[derive(Debug)]
pub struct MultilineStringHelperRefCell;

impl MultilineStringHelperRefCell {
  #[inline]
  pub fn serialize<S, T>(value: &RefCell<T>, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
    T: AsRef<str>,
  {
    MultilineStringHelper::serialize(&*value.borrow(), serializer)
  }

  #[inline]
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
  #[inline(always)]
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
  #[inline(always)]
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

impl CompactUuidHelper {
  /// Based on <https://github.com/uuid-rs/uuid/blob/0.8.2/src/serde_support.rs#L16-L28>.
  pub fn serialize<S>(value: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    if serializer.is_human_readable() {
      let bytes = super::encode_compact_uuid(value);
      serializer.serialize_str(str::from_utf8(&bytes).unwrap())
    } else {
      serializer.serialize_bytes(value.as_bytes())
    }
  }

  /// Based on <https://github.com/uuid-rs/uuid/blob/0.8.2/src/serde_support.rs#L30-L91>.
  pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
  where
    D: Deserializer<'de>,
  {
    fn de_error<E: de::Error>(e: impl fmt::Display) -> E {
      E::custom(format_args!("Compact UUID parsing failed: {}", e))
    }

    if deserializer.is_human_readable() {
      struct UuidStringVisitor;

      impl<'vi> de::Visitor<'vi> for UuidStringVisitor {
        type Value = Uuid;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
          formatter.write_str("a UUID string")
        }

        fn visit_str<E: de::Error>(self, value: &str) -> Result<Uuid, E> {
          match <[u8; super::COMPACT_UUID_BYTE_LEN]>::try_from(value.as_bytes()) {
            Ok(bytes) => match super::decode_compact_uuid(&bytes) {
              Ok(uuid) => Ok(uuid),
              Err(error_idx) => Err(de_error(format_args!(
                "invalid byte {:?} at index {}",
                bytes[error_idx] as char, error_idx
              ))),
            },
            Err(_) => Err(de_error(format_args!(
              "length doesn't match: expected {}, found {}",
              super::COMPACT_UUID_BYTE_LEN,
              value.len(),
            ))),
          }
        }

        fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<Uuid, E> {
          Uuid::from_slice(value).map_err(de_error)
        }
      }

      deserializer.deserialize_str(UuidStringVisitor)
    } else {
      struct UuidBytesVisitor;

      impl<'vi> de::Visitor<'vi> for UuidBytesVisitor {
        type Value = Uuid;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
          formatter.write_str("bytes")
        }

        fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<Uuid, E> {
          Uuid::from_slice(value).map_err(de_error)
        }
      }

      deserializer.deserialize_bytes(UuidBytesVisitor)
    }
  }
}

#[derive(Debug)]
pub struct CompactUuidSerializer(pub Uuid);

impl Serialize for CompactUuidSerializer {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    CompactUuidHelper::serialize(&self.0, serializer)
  }
}

#[derive(Debug)]
pub struct SerializeSeqIterator<I: IntoIterator>
where
  I::Item: Serialize,
{
  iter: RefCell<Option<I>>,
}

impl<I: IntoIterator> SerializeSeqIterator<I>
where
  I::Item: Serialize,
{
  #[inline(always)]
  pub fn new(iter: I) -> Self { Self { iter: RefCell::new(Some(iter)) } }
}

impl<I: IntoIterator> Serialize for SerializeSeqIterator<I>
where
  I::Item: Serialize,
{
  #[inline(always)]
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.collect_seq(self.iter.borrow_mut().take().unwrap())
  }
}
