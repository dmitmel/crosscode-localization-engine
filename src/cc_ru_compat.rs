use crate::utils::Timestamp;

use serde::Deserialize;
use std::borrow::Cow;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct ChapterFragmentsFileSerde<'a> {
  pub fragments: Vec<FragmentSerde<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentSerde<'a> {
  pub chapter_id: i32,
  pub id: i32,
  pub order_number: i32,
  pub original: FragmentOriginalSerde<'a>,
  pub translations: Vec<FragmentTranslationSerde<'a>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentOriginalSerde<'a> {
  pub raw_content: Cow<'a, str>,
  pub lang_uid: i32,
  pub file: Cow<'a, str>,
  pub json_path: Cow<'a, str>,
  pub description_text: Cow<'a, str>,
  pub text: Cow<'a, str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FragmentTranslationSerde<'a> {
  pub id: i32,
  pub raw_text: Cow<'a, str>,
  pub author_username: Cow<'a, str>,
  // <https://github.com/uisky/notabenoid/blob/0840a9dd1932f6d254a1c9a022b77fc478afadc4/init.sql#L1070>
  pub votes: i16,
  pub score: i64,
  pub timestamp: Timestamp,
  pub text: Cow<'a, str>,
  pub flags: HashMap<Cow<'a, str>, bool>,
}
