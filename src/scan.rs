pub mod db;
pub mod fragment_descriptions;
pub mod json_file_finder;
pub mod lang_label_extractor;

use self::json_file_finder::FoundJsonFile;
use self::lang_label_extractor::LangLabel;
use crate::cli;
use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils;
use crate::utils::json;

use lazy_static::lazy_static;
use serde::Deserialize;
use std::borrow::Cow;
use std::char;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;
use std::str::FromStr;

pub fn run(_common_opts: cli::CommonOpts, command_opts: cli::ScanCommandOpts) -> AnyResult<()> {
  info!(
    "Performing a scan of game files in the assets dir '{}'",
    command_opts.assets_dir.display()
  );

  let game_version =
    read_game_version(&command_opts.assets_dir).context("Failed to read the game version")?;
  info!("Game version is {}", game_version);

  info!("Finding all JSON files");
  let all_json_files = json_file_finder::find_all_in_assets_dir(&command_opts.assets_dir)
    .context("Failed to find all JSON files in the assets dir")?;
  info!("Found {} JSON files in total", all_json_files.len());

  let scan_db =
    db::ScanDb::create(command_opts.output.clone(), db::ScanDbCreateOpts { game_version });

  // Currently all fragments are generated with the one and only `en_US` locale
  // anyway, so let's reuse the hashmap and just clone it.
  let mut tmp_fragment_text = HashMap::<RcString, RcString>::with_capacity(1);
  let tmp_extracted_locale = RcString::from(lang_label_extractor::EXTRACTED_LOCALE);

  info!("Extracting localizable strings");
  let mut ignored_lang_labels_count = 0;

  let all_json_files_len = all_json_files.len();
  for (i, found_file) in all_json_files.into_iter().enumerate() {
    trace!("[{}/{}] {}", i + 1, all_json_files_len, found_file.path);
    let mut scan_db_file: Option<Rc<db::ScanDbGameFile>> = None;

    let abs_path = command_opts.assets_dir.join(&found_file.path);
    let json_data: json::Value = utils::json::read_file(&abs_path, &mut Vec::new())
      .with_context(|| format!("Failed to deserialize from JSON file '{}'", abs_path.display()))?;

    let lang_labels_iter = match lang_label_extractor::extract_from_file(&found_file, &json_data) {
      Some(v) => v,
      _ => continue,
    };

    for lang_label in lang_labels_iter {
      if is_lang_label_ignored(&lang_label, &found_file) {
        ignored_lang_labels_count += 1;
        continue;
      }
      let LangLabel { json_path, lang_uid, text } = lang_label;

      let description = if !found_file.is_lang_file {
        match fragment_descriptions::generate(&json_data, &json_path) {
          Ok(v) => v,
          Err(e) => {
            warn!("file '{}': fragment '{}': {:?}", found_file.path, json_path, e);
            continue;
          }
        }
      } else {
        Vec::new()
      };

      let scan_db_file =
        scan_db_file.get_or_insert_with(|| scan_db.new_game_file(found_file.path.share_rc()));

      tmp_fragment_text.insert(tmp_extracted_locale.share_rc(), text);
      scan_db_file.new_fragment(db::ScanDbFragmentInitOpts {
        json_path,
        lang_uid,
        description,
        text: tmp_fragment_text.clone(),
      });
    }
  }

  info!(
    "Found {} localizable strings in {} files, {} were ignored",
    scan_db.total_fragments_count(),
    scan_db.game_files().len(),
    ignored_lang_labels_count,
  );

  info!("Writing the scan database");
  scan_db.write().context("Failed to write the scan database")?;

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

pub fn read_game_version(assets_dir: &Path) -> AnyResult<RcString> {
  let abs_changelog_path = assets_dir.join(*CHANGELOG_FILE_PATH);

  let mut changelog_bytes = Vec::new();
  let changelog_data: ChangelogFileRef =
    utils::json::read_file(&abs_changelog_path, &mut changelog_bytes).with_context(|| {
      format!("Failed to serialize to JSON file '{}'", abs_changelog_path.display())
    })?;

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
    Ok(RcString::from(utils::fast_concat(&[&latest_entry.version, "-", max_hotfix_str])))
  } else {
    Ok(RcString::from(latest_entry.version.clone()))
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

#[allow(clippy::iter_nth_zero)]
fn is_lang_label_ignored(lang_label: &LangLabel, found_file: &FoundJsonFile) -> bool {
  if IGNORED_STRINGS.contains(lang_label.text.trim()) {
    return true;
  }

  // TODO: check the relative file path
  if found_file.path.starts_with("data/credits/") && {
    let mut iter = lang_label.json_path.split('/');
    // Note that `nth` advances the iterator
    iter.nth(0) == Some("entries") && iter.nth(1) == Some("names")
  } {
    return true;
  }

  false
}
