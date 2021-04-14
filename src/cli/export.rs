use crate::impl_prelude::*;
use crate::localize_me;
use crate::project::exporters::{self, ExportedFragment};
use crate::project::splitters;
use crate::project::Project;
use crate::rc_string::{MaybeStaticStr, RcString};
use crate::utils;
use crate::utils::json;

use indexmap::IndexMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ExportCommand;

impl super::Command for ExportCommand {
  fn name(&self) -> &'static str { "export" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about(
        "Exports translations from a project into a different format, for example for compiling \
        into Localize Me translation packs for use in CrossCode mods.",
      )
      .arg(
        clap::Arg::new("project_dir")
          .value_name("PROJECT")
          .required(true)
          .about("Path to the project directory."),
      )
      .arg(
        clap::Arg::new("output")
          .value_name("PATH")
          .short('o')
          .long("output")
          .required(true)
          .about(
            "Path to the destination file or directory for exporting. A directory is used when \
            a splitter is specified.",
          ),
      )
      .arg(
        clap::Arg::new("format")
          .value_name("NAME")
          .short('f')
          .long("format")
          .possible_values(exporters::EXPORTERS_IDS)
          .required(true)
          .about("Format to export to."),
      )
      .arg(
        clap::Arg::new("splitter")
          .value_name("NAME")
          .long("splitter")
          .possible_values(splitters::SPLITTERS_IDS)
          .about("Strategy used for splitting the exported files."),
      )
      .arg(
        clap::Arg::new("remove_untranslated")
          .long("remove-untranslated")
          //
          .about(
            "Whether to remove untranslated strings from the exported files. Note that some \
            formats and/or tasks may still need the empty translations.",
          ),
      )
      .arg(
        clap::Arg::new("mapping_output")
          .value_name("PATH")
          .long("mapping-output")
          //
          .about(
            "Write a JSON file containing a mapping table from game files to the translation \
            files containg their strings.",
          ),
      )
      .arg(
        clap::Arg::new("mapping_lm_paths")
          .long("mapping-lm-paths")
          .about("Use Localize Me-style paths of game files in the mapping table."),
      )
      .arg(
        clap::Arg::new("compact")
          .long("compact")
          //
          .about(
            "Write exported files compactly, for example before packaging them for distribution. \
            Note that this will mean different things depending on the output format.",
          ),
      )
  }

  fn run(&self, _global_opts: super::GlobalOpts, matches: &clap::ArgMatches) -> AnyResult<()> {
    let opt_project_dir = PathBuf::from(matches.value_of_os("project_dir").unwrap());
    let opt_output = PathBuf::from(matches.value_of_os("output").unwrap());
    let opt_format = RcString::from(matches.value_of("format").unwrap());
    let opt_splitter = matches.value_of("splitter").map(RcString::from);
    let opt_remove_untranslated = matches.is_present("remove_untranslated");
    let opt_mapping_output = matches.value_of_os("mapping_output").map(PathBuf::from);
    let opt_mapping_lm_paths = matches.is_present("mapping_lm_paths");
    let opt_compact = matches.is_present("compact");

    info!(
      "Exporting a translation project in {:?} as {:?} into {:?}",
      opt_project_dir, opt_format, opt_output,
    );

    let project = Project::open(opt_project_dir).context("Failed to open the project")?;
    let mut exporter =
      exporters::create(&opt_format, exporters::ExporterConfig { compact: opt_compact })
        .context("Failed to create the exporter")?;

    #[allow(clippy::manual_map)]
    let mut splitter = match opt_splitter {
      Some(id) => Some(splitters::create_by_id(&id).context("Failed to create the splitter")?),
      _ => None,
    };

    let mut total_exported_fragments_count = 0;
    let mut all_exported_fragments = Vec::<ExportedFragment>::new();
    let mut fragments_by_export_path = IndexMap::<RcString, Vec<ExportedFragment>>::new();
    let mut exported_files_mapping = IndexMap::<RcString, RcString>::new();

    let export_file_extension = exporter.file_extension();

    for game_file in project.virtual_game_files().values() {
      let game_file_path = game_file.path();
      let mut fragments_in_export_file: Option<&mut Vec<ExportedFragment>> = None;

      for fragment in game_file.fragments().values() {
        if opt_remove_untranslated && fragment.translations().is_empty() {
          continue;
        }

        if fragments_in_export_file.is_none() {
          fragments_in_export_file = Some(if let Some(splitter) = &mut splitter {
            let export_file_path: MaybeStaticStr = if let Some(path) =
              splitter.get_tr_file_for_entire_game_file(game_file.asset_root(), game_file_path)
            {
              path
            } else {
              bail!(
                "The selected splitter can't be used for export because it has requested \
                per-fragment splitting on the game file {:?}. An entire game file can be assigned \
                to one and only one export file.",
                game_file_path,
              )
            };

            let export_file_path =
              RcString::from(utils::fast_concat(&[&export_file_path, ".", export_file_extension]));

            let mapping_game_file_path = if opt_mapping_lm_paths {
              RcString::from(localize_me::serialize_file_path(game_file_path))
            } else {
              game_file_path.share_rc()
            };
            if let Some(prev_assigned_export_file_path) = exported_files_mapping
              .insert(mapping_game_file_path.share_rc(), export_file_path.share_rc())
            {
              ensure!(
                prev_assigned_export_file_path == export_file_path,
                "The splitter has assigned inconsistent export paths to the game file {:?}: the \
                previous value was {:?}, the new one is {:?}. This is a bug in the splitter.",
                mapping_game_file_path,
                prev_assigned_export_file_path,
                export_file_path,
              );
            }

            fragments_by_export_path.entry(export_file_path.share_rc()).or_insert_with(Vec::new)
          } else {
            &mut all_exported_fragments
          });
        }

        match &mut fragments_in_export_file {
          Some(v) => v,
          None => unreachable!(),
        }
        .push(ExportedFragment::new(fragment));
        total_exported_fragments_count += 1;
      }
    }

    let exported_meta = exporters::ExportedProjectMeta::new(project.meta());
    let mut export_fragments_to_file =
      |path: &Path, fragments: &[ExportedFragment]| -> AnyResult<()> {
        let mut writer = io::BufWriter::new(
          fs::File::create(&path)
            .with_context(|| format!("Failed to open file {:?} for writing", path))?,
        );
        exporter.export(&exported_meta, &fragments, &mut writer)?;
        writer.flush()?;
        Ok(())
      };

    if splitter.is_some() {
      for (export_file_path, fragments) in &fragments_by_export_path {
        if fragments.is_empty() {
          continue;
        }
        let export_file_path = opt_output.join(export_file_path);
        utils::create_dir_recursively(export_file_path.parent().unwrap()).with_context(|| {
          format!("Failed to create the parent directories for {:?}", export_file_path)
        })?;
        export_fragments_to_file(&export_file_path, fragments)
          .with_context(|| format!("Failed to export all fragments to file {:?}", opt_output))?;
      }

      info!(
        "Exported {} fragments to {} files",
        total_exported_fragments_count,
        fragments_by_export_path.len(),
      );
    } else {
      export_fragments_to_file(&opt_output, &all_exported_fragments)
        .with_context(|| format!("Failed to export all fragments to file {:?}", opt_output))?;
      info!("Exported {} fragments", total_exported_fragments_count);
    }

    if let Some(mapping_file_path) = opt_mapping_output {
      if splitter.is_some() {
        json::write_file(
          &mapping_file_path,
          &exported_files_mapping,
          if opt_compact {
            json::UltimateFormatterConfig::compact()
          } else {
            json::UltimateFormatterConfig::pretty()
          },
        )
        .with_context(|| format!("Failed to write the mapping file to {:?}", mapping_file_path))?;

        info!("Written the mapping file with {} entries", exported_files_mapping.len());
      } else {
        warn!(
          "Mapping output file was specified, but splitter wasn't. A mapping file doesn't make \
          sense without splitting."
        );
      }
    }

    Ok(())
  }
}
