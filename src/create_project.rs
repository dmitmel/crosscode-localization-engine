use crate::cli;
use crate::impl_prelude::*;
use crate::project;
use crate::utils::{self, try_any_result_hint};

use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub fn run(
  common_opts: cli::CommonOpts,
  command_opts: cli::CreateProjectCommandOpts,
) -> AnyResult<()> {
  let timestamp = utils::get_timestamp();
  info!(
    "Creating a translation project in '{}', translation from '{}' to '{}'",
    command_opts.project_dir.display(),
    command_opts.original_locale,
    command_opts.translation_locale,
  );

  create_dir_recursively(&command_opts.project_dir).context("Failed to create the project dir")?;

  let meta_file_path = command_opts.project_dir.join(project::META_FILE_PATH);
  let meta_data = project::MetaFileData {
    uuid: utils::new_uuid(),
    created_at: timestamp,
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
      serde_json::to_writer_pretty(&mut writer, &meta_data)?;
      writer.write_all(b"\n")?;
      writer.flush()?;
    },
  )
  .context("Failed to write the scan database")?;

  Ok(())
}

#[inline(never)]
fn create_dir_recursively(path: impl AsRef<Path>) -> io::Result<()> {
  #[inline(never)]
  fn inner(path: &Path) -> io::Result<()> { fs::DirBuilder::new().recursive(true).create(path) }
  inner(path.as_ref())
}
