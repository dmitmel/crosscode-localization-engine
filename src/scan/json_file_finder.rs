use crate::impl_prelude::*;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{fs, io};

#[derive(Debug)]
struct CommonPaths<'a> {
  pub assets_dir: &'a Path,
  pub data_dir: &'static Path,
  pub extensions_dir: &'static Path,
  pub lang_files_dir: &'static Path,
  pub json_extension: &'static OsStr,
}

impl<'a> CommonPaths<'a> {
  pub fn new(assets_dir: &'a Path) -> Self {
    Self {
      assets_dir,
      data_dir: Path::new("data"),
      extensions_dir: Path::new("extension"),
      lang_files_dir: Path::new("lang"),
      json_extension: OsStr::new("json"),
    }
  }

  #[inline(always)]
  pub fn resolve(&self, path: &Path) -> PathBuf { self.assets_dir.join(path) }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FoundJsonFile {
  pub path: String,
  pub is_lang_file: bool,
}

pub fn find_all_in_assets_dir(assets_dir: &Path) -> AnyResult<Vec<FoundJsonFile>> {
  let common_paths = CommonPaths::new(assets_dir);

  // Bail out early to warn the user instead of failing on some obscure "file
  // not found" IO error later.
  ensure!(
    common_paths.resolve(common_paths.data_dir).is_dir(),
    "The data dir doesn't exist in the assets dir, path to the assets dir is incorrect"
  );

  let mut found_files: Vec<FoundJsonFile> = Vec::with_capacity(
    // As of 1.3.0-4 the stock game comes with 2132 JSON assets, 1.2.0-5
    // included 1943 of those, we can use this knowledge (and a simple
    // assumption that the user doesn't put too many additional files) to avoid
    // allocations when filling this vector. Additional capacity is reserved
    // for future game updates and the upcoming post-game DLC.
    2400,
  );

  let mut asset_roots: Vec<PathBuf> = Vec::with_capacity(
    // The JSON directories are usually going to be just the main data dir plus
    // the data dir of the scorpion-robo extension.
    2,
  );
  asset_roots.push(PathBuf::new());

  info!("Listing the extensions");
  let extension_count = read_extensions_dir(&common_paths, &mut asset_roots, &mut found_files)
    .context("Failed to read the extensions dir")?;
  info!("Found {} extensions", extension_count);

  let asset_roots_len = asset_roots.len();
  for (i, asset_root) in asset_roots.into_iter().enumerate() {
    let data_dir = asset_root.join(common_paths.data_dir);
    info!("[{}/{}] Listing all JSON files in '{}'", i + 1, asset_roots_len, data_dir.display());

    let data_dir_abs = common_paths.resolve(&data_dir);

    let mut file_count: usize = 0;
    for entry in walkdir::WalkDir::new(&data_dir_abs).into_iter() {
      let entry = entry.with_context(|| {
        format!("Failed to list all files in dir '{}'", data_dir_abs.display())
      })?;

      if !entry.file_type().is_file()
        || entry.path().extension() != Some(common_paths.json_extension)
      {
        continue;
      }

      let relative_path = match entry.path().strip_prefix(&data_dir_abs) {
        Ok(p) => p,
        _ => continue,
      };
      let path = data_dir.join(relative_path);
      let path_str = path_to_str_with_error(&path)?;

      let is_lang_file = relative_path.starts_with(&common_paths.lang_files_dir);
      // Hacky, but good enough for CC.
      if is_lang_file && !path_str.ends_with(".en_US.json") {
        continue;
      }

      file_count += 1;
      found_files.push(FoundJsonFile { path: path_str.to_owned(), is_lang_file });
    }
    trace!("Found {} JSON files", file_count);
  }

  found_files.sort();
  Ok(found_files)
}

fn read_extensions_dir(
  common_paths: &CommonPaths<'_>,
  asset_roots: &mut Vec<PathBuf>,
  found_files: &mut Vec<FoundJsonFile>,
) -> AnyResult<usize> {
  let mut extension_count = 0;

  if let Some(dir_iter) = match common_paths.resolve(common_paths.extensions_dir).read_dir() {
    Ok(v) => Ok(Some(v)),
    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(e),
  }? {
    for entry in dir_iter {
      let entry: fs::DirEntry = entry?;
      let file_type: fs::FileType = entry
        .file_type()
        .with_context(|| format!("Failed to get the file type of '{}'", entry.path().display()))?;

      if !file_type.is_dir() {
        continue;
      }

      let extension_dir_name = PathBuf::from(entry.file_name());
      let extension_dir = common_paths.extensions_dir.join(&extension_dir_name);
      let metadata_file_name = extension_dir_name.with_extension(common_paths.json_extension);
      let metadata_file = extension_dir.join(metadata_file_name);

      if !common_paths.resolve(&metadata_file).exists() {
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
      found_files.push(FoundJsonFile {
        path: path_to_str_with_error(&metadata_file)?.to_owned(),
        is_lang_file: false,
      });
      asset_roots.push(extension_dir);
    }
  }

  Ok(extension_count)
}

fn path_to_str_with_error(path: &Path) -> AnyResult<&str> {
  path.to_str().ok_or_else(|| format_err!("Non-utf8 file path: '{}'", path.display()))
}
