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
  let all_json_file_paths = find_all_json_files_in(&command_opts.assets_dir)
    .context("Failed to find all JSON files in the assets dir")?;
  info!("Found {} JSON files in total", all_json_file_paths.len());

  if command_opts.output.is_none() {
    for path in all_json_file_paths.into_iter() {
      println!("{}", path.display());
    }
  }
  Ok(())
}

fn find_all_json_files_in(assets_dir: &Path) -> AnyResult<Vec<PathBuf>> {
  const DATA_DIR_NAME: &str = "data";
  const EXTENSIONS_DIR_NAME: &str = "extension";

  let data_dir = Path::new(DATA_DIR_NAME);
  let extensions_dir = Path::new(EXTENSIONS_DIR_NAME);

  // Bail out early to warn the user instead of failing on some obscure "file
  // not found" IO error later.
  ensure!(
    assets_dir.join(data_dir).is_dir(),
    "The data dir doesn't exist in the assets dir, path to the assets dir is incorrect"
  );

  let mut json_dirs: Vec<PathBuf> = Vec::with_capacity(
    // The JSON directories are usually going to be just the main data dir plus
    // the data dir of the scorpion-robo extension.
    2,
  );
  json_dirs.push(data_dir.to_owned());

  let mut json_files: Vec<PathBuf> = Vec::with_capacity(
    // As of 1.3.0-4 the stock game comes with 2132 JSON assets, 1.2.0-5
    // included 1943 of those, we can use this knowledge (and a simple
    // assumption that the user doesn't put too many additional files) to avoid
    // allocations when filling this vector. Additional capacity is reserved
    // for future game updates and the upcoming post-game DLC.
    2400,
  );

  info!("Listing the extensions");
  let extension_count =
    scan_extensions_dir(&assets_dir, &extensions_dir, &mut json_dirs, &mut json_files)
      .context("Failed to read the extensions dir")?;
  info!("Found {} extensions", extension_count);

  fn scan_extensions_dir(
    assets_dir: &Path,
    extensions_dir: &Path,
    json_dirs: &mut Vec<PathBuf>,
    json_files: &mut Vec<PathBuf>,
  ) -> AnyResult<usize> {
    let mut extension_count = 0;

    if let Some(dir_iter) = match assets_dir.join(extensions_dir).read_dir() {
      Ok(v) => Ok(Some(v)),
      Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(e) => Err(e),
    }? {
      for entry in dir_iter {
        let entry: fs::DirEntry = entry?;
        let file_type: fs::FileType = entry.file_type().with_context(|| {
          format!("Failed to get the file type of '{}'", entry.path().display())
        })?;

        if !file_type.is_dir() {
          continue;
        }

        let extension_dir_name = PathBuf::from(entry.file_name());
        let extension_dir = extensions_dir.join(&extension_dir_name);
        let metadata_file_name = extension_dir_name.with_extension("json");
        let metadata_file = extension_dir.join(metadata_file_name);

        if !assets_dir.join(&metadata_file).exists() {
          trace!(
            "Dir '{}' is not an extension - the metadata file '{}' doesn't exist",
            extension_dir_name.display(),
            metadata_file.display(),
          );
          continue;
        }

        extension_count += 1;
        trace!(
          "Found extension '{}' with the metadata file '{}'",
          extension_dir_name.display(),
          metadata_file.display(),
        );
        json_files.push(metadata_file);

        let data_dir = extension_dir.join(DATA_DIR_NAME);
        if !assets_dir.join(&data_dir).exists() {
          trace!(
            "Extension '{}' doesn't contain any JSON files - the data dir '{}' doesn't exist",
            extension_dir_name.display(),
            data_dir.display(),
          );
          continue;
        }

        json_dirs.push(data_dir);
      }
    }

    Ok(extension_count)
  }

  let json_dirs_len = json_dirs.len();
  for (i, json_dir) in json_dirs.into_iter().enumerate() {
    info!("[{}/{}] Listing all JSON files in '{}'", i + 1, json_dirs_len, json_dir.display());

    let dir_entries: Vec<walkdir::DirEntry> = walkdir::WalkDir::new(assets_dir.join(&json_dir))
      .into_iter()
      .collect::<walkdir::Result<_>>()
      .with_context(|| format!("Failed to list all files in dir '{}'", json_dir.display()))?;

    let mut file_count: usize = 0;
    for entry in dir_entries.into_iter() {
      if entry.file_type().is_file() {
        if let Ok(path) = entry.path().strip_prefix(assets_dir) {
          file_count += 1;
          json_files.push(path.to_owned());
        }
      }
    }
    trace!("Found {} JSON files", file_count);
  }

  json_files.sort();
  Ok(json_files)
}
