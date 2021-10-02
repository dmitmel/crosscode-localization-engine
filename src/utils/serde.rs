// TODO: Apparently automatic borrowing via deserialization of Cow doesn't
// work. I need to implement the visitors myself.

use serde::de::{self, Deserialize, DeserializeSeed, Deserializer, Error as DeError};
use serde::ser::{Error as SerError, Serialize, SerializeMap as _, SerializeSeq as _, Serializer};
use std::borrow::Cow;
use std::cell::{Ref, RefCell, RefMut};
use std::convert::TryFrom;
use std::fmt;
use std::marker::PhantomData;
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
    fn de_error<E: DeError>(e: impl fmt::Display) -> E {
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

#[allow(missing_debug_implementations)]
pub struct OnTheFlyConverter<S: Serializer> {
  ser: S,
}

impl<S: Serializer> OnTheFlyConverter<S> {
  #[inline(always)]
  pub fn new(ser: S) -> Self { Self { ser } }

  #[inline(always)]
  pub fn convert<'de, D: Deserializer<'de>>(ser: S, de: D) -> Result<S::Ok, D::Error> {
    DeserializeSeed::deserialize(Self { ser }, de)
  }
}

impl<'de, S: Serializer> de::Visitor<'de> for OnTheFlyConverter<S> {
  type Value = S::Ok;

  fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    formatter.write_str("any valid JSON value")
  }

  #[inline]
  fn visit_bool<E: DeError>(self, value: bool) -> Result<Self::Value, E> {
    self.ser.serialize_bool(value).map_err(E::custom)
  }

  #[inline]
  fn visit_i8<E: DeError>(self, value: i8) -> Result<Self::Value, E> {
    self.ser.serialize_i8(value).map_err(E::custom)
  }

  #[inline]
  fn visit_i16<E: DeError>(self, value: i16) -> Result<Self::Value, E> {
    self.ser.serialize_i16(value).map_err(E::custom)
  }

  #[inline]
  fn visit_i32<E: DeError>(self, value: i32) -> Result<Self::Value, E> {
    self.ser.serialize_i32(value).map_err(E::custom)
  }

  #[inline]
  fn visit_i64<E: DeError>(self, value: i64) -> Result<Self::Value, E> {
    self.ser.serialize_i64(value).map_err(E::custom)
  }

  serde::serde_if_integer128! {
    #[inline]
    fn visit_i128<E: DeError>(self, value: i128) -> Result<Self::Value, E> {
      self.ser.serialize_i128(value).map_err(E::custom)
    }
  }

  #[inline]
  fn visit_u8<E: DeError>(self, value: u8) -> Result<Self::Value, E> {
    self.ser.serialize_u8(value).map_err(E::custom)
  }

  #[inline]
  fn visit_u16<E: DeError>(self, value: u16) -> Result<Self::Value, E> {
    self.ser.serialize_u16(value).map_err(E::custom)
  }

  #[inline]
  fn visit_u32<E: DeError>(self, value: u32) -> Result<Self::Value, E> {
    self.ser.serialize_u32(value).map_err(E::custom)
  }

  #[inline]
  fn visit_u64<E: DeError>(self, value: u64) -> Result<Self::Value, E> {
    self.ser.serialize_u64(value).map_err(E::custom)
  }

  serde::serde_if_integer128! {
    #[inline]
    fn visit_u128<E: DeError>(self, value: u128) -> Result<Self::Value, E> {
      self.ser.serialize_u128(value).map_err(E::custom)
    }
  }

  #[inline]
  fn visit_f32<E: DeError>(self, value: f32) -> Result<Self::Value, E> {
    self.ser.serialize_f32(value).map_err(E::custom)
  }

  #[inline]
  fn visit_f64<E: DeError>(self, value: f64) -> Result<Self::Value, E> {
    self.ser.serialize_f64(value).map_err(E::custom)
  }

  #[inline]
  fn visit_char<E: DeError>(self, value: char) -> Result<Self::Value, E> {
    self.ser.serialize_char(value).map_err(E::custom)
  }

  #[inline]
  fn visit_str<E: DeError>(self, value: &str) -> Result<Self::Value, E> {
    self.ser.serialize_str(value).map_err(E::custom)
  }

  #[inline]
  fn visit_bytes<E: DeError>(self, value: &[u8]) -> Result<Self::Value, E> {
    self.ser.serialize_bytes(value).map_err(E::custom)
  }

  #[inline]
  fn visit_none<E: DeError>(self) -> Result<Self::Value, E> {
    self.ser.serialize_none().map_err(E::custom)
  }

  fn visit_some<D>(self, de: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    let helper = ConverterFinalApproachHelper::new(de);
    self.ser.serialize_some(&helper).map_err(D::Error::custom)
  }

  #[inline]
  fn visit_unit<E: DeError>(self) -> Result<Self::Value, E> {
    self.ser.serialize_unit().map_err(E::custom)
  }

  fn visit_newtype_struct<D>(self, de: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    DeserializeSeed::deserialize(self, de)
  }

  fn visit_seq<A>(self, mut seq_de: A) -> Result<Self::Value, A::Error>
  where
    A: de::SeqAccess<'de>,
  {
    let mut seq_ser = self.ser.serialize_seq(seq_de.size_hint()).map_err(A::Error::custom)?;

    struct SeqDescent<'a, S: Serializer>(&'a mut S::SerializeSeq);

    impl<'de: 'a, 'a, S: Serializer> DeserializeSeed<'de> for SeqDescent<'a, S> {
      type Value = ();
      fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
      where
        D: Deserializer<'de>,
      {
        let helper = ConverterFinalApproachHelper::new(de);
        let Self(seq_ser) = self;
        seq_ser.serialize_element(&helper).map_err(D::Error::custom)
      }
    }

    while seq_de.next_element_seed(SeqDescent::<S>(&mut seq_ser))?.is_some() {}
    seq_ser.end().map_err(A::Error::custom)
  }

  fn visit_map<A>(self, mut map_de: A) -> Result<Self::Value, A::Error>
  where
    A: de::MapAccess<'de>,
  {
    let mut map_ser = self.ser.serialize_map(map_de.size_hint()).map_err(A::Error::custom)?;

    enum MapDescent<'a, S: Serializer> {
      KeyState(&'a mut S::SerializeMap),
      ValueState(&'a mut S::SerializeMap),
    }

    impl<'de: 'a, 'a, S: Serializer> DeserializeSeed<'de> for MapDescent<'a, S> {
      type Value = ();
      fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
      where
        D: Deserializer<'de>,
      {
        let helper = ConverterFinalApproachHelper::new(de);
        match self {
          Self::KeyState(map_ser) => map_ser.serialize_key(&helper),
          Self::ValueState(map_ser) => map_ser.serialize_value(&helper),
        }
        .map_err(D::Error::custom)
      }
    }

    while map_de.next_key_seed(MapDescent::<S>::KeyState(&mut map_ser))?.is_some() {
      map_de.next_value_seed(MapDescent::<S>::ValueState(&mut map_ser))?;
    }
    map_ser.end().map_err(A::Error::custom)
  }

  fn visit_enum<A>(self, enum_de: A) -> Result<Self::Value, A::Error>
  where
    A: de::EnumAccess<'de>,
  {
    let (ok, _variant) = enum_de.variant_seed(self).map_err(A::Error::custom)?;
    Ok(ok)
  }
}

impl<'de, S: Serializer> DeserializeSeed<'de> for OnTheFlyConverter<S> {
  type Value = S::Ok;

  #[inline(always)]
  fn deserialize<D>(self, de: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    de.deserialize_any(self)
  }
}

struct ConverterFinalApproachHelper<'de, D: Deserializer<'de>> {
  de: RefCell<Option<D>>,
  phantom: PhantomData<&'de ()>,
}

impl<'de, D: Deserializer<'de>> ConverterFinalApproachHelper<'de, D> {
  #[inline(always)]
  fn new(deserializer: D) -> Self {
    Self { de: RefCell::new(Some(deserializer)), phantom: PhantomData }
  }
}

impl<'de, D: Deserializer<'de>> Serialize for ConverterFinalApproachHelper<'de, D> {
  fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let de = self.de.borrow_mut().take().unwrap();
    DeserializeSeed::deserialize(OnTheFlyConverter { ser }, de).map_err(S::Error::custom)
  }
}
