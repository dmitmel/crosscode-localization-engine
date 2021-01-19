use crate::impl_prelude::*;

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
}

#[derive(Debug)]
pub struct ScanCommandOpts {
  pub assets_dir: PathBuf,
  pub output: Option<FileOrStdStream>,
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
      _ => unreachable!(),
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
          interesting data",
        )
        .arg(
          Arg::with_name("assets_dir")
            .value_name("ASSETS DIR")
            .help("Path to the assets directory")
            .required(true),
        )
        .arg(
          Arg::with_name("output")
            .value_name("PATH")
            .short("o")
            .long("output")
            .help("Path to the output JSON file"),
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
