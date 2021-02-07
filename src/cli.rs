pub mod backend;
pub mod create_project;
pub mod export;
pub mod import;
pub mod parse_po;
pub mod scan;
pub mod status;

use crate::impl_prelude::*;

use clap::{App, AppSettings, Arg};

#[derive(Debug)]
pub struct GlobalOpts {
  pub verbose: bool,
}

#[derive(Debug)]
pub enum CommandOpts {
  // Individual command options structs are boxed to prevent wasting memory on
  // small variants because their sizes vary a lot.
  Backend(Box<backend::CommandOpts>),
  CreateProject(Box<create_project::CommandOpts>),
  Export(Box<export::CommandOpts>),
  Import(Box<import::CommandOpts>),
  ParsePo(Box<parse_po::CommandOpts>),
  Scan(Box<scan::CommandOpts>),
  Status(Box<status::CommandOpts>),
}

pub fn parse_opts() -> AnyResult<(GlobalOpts, CommandOpts)> {
  let matches = create_arg_parser().get_matches();
  let global_opts = GlobalOpts { verbose: matches.is_present("verbose") };

  let command_opts = match matches.subcommand() {
    (backend::NAME, Some(matches)) => {
      CommandOpts::Backend(Box::new(backend::CommandOpts::from_matches(matches)))
    }

    (create_project::NAME, Some(matches)) => {
      CommandOpts::CreateProject(Box::new(create_project::CommandOpts::from_matches(matches)))
    }

    (export::NAME, Some(matches)) => {
      CommandOpts::Export(Box::new(export::CommandOpts::from_matches(matches)))
    }

    (import::NAME, Some(matches)) => {
      CommandOpts::Import(Box::new(import::CommandOpts::from_matches(matches)))
    }

    (parse_po::NAME, Some(matches)) => {
      CommandOpts::ParsePo(Box::new(parse_po::CommandOpts::from_matches(matches)))
    }

    (scan::NAME, Some(matches)) => {
      CommandOpts::Scan(Box::new(scan::CommandOpts::from_matches(matches)))
    }

    (status::NAME, Some(matches)) => {
      CommandOpts::Status(Box::new(status::CommandOpts::from_matches(matches)))
    }

    _ => unreachable!("{:#?}", matches),
  };

  Ok((global_opts, command_opts))
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
        .help("Print more logs, may be helpful for troubleshooting.")
        .global(true),
    )
    .subcommand(backend::create_arg_parser())
    .subcommand(create_project::create_arg_parser())
    .subcommand(export::create_arg_parser())
    .subcommand(import::create_arg_parser())
    .subcommand(parse_po::create_arg_parser())
    .subcommand(scan::create_arg_parser())
    .subcommand(status::create_arg_parser())
}
