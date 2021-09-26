pub mod backend;
pub mod completions;
pub mod convert;
pub mod create_project;
pub mod dump_common;
pub mod dump_project;
pub mod dump_scan;
pub mod export;
pub mod import;
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
  pub mmap_preference: MmapPreference,
}

#[derive(Debug, Clone, Copy)]
pub enum ProgressMode {
  Auto,
  Always,
  Never,
}

#[derive(Debug, Clone, Copy)]
pub enum MmapPreference {
  Auto,
  Never,
}

impl GlobalOpts {
  pub fn create_arg_parser<'help>() -> clap::App<'help> {
    clap::App::new(crate::CRATE_TITLE)
      .version(crate::CRATE_NICE_VERSION)
      .about("CrossCode Localization Engine command-line tool")
      .global_setting(clap::AppSettings::ColoredHelp)
      .global_setting(clap::AppSettings::DisableVersionForSubcommands)
      .global_setting(clap::AppSettings::AllowLeadingHyphen)
      .global_setting(clap::AppSettings::AllowInvalidUtf8) // just in case, for file paths
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
          .value_name("MODE")
          .value_hint(clap::ValueHint::Other)
          .short('p')
          .long("progress")
          .about("Enable the fancy progress bars.")
          .possible_values(&["auto", "always", "never"])
          .default_value("auto")
          .global(true),
      )
      .arg(
        clap::Arg::new("cd")
          .value_name("DIR")
          .value_hint(clap::ValueHint::DirPath)
          .short('C')
          .long("cd")
          .about("Change the working directory first before doing anything.")
          .global(true),
      )
      .arg(
        clap::Arg::new("mmap_preference")
          .value_name("MODE")
          .value_hint(clap::ValueHint::Other)
          .long("mmap")
          .about(
            "Whether to allow usage memory-mapping for reading files which may (???) result in \
            faster performance. Disabled by default though.",
          )
          .possible_values(&["auto", "never"])
          .default_value("never")
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
      mmap_preference: match matches.value_of("mmap_preference").unwrap() {
        "auto" => MmapPreference::Auto,
        "never" => MmapPreference::Never,
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
