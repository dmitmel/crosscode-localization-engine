use crate::cli;
use crate::impl_prelude::*;
use crate::project::exporters;
use crate::project::splitting_strategies;
use crate::project::{Fragment, Project};
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::{self, RcExt};

use indexmap::IndexMap;
use std::borrow::Cow;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;

pub fn run(_common_opts: cli::CommonOpts, command_opts: cli::ExportCommandOpts) -> AnyResult<()> {
  let output_path = command_opts.output;
  info!(
    "Exporting a translation project in '{}' as '{}' into '{}'",
    command_opts.project_dir.display(),
    command_opts.format,
    output_path.display(),
  );

  let project = Project::open(command_opts.project_dir).context("Failed to open the project")?;
  let mut exporter = exporters::create(&command_opts.format, exporters::ExporterConfig {
    compact: command_opts.compact,
  })
  .context("Failed to create the exporter")?;

  let mut splitting_strategy = if let Some(id) = command_opts.splitting_strategy {
    Some(
      splitting_strategies::create_by_id(&id)
        .context("Failed to create the splitting strategy")?,
    )
  } else {
    None
  };

  let mut all_exported_fragments = Vec::<Rc<Fragment>>::new();
  let mut fragments_by_export_path = IndexMap::<RcString, Vec<Rc<Fragment>>>::new();
  let mut exported_files_mapping = IndexMap::<RcString, RcString>::new();

  let export_file_extension = exporter.file_extension();

  for game_file in project.virtual_game_files().values() {
    let game_file_path = game_file.path();
    let mut fragments_in_export_file: Option<&mut Vec<_>> = if let Some(splitting_strategy) =
      &mut splitting_strategy
    {
      let export_file_path: Cow<'static, str> =
        if let Some(path) = splitting_strategy.get_tr_file_for_entire_game_file(game_file_path) {
          path
        } else {
          bail!(
            "The selected splitting strategy can't be used for export because it has requested \
            per-fragment splitting on the game file '{}'. An entire game file can be assigned \
            to one and only one export file.",
            game_file_path,
          )
        };

      let export_file_path =
        RcString::from(utils::fast_concat(&[&export_file_path, ".", export_file_extension]));

      if let Some(prev_assigned_export_file_path) =
        exported_files_mapping.insert(game_file_path.share_rc(), export_file_path.share_rc())
      {
        ensure!(
          prev_assigned_export_file_path == export_file_path,
          "The splitting strategy has assigned inconsistent export paths to the game file \
          '{}': the previous value was '{}', the new one is '{}'. This is a bug in the \
          splitting strategy.",
          game_file_path,
          prev_assigned_export_file_path,
          export_file_path,
        );
      }

      Some(fragments_by_export_path.entry(export_file_path.share_rc()).or_insert_with(Vec::new))
    } else {
      None
    };

    for fragment in game_file.fragments().values() {
      if command_opts.remove_untranslated && fragment.translations().is_empty() {
        continue;
      }

      all_exported_fragments.push(fragment.share_rc());
      if let Some(fragments_in_export_file) = &mut fragments_in_export_file {
        fragments_in_export_file.push(fragment.share_rc());
      }
    }
  }

  let mut export_fragments_to_file = |path: &Path, fragments: &[Rc<Fragment>]| -> AnyResult<()> {
    let mut writer = io::BufWriter::new(
      fs::File::create(&path)
        .with_context(|| format!("Failed to open file '{}' for writing", path.display()))?,
    );
    exporter.export(project.meta(), &fragments, &mut writer)?;
    writer.flush()?;
    Ok(())
  };

  if splitting_strategy.is_some() {
    for (export_file_path, fragments) in &fragments_by_export_path {
      let export_file_path = output_path.join(export_file_path);
      utils::create_dir_recursively(export_file_path.parent().unwrap()).with_context(|| {
        format!("Failed to create the parent directories for '{}'", export_file_path.display())
      })?;
      export_fragments_to_file(&export_file_path, fragments).with_context(|| {
        format!("Failed to export all fragments to file '{}'", output_path.display())
      })?;
    }

    info!(
      "Exported {} fragments to {} files",
      all_exported_fragments.len(),
      fragments_by_export_path.len(),
    );
  } else {
    export_fragments_to_file(&output_path, &all_exported_fragments).with_context(|| {
      format!("Failed to export all fragments to file '{}'", output_path.display())
    })?;
    info!("Exported {} fragments", all_exported_fragments.len());
  }

  if let Some(mapping_file_path) = command_opts.mapping_file_output {
    if splitting_strategy.is_some() {
      json::write_file(
        &mapping_file_path,
        &exported_files_mapping,
        json::UltimateFormatterConfig {
          indent: if command_opts.compact { None } else { Some(json::DEFAULT_INDENT) },
          ..Default::default()
        },
      )
      .with_context(|| {
        format!("Failed to write the mapping file to '{}'", mapping_file_path.display())
      })?;

      info!("Written the mapping file with {} entries", exported_files_mapping.len());
    } else {
      warn!(
        "mapping_file_output was specified, but splitting_strategy wasn't. A mapping file doesn't \
        make sense without splitting."
      );
    }
  }

  Ok(())
}
