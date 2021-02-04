use crate::cli;
use crate::impl_prelude::*;
use crate::project;
use crate::rc_string::RcString;
use crate::scan::db::ScanDb;
use crate::utils;

use std::collections::HashMap;

pub fn run(
  _common_opts: cli::CommonOpts,
  command_opts: cli::CreateProjectCommandOpts,
) -> AnyResult<()> {
  let project_dir = command_opts.project_dir;
  info!(
    "Creating a translation project in '{}', translation from '{}' to '{}'",
    project_dir.display(),
    command_opts.original_locale,
    command_opts.translation_locale,
  );

  let scan_db = ScanDb::open(command_opts.scan_db).context("Failed to open the scan database")?;

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
    splitting_strategy: command_opts.splitting_strategy,
  })
  .context("Failed to create the project structure")?;

  info!("Generating project translation files");

  for scan_game_file in scan_db.game_files().values() {
    let global_tr_file_path: Option<RcString> = project
      .meta()
      .splitting_strategy_mut()
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
            .splitting_strategy_mut()
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
        flags: HashMap::new(),
      });
    }
  }

  info!("Generated {} translation files", project.tr_files().len());

  project.write().context("Failed to write the project")?;

  info!("Done!");

  Ok(())
}
