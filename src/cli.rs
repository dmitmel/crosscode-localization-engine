use crate::impl_prelude::*;
use crate::project::splitting_strategies;

use clap::{App, AppSettings, Arg};
use lazy_static::lazy_static;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

#[derive(Debug)]
pub struct CommonOpts {
  pub verbose: bool,
  pub pretty_json: bool,
}

#[derive(Debug)]
pub struct Opts {
  pub common_opts: CommonOpts,
  pub command_opts: CommandOpts,
}

#[derive(Debug)]
pub enum CommandOpts {
  Scan(ScanCommandOpts),
  CreateProject(CreateProjectCommandOpts),
}

#[derive(Debug)]
pub struct ScanCommandOpts {
  pub assets_dir: PathBuf,
  pub output: Option<FileOrStdStream>,
}

#[derive(Debug)]
pub struct CreateProjectCommandOpts {
  pub scan_db: FileOrStdStream,
  pub original_locale: String,
  pub reference_locales: Vec<String>,
  pub translation_locale: String,
  pub splitting_strategy: String,
}

#[derive(Debug)]
pub enum FileOrStdStream {
  File(PathBuf),
  StdStream,
}

pub fn parse_opts() -> AnyResult<Opts> {
  let matches = create_arg_parser().get_matches();
  Ok(Opts {
    common_opts: CommonOpts {
      verbose: matches.is_present("verbose"),
      pretty_json: matches.is_present("pretty_json"),
    },
    command_opts: match matches.subcommand() {
      ("scan", Some(matches)) => CommandOpts::Scan(ScanCommandOpts {
        assets_dir: PathBuf::from(matches.value_of_os("assets_dir").unwrap()),
        output: matches.value_of_os("output").map(FileOrStdStream::from),
      }),

      ("create-project", Some(matches)) => CommandOpts::CreateProject(CreateProjectCommandOpts {
        scan_db: FileOrStdStream::from(matches.value_of_os("scan_db").unwrap()),
        original_locale: matches.value_of("original_locale").unwrap().to_owned(),
        reference_locales: matches
          .values_of("original_locale")
          .unwrap()
          .map(ToOwned::to_owned)
          .collect(),
        translation_locale: matches.value_of("translation_locale").unwrap().to_owned(),
        splitting_strategy: matches.value_of("splitting_strategy").unwrap().to_owned(),
      }),

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
    .arg(
      Arg::with_name("pretty_json")
        .long("pretty-json")
        .help("Pretty-print the JSON files")
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
            .possible_values(
              &splitting_strategies::STRATEGIES_MAP.keys().copied().collect::<Vec<&'static str>>(),
            )
            .default_value(splitting_strategies::SameFileTreeStrategy::ID)
            .help(
              "Strategy used for assigning game files (and individual fragments in them) to \
              translation storage files",
            ),
        ),
    )
}

lazy_static! {
  static ref STD_STREAM_STR: &'static OsStr = OsStr::new("-");
}

impl<T: ?Sized + AsRef<OsStr>> From<&T> for FileOrStdStream {
  fn from(s: &T) -> Self { Self::from(s.as_ref().to_os_string()) }
}

impl From<OsString> for FileOrStdStream {
  fn from(v: OsString) -> Self {
    if v == *STD_STREAM_STR {
      Self::StdStream
    } else {
      Self::File(PathBuf::from(v))
    }
  }
}
