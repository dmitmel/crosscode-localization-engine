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

#[derive(Debug)]
pub struct MultilineStringHelper;

impl MultilineStringHelper {
  pub fn serialize<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
    T: AsRef<str>,
  {
    let lines: Vec<&str> = super::LinesWithEndings::new(value.as_ref()).collect();
    lines.serialize(serializer)
  }

  pub fn deserialize<'de, D, T>(deserializer: D) -> Result<T, D::Error>
  where
    D: Deserializer<'de>,
    T: From<String>,
  {
    let lines = Vec::<Cow<'de, str>>::deserialize(deserializer)?;
    let mut capacity = 0;
    for s in &lines {
      capacity += s.as_ref().len();
    }
    let mut result = String::with_capacity(capacity);
    for s in &lines {
      result.push_str(s.as_ref());
    }
    Ok(result.into())
  }
}
