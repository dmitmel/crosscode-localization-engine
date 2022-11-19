use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use crate::rc_string::RcString;
use crate::scan;
use crate::scan::fragment_descriptions;
use crate::scan::json_file_finder::{self, FoundJsonFile};
use crate::scan::lang_label_extractor::{self, LangLabel};
use crate::utils::json;
use crate::utils::{self, ArcExt};

use once_cell::sync::Lazy;
use serde::Deserialize;
use std::borrow::Cow;
use std::char;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{mpsc, Arc};

#[derive(Debug)]
pub struct ScanCommand;

impl super::Command for ScanCommand {
  fn name(&self) -> &'static str { "scan" }

  fn create_arg_parser(&self, app: clap::Command) -> clap::Command {
    app
      .about(
        "Scans the assets directory of the game and extracts the localizable strings and other \
        interesting data.",
      )
      .arg(
        clap::Arg::new("assets_dir")
          .value_name("ASSETS")
          .value_hint(clap::ValueHint::DirPath)
          .value_parser(clap::value_parser!(PathBuf))
          .required(true)
          .help("Path to the primary assets directory."),
      )
      .arg(
        clap::Arg::new("output")
          .value_name("PATH")
          .value_hint(clap::ValueHint::FilePath)
          .value_parser(clap::value_parser!(PathBuf))
          .short('o')
          .long("output")
          .required(true)
          .help("Path to the output JSON file."),
      )
      .arg(
        clap::Arg::new("locales")
          .value_name("LOCALE")
          .value_hint(clap::ValueHint::Other)
          .action(clap::ArgAction::Append)
          .short('l')
          .long("locales")
          .help("Locales to extract. By default only the main locale is extracted.")
          .default_value(lang_label_extractor::MAIN_LOCALE),
      )
      .arg(
        clap::Arg::new("all_locales")
          .action(clap::ArgAction::SetTrue)
          .long("all-locales")
          .conflicts_with("locales")
          .help("Extact absolutely all locales."),
      )
      .arg(
        clap::Arg::new("compact")
          .action(clap::ArgAction::SetTrue)
          .short('c')
          .long("compact")
          .help("Disable pretty-printing of the resulting JSON file."),
      )
      .arg(
        clap::Arg::new("jobs")
          .short('j')
          .long("jobs")
          .help(
            "The number of parallel worker threads allocated for the scanner. Zero means using \
            as many threads as there are CPU cores available.",
          )
          .value_parser(clap::value_parser!(usize))
          .default_value("0"),
      )
      .arg(
        clap::Arg::new("assets_overrides_dirs")
          .value_name("ASSETS")
          .value_hint(clap::ValueHint::DirPath)
          .value_parser(clap::value_parser!(PathBuf))
          .action(clap::ArgAction::Append)
          .long("add-assets-overrides-dir")
          .help(
            "Appends a directory to the list of assets overrides directories: for each file \
            found in the primary assets directory, they are checked for containing a replacement \
            for that file (in order of addition, the first matched file is used). Note that this \
            currently doesn't enable support for mods - these directories themselves are not \
            scanned for asset files, and patches of any kind are not supported.",
          ),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    mut progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let opt_assets_dir = matches.get_one::<PathBuf>("assets_dir").unwrap().clone();
    let opt_output = matches.get_one::<PathBuf>("output").unwrap();
    let opt_extra_locales: HashSet<_> = matches
      .get_many::<String>("locales")
      .map_or_else(HashSet::new, |values| values.map(RcString::from).collect());
    let opt_all_locales = matches.get_flag("all_locales");
    let opt_compact = matches.get_flag("compact");
    let opt_jobs = *matches.get_one::<usize>("jobs").unwrap();
    let opt_assets_overrides_dirs: Vec<_> = matches
      .get_many::<PathBuf>("assets_overrides_dirs")
      .map_or_else(Vec::new, |values| values.cloned().collect());

    info!("Performing a scan of game files in the assets dir {:?}", opt_assets_dir);

    let assets_resolver = Arc::new(AssetsResolver {
      assets_dir: opt_assets_dir,
      assets_overrides_dirs: opt_assets_overrides_dirs,
    });

    // Note that this is just a pre-emptive check that may become false if
    // another program modifies the file system in parallel...
    if match fs::metadata(&opt_output) {
      Ok(metadata) => metadata.permissions().readonly(),
      // Only proceed with the warning if the parent directory doesn't exist.
      // If it does, then the file will be created anyway. However, this check
      // will fail (be a false-negative) when the output path points to a
      // symlink to a non-existent file, so TODO.
      Err(e) if e.kind() == io::ErrorKind::NotFound => {
        // These checks are really getting out of hand. normalize_path may fail
        // on symlinks (i.e. `/a/../b` will not always be equivalent to `/b`),
        // I may want to do some probing by trying to create an empty file.
        // Anyway, the concatenation with the path `.` will be ignored on
        // absolute paths and on relative ones the subsequent `parent()`
        // invocation will return the current directory as a fallback.
        match Path::new(".").join(utils::normalize_path(opt_output)).parent() {
          Some(output_dir) => !output_dir.exists(),
          // We are at the FS root (the parent is None), but it (the root)
          // doesn't exist? But anyway, the root is always a directory to my
          // knowledge, so we shouldn't be able to write to it as a file anyways.
          None => true,
        }
      }
      Err(_) => true,
    } {
      warn!("The output location is not writable, this may result in a crash");
    }

    let game_version =
      read_game_version(&assets_resolver).context("Failed to read the game version")?;
    info!("Game version is {}", game_version);

    info!("Finding all JSON files");
    let all_json_files = json_file_finder::find_all_in_assets_dir(&assets_resolver.assets_dir)
      .context("Failed to find all JSON files in the assets dir")?;
    info!("Found {} JSON files in total", all_json_files.len());

    let scan_db =
      scan::ScanDb::create(opt_output.clone(), scan::ScanDbCreateOpts { game_version });

    info!("Extracting localizable strings");
    let extractor_opts = Arc::new(lang_label_extractor::ExtractionOptions {
      locales_filter: if opt_all_locales { None } else { Some(opt_extra_locales) },
    });

    let all_json_files_len = all_json_files.len();
    progress.begin_task(all_json_files_len)?;
    progress.set_task_info(&RcString::from("<Starting...>"))?;
    progress.set_task_progress(0)?;

    let pool: threadpool::ThreadPool = {
      let mut builder = threadpool::Builder::new();
      if opt_jobs != 0 {
        builder = builder.num_threads(opt_jobs);
      }
      builder.build()
    };

    #[derive(Debug)]
    struct TaskResult {
      task_index: usize,
      found_file: FoundJsonFile,
      lang_labels: Vec<LangLabel>,
    }

    // The task results are boxed to reduce the size of the memory block which
    // needs to be copied on transmissions and when sorting.
    let (lang_labels_tx, lang_labels_rx) = mpsc::channel::<Box<TaskResult>>();

    for (task_index, found_file) in all_json_files.into_iter().enumerate() {
      let lang_labels_tx = lang_labels_tx.clone();
      let assets_resolver = assets_resolver.share_rc();
      let extractor_opts = extractor_opts.share_rc();

      pool.execute(move || {
        let json_path = Path::new(&found_file.path);
        let json_data: json::Value = match assets_resolver.load_json(json_path, &mut Vec::new()) {
          Ok(v) => v,
          Err(e) => {
            crate::report_error!(
              e.context(format!("Failed to deserialize from JSON file {:?}", json_path)),
            );
            return;
          }
        };

        let lang_labels_iter = match lang_label_extractor::extract_from_file(
          &found_file,
          &json_data,
          &extractor_opts,
        ) {
          Some(v) => v,
          _ => return,
        };

        let mut collected_lang_labels = Vec::<LangLabel>::new();
        for mut lang_label in lang_labels_iter {
          lang_label.description = if !found_file.is_lang_file {
            match fragment_descriptions::generate(&json_data, &lang_label.json_path) {
              Ok(v) => v,
              Err(e) => {
                warn!("file {:?}: fragment {:?}: {:?}", found_file.path, lang_label.json_path, e);
                continue;
              }
            }
          } else {
            Vec::new()
          };
          collected_lang_labels.push(lang_label);
        }

        lang_labels_tx
          .send(Box::new(TaskResult {
            task_index,
            found_file,
            lang_labels: collected_lang_labels,
          }))
          .unwrap();
      });
    }

    // Drop the main instance from which all others have been cloned, to allow
    // the receiving side to exit without deadlocks.
    drop(lang_labels_tx);

    let mut sorted_results = Vec::<Option<Box<TaskResult>>>::with_capacity(all_json_files_len);
    for _ in 0..all_json_files_len {
      sorted_results.push(None);
    }

    for (i, task_result) in lang_labels_rx.into_iter().enumerate() {
      progress.set_task_info(&task_result.found_file.path)?;
      progress.set_task_progress(i + 1)?;
      let i = task_result.task_index;
      sorted_results[i] = Some(task_result);
    }

    progress.set_task_progress(all_json_files_len)?;
    pool.join();
    progress.end_task()?;

    let mut total_fragments_count: usize = 0;
    // This loop isn't actually a bottleneck.
    for task_result in sorted_results.into_iter() {
      let task_result = task_result.unwrap();
      let mut scan_db_file: Option<Rc<scan::ScanGameFile>> = None;

      for lang_label in task_result.lang_labels {
        let LangLabel { json_path, lang_uid, description, text, .. } = lang_label;

        if scan_db_file.is_none() {
          scan_db_file = Some(scan_db.new_game_file(scan::ScanGameFileInitOpts {
            path: task_result.found_file.path.share_rc(),
            asset_root: task_result.found_file.asset_root.share_rc(),
          })?);
        }
        let scan_db_file = scan_db_file.as_mut().unwrap();

        scan_db_file.new_fragment(scan::ScanFragmentInitOpts {
          json_path,
          lang_uid,
          description: Rc::new(description),
          text: Rc::new(text),
          flags: Rc::new(HashSet::new()),
        });
        total_fragments_count += 1;
      }
    }

    info!(
      "Found {} localizable strings in {} files",
      total_fragments_count,
      scan_db.game_files().len(),
    );

    info!("Writing the scan database");
    let json_config = if opt_compact {
      json::UltimateFormatterConfig::COMPACT
    } else {
      json::UltimateFormatterConfig::PRETTY
    };
    scan_db.write(json_config).context("Failed to write the scan database")?;

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
#[allow(dead_code)]
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

fn read_game_version(assets_resolver: &AssetsResolver) -> AnyResult<RcString> {
  let mut changelog_bytes = Vec::new();
  let changelog_data: ChangelogFileRef = assets_resolver
    .load_json(*CHANGELOG_FILE_PATH, &mut changelog_bytes)
    .with_context(|| format!("Failed to load JSON asset file {:?}", CHANGELOG_FILE_PATH))?;

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

    fn try_extract_hotfix(mut change: &str) -> Option<(&str, u32)> {
      let (i, _): (usize, char) =
        change.char_indices().find(|(_, c)| !matches!(c, '+' | '-' | '~' | ' '))?;
      change = unsafe { change.get_unchecked(i..) };
      change = change.strip_prefix("HOTFIX(")?;
      let i = change.char_indices().take_while(|(_, c)| char::is_ascii_digit(c)).count();
      let hotfix_str = unsafe { change.get_unchecked(..i) };
      let hotfix = u32::from_str(hotfix_str).ok()?;
      change = unsafe { change.get_unchecked(i..) };
      change = change.strip_prefix(')')?;
      let _ = change;
      Some((hotfix_str, hotfix))
    }
  }

  if max_hotfix > 0 {
    Ok(RcString::from(strcat!(&*latest_entry.version, "-", max_hotfix_str)))
  } else {
    Ok(RcString::from(latest_entry.version.clone()))
  }
}

#[derive(Debug)]
pub struct AssetsResolver {
  pub assets_dir: PathBuf,
  pub assets_overrides_dirs: Vec<PathBuf>,
}

impl AssetsResolver {
  pub fn open(&self, path: &Path, options: fs::OpenOptions) -> AnyResult<fs::File> {
    for assets_dir in &self.assets_overrides_dirs {
      let full_path = assets_dir.join(path);
      match options.open(&full_path) {
        Ok(f) => return Ok(f),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => {
          return Err(AnyError::new(e).context(format!("Error when opening file {:?}", full_path)))
        }
      }
    }
    let full_path = self.assets_dir.join(path);
    match options.open(&full_path) {
      Ok(f) => Ok(f),
      Err(e) => {
        return Err(AnyError::new(e).context(format!("Error when opening file {:?}", full_path)))
      }
    }
  }

  pub fn read(&self, path: &Path) -> AnyResult<Vec<u8>> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    let mut file = self.open(path, options)?;
    let mut bytes = Vec::with_capacity(utils::buffer_capacity_for_reading_file(&file));
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
  }

  pub fn load_json<'a, T: serde::Deserialize<'a>>(
    &self,
    path: &Path,
    out_bytes: &'a mut Vec<u8>,
  ) -> AnyResult<T> {
    *out_bytes = self.read(path)?;
    let value = serde_json::from_slice(out_bytes)?;
    Ok(value)
  }
}
