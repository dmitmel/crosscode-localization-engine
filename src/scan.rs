pub mod json_file_finder;
pub mod lang_label_extractor;

use crate::cli;
use crate::impl_prelude::*;
use crate::utils::json;

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

  use lang_label_extractor::LangLabel;
  let mut all_lang_labels: Vec<LangLabel> = Vec::with_capacity(37000);

  let all_json_files_len = all_json_files.len();
  for (i, found_file) in all_json_files.into_iter().enumerate() {
    trace!("[{}/{}] {}", i + 1, all_json_files_len, found_file.path);

    let abs_path = command_opts.assets_dir.join(&found_file.path);
    let json_bytes = fs::read(&abs_path)
      .with_context(|| format!("Failed to read file '{}'", abs_path.display()))?;
    let json_data = serde_json::from_slice::<json::Value>(&json_bytes)
      .with_context(|| format!("Failed to parse JSON file '{}'", found_file.path))?;

    for lang_label in lang_label_extractor::extract_from_file(&found_file, &json_data) {
      all_lang_labels.push(lang_label);
    }
  }

  info!("Found {} localizable strings in total", all_lang_labels.len());

  Ok(())
}
