pub mod backend;
pub mod completions;
pub mod convert;
pub mod create_project;
pub mod dump_scan;
pub mod export;
pub mod import;
pub mod parse_po;
pub mod scan;
pub mod status;

use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use std::collections::HashMap;

#[derive(Debug)]
pub struct GlobalOpts {
  pub verbose: bool,
  pub progress_mode: ProgressMode,
}

#[derive(Debug)]
pub enum ProgressMode {
  Auto,
  Always,
  Never,
}

impl GlobalOpts {
  pub fn create_arg_parser<'help>() -> clap::App<'help> {
    clap::App::new(crate::CRATE_TITLE)
      .version(crate::CRATE_VERSION)
      .about("CrossCode Localization Engine command-line tool")
      .global_setting(clap::AppSettings::ColoredHelp)
      .global_setting(clap::AppSettings::VersionlessSubcommands)
      .global_setting(clap::AppSettings::AllowLeadingHyphen)
      .setting(clap::AppSettings::SubcommandRequiredElseHelp)
      .arg(
        clap::Arg::new("verbose")
          .short('v')
          .long("verbose")
          .about("Print more logs, may be helpful for troubleshooting.")
          .global(true),
      )
      .arg(
        clap::Arg::new("progress_mode")
          .short('p')
          .long("progress")
          .about("Enable the fancy progress bars.")
          .possible_values(&["auto", "always", "never"])
          .default_value("auto")
          .global(true),
      )
  }

  pub fn from_matches(matches: &clap::ArgMatches) -> Self {
    Self {
      verbose: matches.is_present("verbose"),
      progress_mode: match matches.value_of("progress_mode").unwrap() {
        "auto" => ProgressMode::Auto,
        "always" => ProgressMode::Always,
        "never" => ProgressMode::Never,
        _ => unreachable!(),
      },
    }
  }
}

assert_trait_is_object_safe!(Command);
pub trait Command {
  fn name(&self) -> &'static str;
  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help>;
  fn run(
    &self,
    global_opts: GlobalOpts,
    matches: &clap::ArgMatches,
    progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()>;
}

pub fn all_commands() -> Vec<Box<dyn Command>> {
  vec![
    Box::new(backend::BackendCommand),
    Box::new(completions::CompletionsCommand),
    Box::new(convert::ConvertCommand),
    Box::new(create_project::CreateProjectCommand),
    Box::new(dump_scan::DumpScanCommand),
    Box::new(export::ExportCommand),
    Box::new(import::ImportCommand),
    Box::new(parse_po::ParsePoCommand),
    Box::new(scan::ScanCommand),
    Box::new(status::StatusCommand),
  ]
}

pub fn create_complete_arg_parser<'help>(
) -> (clap::App<'help>, HashMap<&'static str, Box<dyn Command>>) {
  let mut arg_parser = GlobalOpts::create_arg_parser();
  let all_commands: Vec<Box<dyn Command>> = all_commands();
  let mut all_commands_map = HashMap::with_capacity(all_commands.len());
  for command in all_commands {
    arg_parser = arg_parser.subcommand(command.create_arg_parser(clap::App::new(command.name())));
    all_commands_map.insert(command.name(), command);
  }
  (arg_parser, all_commands_map)
}
