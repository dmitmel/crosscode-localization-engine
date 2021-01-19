pub mod database;
pub mod fragment_descriptions;
pub mod json_file_finder;
pub mod lang_label_extractor;

use self::database as db;
use self::json_file_finder::FoundJsonFile;
use self::lang_label_extractor::LangLabel;
use crate::cli;
use crate::impl_prelude::*;
use crate::utils;
use crate::utils::json;

use indexmap::IndexMap;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::borrow::Cow;
use std::char;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::str::FromStr;

pub fn run(common_opts: &cli::CommonOpts, command_opts: &cli::ScanCommandOpts) -> AnyResult<()> {
  info!(
    "Performing a scan of game files in the assets dir '{}'",
    command_opts.assets_dir.display()
  );

  info!("Finding all JSON files");
  let all_json_files = json_file_finder::find_all_in_assets_dir(&command_opts.assets_dir)
    .context("Failed to find all JSON files in the assets dir")?;
  info!("Found {} JSON files in total", all_json_files.len());

  let game_version =
    read_game_version(&command_opts.assets_dir).context("Failed to read the game version")?;
  info!("Game version is {}", game_version);

  let mut files = IndexMap::<String, db::FileData>::new();
  // Currently all fragments are generated with the one and only `en_US` locale
  // anyway, so let's reuse the hashmap and just clone it.
  let mut tmp_fragment_text = HashMap::<String, String>::with_capacity(1);

  info!("Extracting localizable strings");
  let mut lang_labels_count = 0;
  let mut ignored_lang_labels_count = 0;

  let all_json_files_len = all_json_files.len();
  for (i, found_file) in all_json_files.into_iter().enumerate() {
    trace!("[{}/{}] {}", i + 1, all_json_files_len, found_file.path);

    let mut fragments = IndexMap::<String, db::FragmentData>::new();

    let abs_path = command_opts.assets_dir.join(&found_file.path);
    let json_bytes = fs::read(&abs_path)
      .with_context(|| format!("Failed to read file '{}'", abs_path.display()))?;
    let json_data = serde_json::from_slice::<json::Value>(&json_bytes)
      .with_context(|| format!("Failed to parse JSON file '{}'", found_file.path))?;

    let lang_labels_iter = match lang_label_extractor::extract_from_file(&found_file, &json_data) {
      Some(v) => v,
      _ => continue,
    };
    for lang_label in lang_labels_iter {
      if is_lang_label_ignored(&lang_label, &found_file) {
        ignored_lang_labels_count += 1;
        continue;
      }

      let description = match fragment_descriptions::generate(&json_data, &lang_label) {
        Ok(v) => v,
        Err(e) => {
          warn!(
            "file '{}': fragment '{}': {:?}",
            found_file.path,
            lang_label.json_path.join("/"),
            e,
          );
          continue;
        }
      };

      tmp_fragment_text.insert(lang_label_extractor::EXTRACTED_LOCALE.to_owned(), lang_label.text);
      fragments.insert(
        lang_label.json_path.join("/"),
        db::FragmentData {
          lang_uid: lang_label.lang_uid,
          description,
          text: tmp_fragment_text.clone(),
        },
      );
      lang_labels_count += 1;
    }

    if !fragments.is_empty() {
      files.insert(
        found_file.path,
        db::FileData { is_lang_file: found_file.is_lang_file, fragments },
      );
    }
  }

  info!(
    "Found {} localizable strings, {} were ignored",
    lang_labels_count, ignored_lang_labels_count,
  );

  info!("Writing the scan database");
  let database = db::DatabaseData { game_version, files };

  let mut database_writer: Box<dyn io::Write> = match &command_opts.output {
    Some(cli::FileOrStdStream::File(path)) => {
      Box::new(io::BufWriter::new(fs::File::create(&path).with_context(|| {
        format!("Failed to open file '{}' for writing the scan database", path.display())
      })?))
    }
    Some(cli::FileOrStdStream::StdStream) => Box::new(io::stdout()),
    None => Box::new(io::sink()),
  };

  let mut write_database = || -> AnyResult<()> {
    if common_opts.pretty_json {
      serde_json::to_writer_pretty(&mut database_writer, &database)?;
    } else {
      serde_json::to_writer(&mut database_writer, &database)?;
    }
    database_writer.write_all(b"\n")?;
    database_writer.flush()?;
    Ok(())
  };
  write_database().context("Failed to serialize the scan database")?;

  Ok(())
}

lazy_static! {
  static ref CHANGELOG_FILE_PATH: &'static Path = Path::new("data/changelog.json");
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
struct ChangelogFileRef<'a> {
  #[serde(borrow)]
  changelog: Vec<ChangelogEntryRef<'a>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
struct ChangelogEntryRef<'a> {
  #[serde(borrow)]
  name: Cow<'a, str>,
  #[serde(borrow)]
  version: Cow<'a, str>,
  #[serde(borrow)]
  date: Cow<'a, str>,
  #[serde(borrow, default)]
  fixes: Vec<Cow<'a, str>>,
  #[serde(borrow, default)]
  changes: Vec<Cow<'a, str>>,
}

pub fn read_game_version(assets_dir: &Path) -> AnyResult<String> {
  let abs_changelog_path = assets_dir.join(*CHANGELOG_FILE_PATH);
  let changelog_bytes = fs::read(&abs_changelog_path)
    .with_context(|| format!("Failed to read file '{}'", abs_changelog_path.display()))?;
  let changelog_data = serde_json::from_slice::<ChangelogFileRef>(&changelog_bytes)
    .with_context(|| format!("Failed to parse JSON file '{}'", CHANGELOG_FILE_PATH.display()))?;
  let latest_entry = changelog_data
    .changelog
    .get(0)
    .ok_or_else(|| format_err!("Changelog is empty, can't determine the game version"))?;

  let mut max_hotfix: u32 = 0;
  let mut max_hotfix_str = "";
  for change in latest_entry.changes.iter().chain(latest_entry.fixes.iter()) {
    if let Some((hotfix_str, hotfix)) = try_extract_hotfix(change) {
      if hotfix > max_hotfix {
        max_hotfix = hotfix;
        max_hotfix_str = hotfix_str;
      }
    }

    #[allow(unused_assignments)]
    fn try_extract_hotfix(mut change: &str) -> Option<(&str, u32)> {
      let (i, _): (usize, char) =
        change.char_indices().find(|(_, c)| !matches!(c, '+' | '-' | '~' | ' '))?;
      change = unsafe { change.get_unchecked(i..) };
      change = change.strip_prefix("HOTFIX(")?;
      let i = change.char_indices().take_while(|(_, c)| char::is_ascii_digit(c)).count();
      let hotfix_str = unsafe { change.get_unchecked(..i) };
      let hotfix = u32::from_str(hotfix_str).ok()?;
      change = unsafe { change.get_unchecked(i..) };
      change = change.strip_prefix(")")?;
      Some((hotfix_str, hotfix))
    }
  }

  if max_hotfix > 0 {
    Ok(utils::fast_concat(&[&latest_entry.version, "-", max_hotfix_str]))
  } else {
    Ok(latest_entry.version.clone().into_owned())
  }
}

lazy_static! {
  static ref IGNORED_STRINGS: HashSet<&'static str> = hashset![
    "",
    "en_US",
    "LOL, DO NOT TRANSLATE THIS!",
    "LOL, DO NOT TRANSLATE THIS! (hologram)",
    "\\c[1][DO NOT TRANSLATE THE FOLLOWING]\\c[0]",
    "\\c[1][DO NOT TRANSLATE FOLLOWING TEXTS]\\c[0]",
  ];
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
