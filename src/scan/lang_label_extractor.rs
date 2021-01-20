use super::db::ScanDbFileInitOpts;
use crate::impl_prelude::*;
use crate::utils::json::{self, ValueExt as _};

use std::convert::TryFrom;

pub const EXTRACTED_LOCALE: &str = "en_US";

pub fn extract_from_file<'json>(
  found_file: &'json ScanDbFileInitOpts,
  json_data: &'json json::Value,
) -> Option<LangLabelIter<'json>> {
  let extraction_fn = if found_file.is_lang_file {
    if json_data.get("DOCTYPE").and_then(|v| v.as_str()) != Some("STATIC-LANG-FILE") {
      warn!("{}: lang file is invalid: DOCTYPE isn't 'STATIC-LANG-FILE'", found_file.path);
      return None;
    }
    try_extract_lang_label_from_lang_file
  } else {
    try_extract_lang_label
  };
  Some(LangLabelIter::new(&found_file.path, &json_data, extraction_fn))
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct LangLabel {
  pub json_path: String,
  pub lang_uid: i32, // 0 represents the lack of a lang UID
  pub text: String,
}

fn try_extract_lang_label<'json>(
  file_path: &'json str,
  json_path: &[String],
  value: &'json json::Value,
) -> Option<LangLabel> {
  let object = value.as_object()?;

  let text = object.get(EXTRACTED_LOCALE)?.as_str().or_else(|| {
    warn!(
      "{}: lang label {} is invalid: property '{}' is not a string",
      file_path,
      json_path.join("/"),
      EXTRACTED_LOCALE,
    );
    None
  })?;

  let lang_uid = match object.get("langUid")? {
    json::Value::Null => 0,
    json::Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()).or_else(|| {
      warn!(
        "{}: lang label {} is invalid: lang UID {} can't be converted to i32",
        file_path,
        json_path.join("/"),
        n,
      );
      None
    })?,
    _ => {
      warn!(
        "{}: lang label {} is invalid: optional property 'langUid' is not a number",
        file_path,
        json_path.join("/"),
      );
      return None;
    }
  };

  Some(LangLabel { json_path: json_path.join("/"), lang_uid, text: text.to_owned() })
}

fn try_extract_lang_label_from_lang_file<'json>(
  _file_path: &'json str,
  json_path: &[String],
  value: &'json json::Value,
) -> Option<LangLabel> {
  if json_path[0].as_str() != "labels" {
    return None;
  }
  let text = value.as_str()?;
  Some(LangLabel { json_path: json_path.join("/"), lang_uid: 0, text: text.to_owned() })
}

type TryExtractLangLabelFn<'json> =
  fn(file_path: &'json str, json_path: &[String], value: &'json json::Value) -> Option<LangLabel>;

#[allow(missing_debug_implementations)]
pub struct LangLabelIter<'json> {
  file_path: &'json str,
  stack: Vec<json::ValueEntriesIter<'json>>,
  current_json_path: Vec<String>,
  try_extract_lang_label_fn: TryExtractLangLabelFn<'json>,
}

impl<'json> LangLabelIter<'json> {
  pub fn new(
    file_path: &'json str,
    value: &'json json::Value,
    try_extract_lang_label_fn: TryExtractLangLabelFn<'json>,
  ) -> Self {
    let mut stack = Vec::with_capacity(
      // Capacity was determined experimentally. The max stack depth when
      // processing asset files as of CC 1.3.0-4 is 21, plus 1 for the root
      // value iterator
      22,
    );
    if let Some(entries_iter) = value.entries_iter() {
      stack.push(entries_iter);
    }
    let current_json_path = Vec::with_capacity(stack.capacity());
    Self { file_path, stack, current_json_path, try_extract_lang_label_fn }
  }
}

impl<'json> Iterator for LangLabelIter<'json> {
  type Item = LangLabel;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(current_iter) = self.stack.last_mut() {
      if let Some((key, value)) = current_iter.next() {
        self.current_json_path.push(key.into_owned());
        if let Some(lang_label) =
          (self.try_extract_lang_label_fn)(self.file_path, &self.current_json_path, value)
        {
          // We've found a lang label! Let's emit it.
          self.current_json_path.pop().unwrap();
          return Some(lang_label);
        } else if let Some(entries_iter) = value.entries_iter() {
          // Not exactly a lang label, but an iterable value we can descend into. Enter it.
          self.stack.push(entries_iter);
        } else {
          // A value we don't care about. Ignore it.
          self.current_json_path.pop().unwrap();
        };
      } else {
        // We are done with the current iterable value. Exit it.
        self.stack.pop().unwrap();
        if !self.stack.is_empty() {
          // On the root value the JSON path is empty.
          self.current_json_path.pop().unwrap();
        }
      }
    }

    None
  }
}
