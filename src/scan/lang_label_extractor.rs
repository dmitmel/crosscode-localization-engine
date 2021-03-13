use super::json_file_finder::FoundJsonFile;
use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils::json;

use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;

pub static MAIN_LOCALE: &str = "en_US";
pub static KNOWN_BUILTIN_LOCALES: &[&str] =
  &["en_US", "de_DE", "zh_CN", "zh_TW", "ja_JP", "ko_KR"];
pub static LANG_UID_PROPERTY_NAME: &str = "langUid";

#[allow(missing_debug_implementations)]
pub struct ExtractionOptions {
  pub locales_filter: Option<HashSet<RcString>>,
}

pub fn extract_from_file<'json>(
  found_file: &'json FoundJsonFile,
  json_data: &'json json::Value,
  options: &'json ExtractionOptions,
) -> Option<LangLabelIter<'json>> {
  let extraction_fn: LangLabelExtractionFn = if found_file.is_lang_file {
    if json_data.get("DOCTYPE").and_then(|v| v.as_str()) != Some("STATIC-LANG-FILE") {
      warn!("{:?}: lang file is invalid: DOCTYPE isn't 'STATIC-LANG-FILE'", found_file.path);
      return None;
    }
    try_extract_lang_label_from_lang_file
  } else {
    try_extract_lang_label
  };
  Some(LangLabelIter::new(&found_file.path, &json_data, extraction_fn, options))
}

#[derive(Debug)]
pub struct LangLabel {
  pub json_path: RcString,
  /// 0 represents the lack of a lang UID
  pub lang_uid: i32,
  /// mainly intended for preliminary filtering
  pub main_locale_text: RcString,
  pub text: HashMap<RcString, RcString>,
}

fn try_extract_lang_label(
  options: &ExtractionOptions,
  file_path: &str,
  json_path: &[String],
  value: &json::Value,
) -> Option<LangLabel> {
  let object = value.as_object()?;
  if object.is_empty() {
    return None;
  }

  let main_locale_text = match object.get(MAIN_LOCALE)?.as_str() {
    Some(s) => RcString::from(s),
    None => {
      warn!(
        "{:?}: lang label {:?} is invalid: property {:?} is not a string",
        file_path, json_path, MAIN_LOCALE,
      );
      return None;
    }
  };

  let json_path = RcString::from(json_path.join("/"));
  let mut lang_uid = 0;
  let mut text = HashMap::with_capacity(KNOWN_BUILTIN_LOCALES.len().min(object.len()));

  for (k, v) in object {
    if k == LANG_UID_PROPERTY_NAME {
      lang_uid = match v {
        json::Value::Null => 0,
        json::Value::Number(n) => match try_option!({ i32::try_from(n.as_i64()?).ok()? }) {
          Some(n) => n,
          None => {
            warn!(
              "{:?}: lang label {:?} is invalid: lang UID {:?} can't be converted to i32",
              file_path, json_path, n,
            );
            return None;
          }
        },
        _ => {
          warn!(
            "{:?}: lang label {:?} is invalid: optional property {:?} is not a number",
            file_path, json_path, LANG_UID_PROPERTY_NAME,
          );
          return None;
        }
      };
    } else if options.locales_filter.as_ref().map_or(true, |l| l.contains(k)) {
      let locale_text = match v.as_str() {
        Some(s) => RcString::from(s),
        None => {
          warn!(
            "{:?}: lang label {:?} is invalid: property {:?} is not a string",
            file_path, json_path, k,
          );
          return None;
        }
      };
      text.insert(RcString::from(k), locale_text);
    }
  }

  Some(LangLabel { json_path, lang_uid, main_locale_text, text })
}

fn try_extract_lang_label_from_lang_file(
  _options: &ExtractionOptions,
  _file_path: &str,
  json_path: &[String],
  value: &json::Value,
) -> Option<LangLabel> {
  if json_path[0].as_str() != "labels" {
    return None;
  }
  let main_locale_text = RcString::from(value.as_str()?);
  let mut text_map = HashMap::with_capacity(1);
  text_map.insert(RcString::from(MAIN_LOCALE), main_locale_text.share_rc());
  Some(LangLabel {
    json_path: RcString::from(json_path.join("/")),
    lang_uid: 0,
    main_locale_text,
    text: text_map,
  })
}

type LangLabelExtractionFn = fn(
  options: &ExtractionOptions,
  file_path: &str,
  json_path: &[String],
  value: &json::Value,
) -> Option<LangLabel>;

#[allow(missing_debug_implementations)]
pub struct LangLabelIter<'json> {
  file_path: &'json str,
  stack: Vec<json::ValueEntriesIter<'json>>,
  current_json_path: Vec<String>,
  try_extract_lang_label_fn: LangLabelExtractionFn,
  options: &'json ExtractionOptions,
}

impl<'json> LangLabelIter<'json> {
  pub fn new(
    file_path: &'json str,
    value: &'json json::Value,
    try_extract_lang_label_fn: LangLabelExtractionFn,
    options: &'json ExtractionOptions,
  ) -> Self {
    let mut stack = Vec::with_capacity(
      // Capacity was determined experimentally. The max stack depth when
      // processing asset files as of CC 1.3.0-4 is 21, plus 1 for the root
      // value iterator
      22,
    );
    if let Some(entries_iter) = json::ValueEntriesIter::new(value) {
      stack.push(entries_iter);
    }
    let current_json_path = Vec::with_capacity(stack.capacity());
    Self { file_path, stack, current_json_path, try_extract_lang_label_fn, options }
  }
}

impl<'json> Iterator for LangLabelIter<'json> {
  type Item = LangLabel;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(current_iter) = self.stack.last_mut() {
      if let Some((key, value)) = current_iter.next() {
        self.current_json_path.push(key.into_owned());
        if let Some(lang_label) = (self.try_extract_lang_label_fn)(
          &self.options,
          self.file_path,
          &self.current_json_path,
          value,
        ) {
          // We've found a lang label! Let's emit it.
          self.current_json_path.pop().unwrap();
          return Some(lang_label);
        } else if let Some(entries_iter) = json::ValueEntriesIter::new(value) {
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
