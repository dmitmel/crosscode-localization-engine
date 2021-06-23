use crate::impl_prelude::*;
use crate::localize_me;
use crate::progress::ProgressReporter;
use crate::project::exporters::{self, ExportedFragment, ExportedTranslation};
use crate::project::importers::{self, ImportedFragment};
use crate::project::splitters;
use crate::rc_string::{MaybeStaticStr, RcString};
use crate::scan;
use crate::utils::json;
use crate::utils::{self, RcExt};

use indexmap::IndexMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug)]
pub struct ConvertCommand;

impl super::Command for ConvertCommand {
  fn name(&self) -> &'static str { "convert" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about(
        "Converts between various translation file formats without the need for import/export.",
      )
      .arg(
        clap::Arg::new("scan_db")
          .value_name("SCAN_DB_PATH")
          .value_hint(clap::ValueHint::FilePath)
          .long("scan")
          .required(true)
          //
          .about(
            "A scan database to use for referencing data like fragment descriptions if the \
            input format doesn't contain it.",
          ),
      )
      .arg(
        clap::Arg::new("original_locale")
          .value_name("LOCALE")
          .value_hint(clap::ValueHint::Other)
          .long("original-locale")
          //
          .about(
            "Locale of the original strings in the input files, used for warning about staleness \
            of the translations. Normally, during exports, this is determined from the project \
            meta file.",
          ),
      )
      .arg(
        clap::Arg::new("inputs")
          .value_name("INPUT_PATH")
          .value_hint(clap::ValueHint::AnyPath)
          .multiple(true)
          .required(true)
          .conflicts_with("inputs_file")
          .about("Paths to the input files."),
      )
      .arg(
        clap::Arg::new("inputs_file")
          .value_name("PATH")
          .value_hint(clap::ValueHint::FilePath)
          .short('i')
          //
          .about(
            "Read paths to input files from a file. If there are other paths specified via \
            command-line arguments, then those will be used instead and the inputs file will be \
            ignored.",
          ),
      )
      .arg(
        clap::Arg::new("output")
          .value_name("PATH")
          .value_hint(clap::ValueHint::AnyPath)
          .short('o')
          .long("output")
          .required(true)
          .about(
            "Path to the destination file or directory. A directory is used when a splitter is \
            specified.",
          ),
      )
      .arg(
        clap::Arg::new("input_format")
          .value_name("FORMAT")
          .value_hint(clap::ValueHint::Other)
          .short('f')
          .long("format")
          .possible_values(importers::IMPORTERS_IDS)
          .required(true)
          .about("The format to convert from."),
      )
      .arg(
        clap::Arg::new("output_format")
          .value_name("FORMAT")
          .value_hint(clap::ValueHint::Other)
          .short('F')
          .long("output-format")
          .possible_values(exporters::EXPORTERS_IDS)
          .required(true)
          .about("The format to convert to."),
      )
      .arg(
        clap::Arg::new("default_author")
          .value_name("USERNAME")
          .value_hint(clap::ValueHint::Username)
          .long("default-author")
          .default_value("__convert")
          .about(
            "The default username to add translations with when the real author can't be \
            determined, for example if the input format simply doesn't store such data.",
          ),
      )
      .arg(
        clap::Arg::new("splitter")
          .value_name("SPLITTER")
          .value_hint(clap::ValueHint::Other)
          .long("splitter")
          .possible_values(splitters::SPLITTERS_IDS)
          .about("Strategy used for splitting the output files."),
      )
      .arg(
        clap::Arg::new("remove_untranslated")
          .long("remove-untranslated")
          //
          .about(
            "Whether to remove untranslated fragments when converting. Note that some formats \
            and/or tasks may still need the empty translations.",
          ),
      )
      .arg(
        clap::Arg::new("mapping_output")
          .value_name("PATH")
          .value_hint(clap::ValueHint::FilePath)
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
            "Write output files compactly, for example before packaging them for distribution. \
            Note that this will mean different things depending on the output format.",
          ),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let opt_scan_db = PathBuf::from(matches.value_of_os("scan_db").unwrap());
    let opt_original_locale = matches.value_of("original_locale");
    let opt_inputs: Vec<_> = matches
      .values_of_os("inputs")
      .map_or_else(Vec::new, |values| values.map(PathBuf::from).collect());
    let opt_inputs_file = matches.value_of_os("inputs_file").map(PathBuf::from);
    let opt_output = PathBuf::from(matches.value_of_os("output").unwrap());
    let opt_input_format = RcString::from(matches.value_of("input_format").unwrap());
    let opt_output_format = RcString::from(matches.value_of("output_format").unwrap());
    let opt_default_author = RcString::from(matches.value_of("default_author").unwrap());
    let opt_splitter = matches.value_of("splitter").map(RcString::from);
    let opt_remove_untranslated = matches.is_present("remove_untranslated");
    let opt_mapping_output = matches.value_of_os("mapping_output").map(PathBuf::from);
    let opt_mapping_lm_paths = matches.is_present("mapping_lm_paths");
    let opt_compact = matches.is_present("compact");

    info!("Converting files from {:?} to {:?}", opt_input_format, opt_output_format);

    let mut importer =
      importers::create_by_id(&opt_input_format).context("Failed to create the importer")?;
    let mut exporter =
      exporters::create(&opt_output_format, exporters::ExporterConfig { compact: opt_compact })
        .context("Failed to create the exporter")?;
    #[allow(clippy::manual_map)]
    let mut splitter = match opt_splitter {
      Some(id) => Some(splitters::create_by_id(&id).context("Failed to create the splitter")?),
      _ => None,
    };
    let scan_db = scan::ScanDb::open(opt_scan_db).context("Failed to open the scan database")?;

    let mut total_imported_fragments_count = 0;
    let mut all_imported_fragments =
      IndexMap::<RcString, Vec<(Rc<PathBuf>, ImportedFragment)>>::new();

    let inputs = super::import::collect_input_files(&opt_inputs, &opt_inputs_file, &*importer)?;

    let inputs_len = inputs.len();
    for (i, input_path) in inputs.into_iter().enumerate() {
      trace!("[{}/{}] {:?}", i + 1, inputs_len, input_path);

      let input = fs::read_to_string(&*input_path)
        .with_context(|| format!("Failed to read file {:?}", input_path))?;
      let mut imported_fragments = Vec::new();
      importer
        .import(&input_path, &input, &mut imported_fragments)
        .with_context(|| format!("Failed to import file {:?}", input_path))?;

      for imported_fragment in imported_fragments {
        let fragments_in_import_file = all_imported_fragments
          .entry(imported_fragment.file_path.share_rc())
          .or_insert_with(Vec::new);
        fragments_in_import_file.push((input_path.share_rc(), imported_fragment));
        total_imported_fragments_count += 1;
      }
    }

    info!("Imported {} fragments", total_imported_fragments_count);

    let mut total_converted_fragments_count = 0;
    let mut all_exported_fragments = Vec::<ExportedFragment>::new();
    let mut fragments_by_export_path = IndexMap::<RcString, Vec<ExportedFragment>>::new();
    let mut exported_files_mapping = IndexMap::<RcString, RcString>::new();

    let export_file_extension = exporter.file_extension();

    for (game_file_path, fragments_in_import_file) in all_imported_fragments {
      // Don't stop on not found files just yet, so that we can report an error
      // for each fragment in that game file, as such keep the Option wrapped.
      let scan_game_file: Option<_> = scan_db.get_game_file(&game_file_path);
      let mut fragments_in_export_file: Option<&mut Vec<ExportedFragment>> = None;

      for (input_path, f) in fragments_in_import_file {
        let (scan_game_file, scan_fragment) = if let Some(v) = try {
          let sgf = scan_game_file.as_ref()?;
          (sgf, sgf.get_fragment(&f.json_path)?)
        } {
          v
        } else {
          warn!(
            "Import {:?}:\n\
            fragment {:?} {:?}: not found in the scan database",
            input_path, f.file_path, f.json_path,
          );
          continue;
        };

        if opt_remove_untranslated && f.translations.is_empty() {
          continue;
        }

        if let Some(real_original_text) = try { scan_fragment.text().get(opt_original_locale?)? } {
          if *real_original_text != f.original_text {
            warn!(
              "Import {:?}:\n\
              fragment {:?} {:?}: stale original text, translation are likely outdated",
              input_path, f.file_path, f.json_path,
            );
          }
        }

        if fragments_in_export_file.is_none() {
          fragments_in_export_file = Some(if let Some(splitter) = &mut splitter {
            let export_file_path: MaybeStaticStr = if let Some(path) = splitter
              .get_tr_file_for_entire_game_file(scan_game_file.asset_root(), &game_file_path)
            {
              path
            } else {
              bail!(
                "The selected splitter can't be used for export because it has requested \
                per-fragment splitting on the game file {:?}. An entire game file can be \
                assigned to one and only one export file.",
                game_file_path,
              )
            };

            let export_file_path =
              RcString::from(utils::fast_concat(&[&export_file_path, ".", export_file_extension]));

            let mapping_game_file_path = if opt_mapping_lm_paths {
              RcString::from(localize_me::serialize_file_path(&game_file_path))
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

        let translations: Vec<ExportedTranslation> = f
          .translations
          .into_iter()
          .map(|t| {
            let author = t.author_username.unwrap_or_else(|| opt_default_author.share_rc());
            let editor = t.editor_username.unwrap_or_else(|| author.share_rc());
            ExportedTranslation {
              id: None,
              author_username: Some(author),
              editor_username: Some(editor),
              creation_timestamp: t.creation_timestamp,
              modification_timestamp: t.modification_timestamp,
              text: t.text,
              flags: Some(Rc::new(t.flags)),
            }
          })
          .collect();

        let best_translation =
          translations.iter().max_by_key(|f| f.modification_timestamp).cloned();

        match &mut fragments_in_export_file {
          Some(v) => v,
          None => unreachable!(),
        }
        .push(ExportedFragment {
          id: None,
          file_path: f.file_path,
          json_path: f.json_path,
          lang_uid: Some(scan_fragment.lang_uid()),
          description: Some(scan_fragment.description().share_rc()),
          original_text: f.original_text,
          flags: Some(scan_fragment.flags().share_rc()),
          best_translation,
          translations,
        });
        total_converted_fragments_count += 1;
      }
    }

    let exported_meta = exporters::ExportedProjectMeta {
      id: None,
      creation_timestamp: None,
      modification_timestamp: None,
      game_version: Some(scan_db.meta().game_version.share_rc()),
      original_locale: None,
      reference_locales: None,
      translation_locale: None,
    };
    let mut export_fragments_to_file =
      |path: &Path, fragments: &[ExportedFragment]| -> AnyResult<()> {
        let mut writer = io::BufWriter::new(
          fs::File::create(&path)
            .with_context(|| format!("Failed to open file {:?} for writing", path))?,
        );
        exporter.export(&exported_meta, fragments, &mut writer)?;
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
        "Converted {} fragments to {} files",
        total_converted_fragments_count,
        fragments_by_export_path.len(),
      );
    } else {
      export_fragments_to_file(&opt_output, &all_exported_fragments)
        .with_context(|| format!("Failed to export all fragments to file {:?}", opt_output))?;
      info!("Converted {} fragments", total_converted_fragments_count);
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

    info!("Done!");

    Ok(())
  }
}
