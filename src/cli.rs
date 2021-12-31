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

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct GlobalOpts {
  pub verbose: bool,
  pub progress_mode: ProgressMode,
  pub cd: Option<PathBuf>,
  pub no_banner_message: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ProgressMode {
  Auto,
  Always,
  Never,
}

impl GlobalOpts {
  pub fn create_arg_parser<'help>() -> clap::App<'help> {
    clap::App::new(crate::CRATE_TITLE)
      .version(crate::CRATE_NICE_VERSION)
      .about("CrossCode Localization Engine command-line tool")
      .global_setting(clap::AppSettings::AllowHyphenValues)
      .global_setting(clap::AppSettings::NextLineHelp)
      .setting(clap::AppSettings::SubcommandRequiredElseHelp)
      .arg(
        clap::Arg::new("verbose")
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
          .possible_values(&["auto", "always", "never"])
          .default_value("auto")
          .global(true),
      )
      .arg(
        clap::Arg::new("cd")
          .value_name("DIR")
          .value_hint(clap::ValueHint::DirPath)
          .setting(clap::ArgSettings::AllowInvalidUtf8)
          .short('C')
          .long("cd")
          .help("Change the working directory first before doing anything.")
          .global(true),
      )
      .arg(
        clap::Arg::new("no_banner_message")
          .long("no-banner-message")
          .help("Don't print the banner message with the program information when starting.")
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
      cd: matches.value_of_os("cd").map(PathBuf::from),
      no_banner_message: matches.is_present("no_banner_message"),
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

inventory::collect!(&'static dyn Command);

pub fn create_complete_arg_parser<'help>(
) -> (clap::App<'help>, HashMap<&'static str, &'static dyn Command>) {
  let mut arg_parser = GlobalOpts::create_arg_parser();
  let mut all_commands_map = HashMap::new();
  for &command in inventory::iter::<&dyn Command> {
    arg_parser = arg_parser.subcommand(command.create_arg_parser(clap::App::new(command.name())));
    all_commands_map.insert(command.name(), command);
  }
  (arg_parser, all_commands_map)
}
