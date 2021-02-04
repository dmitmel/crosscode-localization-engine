use crate::cli;
use crate::impl_prelude::*;
use crate::project;
use crate::project::splitters;
use crate::rc_string::RcString;
use crate::scan;
use crate::utils;

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CommandOpts {
  pub project_dir: PathBuf,
  pub scan_db: PathBuf,
  pub original_locale: RcString,
  pub reference_locales: Vec<RcString>,
  pub translation_locale: RcString,
  pub splitter: RcString,
  pub translations_dir: RcString,
}

impl CommandOpts {
  pub fn from_matches(matches: &clap::ArgMatches<'_>) -> Self {
    Self {
      project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()),
      scan_db: PathBuf::from(matches.value_of_os("scan_db").unwrap()),
      original_locale: RcString::from(matches.value_of("original_locale").unwrap()),
      reference_locales: matches
        .values_of("reference_locales")
        .map_or_else(Vec::new, |values| values.map(RcString::from).collect()),
      translation_locale: RcString::from(matches.value_of("translation_locale").unwrap()),
      splitter: RcString::from(matches.value_of("splitter").unwrap()),
      translations_dir: RcString::from(matches.value_of("translations_dir").unwrap()),
    }
  }
}

pub fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
  clap::App::new("create-project")
    .about("Creates an empty translation project using the data obtained by scanning the game.")
    .arg(
      clap::Arg::with_name("project_dir")
        .value_name("PROJECT")
        .required(true)
        .help("Path to the project directory."),
    )
    .arg(
      clap::Arg::with_name("scan_db")
        .value_name("PATH")
        .long("scan-db")
        .required(true)
        .help("Path to the scan database."),
    )
    .arg(
      clap::Arg::with_name("original_locale")
        .value_name("LOCALE")
        .long("original-locale")
        .default_value("en_US")
        .help("Locale to translate from."),
    )
    .arg(
      clap::Arg::with_name("reference_locales")
        .value_name("LOCALE")
        .multiple(true)
        .number_of_values(1)
        .long("reference-locales")
        .help("Other original locales to include for reference."),
    )
    .arg(
      clap::Arg::with_name("translation_locale")
        .value_name("LOCALE")
        .long("translation-locale")
        .required(true)
        .help("Locale of the translation."),
    )
    .arg(
      clap::Arg::with_name("splitter")
        .value_name("NAME")
        .long("splitter")
        .possible_values(splitters::SPLITTERS_IDS)
        .default_value(splitters::NextGenerationSplitter::ID)
        .help(
          "Strategy used for assigning game files (and individual fragments in them) to \
          translation storage files.",
        ),
    )
    .arg(
      clap::Arg::with_name("translations_dir")
        .value_name("PATH")
        .long("translations-dir")
        .validator_os(|s| {
          if !Path::new(s).is_relative() {
            return Err(OsString::from("Path must be relative"));
          }
          Ok(())
        })
        .default_value("tr")
        .help("Path to project's translation storage files, relative to project's directory."),
    )
}

pub fn run(_global_opts: cli::GlobalOpts, command_opts: CommandOpts) -> AnyResult<()> {
  let project_dir = command_opts.project_dir;
  info!(
    "Creating a translation project in {:?}, translation from {:?} to {:?}",
    project_dir, command_opts.original_locale, command_opts.translation_locale,
  );

  let scan_db =
    scan::ScanDb::open(command_opts.scan_db).context("Failed to open the scan database")?;

  utils::create_dir_recursively(&project_dir).context("Failed to create the project dir")?;
  let timestamp = utils::get_timestamp();
  let project = project::Project::create(project_dir, project::ProjectMetaInitOpts {
    uuid: utils::new_uuid(),
    creation_timestamp: timestamp,
    modification_timestamp: timestamp,
    game_version: scan_db.meta().game_version.share_rc(),
    original_locale: command_opts.original_locale,
    reference_locales: command_opts.reference_locales,
    translation_locale: command_opts.translation_locale,
    translations_dir: command_opts.translations_dir,
    splitter: command_opts.splitter,
  })
  .context("Failed to create the project structure")?;

  info!("Generating project translation files");

  for scan_game_file in scan_db.game_files().values() {
    let global_tr_file_path: Option<RcString> = project
      .meta()
      .splitter_mut()
      .get_tr_file_for_entire_game_file(scan_game_file.path())
      .map(RcString::from);

    for scan_fragment in scan_game_file.fragments().values() {
      let original_text = match scan_fragment.text().get(project.meta().original_locale()) {
        Some(v) => v.share_rc(),
        None => continue,
      };

      let fragment_tr_file_path: RcString = match &global_tr_file_path {
        Some(v) => v.share_rc(),
        None => RcString::from(
          project
            .meta()
            .splitter_mut()
            .get_tr_file_for_fragment(scan_fragment.file_path(), scan_fragment.json_path()),
        ),
      };

      let tr_file = {
        project.get_tr_file(&fragment_tr_file_path).unwrap_or_else(|| {
          let timestamp = utils::get_timestamp();
          project.new_tr_file(project::TrFileInitOpts {
            uuid: utils::new_uuid(),
            creation_timestamp: timestamp,
            modification_timestamp: timestamp,
            relative_path: fragment_tr_file_path.share_rc(),
          })
        })
      };

      let game_file_chunk = {
        let path = scan_game_file.path();
        tr_file.get_game_file_chunk(path).unwrap_or_else(|| {
          tr_file.new_game_file_chunk(project::GameFileChunkInitOpts { path: path.share_rc() })
        })
      };

      game_file_chunk.new_fragment(project::FragmentInitOpts {
        file_path: scan_fragment.file_path().share_rc(),
        json_path: scan_fragment.json_path().share_rc(),
        lang_uid: scan_fragment.lang_uid(),
        description: scan_fragment.description().to_owned(),
        original_text,
        // reference_texts: HashMap::new(),
        flags: HashSet::new(),
      });
    }
  }

  info!("Generated {} translation files", project.tr_files().len());

  project.write().context("Failed to write the project")?;

  info!("Done!");

  Ok(())
}
