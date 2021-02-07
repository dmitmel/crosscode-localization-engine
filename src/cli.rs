pub mod backend;
pub mod create_project;
pub mod export;
pub mod import;
pub mod parse_po;
pub mod scan;
pub mod status;

use crate::impl_prelude::*;

#[derive(Debug)]
pub struct GlobalOpts {
  pub verbose: bool,
}

impl GlobalOpts {
  pub fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
    clap::App::new(crate::CRATE_TITLE)
      .version(crate::CRATE_VERSION)
      .about("CrossCode Localization Engine command-line tool")
      .global_settings(&[
        clap::AppSettings::ColoredHelp,
        clap::AppSettings::VersionlessSubcommands,
        clap::AppSettings::AllowLeadingHyphen,
      ])
      .settings(&[clap::AppSettings::SubcommandRequiredElseHelp])
      .arg(
        clap::Arg::with_name("verbose")
          .short("v")
          .long("verbose")
          .help("Print more logs, may be helpful for troubleshooting.")
          .global(true),
      )
  }

  pub fn from_matches(matches: &clap::ArgMatches<'_>) -> Self {
    Self { verbose: matches.is_present("verbose") }
  }
}

assert_trait_is_object_safe!(Command);
pub trait Command {
  fn name(&self) -> &'static str;
  fn create_arg_parser<'a, 'b>(&self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b>;
  fn run(&self, global_opts: GlobalOpts, matches: &clap::ArgMatches<'_>) -> AnyResult<()>;
}

pub fn all_commands() -> Vec<Box<dyn Command>> {
  vec![
    Box::new(backend::BackendCommand),
    Box::new(create_project::CreateProjectCommand),
    Box::new(export::ExportCommand),
    Box::new(import::ImportCommand),
    Box::new(parse_po::ParsePoCommand),
    Box::new(scan::ScanCommand),
    Box::new(status::StatusCommand),
  ]
}
