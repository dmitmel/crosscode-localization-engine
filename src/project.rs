pub mod splitting_strategies;

use crate::utils::Timestamp;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const META_FILE_PATH: &str = "crosslocale-project.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct MetaFileData {
  pub uuid: Uuid,
  pub created_at: Timestamp,
  pub original_locale: String,
  pub reference_locales: Vec<String>,
  pub translation_locale: String,
  pub splitting_strategy: String,
  pub translations_dir: String,
}
