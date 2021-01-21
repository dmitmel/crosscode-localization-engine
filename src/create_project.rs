use crate::cli;
use crate::impl_prelude::*;
use crate::project;
use crate::project::splitting_strategies::{SplittingStrategy, SPLITTING_STRATEGIES_MAP};
use crate::scan::db::ScanDb;
use crate::utils;

use indexmap::IndexMap;
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

  let meta_file_path = project_dir.join(project::META_FILE_PATH);
  let meta_data = project::MetaFileSerde {
    uuid: utils::new_uuid(),
    creation_timestamp: utils::get_timestamp(),
    game_version: scan_db.meta().game_version.clone(),
    original_locale: command_opts.original_locale,
    reference_locales: command_opts.reference_locales,
    translation_locale: command_opts.translation_locale,
    splitting_strategy: command_opts.splitting_strategy,
    translations_dir: command_opts.translations_dir,
  };

  info!("Writing the project meta file");
  utils::json::write_file(&meta_file_path, &meta_data)
    .with_context(|| format!("Failed to serialize to JSON file '{}'", meta_file_path.display()))
    .context("Failed to write the project meta file")?;

  let mut splitting_strategy: Box<dyn SplittingStrategy> = {
    let constructor: &fn() -> Box<dyn SplittingStrategy> =
      SPLITTING_STRATEGIES_MAP.get(meta_data.splitting_strategy.as_str()).ok_or_else(|| {
        format_err!("No such splitting strategy '{}'", meta_data.splitting_strategy)
      })?;
    constructor()
  };

  let mut translation_db_files = IndexMap::<String, project::TranslationDbSerde>::new();

  for file in scan_db.files().values() {
    let global_translation_file: Option<Cow<'static, str>> =
      splitting_strategy.get_translation_file_for_entire_game_file(file.path());

    for fragment in file.fragments().values() {
      let original_text = match fragment.text().get(&meta_data.original_locale) {
        Some(v) => v.to_owned(),
        None => continue,
      };

      let fragment_translation_file: Cow<'static, str> = match &global_translation_file {
        Some(v) => v.clone(),
        None => {
          splitting_strategy.get_translation_file_for_fragment(file.path(), fragment.json_path())
        }
      };

      let tr_db =
        translation_db_files.entry(fragment_translation_file.into_owned()).or_insert_with(|| {
          let creation_timestamp = utils::get_timestamp();
          project::TranslationDbSerde {
            uuid: utils::new_uuid(),
            creation_timestamp,
            modification_timestamp: creation_timestamp,
            project_meta_file: "TODO".to_owned(),
            files: IndexMap::new(),
          }
        });

      let tr_file = tr_db.files.entry((**file.path()).clone()).or_insert_with(|| {
        project::TranslationDbFileSerde {
          is_lang_file: file.is_lang_file(),
          fragments: IndexMap::new(),
        }
      });

      tr_file.fragments.insert(
        (**fragment.json_path()).clone(),
        project::TranslationDbFragmentSerde {
          lang_uid: fragment.lang_uid(),
          description: fragment.description().to_owned(),
          original_text,
          reference_texts: Vec::new(),
          flags: HashMap::new(),
          translations: Vec::new(),
          comments: Vec::new(),
        },
      );
    }
  }

  let translation_files_dir = project_dir.join(&meta_data.translations_dir);
  let translation_db_files_len = translation_db_files.len();
  for (i, (translation_file_path, translation_db)) in translation_db_files.into_iter().enumerate()
  {
    let translation_file_path = translation_files_dir.join(translation_file_path + ".json");
    info!(
      "[{}/{}] Writing translation file '{}'",
      i + 1,
      translation_db_files_len,
      translation_file_path.display(),
    );

    utils::create_dir_recursively(translation_file_path.parent().unwrap()).with_context(|| {
      format!("Failed to create the parent directories for '{}'", translation_file_path.display())
    })?;
    utils::json::write_file(&translation_file_path, &translation_db).with_context(|| {
      format!("Failed to serialize to JSON file '{}'", translation_file_path.display())
    })?;
  }

  Ok(())
}
