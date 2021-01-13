use crate::impl_prelude::*;
use clap::{App, AppSettings, Arg};
use std::path::PathBuf;

#[derive(Debug)]
pub struct CommonOptions {
  pretty_json: bool,
}

#[derive(Debug)]
pub struct Args {
  common: CommonOptions,
  command: CommandArgs,
}

#[derive(Debug)]
pub enum CommandArgs {
  Scan {
    //
    assets_dir: PathBuf,
    output: Option<PathBuf>,
  },
}

pub fn parse_args() -> AnyResult<Args> {
  let matches = create_arg_parser().get_matches();
  Ok(Args {
    common: CommonOptions {
      //
      pretty_json: matches.is_present("pretty_json"),
    },
    command: match matches.subcommand() {
      ("scan", Some(matches)) => CommandArgs::Scan {
        assets_dir: PathBuf::from(matches.value_of_os("assets_dir").unwrap()),
        output: matches.value_of_os("output").map(PathBuf::from),
      },
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
