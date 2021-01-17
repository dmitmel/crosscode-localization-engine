pub mod database;
pub mod json_file_finder;
pub mod lang_label_extractor;

use self::json_file_finder::FoundJsonFile;
use self::lang_label_extractor::LangLabel;
use crate::cli;
use crate::impl_prelude::*;
use crate::utils::json;

use lazy_static::lazy_static;
use std::collections::HashSet;
use std::fs;

pub fn run(common_opts: &cli::CommonOpts, command_opts: &cli::ScanCommandOpts) -> AnyResult<()> {
  info!(
    "Performing a scan of game files in the assets dir '{}'",
    command_opts.assets_dir.display()
  );

  info!("Finding all JSON files");
  let all_json_files = json_file_finder::find_all_in_assets_dir(&command_opts.assets_dir)
    .context("Failed to find all JSON files in the assets dir")?;
  info!("Found {} JSON files in total", all_json_files.len());

  info!("Extracting localizable strings");
  let mut all_lang_labels: Vec<LangLabel> = Vec::with_capacity(37000);
  let mut ignored_lang_labels_count = 0;

  let all_json_files_len = all_json_files.len();
  for (i, found_file) in all_json_files.into_iter().enumerate() {
    trace!("[{}/{}] {}", i + 1, all_json_files_len, found_file.path);

    let abs_path = command_opts.assets_dir.join(&found_file.path);
    let json_bytes = fs::read(&abs_path)
      .with_context(|| format!("Failed to read file '{}'", abs_path.display()))?;
    let json_data = serde_json::from_slice::<json::Value>(&json_bytes)
      .with_context(|| format!("Failed to parse JSON file '{}'", found_file.path))?;

    if let Some(lang_label_iter) = lang_label_extractor::extract_from_file(&found_file, &json_data)
    {
      for lang_label in lang_label_iter {
        if !is_lang_label_ignored(&lang_label, &found_file) {
          all_lang_labels.push(lang_label);
        } else {
          ignored_lang_labels_count += 1;
        }
      }
    }
  }

  info!(
    "Found {} localizable strings in total, {} were ignored",
    all_lang_labels.len(),
    ignored_lang_labels_count,
  );

  Ok(())
}

lazy_static! {
  static ref IGNORED_STRINGS: HashSet<&'static str> = {
    let mut s = HashSet::with_capacity(5);
    s.insert("");
    s.insert("en_US");
    s.insert("LOL, DO NOT TRANSLATE THIS!");
    s.insert("LOL, DO NOT TRANSLATE THIS! (hologram)");
    s.insert("\\c[1][DO NOT TRANSLATE THE FOLLOWING]\\c[0]");
    s.insert("\\c[1][DO NOT TRANSLATE FOLLOWING TEXTS]\\c[0]");
    s
  };
}

fn is_lang_label_ignored(lang_label: &LangLabel, found_file: &FoundJsonFile) -> bool {
  if IGNORED_STRINGS.contains(lang_label.text.trim()) {
    return true;
  }

  // TODO: check the relative file path
  if found_file.path.starts_with("data/credits/")
    && lang_label.json_path[0] == "entries"
    && lang_label.json_path[2] == "names"
  {
    return true;
  }

  false
}
