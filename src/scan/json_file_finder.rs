use crate::impl_prelude::*;
use crate::rc_string::RcString;

use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

static DATA_DIR: Lazy<&'static Path> = Lazy::new(|| Path::new("data"));
static EXTENSIONS_DIR: Lazy<&'static Path> = Lazy::new(|| Path::new("extension"));
static LANG_DIR: Lazy<&'static Path> = Lazy::new(|| Path::new("lang"));
static JSON_EXTENSION: Lazy<&'static OsStr> = Lazy::new(|| OsStr::new("json"));

#[derive(Debug)]
pub struct FoundJsonFile {
  pub path: RcString,
  pub asset_root: RcString,
  // TODO: remove
  pub is_lang_file: bool,
}

pub fn find_all_in_assets_dir(assets_dir: &Path) -> AnyResult<Vec<FoundJsonFile>> {
  // Bail out early to warn the user instead of failing on some obscure "file
  // not found" IO error later.
  ensure!(
    assets_dir.join(*DATA_DIR).is_dir(),
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

  let mut asset_roots: Vec<RcString> = Vec::with_capacity(
    // The JSON directories are usually going to be just the main data dir plus
    // the data dir of the scorpion-robo extension.
    2,
  );
  asset_roots.push(RcString::from(""));

  info!("Listing the extensions");
  let extension_count = read_extensions_dir(assets_dir, &mut asset_roots, &mut found_files)
    .context("Failed to read the extensions dir")?;
  info!("Found {} extensions", extension_count);

  let asset_roots_len = asset_roots.len();
  for (i, asset_root) in asset_roots.into_iter().enumerate() {
    let data_dir = Path::new(&asset_root).join(*DATA_DIR);
    let lang_dir = data_dir.join(*LANG_DIR);
    info!("[{}/{}] Listing all JSON files in {:?}", i + 1, asset_roots_len, data_dir);

    let data_dir_abs = assets_dir.join(&data_dir);
    let mut file_count: usize = 0;
    for entry in walkdir::WalkDir::new(&data_dir_abs).into_iter() {
      let entry =
        entry.with_context(|| format!("Failed to list all files in dir {:?}", data_dir_abs))?;

      if !(entry.file_type().is_file() && entry.path().extension() == Some(*JSON_EXTENSION)) {
        continue;
      }

      let path = match entry.path().strip_prefix(&assets_dir) {
        Ok(p) => p,
        _ => continue,
      };
      let path_str = path_to_str_with_error(path)?;

      let is_lang_file = path.starts_with(&lang_dir);
      // Hacky, but good enough for CC.
      if is_lang_file && !path_str.ends_with(".en_US.json") {
        continue;
      }

      file_count += 1;
      found_files.push(FoundJsonFile {
        path: RcString::from(path_str),
        asset_root: asset_root.share_rc(),
        is_lang_file,
      });
    }
    trace!("Found {} JSON files", file_count);
  }

  found_files.sort_by(|a, b| a.path.cmp(&b.path));
  Ok(found_files)
}

fn read_extensions_dir(
  assets_dir: &Path,
  asset_roots: &mut Vec<RcString>,
  found_files: &mut Vec<FoundJsonFile>,
) -> AnyResult<usize> {
  let mut extension_count = 0;

  if let Some(dir_iter) = match assets_dir.join(*EXTENSIONS_DIR).read_dir() {
    Ok(v) => Ok(Some(v)),
    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
    Err(e) => Err(e),
  }? {
    for entry in dir_iter {
      let entry: fs::DirEntry = entry?;
      let file_type: fs::FileType = entry
        .file_type()
        .with_context(|| format!("Failed to get the file type of {:?}", entry.path()))?;

      if !file_type.is_dir() {
        continue;
      }

      let extension_dir_name = PathBuf::from(entry.file_name());
      let extension_dir = EXTENSIONS_DIR.join(&extension_dir_name);
      let metadata_file_name = extension_dir_name.with_extension(*JSON_EXTENSION);
      let metadata_file = extension_dir.join(metadata_file_name);

      if !assets_dir.join(&metadata_file).exists() {
        trace!(
          "Dir {:?} is not an extension - the metadata file {:?} doesn't exist",
          extension_dir_name,
          metadata_file,
        );
        continue;
      }

      let asset_root = RcString::from(strcat!(&*path_to_str_with_error(&extension_dir)?, "/"));
      let path = RcString::from(path_to_str_with_error(&metadata_file)?);

      extension_count += 1;
      trace!(
        "Found extension {:?} with the metadata file {:?}",
        extension_dir_name,
        metadata_file,
      );
      found_files.push(FoundJsonFile {
        path,
        asset_root: asset_root.share_rc(),
        is_lang_file: false,
      });
      asset_roots.push(asset_root);
    }
  }

  Ok(extension_count)
}

fn path_to_str_with_error(path: &Path) -> AnyResult<Cow<str>> {
  let path_str = path.to_str().ok_or_else(|| format_err!("Non-utf8 file path: {:?}", path))?;
  let sep = std::path::MAIN_SEPARATOR;
  if sep != '/' {
    Ok(Cow::Owned(path_str.replace(sep, "/")))
  } else {
    Ok(Cow::Borrowed(path_str))
  }
}
