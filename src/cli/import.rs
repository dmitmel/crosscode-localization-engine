use crate::project::importers;
use crate::rc_string::RcString;

use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CommandOpts {
  pub project_dir: PathBuf,
  pub inputs: Vec<PathBuf>,
  pub format: RcString,
  pub default_username: RcString,
  pub marker_flag: RcString,
  pub delete_other_translations: bool,
  pub edit_prev_imports: bool,
  pub add_flags: Vec<RcString>,
}

impl CommandOpts {
  pub fn from_matches(matches: &clap::ArgMatches<'_>) -> Self {
    Self {
      project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()),
      inputs: matches.values_of_os("inputs").unwrap().map(PathBuf::from).collect(),
      format: RcString::from(matches.value_of("format").unwrap()),
      default_username: RcString::from(matches.value_of("default_username").unwrap()),
      marker_flag: RcString::from(matches.value_of("marker_flag").unwrap()),
      delete_other_translations: matches.is_present("delete_other_translations"),
      edit_prev_imports: matches.is_present("edit_prev_imports"),
      add_flags: matches
        .values_of("add_flags")
        .map_or_else(Vec::new, |values| values.map(RcString::from).collect()),
    }
  }
}

pub fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
  clap::App::new("import")
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
      clap::Arg::with_name("default_username")
        .value_name("USERNAME")
        .long("default-username")
        .default_value("__import")
        .help(
          "The default username to add translations with when the real author can't be determined,
          for example if the input format simply doesn't store such data.",
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
      clap::Arg::with_name("edit_prev_imports")
        .long("edit-prev-imports")
        //
        .help(
          "Edit the translations created from previous imports instead of creating new ones. The \
          import marker flag is used for determining if a translation was imported.",
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
