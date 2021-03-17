use crate::impl_prelude::*;
use crate::project;
use crate::project::splitters;
use crate::rc_string::RcString;
use crate::scan;
use crate::utils::{self, RcExt};

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct CreateProjectCommand;

impl super::Command for CreateProjectCommand {
  fn name(&self) -> &'static str { "create-project" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about("Creates an empty translation project using the data obtained by scanning the game.")
      .arg(
        clap::Arg::new("project_dir")
          .value_name("PROJECT")
          .required(true)
          .about("Path to the project directory."),
      )
      .arg(
        clap::Arg::new("main_scan_db")
          .value_name("MAIN_SCAN_DB_PATH")
          .required(true)
          .about("Path to the main scan database from which the project will be generated."),
      )
      .arg(
        clap::Arg::new("extra_scan_dbs")
          .value_name("EXTRA_SCAN_DB_PATHS")
          .multiple(true)
          //
          .about(
            "Paths to extra scan databases from which additional fragments will be read. Keep \
            in mind that the metadata only of the main database will be used.",
          ),
      )
      .arg(
        clap::Arg::new("original_locale")
          .value_name("LOCALE")
          .long("original-locale")
          .default_value("en_US")
          .about("Locale to translate from."),
      )
      .arg(
        clap::Arg::new("reference_locales")
          .value_name("LOCALE")
          .multiple(true)
          .number_of_values(1)
          .long("reference-locales")
          .about("Other original locales to include for reference."),
      )
      .arg(
        clap::Arg::new("translation_locale")
          .value_name("LOCALE")
          .long("translation-locale")
          .required(true)
          .about("Locale of the translation."),
      )
      .arg(
        clap::Arg::new("splitter")
          .value_name("NAME")
          .long("splitter")
          .possible_values(splitters::SPLITTERS_IDS)
          .default_value(splitters::NextGenerationSplitter::ID)
          .about(
            "Strategy used for assigning game files (and individual fragments in them) to \
            translation storage files.",
          ),
      )
      .arg(
        clap::Arg::new("translations_dir")
          .value_name("PATH")
          .long("translations-dir")
          .validator_os(|s| {
            if !Path::new(s).is_relative() {
              return Err("Path must be relative".to_owned());
            }
            Ok(())
          })
          .default_value("tr")
          .about("Path to project's translation storage files, relative to project's directory."),
      )
  }

  fn run(&self, _global_opts: super::GlobalOpts, matches: &clap::ArgMatches) -> AnyResult<()> {
    let opt_project_dir = PathBuf::from(matches.value_of_os("project_dir").unwrap());
    let opt_main_scan_db = PathBuf::from(matches.value_of_os("main_scan_db").unwrap());
    let opt_extra_scan_dbs: Vec<_> = matches
      .values_of("extra_scan_dbs")
      .map_or_else(Vec::new, |values| values.map(PathBuf::from).collect());
    let opt_original_locale = RcString::from(matches.value_of("original_locale").unwrap());
    let opt_reference_locales: Vec<_> = matches
      .values_of("reference_locales")
      .map_or_else(Vec::new, |values| values.map(RcString::from).collect());
    let opt_translation_locale = RcString::from(matches.value_of("translation_locale").unwrap());
    let opt_splitter = RcString::from(matches.value_of("splitter").unwrap());
    let opt_translations_dir = RcString::from(matches.value_of("translations_dir").unwrap());

    info!(
      "Creating a translation project in {:?}, translation from {:?} to {:?}",
      opt_project_dir, opt_original_locale, opt_translation_locale,
    );

    info!("Reading the main scan database from {:?}", opt_main_scan_db);
    let main_scan_db =
      scan::ScanDb::open(opt_main_scan_db).context("Failed to open a scan database")?;
    let scan_game_version = main_scan_db.meta().game_version.share_rc();

    let mut scan_dbs = Vec::with_capacity(1 + opt_extra_scan_dbs.len());
    scan_dbs.push(main_scan_db);
    for path in opt_extra_scan_dbs {
      info!("Reading an extra scan database from {:?}", path);
      let extra_scan_db = scan::ScanDb::open(path).context("Failed to open a scan database")?;
      let extra_scan_game_version = &extra_scan_db.meta().game_version;
      if *extra_scan_game_version != scan_game_version {
        warn!(
          "The game version of an extra scan database ({}) doesn't match the game version of the \
        main one ({})",
          extra_scan_game_version, scan_game_version,
        );
      }
      scan_dbs.push(extra_scan_db);
    }

    utils::create_dir_recursively(&opt_project_dir).context("Failed to create the project dir")?;
    let timestamp = utils::get_timestamp();
    let project = project::Project::create(opt_project_dir, project::ProjectMetaInitOpts {
      id: utils::new_uuid(),
      creation_timestamp: timestamp,
      modification_timestamp: timestamp,
      game_version: scan_game_version,
      original_locale: opt_original_locale,
      reference_locales: opt_reference_locales,
      translation_locale: opt_translation_locale,
      translations_dir: opt_translations_dir,
      splitter: opt_splitter,
    })
    .context("Failed to create the project structure")?;

    info!("Generating project translation files");

    for scan_db in scan_dbs {
      for scan_game_file in scan_db.game_files().values() {
        let global_tr_file_path: Option<RcString> = project
          .meta()
          .splitter_mut()
          .get_tr_file_for_entire_game_file(scan_game_file.asset_root(), scan_game_file.path())
          .map(RcString::from);

        for scan_fragment in scan_game_file.fragments().values() {
          let original_text = match scan_fragment.text().get(project.meta().original_locale()) {
            Some(v) => v.share_rc(),
            None => continue,
          };

          let fragment_tr_file_path: RcString = match &global_tr_file_path {
            Some(v) => v.share_rc(),
            None => RcString::from(project.meta().splitter_mut().get_tr_file_for_fragment(
              scan_fragment.file_asset_root(),
              scan_fragment.file_path(),
              scan_fragment.json_path(),
            )),
          };

          let tr_file = match project.get_tr_file(&fragment_tr_file_path) {
            Some(v) => v,
            None => {
              let timestamp = utils::get_timestamp();
              project.new_tr_file(project::TrFileInitOpts {
                id: utils::new_uuid(),
                creation_timestamp: timestamp,
                modification_timestamp: timestamp,
                relative_path: fragment_tr_file_path.share_rc(),
              })
            }
          };

          let game_file_chunk = match tr_file.get_game_file_chunk(scan_game_file.path()) {
            Some(v) => v,
            None => tr_file.new_game_file_chunk(project::GameFileChunkInitOpts {
              asset_root: scan_game_file.asset_root().share_rc(),
              path: scan_game_file.path().share_rc(),
            })?,
          };

          game_file_chunk.new_fragment(project::FragmentInitOpts {
            id: utils::new_uuid(),
            file_path: scan_fragment.file_path().share_rc(),
            json_path: scan_fragment.json_path().share_rc(),
            lang_uid: scan_fragment.lang_uid(),
            description: scan_fragment.description().share_rc(),
            original_text,
            // reference_texts: HashMap::new(),
            flags: scan_fragment.flags().share_rc(),
          });
        }
      }
    }

    info!("Generated {} translation files", project.tr_files().len());

    info!("Writing the project");
    project.write().context("Failed to write the project")?;
    info!("Done!");

    Ok(())
  }
}
