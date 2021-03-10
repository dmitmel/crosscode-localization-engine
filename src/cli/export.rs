use crate::impl_prelude::*;
use crate::project::exporters;
use crate::project::splitters;
use crate::project::{Fragment, Project};
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::{self, RcExt};

use indexmap::IndexMap;
use std::borrow::Cow;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug)]
pub struct ExportCommand;

impl super::Command for ExportCommand {
  fn name(&self) -> &'static str { "export" }

  fn create_arg_parser<'a, 'b>(&self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b> {
    app
      .about(
        "Exports translations from a project into a different format, for example for compiling \
        into Localize Me translation packs for use in CrossCode mods.",
      )
      .arg(
        clap::Arg::with_name("project_dir")
          .value_name("PROJECT")
          .required(true)
          .help("Path to the project directory."),
      )
      .arg(
        clap::Arg::with_name("output")
          .value_name("PATH")
          .short("o")
          .long("output")
          .required(true)
          .help(
            "Path to the destination file or directory for exporting. A directory is used when \
            a splitter is specified.",
          ),
      )
      .arg(
        clap::Arg::with_name("format")
          .value_name("NAME")
          .short("f")
          .long("format")
          .possible_values(exporters::EXPORTERS_IDS)
          .required(true)
          .help("Format to export to."),
      )
      .arg(
        clap::Arg::with_name("splitter")
          .value_name("NAME")
          .long("splitter")
          .possible_values(splitters::SPLITTERS_IDS)
          .help("Strategy used for splitting the exported files."),
      )
      .arg(
        clap::Arg::with_name("remove_untranslated")
          .long("remove-untranslated")
          //
          .help(
            "Whether to remove untranslated strings from the exported files. Note that some \
            formats and/or tasks may still need the empty translations.",
          ),
      )
      .arg(
        clap::Arg::with_name("mapping_file_output")
          .value_name("PATH")
          .long("mapping-file-output")
          .help("File to write a Localize Me-style mapping table to."),
      )
      .arg(
        clap::Arg::with_name("compact")
          .long("compact")
          //
          .help(
            "Write exported files compactly, for example before packaging them for distribution. \
            Note that this will mean different things depending on the output format.",
          ),
      )
  }

  fn run(&self, _global_opts: super::GlobalOpts, matches: &clap::ArgMatches<'_>) -> AnyResult<()> {
    let opt_project_dir = PathBuf::from(matches.value_of_os("project_dir").unwrap());
    let opt_output = PathBuf::from(matches.value_of_os("output").unwrap());
    let opt_format = RcString::from(matches.value_of("format").unwrap());
    let opt_splitter = matches.value_of("splitter").map(RcString::from);
    let opt_remove_untranslated = matches.is_present("remove_untranslated");
    let opt_mapping_file_output = matches.value_of_os("mapping_file_output").map(PathBuf::from);
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

    let mut all_exported_fragments = Vec::<Rc<Fragment>>::new();
    let mut fragments_by_export_path = IndexMap::<RcString, Vec<Rc<Fragment>>>::new();
    let mut exported_files_mapping = IndexMap::<RcString, RcString>::new();

    let export_file_extension = exporter.file_extension();

    for game_file in project.virtual_game_files().values() {
      let game_file_path = game_file.path();
      let mut fragments_in_export_file: Option<&mut Vec<_>> = if let Some(splitter) = &mut splitter
      {
        let export_file_path: Cow<'static, str> = if let Some(path) =
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

        if let Some(prev_assigned_export_file_path) =
          exported_files_mapping.insert(game_file_path.share_rc(), export_file_path.share_rc())
        {
          ensure!(
            prev_assigned_export_file_path == export_file_path,
            "The splitter has assigned inconsistent export paths to the game file {:?}: the \
          previous value was {:?}, the new one is {:?}. This is a bug in the splitter.",
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
        if opt_remove_untranslated && fragment.translations().is_empty() {
          continue;
        }

        all_exported_fragments.push(fragment.share_rc());
        if let Some(fragments_in_export_file) = &mut fragments_in_export_file {
          fragments_in_export_file.push(fragment.share_rc());
        }
      }
    }

    let mut export_fragments_to_file =
      |path: &Path, fragments: &[Rc<Fragment>]| -> AnyResult<()> {
        let mut writer = io::BufWriter::new(
          fs::File::create(&path)
            .with_context(|| format!("Failed to open file {:?} for writing", path))?,
        );
        exporter.export(project.meta(), &fragments, &mut writer)?;
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
        all_exported_fragments.len(),
        fragments_by_export_path.len(),
      );
    } else {
      export_fragments_to_file(&opt_output, &all_exported_fragments)
        .with_context(|| format!("Failed to export all fragments to file {:?}", opt_output))?;
      info!("Exported {} fragments", all_exported_fragments.len());
    }

    if let Some(mapping_file_path) = opt_mapping_file_output {
      if splitter.is_some() {
        json::write_file(
          &mapping_file_path,
          &exported_files_mapping,
          json::UltimateFormatterConfig {
            indent: if opt_compact { None } else { Some(json::DEFAULT_INDENT) },
            ..Default::default()
          },
        )
        .with_context(|| format!("Failed to write the mapping file to {:?}", mapping_file_path))?;

        info!("Written the mapping file with {} entries", exported_files_mapping.len());
      } else {
        warn!(
          "mapping_file_output was specified, but splitter wasn't. A mapping file doesn't \
        make sense without splitting."
        );
      }
    }

    Ok(())
  }
}
