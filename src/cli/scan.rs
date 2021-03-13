use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::scan;
use crate::scan::fragment_descriptions;
use crate::scan::json_file_finder::{self, FoundJsonFile};
use crate::scan::lang_label_extractor::{self, LangLabel};
use crate::utils;
use crate::utils::json;

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::borrow::Cow;
use std::char;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;

#[derive(Debug)]
pub struct ScanCommand;

impl super::Command for ScanCommand {
  fn name(&self) -> &'static str { "scan" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about(
        "Scans the assets directory of the game and extracts the localizable strings and other \
        interesting data.",
      )
      .arg(
        clap::Arg::new("assets_dir")
          .value_name("ASSETS")
          .required(true)
          .about("Path to the assets directory."),
      )
      .arg(
        clap::Arg::new("output")
          .value_name("PATH")
          .short('o')
          .long("output")
          .required(true)
          .about("Path to the output JSON file."),
      )
      .arg(
        clap::Arg::new("locales")
          .value_name("LOCALE")
          .multiple(true)
          .number_of_values(1)
          .short('l')
          .long("locales")
          .about("Locales to extract. By default only the main locale is extracted.")
          .default_values(&[lang_label_extractor::MAIN_LOCALE]),
      )
      .arg(
        clap::Arg::new("all_locales")
          .long("all-locales")
          .conflicts_with("locales")
          .about("Extact absolutely all locales."),
      )
  }

  fn run(&self, _global_opts: super::GlobalOpts, matches: &clap::ArgMatches) -> AnyResult<()> {
    let opt_assets_dir = PathBuf::from(matches.value_of_os("assets_dir").unwrap());
    let opt_output = PathBuf::from(matches.value_of_os("output").unwrap());
    let opt_extra_locales: HashSet<_> = matches
      .values_of("locales")
      .map_or_else(HashSet::new, |values| values.map(RcString::from).collect());
    let opt_all_locales = matches.is_present("all_locales");

    info!("Performing a scan of game files in the assets dir {:?}", opt_assets_dir);

    let game_version =
      read_game_version(&opt_assets_dir).context("Failed to read the game version")?;
    info!("Game version is {}", game_version);

    info!("Finding all JSON files");
    let all_json_files = json_file_finder::find_all_in_assets_dir(&opt_assets_dir)
      .context("Failed to find all JSON files in the assets dir")?;
    info!("Found {} JSON files in total", all_json_files.len());

    let scan_db = scan::ScanDb::create(opt_output, scan::ScanDbCreateOpts { game_version });

    info!("Extracting localizable strings");
    let mut total_fragments_count = 0;
    let mut ignored_lang_labels_count = 0;
    let extractor_opts = lang_label_extractor::ExtractionOptions {
      locales_filter: if opt_all_locales { None } else { Some(opt_extra_locales) },
    };

    let all_json_files_len = all_json_files.len();
    for (i, found_file) in all_json_files.into_iter().enumerate() {
      trace!("[{}/{}] {:?}", i + 1, all_json_files_len, found_file.path);
      let mut scan_db_file: Option<Rc<scan::ScanDbGameFile>> = None;

      let abs_path = opt_assets_dir.join(&found_file.path);
      let json_data: json::Value = utils::json::read_file(&abs_path, &mut Vec::new())
        .with_context(|| format!("Failed to deserialize from JSON file {:?}", abs_path))?;

      let lang_labels_iter =
        match lang_label_extractor::extract_from_file(&found_file, &json_data, &extractor_opts) {
          Some(v) => v,
          _ => continue,
        };

      for lang_label in lang_labels_iter {
        if is_lang_label_ignored(&lang_label, &found_file) {
          ignored_lang_labels_count += 1;
          continue;
        }
        let LangLabel { json_path, lang_uid, text, .. } = lang_label;

        let description = if !found_file.is_lang_file {
          match fragment_descriptions::generate(&json_data, &json_path) {
            Ok(v) => v,
            Err(e) => {
              warn!("file {:?}: fragment {:?}: {:?}", found_file.path, json_path, e);
              continue;
            }
          }
        } else {
          Vec::new()
        };

        if scan_db_file.is_none() {
          scan_db_file = Some(scan_db.new_game_file(scan::ScanDbGameFileInitOpts {
            path: found_file.path.share_rc(),
            asset_root: found_file.asset_root.share_rc(),
          })?);
        }
        let scan_db_file = scan_db_file.as_mut().unwrap();

        scan_db_file.new_fragment(scan::ScanDbFragmentInitOpts {
          json_path,
          lang_uid,
          description,
          text,
          flags: HashSet::new(),
        });
        total_fragments_count += 1;
      }
    }

    info!(
      "Found {} localizable strings in {} files, {} were ignored",
      total_fragments_count,
      scan_db.game_files().len(),
      ignored_lang_labels_count,
    );

    info!("Writing the scan database");
    scan_db.write().context("Failed to write the scan database")?;

    Ok(())
  }
}

static CHANGELOG_FILE_PATH: Lazy<&'static Path> = Lazy::new(|| Path::new("data/changelog.json"));

#[derive(Debug, Deserialize)]
struct ChangelogFileRef<'a> {
  #[serde(borrow)]
  changelog: Vec<ChangelogEntryRef<'a>>,
}

#[derive(Debug, Deserialize)]
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
    utils::json::read_file(&abs_changelog_path, &mut changelog_bytes)
      .with_context(|| format!("Failed to serialize to JSON file {:?}", abs_changelog_path))?;

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

static IGNORED_STRINGS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
  hashset![
    "",
    "en_US",
    "LOL, DO NOT TRANSLATE THIS!",
    "LOL, DO NOT TRANSLATE THIS! (hologram)",
    "\\c[1][DO NOT TRANSLATE THE FOLLOWING]\\c[0]",
    "\\c[1][DO NOT TRANSLATE FOLLOWING TEXTS]\\c[0]",
  ]
});

#[allow(clippy::iter_nth_zero)]
fn is_lang_label_ignored(lang_label: &LangLabel, found_file: &FoundJsonFile) -> bool {
  if IGNORED_STRINGS.contains(lang_label.main_locale_text.as_str()) {
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
