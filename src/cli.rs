pub mod backend;
pub mod completions;
pub mod convert;
pub mod create_project;
pub mod dump_common;
pub mod dump_project;
pub mod dump_scan;
pub mod export;
pub mod import;
pub mod mass_json_format;
pub mod parse_po;
pub mod scan;
pub mod status;

use crate::impl_prelude::*;
use crate::progress::ProgressReporter;

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct GlobalOpts<'arg> {
  pub verbose: bool,
  pub progress_mode: ProgressMode,
  pub cd: Option<&'arg Path>,
  pub no_banner_message: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ProgressMode {
  Auto,
  Always,
  Never,
}

impl clap::ValueEnum for ProgressMode {
  fn value_variants<'a>() -> &'a [Self] { &[Self::Auto, Self::Always, Self::Never] }

  fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
    Some(clap::builder::PossibleValue::new(match self {
      Self::Auto => "auto",
      Self::Always => "always",
      Self::Never => "never",
    }))
  }
}

impl<'arg> GlobalOpts<'arg> {
  pub fn create_arg_parser() -> clap::Command {
    clap::Command::new(crate::CRATE_TITLE)
      .version(crate::CRATE_NICE_VERSION)
      .about("CrossCode Localization Engine command-line tool")
      .next_line_help(true)
      .subcommand_required(true)
      .arg_required_else_help(true)
      .arg(
        clap::Arg::new("verbose")
          .action(clap::ArgAction::SetTrue)
          .short('v')
          .long("verbose")
          .help("Print more logs, may be helpful for troubleshooting.")
          .global(true),
      )
      .arg(
        clap::Arg::new("progress_mode")
          .value_name("MODE")
          .value_hint(clap::ValueHint::Other)
          .short('p')
          .long("progress")
          .help("Enable the fancy progress bars.")
          .value_parser(["auto", "always", "never"])
          .default_value("auto")
          .global(true),
      )
      .arg(
        clap::Arg::new("cd")
          .value_name("DIR")
          .value_hint(clap::ValueHint::DirPath)
          .value_parser(clap::value_parser!(PathBuf))
          .short('C')
          .long("cd")
          .help("Change the working directory first before doing anything.")
          .global(true),
      )
      .arg(
        clap::Arg::new("no_banner_message")
          .action(clap::ArgAction::SetTrue)
          .long("no-banner-message")
          .help("Don't print the banner message with the program information when starting.")
          .global(true),
      )
  }

  pub fn from_matches(matches: &'arg clap::ArgMatches) -> Self {
    Self {
      verbose: matches.get_flag("verbose"),
      progress_mode: match matches.get_one::<String>("progress_mode").unwrap().as_str() {
        "auto" => ProgressMode::Auto,
        "always" => ProgressMode::Always,
        "never" => ProgressMode::Never,
        _ => unreachable!(),
      },
      cd: matches.get_one::<PathBuf>("cd").map(|p| p.as_path()),
      no_banner_message: matches.get_flag("no_banner_message"),
    }
  }
}

assert_trait_is_object_safe!(Command);
pub trait Command: Send + Sync {
  fn name(&self) -> &'static str;
  fn create_arg_parser(&self, app: clap::Command) -> clap::Command;
  fn run(
    &self,
    global_opts: GlobalOpts,
    matches: &clap::ArgMatches,
    progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()>;
}

pub static ALL_COMMANDS: Lazy<Vec<&'static dyn Command>> = Lazy::new(|| {
  vec![
    &backend::BackendCommand,
    &completions::CompletionsCommand,
    &convert::ConvertCommand,
    &create_project::CreateProjectCommand,
    &dump_project::DumpProjectCommand,
    &dump_scan::DumpScanCommand,
    &export::ExportCommand,
    &import::ImportCommand,
    &mass_json_format::MassJsonFormatCommand,
    &parse_po::ParsePoCommand,
    &scan::ScanCommand,
    &status::StatusCommand,
  ]
});

pub fn create_complete_arg_parser() -> (clap::Command, HashMap<&'static str, &'static dyn Command>)
{
  let mut arg_parser = GlobalOpts::create_arg_parser();
  let mut all_commands_map = HashMap::new();
  for command in ALL_COMMANDS.iter() {
    arg_parser =
      arg_parser.subcommand(command.create_arg_parser(clap::Command::new(command.name())));
    all_commands_map.insert(command.name(), &**command);
  }
  (arg_parser, all_commands_map)
}
