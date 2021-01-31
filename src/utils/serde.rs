use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::RefCell;
use std::collections::HashMap;

#[inline]
pub fn is_refcell_vec_empty<T>(v: &RefCell<Vec<T>>) -> bool { v.borrow().is_empty() }
#[inline]
pub fn is_refcell_hashmap_empty<K, V>(v: &RefCell<HashMap<K, V>>) -> bool { v.borrow().is_empty() }

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
    let lines = Vec::<&str>::deserialize(deserializer)?;
    Ok(super::fast_concat(&lines).into())
  }
}
