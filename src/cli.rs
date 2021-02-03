use crate::impl_prelude::*;
use crate::project::exporters;
use crate::project::splitting_strategies;
use crate::rc_string::RcString;

use clap::{App, AppSettings, Arg};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CommonOpts {
  pub verbose: bool,
}

#[derive(Debug, Clone)]
pub struct Opts {
  pub common_opts: CommonOpts,
  pub command_opts: CommandOpts,
}

#[derive(Debug, Clone)]
pub enum CommandOpts {
  // Individual command options structs are boxed to prevent wasting memory on
  // small variants because their sizes vary a lot.
  Scan(Box<ScanCommandOpts>),
  CreateProject(Box<CreateProjectCommandOpts>),
  ParsePo(Box<ParsePoCommandOpts>),
  Export(Box<ExportCommandOpts>),
}

#[derive(Debug, Clone)]
pub struct ScanCommandOpts {
  pub assets_dir: PathBuf,
  pub output: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CreateProjectCommandOpts {
  pub project_dir: PathBuf,
  pub scan_db: PathBuf,
  pub original_locale: RcString,
  pub reference_locales: Vec<RcString>,
  pub translation_locale: RcString,
  pub splitting_strategy: RcString,
  pub translations_dir: RcString,
}

#[derive(Debug, Clone)]
pub struct ParsePoCommandOpts {
  pub file: Option<PathBuf>,
  pub json: bool,
}

#[derive(Debug, Clone)]
pub struct ExportCommandOpts {
  pub project_dir: PathBuf,
  pub output: PathBuf,
  pub format: RcString,
  pub splitting_strategy: Option<RcString>,
  pub remove_untranslated: bool,
  pub mapping_file_output: Option<PathBuf>,
  pub compact: bool,
}

pub fn parse_opts() -> AnyResult<Opts> {
  let matches = create_arg_parser().get_matches();
  Ok(Opts {
    common_opts: CommonOpts { verbose: matches.is_present("verbose") },

    command_opts: match matches.subcommand() {
      ("scan", Some(matches)) => CommandOpts::Scan(Box::new(ScanCommandOpts {
        assets_dir: PathBuf::from(matches.value_of_os("assets_dir").unwrap()),
        output: PathBuf::from(matches.value_of_os("output").unwrap()),
      })),

      ("create-project", Some(matches)) => {
        CommandOpts::CreateProject(Box::new(CreateProjectCommandOpts {
          project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()),
          scan_db: PathBuf::from(matches.value_of_os("scan_db").unwrap()),
          original_locale: RcString::from(matches.value_of("original_locale").unwrap()),
          reference_locales: matches
            .values_of("reference_locales")
            .map(|values| values.map(RcString::from).collect())
            .unwrap_or_else(Vec::new),
          translation_locale: RcString::from(matches.value_of("translation_locale").unwrap()),
          splitting_strategy: RcString::from(matches.value_of("splitting_strategy").unwrap()),
          translations_dir: RcString::from(matches.value_of("translations_dir").unwrap()),
        }))
      }

      ("parse-po", Some(matches)) => CommandOpts::ParsePo(Box::new(ParsePoCommandOpts {
        file: matches.value_of("file").map(PathBuf::from),
        json: matches.is_present("json"),
      })),

      ("export", Some(matches)) => CommandOpts::Export(Box::new(ExportCommandOpts {
        project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()),
        output: PathBuf::from(matches.value_of_os("output").unwrap()),
        format: RcString::from(matches.value_of("format").unwrap()),
        splitting_strategy: matches.value_of("splitting_strategy").map(RcString::from),
        remove_untranslated: matches.is_present("remove_untranslated"),
        mapping_file_output: matches.value_of_os("mapping_file_output").map(PathBuf::from),
        compact: matches.is_present("compact"),
      })),

      _ => unreachable!("{:#?}", matches),
    },
  })
}

fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
  App::new(crate::CRATE_TITLE)
    .version(crate::CRATE_VERSION)
    .about("CrossCode Localization Engine command-line tool")
    .global_settings(&[
      AppSettings::ColoredHelp,
      AppSettings::VersionlessSubcommands,
      AppSettings::AllowLeadingHyphen,
    ])
    .settings(&[AppSettings::SubcommandRequiredElseHelp])
    .arg(
      Arg::with_name("verbose")
        .short("v")
        .long("verbose")
        .help("Print more logs, may help with troubleshooting.")
        .global(true),
    )
    .subcommand(
      App::new("scan")
        .about(
          "Scans the assets directory of the game and extracts the localizable strings and other \
          interesting data.",
        )
        .arg(
          Arg::with_name("assets_dir")
            .value_name("ASSETS")
            .required(true)
            .help("Path to the assets directory."),
        )
        .arg(
          Arg::with_name("output")
            .value_name("PATH")
            .short("o")
            .long("output")
            .required(true)
            .help("Path to the output JSON file."),
        ),
    )
    .subcommand(
      App::new("create-project")
        .about(
          "Creates an empty translation project using the data obtained by scanning the game.",
        )
        .arg(
          Arg::with_name("project_dir")
            .value_name("PROJECT")
            .required(true)
            .help("Path to the project directory."),
        )
        .arg(
          Arg::with_name("scan_db")
            .value_name("PATH")
            .long("scan-db")
            .required(true)
            .help("Path to the scan database."),
        )
        .arg(
          Arg::with_name("original_locale")
            .value_name("LOCALE")
            .long("original-locale")
            .default_value("en_US")
            .help("Locale to translate from."),
        )
        .arg(
          Arg::with_name("reference_locales")
            .value_name("LOCALE")
            .multiple(true)
            .number_of_values(1)
            .long("reference-locales")
            .help("Other original locales to include for reference."),
        )
        .arg(
          Arg::with_name("translation_locale")
            .value_name("LOCALE")
            .long("translation-locale")
            .required(true)
            .help("Locale of the translation."),
        )
        .arg(
          Arg::with_name("splitting_strategy")
            .value_name("NAME")
            .long("splitting-strategy")
            .possible_values(splitting_strategies::SPLITTING_STRATEGIES_IDS)
            .default_value(splitting_strategies::NextGenerationStrategy::ID)
            .help(
              "Strategy used for assigning game files (and individual fragments in them) to \
              translation storage files.",
            ),
        )
        .arg(
          Arg::with_name("translations_dir")
            .value_name("PATH")
            .long("translations-dir")
            .validator_os(|s| {
              if !Path::new(s).is_relative() {
                return Err(OsString::from("Path must be relative"));
              }
              Ok(())
            })
            .default_value("tr")
            .help("Path to project's translation storage files, relative to project's directory."),
        ),
    )
    .subcommand(
      App::new("parse-po")
        .arg(Arg::with_name("file").value_name("FILE"))
        .arg(Arg::with_name("json").short("J").long("json")),
    )
    .subcommand(
      App::new("export")
        .about(
          "Exports a project into a different format, for example for compiling translations \
          into Localize Me translation packs for use in CrossCode mods.",
        )
        .arg(
          Arg::with_name("project_dir")
            .value_name("PROJECT")
            .required(true)
            .help("Path to the project directory."),
        )
        .arg(
          Arg::with_name("output")
            .value_name("PATH")
            .short("o")
            .long("output")
            .required(true)
            .help(
              "Path to the destination file or directory for exporting. A directory is used when \
              a splitting strategy is specified.",
            ),
        )
        .arg(
          Arg::with_name("format")
            .value_name("NAME")
            .short("f")
            .long("format")
            .possible_values(exporters::EXPORTERS_IDS)
            .required(true)
            .help("Format to export to."),
        )
        .arg(
          Arg::with_name("splitting_strategy")
            .value_name("NAME")
            .long("splitting-strategy")
            .possible_values(splitting_strategies::SPLITTING_STRATEGIES_IDS)
            .help("Strategy used for splitting exported files."),
        )
        .arg(
          Arg::with_name("remove_untranslated")
            .long("remove-untranslated")
            //
            .help(
              "Whether to remove untranslated strings from the exported files. Note that some \
              formats and/or tasks may still need the empty translations.",
            ),
        )
        .arg(
          Arg::with_name("mapping_file_output")
            .value_name("PATH")
            .long("mapping-file-output")
            .help("File to write a Localize Me-style mapping table to."),
        )
        .arg(
          Arg::with_name("compact")
            .long("compact")
            //
            .help(
              "Write exported files compactly, for example before packaging them for \
              distribution. Note that this will mean different things depending on the output \
              format.",
            ),
        ),
    )
}
