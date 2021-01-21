pub mod splitting_strategies;

use crate::utils::{is_default, Timestamp};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

pub const META_FILE_PATH: &str = "crosslocale-project.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaFileSerde {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub game_version: String,
  pub original_locale: String,
  pub reference_locales: Vec<String>,
  pub translation_locale: String,
  pub splitting_strategy: String,
  pub translations_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationDbSerde {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub project_meta_file: String,
  pub files: IndexMap<String, TranslationDbFileSerde>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationDbFileSerde {
  #[serde(default, skip_serializing_if = "is_default")]
  pub is_lang_file: bool,
  pub fragments: IndexMap<String, TranslationDbFragmentSerde>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationDbFragmentSerde {
  #[serde(default, skip_serializing_if = "is_default")]
  pub lang_uid: i32,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub description: Vec<String>,
  pub original_text: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub reference_texts: Vec<String>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub flags: HashMap<String, bool>,
  pub translations: Vec<TranslationDbTranslationSerde>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub comments: Vec<TranslationDbCommentSerde>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationDbTranslationSerde {
  pub uuid: Uuid,
  pub author: String,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub text: String,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub flags: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationDbCommentSerde {
  pub uuid: Uuid,
  pub author: String,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub text: String,
}
