use crate::impl_prelude::*;
use crate::project::importers;
use crate::project::{self, Project, Translation};
use crate::rc_string::RcString;
use crate::utils;

use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug)]
pub struct ImportCommand;

impl super::Command for ImportCommand {
  fn name(&self) -> &'static str { "import" }

  fn create_arg_parser<'a, 'b>(&self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b> {
    app
      .about(
        "Imports translations from a different format into a project, for example for migrating \
        projects created with the old translation tools.",
      )
      .arg(
        clap::Arg::with_name("project_dir")
          .value_name("PROJECT")
          .required(true)
          .help("Path to the project directory."),
      )
      .arg(
        clap::Arg::with_name("inputs")
          .value_name("IMPORT_PATH")
          .multiple(true)
          .required(true)
          .conflicts_with("inputs_file")
          .help("Path to files to import translations from."),
      )
      .arg(clap::Arg::with_name("inputs_file").value_name("PATH").short("i").help(
        "Read paths to input files from a file. If there are other paths specified via \
            command-line arguments, then those will be used instead and the inputs file will be \
            ignored.",
      ))
      .arg(
        clap::Arg::with_name("format")
          .value_name("NAME")
          .short("f")
          .long("format")
          .possible_values(importers::IMPORTERS_IDS)
          .required(true)
          .help("Format to import from."),
      )
      .arg(
        clap::Arg::with_name("default_author")
          .value_name("USERNAME")
          .long("default-author")
          .default_value("__import")
          .help(
            "The default username to add translations with when the real author can't be \
            determined, for example if the input format simply doesn't store such data.",
          ),
      )
      .arg(
        clap::Arg::with_name("marker_flag")
          .value_name("FLAG")
          .long("marker-flag")
          .default_value("imported")
          .help("Name of the flag used for marking automatically imported translations."),
      )
      .arg(
        clap::Arg::with_name("delete_other_translations")
          .long("delete-other-translations")
          //
          .help(
            "Delete other translations (by other users) on fragments before adding the imported \
            translation.",
          ),
      )
      .arg(
        clap::Arg::with_name("always_add_new_translations")
          .long("always-add-new-translations")
          .help(
            "Always add new translations instead of editing the translations created from \
            previous imports. The import marker flag is used for determining if a translation \
            was imported.",
          ),
      )
      .arg(
        clap::Arg::with_name("add_flags")
          .value_name("FLAG")
          .long("add-flag")
          .multiple(true)
          .number_of_values(1)
          .help("Add flags to the imported translations."),
      )
  }

  fn run(&self, _global_opts: super::GlobalOpts, matches: &clap::ArgMatches<'_>) -> AnyResult<()> {
    let opt_project_dir = PathBuf::from(matches.value_of_os("project_dir").unwrap());
    let mut opt_inputs: Vec<_> = matches
      .values_of_os("inputs")
      .map_or_else(Vec::new, |values| values.map(PathBuf::from).collect());
    let opt_inputs_file = matches.value_of_os("inputs_file").map(PathBuf::from);
    let opt_format = RcString::from(matches.value_of("format").unwrap());
    let opt_default_author = RcString::from(matches.value_of("default_author").unwrap());
    let opt_marker_flag = RcString::from(matches.value_of("marker_flag").unwrap());
    let opt_delete_other_translations = matches.is_present("delete_other_translations");
    let opt_always_add_new_translations = matches.is_present("always_add_new_translations");
    let opt_add_flags: HashSet<_> = matches
      .values_of("add_flags")
      .map_or_else(HashSet::new, |values| values.map(RcString::from).collect());

    info!(
      "Importing into a translation project in {:?} from {:?}",
      opt_project_dir.display(),
      opt_format,
    );

    let project = Project::open(opt_project_dir).context("Failed to open the project")?;
    let mut importer =
      importers::create_by_id(&opt_format).context("Failed to create the importer")?;
    let mut total_imported_fragments_count = 0;

    let default_author = opt_default_author;
    let marker_flag = opt_marker_flag;

    if opt_inputs.is_empty() {
      if let Some(opt_inputs_file) = opt_inputs_file {
        try_any_result!({
          let reader = io::BufReader::new(fs::File::open(&opt_inputs_file)?);
          for line in reader.lines() {
            opt_inputs.push(PathBuf::from(line?));
          }
        })
        .with_context(|| format_err!("Failed to read inputs from file {:?}", opt_inputs_file))?;
      }
    }

    let imported_file_ext = OsStr::new(importer.file_extension());
    let mut inputs: Vec<PathBuf> = Vec::new();
    for input_path in opt_inputs {
      for entry in walkdir::WalkDir::new(&input_path).into_iter() {
        let entry =
          entry.with_context(|| format!("Failed to list all files in dir {:?}", input_path))?;
        if entry.file_type().is_file() && entry.path().extension() == Some(imported_file_ext) {
          inputs.push(entry.path().to_owned());
        }
      }
    }

    let inputs_len = inputs.len();
    for (i, input_path) in inputs.into_iter().enumerate() {
      trace!("[{}/{}] {:?}", i + 1, inputs_len, input_path);

      let input = fs::read_to_string(&input_path)
        .with_context(|| format!("Failed to read file {:?}", input_path))?;
      let mut imported_fragments = Vec::new();
      importer
        .import(&input_path, &input, &mut imported_fragments)
        .with_context(|| format!("Failed to import file {:?}", input_path))?;

      for imported_fragment in imported_fragments {
        let fragment = if let Some(v) = project
          .get_virtual_game_file(&imported_fragment.file_path)
          .and_then(|virt_file| virt_file.get_fragment(&imported_fragment.json_path))
        {
          v
        } else {
          warn!(
            "Import {:?}:\n\
          fragment {:?} {:?}: not found in the project",
            input_path, imported_fragment.file_path, imported_fragment.json_path,
          );
          continue;
        };

        if *fragment.original_text() != imported_fragment.original_text {
          warn!(
            "Import {:?}:\n\
          fragment {:?} {:?}: stale original text, translation are likely outdated",
            input_path, imported_fragment.file_path, imported_fragment.json_path,
          );
        }

        if opt_delete_other_translations {
          fragment.clear_translations();
        }

        if opt_always_add_new_translations {
          fragment.reserve_additional_translations(imported_fragment.translations.len());
        }

        let mut remaining_existing_translations: Option<Vec<Rc<Translation>>> = None;
        for imported_translation in imported_fragment.translations {
          let imported_translation_author =
            imported_translation.author_username.unwrap_or_else(|| default_author.share_rc());

          let existing_translation = if !opt_always_add_new_translations {
            let remaining_existing_translations = remaining_existing_translations
              .get_or_insert_with(|| fragment.translations().to_owned());

            remaining_existing_translations
              .iter()
              .position(|tr| {
                tr.has_flag(&marker_flag) && *tr.author_username() == imported_translation_author
              })
              .map(|existing_translation_i: usize| -> Rc<Translation> {
                remaining_existing_translations.remove(existing_translation_i)
              })
          } else {
            None
          };

          let timestamp = utils::get_timestamp();

          if let Some(existing_translation) = existing_translation {
            existing_translation.set_modification_timestamp(
              imported_translation.modification_timestamp.unwrap_or(timestamp),
            );
            existing_translation.set_text(imported_translation.text);
            for flag in imported_translation.flags {
              existing_translation.add_flag(flag);
            }
            for flag in &opt_add_flags {
              existing_translation.add_flag(flag.share_rc());
            }
          } else {
            let mut flags =
              HashSet::with_capacity(1 + imported_translation.flags.len() + opt_add_flags.len());
            flags.insert(marker_flag.share_rc());
            flags.extend(imported_translation.flags.into_iter());
            flags.extend(opt_add_flags.iter().cloned());

            fragment.new_translation(project::TranslationInitOpts {
              id: utils::new_uuid(),
              author_username: imported_translation_author.share_rc(),
              editor_username: imported_translation_author,
              creation_timestamp: timestamp,
              modification_timestamp: imported_translation
                .modification_timestamp
                .unwrap_or(timestamp),
              text: imported_translation.text,
              flags,
            });
          }
        }

        total_imported_fragments_count += 1;
      }
    }

    info!("Imported {} fragments", total_imported_fragments_count);

    info!("Writing the project");
    project.write().context("Failed to write the project")?;
    info!("Done!");

    Ok(())
  }
}
