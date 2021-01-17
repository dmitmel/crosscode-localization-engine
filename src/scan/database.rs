use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseData {
  pub game_version: String,
  pub files: IndexMap<String, FileData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FileData {
  pub is_lang_file: bool,
  pub fragments: IndexMap<String, FragmentData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FragmentData {
  pub lang_uid: i32,
  pub description: Vec<String>,
  pub text: HashMap<String, String>,
}
