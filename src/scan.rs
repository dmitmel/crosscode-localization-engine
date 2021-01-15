pub mod json_file_finder;

use crate::cli;
use crate::impl_prelude::*;

use std::path::{Path, PathBuf};
use std::{fs, io};

pub fn run(common_opts: &cli::CommonOpts, command_opts: &cli::ScanCommandOpts) -> AnyResult<()> {
  info!(
    "Performing a scan of game files in the assets dir '{}'",
    command_opts.assets_dir.display()
  );

  info!("Finding all JSON files");
  let all_json_files = json_file_finder::find_all_in_assets_dir(&command_opts.assets_dir)
    .context("Failed to find all JSON files in the assets dir")?;
  info!("Found {} JSON files in total", all_json_files.len());

  if command_opts.output.is_none() {
    for found_file in all_json_files.into_iter() {
      // println!("{}", found_file.path.display());
      println!("{:?}", found_file);
    }
  }
  Ok(())
}
