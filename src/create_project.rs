use crate::cli;
use crate::impl_prelude::*;
use crate::project;
use crate::rc_string::RcString;
use crate::scan::db::ScanDb;
use crate::utils;

use std::borrow::Cow;
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
  let project = project::Project::create(project_dir, project::ProjectCreateOpts {
    game_version: scan_db.meta().game_version.clone(),
    original_locale: command_opts.original_locale,
    reference_locales: command_opts.reference_locales,
    translation_locale: command_opts.translation_locale,
    translations_dir: command_opts.translations_dir,
    splitting_strategy: command_opts.splitting_strategy,
  })?;

  info!("Generating project translation files");

  for scan_game_file in scan_db.game_files().values() {
    let global_tr_file_path: Option<Cow<'static, str>> =
      project.meta().splitting_strategy().get_tr_file_for_entire_game_file(scan_game_file.path());

    for scan_fragment in scan_game_file.fragments().values() {
      let original_text = match scan_fragment.text().get(project.meta().original_locale()) {
        Some(v) => v.to_owned(),
        None => continue,
      };

      let fragment_tr_file_path: Cow<'static, str> = match &global_tr_file_path {
        Some(v) => v.clone(),
        None => project
          .meta()
          .splitting_strategy()
          .get_tr_file_for_fragment(scan_fragment.file_path(), scan_fragment.json_path()),
      };

      let tr_file = {
        let path = RcString::from(Cow::into_owned(fragment_tr_file_path.clone()));
        project.get_tr_file(&path).unwrap_or_else(|| project.new_tr_file(path))
      };

      let game_file_chunk = tr_file
        .get_game_file_chunk(scan_game_file.path())
        .unwrap_or_else(|| tr_file.new_game_file_chunk(scan_game_file.path().share_rc()));

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

  info!("Writing the project meta file");
  let meta_file_path = project.root_dir().join(project::META_FILE_PATH);
  utils::json::write_file(&meta_file_path, project.meta())
    .with_context(|| format!("Failed to serialize to JSON file '{}'", meta_file_path.display()))
    .context("Failed to write the project meta file")?;

  info!("Writing translation files");

  let tr_files_dir = project.root_dir().join(project.meta().translations_dir());
  let translation_db_files_len = project.tr_files().len();
  for (i, (tr_file_path, translation_db)) in project.tr_files().iter().enumerate() {
    let tr_file_path = tr_files_dir.join(tr_file_path.rc_clone_inner() + ".json");
    trace!(
      "[{}/{}] Writing translation file '{}'",
      i + 1,
      translation_db_files_len,
      tr_file_path.display(),
    );

    utils::create_dir_recursively(tr_file_path.parent().unwrap()).with_context(|| {
      format!("Failed to create the parent directories for '{}'", tr_file_path.display())
    })?;
    utils::json::write_file(&tr_file_path, &translation_db)
      .with_context(|| format!("Failed to serialize to JSON file '{}'", tr_file_path.display()))?;
  }

  info!("Done!");

  Ok(())
}
