use crate::impl_prelude::*;
use crate::project::importers;
use crate::project::{self, Project, Translation};
use crate::rc_string::RcString;
use crate::utils;

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

pub const NAME: &str = "import";

#[derive(Debug)]
pub struct CommandOpts {
  pub project_dir: PathBuf,
  pub inputs: Vec<PathBuf>,
  pub format: RcString,
  pub default_author: RcString,
  pub marker_flag: RcString,
  pub delete_other_translations: bool,
  pub always_add_new_translations: bool,
  pub add_flags: HashSet<RcString>,
}

impl CommandOpts {
  pub fn from_matches(matches: &clap::ArgMatches<'_>) -> Self {
    Self {
      project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()),
      inputs: matches.values_of_os("inputs").unwrap().map(PathBuf::from).collect(),
      format: RcString::from(matches.value_of("format").unwrap()),
      default_author: RcString::from(matches.value_of("default_author").unwrap()),
      marker_flag: RcString::from(matches.value_of("marker_flag").unwrap()),
      delete_other_translations: matches.is_present("delete_other_translations"),
      always_add_new_translations: matches.is_present("always_add_new_translations"),
      add_flags: matches
        .values_of("add_flags")
        .map_or_else(HashSet::new, |values| values.map(RcString::from).collect()),
    }
  }
}

pub fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
  clap::App::new(NAME)
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
        .value_name("PATH")
        .multiple(true)
        .required(true)
        .help("Path to files to import translations from."),
    )
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
          "Delete other translations (by other users) on fragments before adding the \
          imported translation.",
        ),
    )
    .arg(
      clap::Arg::with_name("always_add_new_translations")
        .long("always-add-new-translations")
        //
        .help(
          "Always add new translations instead of editing the translations created from previous \
          imports. The import marker flag is used for determining if a translation was imported.",
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

pub fn run(_global_opts: super::GlobalOpts, command_opts: CommandOpts) -> AnyResult<()> {
  info!(
    "Importing into a translation project in {:?} from {:?}",
    command_opts.project_dir.display(),
    command_opts.format,
  );

  let project = Project::open(command_opts.project_dir).context("Failed to open the project")?;
  let mut importer =
    importers::create_by_id(&command_opts.format).context("Failed to create the importer")?;
  let mut total_imported_fragments_count = 0;

  let default_author = command_opts.default_author;
  let marker_flag = command_opts.marker_flag;

  let inputs_len = command_opts.inputs.len();
  for (i, input_path) in command_opts.inputs.into_iter().enumerate() {
    trace!("[{}/{}] {:?}", i + 1, inputs_len, input_path);

    // TODO: handle directories in input_path
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

      if command_opts.delete_other_translations {
        fragment.clear_translations();
      }

      if command_opts.always_add_new_translations {
        fragment.reserve_additional_translations(imported_fragment.translations.len());
      }

      let mut remaining_existing_translations: Option<Vec<Rc<Translation>>> = None;
      for imported_translation in imported_fragment.translations {
        let imported_translation_author =
          imported_translation.author.unwrap_or_else(|| default_author.share_rc());

        let existing_translation = if !command_opts.always_add_new_translations {
          let remaining_existing_translations = remaining_existing_translations
            .get_or_insert_with(|| fragment.translations().to_owned());

          remaining_existing_translations
            .iter()
            .position(|tr| {
              tr.has_flag(&marker_flag) && *tr.author() == imported_translation_author
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
          for flag in &command_opts.add_flags {
            existing_translation.add_flag(flag.share_rc());
          }
        } else {
          let mut flags = HashSet::with_capacity(
            1 + imported_translation.flags.len() + command_opts.add_flags.len(),
          );
          flags.insert(marker_flag.share_rc());
          flags.extend(imported_translation.flags.into_iter());
          flags.extend(command_opts.add_flags.iter().cloned());

          fragment.new_translation(project::TranslationInitOpts {
            uuid: utils::new_uuid(),
            author: imported_translation_author,
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
