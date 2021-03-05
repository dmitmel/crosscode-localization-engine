use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

#[inline]
pub fn is_refcell_vec_empty<T>(v: &RefCell<Vec<T>>) -> bool { v.borrow().is_empty() }
#[inline]
pub fn is_refcell_hashmap_empty<K, V>(v: &RefCell<HashMap<K, V>>) -> bool { v.borrow().is_empty() }
#[inline]
pub fn is_refcell_hashset_empty<K, V>(v: &RefCell<HashSet<K, V>>) -> bool { v.borrow().is_empty() }

pub const MULTILINE_STRING_WRAP_WIDTH: usize = 80;

#[derive(Debug)]
pub struct MultilineStringHelper;

impl MultilineStringHelper {
  pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
    T: AsRef<str>,
  {
    let wrapper =
      textwrap::Wrapper::with_splitter(MULTILINE_STRING_WRAP_WIDTH, textwrap::NoHyphenation)
        .break_words(false);
    let lines: Vec<Cow<str>> = super::LinesWithEndings::new(value.as_ref())
      .flat_map(|line| wrapper.wrap_iter(line))
      .collect();
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
