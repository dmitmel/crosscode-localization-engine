use crate::utils;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

/// See <https://github.com/L-Sherry/Localize-me/blob/9d0ff32abde457997ff58c35f20864d37ac8b2bf/Documentation.md#file_dict_path_str>.
/// Adapted from <https://github.com/L-Sherry/Localize-Me-Tools/blob/cb8863cef80d1c7361b7142ab9206226e9669bdf/common.py#L399-L404>.
pub fn parse_file_dict_path(lm_file_dict_path: &str) -> Option<(&str, &str)> {
  let mut curr_char_index = 0;
  for component in lm_file_dict_path.split('/') {
    let next_char_index = curr_char_index + component.len() + 1;
    if component.ends_with(".json") {
      let (file_path, json_path) =
        (&lm_file_dict_path[..next_char_index - 1], &lm_file_dict_path[next_char_index..]);
      return Some((file_path, json_path));
    }
    curr_char_index = next_char_index;
  }
  None
}

/// See <https://github.com/L-Sherry/Localize-me/blob/9d0ff32abde457997ff58c35f20864d37ac8b2bf/Documentation.md#file_path>.
pub fn serialize_file_path(file_path: &str) -> &str {
  file_path.strip_prefix("data/").unwrap_or(file_path)
}

/// See <https://github.com/L-Sherry/Localize-me/blob/9d0ff32abde457997ff58c35f20864d37ac8b2bf/Documentation.md#file_path>.
pub fn deserialize_file_path(lm_file_path: &str) -> Cow<str> {
  if lm_file_path.starts_with("extension") {
    Cow::Borrowed(lm_file_path)
  } else {
    Cow::Owned(utils::fast_concat(&["data/", lm_file_path]))
  }
}

/// See <https://github.com/L-Sherry/Localize-me/blob/9d0ff32abde457997ff58c35f20864d37ac8b2bf/Documentation.md#plain-text-variant>.
#[derive(Debug, Deserialize)]
pub struct TrPackEntrySerde<'a> {
  #[serde(borrow)]
  pub orig: Cow<'a, str>,
  #[serde(borrow)]
  pub text: Cow<'a, str>,
  pub quality: Option<Quality>,
  #[serde(borrow)]
  pub note: Option<Cow<'a, str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
  Unknown,
  Bad,
  Incomplete,
  Wrong,
  Spell,
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct TrPackSerde<'a> {
  #[serde(borrow)]
  pub entries: IndexMap<Cow<'a, str>, TrPackEntrySerde<'a>>,
}

#[derive(Debug)]
pub struct OptiTrPackEntrySerde<'a> {
  pub original_text: Option<Cow<'a, str>>,
  pub translation_text: Option<Cow<'a, str>>,
  pub children: IndexMap<Cow<'a, str>, OptiTrPackEntrySerde<'a>>,
}

impl<'a> OptiTrPackEntrySerde<'a> {
  pub fn new() -> Self {
    Self { original_text: None, translation_text: None, children: IndexMap::new() }
  }
}

impl<'a> Serialize for OptiTrPackEntrySerde<'a> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    use serde::ser::SerializeMap as _;

    let mut map_len = self.children.len();
    map_len += self.original_text.is_some() as usize;
    map_len += self.translation_text.is_some() as usize;
    let mut map = serializer.serialize_map(Some(map_len))?;

    if let Some(value) = &self.original_text {
      map.serialize_entry("o", &value)?;
    }
    if let Some(value) = &self.translation_text {
      map.serialize_entry("t", &value)?;
    }
    for (key, value) in &self.children {
      map.serialize_entry(&utils::fast_concat(&["/", &*key]), value)?;
    }

    map.end()
  }
}

#[derive(Debug, Serialize)]
#[serde(transparent)]
pub struct OptiTrPackSerde<'a> {
  #[serde(borrow)]
  pub file_entries: IndexMap<Cow<'a, str>, OptiTrPackEntrySerde<'a>>,
}

impl<'a> OptiTrPackSerde<'a> {
  pub fn new() -> Self { Self { file_entries: IndexMap::new() } }
}
