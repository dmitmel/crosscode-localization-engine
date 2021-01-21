use crate::cli;
use crate::impl_prelude::*;
use crate::project;
use crate::project::splitting_strategies::{SplittingStrategy, SPLITTING_STRATEGIES_MAP};
use crate::scan::db::ScanDb;
use crate::utils::{self, try_any_result_hint};

use indexmap::IndexMap;
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

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

  create_dir_recursively(&project_dir).context("Failed to create the project dir")?;

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
  try_any_result_hint(
    try {
      let mut writer = io::BufWriter::new(
        fs::File::create(&meta_file_path)
          .with_context(|| format!("Failed to open file '{}'", meta_file_path.display()))?,
      );
      serde_json::to_writer_pretty(&mut writer, &meta_data).with_context(|| {
        format!("Failed to write JSON into file '{}'", meta_file_path.display())
      })?;
      writer.write_all(b"\n")?;
      writer.flush()?;
    },
  )
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

      let translation_db =
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
      let file = translation_db.files.entry((**file.path()).clone()).or_insert_with(|| {
        project::TranslationDbFileSerde {
          is_lang_file: file.is_lang_file(),
          fragments: IndexMap::new(),
        }
      });
      file.fragments.insert(
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

  // TODO: Get rid of unwraps! They are here only for a quick prototype!
  let translation_files_dir = project_dir.join(&meta_data.translations_dir);
  for (translation_file_path, translation_db) in translation_db_files {
    let translation_file_path = translation_files_dir.join(translation_file_path + ".json");
    info!("Writing translation file '{}'", translation_file_path.display());
    create_dir_recursively(translation_file_path.parent().unwrap()).unwrap();
    let mut file = fs::File::create(translation_file_path).unwrap();
    serde_json::to_writer_pretty(&mut file, &translation_db).unwrap();
    file.write_all(b"\n").unwrap();
    file.flush().unwrap();
  }

  Ok(())
}

#[inline(never)]
fn create_dir_recursively(path: impl AsRef<Path>) -> io::Result<()> {
  #[inline(never)]
  fn inner(path: &Path) -> io::Result<()> { fs::DirBuilder::new().recursive(true).create(path) }
  inner(path.as_ref())
}
