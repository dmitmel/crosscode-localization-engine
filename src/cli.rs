use crate::impl_prelude::*;
use crate::project::splitting_strategies;

use clap::{App, AppSettings, Arg};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct CommonOpts {
  pub verbose: bool,
}

#[derive(Debug)]
pub struct Opts {
  pub common_opts: CommonOpts,
  pub command_opts: CommandOpts,
}

#[derive(Debug)]
pub enum CommandOpts {
  // Individual command options structs are boxed to prevent wasting memory on
  // small variants because their sizes vary a lot.
  Scan(Box<ScanCommandOpts>),
  CreateProject(Box<CreateProjectCommandOpts>),
  ParsePo(Box<ParsePoCommandOpts>),
}

#[derive(Debug)]
pub struct ScanCommandOpts {
  pub assets_dir: PathBuf,
  pub output: PathBuf,
}

#[derive(Debug)]
pub struct CreateProjectCommandOpts {
  pub project_dir: PathBuf,
  pub scan_db: PathBuf,
  pub original_locale: String,
  pub reference_locales: Vec<String>,
  pub translation_locale: String,
  pub splitting_strategy: String,
  pub translations_dir: String,
}

#[derive(Debug)]
pub struct ParsePoCommandOpts {
  pub file: Option<PathBuf>,
  pub json: bool,
}

pub fn parse_opts() -> AnyResult<Opts> {
  let matches = create_arg_parser().get_matches();
  Ok(Opts {
    common_opts: CommonOpts { verbose: matches.is_present("verbose") },
    command_opts: match matches.subcommand() {
      ("scan", Some(matches)) => {
        //
        CommandOpts::Scan(Box::new(ScanCommandOpts {
          assets_dir: PathBuf::from(matches.value_of_os("assets_dir").unwrap()),
          output: PathBuf::from(matches.value_of_os("output").unwrap()),
        }))
      }

      ("create-project", Some(matches)) => {
        CommandOpts::CreateProject(Box::new(CreateProjectCommandOpts {
          project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()),
          scan_db: PathBuf::from(matches.value_of_os("scan_db").unwrap()),
          original_locale: matches.value_of("original_locale").unwrap().to_owned(),
          reference_locales: matches
            .values_of("reference_locales")
            .map(|values| values.map(ToOwned::to_owned).collect())
            .unwrap_or_else(Vec::new),
          translation_locale: matches.value_of("translation_locale").unwrap().to_owned(),
          splitting_strategy: matches.value_of("splitting_strategy").unwrap().to_owned(),
          translations_dir: matches.value_of("translations_dir").unwrap().to_owned(),
        }))
      }

      ("parse-po", Some(matches)) => {
        //
        CommandOpts::ParsePo(Box::new(ParsePoCommandOpts {
          file: matches.value_of("file").map(PathBuf::from),
          json: matches.is_present("json"),
        }))
      }

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
        .help("Print more logs, may help with troubleshooting")
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
            .value_name("PATH")
            .required(true)
            .help("Path to the assets directory"),
        )
        .arg(
          Arg::with_name("output")
            .value_name("PATH")
            .short("o")
            .long("output")
            .required(true)
            .help("Path to the output JSON file"),
        ),
    )
    .subcommand(
      App::new("create-project")
        .about(
          "Creates an empty translation project using the data obtained by scanning the game.",
        )
        .arg(
          Arg::with_name("project_dir")
            .value_name("PATH")
            .required(true)
            .help("Path to the project directory"),
        )
        .arg(
          Arg::with_name("scan_db")
            .value_name("PATH")
            .long("scan-db")
            .required(true)
            .help("Path to the scan database"),
        )
        .arg(
          Arg::with_name("original_locale")
            .value_name("LOCALE")
            .long("original-locale")
            .default_value("en_US")
            .help("Locale to translate from"),
        )
        .arg(
          Arg::with_name("reference_locales")
            .value_name("LOCALE")
            .multiple(true)
            .number_of_values(1)
            .long("reference-locales")
            .help("Other original locales to include for reference"),
        )
        .arg(
          Arg::with_name("translation_locale")
            .value_name("LOCALE")
            .long("translation-locale")
            .required(true)
            .help("Locale of the translation"),
        )
        .arg(
          Arg::with_name("splitting_strategy")
            .value_name("NAME")
            .long("splitting-strategy")
            .possible_values(splitting_strategies::SPLITTING_STRATEGIES_IDS)
            .default_value(splitting_strategies::NextGenerationStrategy::ID)
            .help(
              "Strategy used for assigning game files (and individual fragments in them) to \
              translation storage files",
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
            .help("Path to project's translation storage files, relative to project's directory"),
        ),
    )
    .subcommand(
      App::new("parse-po")
        .arg(Arg::with_name("file").value_name("FILE"))
        .arg(Arg::with_name("json").short("J").long("json")),
    )
}
