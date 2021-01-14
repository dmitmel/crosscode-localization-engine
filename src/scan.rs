use crate::cli;
use crate::impl_prelude::*;

use std::path::{Path, PathBuf};
use std::{fs, io};

pub fn run(common_opts: &cli::CommonOpts, command_opts: &cli::ScanCommandOpts) -> AnyResult<()> {
  let mut all_json_file_paths = find_all_json_files_in(&command_opts.assets_dir)
    .context("Failed to find all JSON files in the assets dir")?;
  all_json_file_paths.sort();
  for path in all_json_file_paths.into_iter() {
    println!("{}", path.display());
  }
  Ok(())
}

fn find_all_json_files_in(assets_dir: &Path) -> AnyResult<Vec<PathBuf>> {
  const DATA_DIR_NAME: &str = "data";
  const EXTENSIONS_DIR_NAME: &str = "extension";

  let data_dir = assets_dir.join(DATA_DIR_NAME);
  let extensions_dir = assets_dir.join(EXTENSIONS_DIR_NAME);

  // Bail out early to warn the user instead of failing on some obscure "file
  // not found" IO error later.
  ensure!(
    data_dir.is_dir(),
    "The data dir doesn't exist in the assets dir, path to the assets dir is incorrect"
  );

  let mut json_dirs: Vec<PathBuf> = Vec::with_capacity(
    // The JSON directories are usually going to be just the main data dir plus
    // the data dir of the scorpion-robo extension.
    2,
  );
  json_dirs.push(data_dir);

  let mut json_files: Vec<PathBuf> = Vec::with_capacity(
    // As of 1.3.0-4 the stock game comes with 2132 JSON assets, 1.2.0-5
    // included 1943 of those, we can use this knowledge (and a simple
    // assumption that the user doesn't put too many additional files) to avoid
    // allocations when filling this vector. Additional capacity is reserved
    // for future game updates and the upcoming post-game DLC.
    2400,
  );

  fn push_json_file_path(assets_dir: &Path, json_files: &mut Vec<PathBuf>, path: &Path) {
    if let Ok(path) = path.strip_prefix(assets_dir) {
      json_files.push(path.to_owned());
    }
  }

  scan_extensions_dir(&assets_dir, &extensions_dir, &mut json_dirs, &mut json_files)
    .context("Failed to read the extensions dir")?;

  fn scan_extensions_dir(
    assets_dir: &Path,
    extensions_dir: &Path,
    json_dirs: &mut Vec<PathBuf>,
    json_files: &mut Vec<PathBuf>,
  ) -> AnyResult<()> {
    if let Some(dir_iter) = match extensions_dir.read_dir() {
      Ok(v) => Ok(Some(v)),
      Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(e) => Err(e),
    }? {
      for entry in dir_iter {
        let entry: fs::DirEntry = entry?;
        let path = entry.path();
        let file_type: fs::FileType = entry
          .file_type()
          .with_context(|| format!("Failed to get the file type of '{}'", path.display()))?;

        if !file_type.is_dir() {
          continue;
        }

        if let Some(name) = entry.file_name().to_str() {
          json_dirs.push(path.join(DATA_DIR_NAME));

          let metadata_file = path.join(name.to_owned() + ".json");
          if metadata_file.exists() {
            push_json_file_path(assets_dir, json_files, &metadata_file);
          }
        }
      }
    }
    Ok(())
  }

  for json_dir in json_dirs.into_iter() {
    let dir_entries: Vec<walkdir::DirEntry> = walkdir::WalkDir::new(&json_dir)
      .into_iter()
      .collect::<walkdir::Result<_>>()
      .with_context(|| format!("Failed to list all files in dir '{}'", json_dir.display()))?;

    for entry in dir_entries.into_iter() {
      if entry.file_type().is_file() {
        push_json_file_path(assets_dir, &mut json_files, entry.path());
      }
    }
  }

  Ok(json_files)
}
