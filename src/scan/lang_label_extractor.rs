use crate::impl_prelude::*;
use crate::utils::json::{self, ValueExt as _};

use std::convert::TryFrom;

pub fn extract_from_file<'json>(
  found_file: &'json super::json_file_finder::FoundJsonFile,
  json_data: &'json json::Value,
) -> Box<dyn Iterator<Item = LangLabel> + 'json> {
  if !found_file.is_lang_file {
    Box::new(LangLabelIter::new(&found_file.path, &json_data))
  } else {
    Box::new(std::iter::empty())
  }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct LangLabel {
  pub json_path: Vec<String>,
  pub lang_uid: i32, // 0 represents the lack of a lang UID
  pub text: String,
}

#[allow(missing_debug_implementations)]
pub struct LangLabelIter<'json> {
  file_path: &'json str,
  stack: Vec<json::ValueEntriesIter<'json>>,
  current_json_path: Vec<String>,
}

impl<'json> LangLabelIter<'json> {
  pub fn new(file_path: &'json str, value: &'json json::Value) -> Self {
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
    Self { file_path, stack, current_json_path }
  }

  fn try_extract_lang_label(&self, value: &'json json::Value) -> Option<LangLabel> {
    let object = value.as_object()?;

    let text = object.get("en_US")?.as_str().or_else(|| {
      warn!(
        "{}: lang label {} is invalid: property 'en_US' is not a string",
        self.file_path,
        self.current_json_path.join("/")
      );
      None
    })?;

    let lang_uid = match object.get("langUid")? {
      json::Value::Null => 0,
      json::Value::Number(n) => n.as_i64().and_then(|n| i32::try_from(n).ok()).or_else(|| {
        warn!(
          "{}: lang label {} is invalid: lang UID {} can't be converted to i32",
          self.file_path,
          self.current_json_path.join("/"),
          n,
        );
        None
      })?,
      _ => {
        warn!(
          "{}: lang label {} is invalid: optional property 'langUid' is not a number",
          self.file_path,
          self.current_json_path.join("/"),
        );
        return None;
      }
    };

    Some(LangLabel { json_path: self.current_json_path.clone(), lang_uid, text: text.to_owned() })
  }
}

impl<'json> Iterator for LangLabelIter<'json> {
  type Item = LangLabel;

  fn next(&mut self) -> Option<Self::Item> {
    while let Some(current_iter) = self.stack.last_mut() {
      if let Some((key, value)) = current_iter.next() {
        self.current_json_path.push(key.into_owned());
        if let Some(lang_label) = self.try_extract_lang_label(value) {
          self.current_json_path.pop().unwrap();
          return Some(lang_label);
        } else if let Some(entries_iter) = value.entries_iter() {
          // Enter object
          self.stack.push(entries_iter);
        } else {
          self.current_json_path.pop().unwrap();
        };
      } else {
        // Exit object
        self.stack.pop().unwrap();
        if !self.stack.is_empty() {
          self.current_json_path.pop().unwrap();
        }
      }
    }

    None
  }
}
