use crate::utils::Timestamp;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseData {
  pub uuid: Uuid,
  pub generated_at: Timestamp,
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
