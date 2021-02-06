use crate::impl_prelude::*;
use crate::project::Project;

use std::path::PathBuf;

pub const NAME: &str = "backend";

#[derive(Debug)]
pub struct CommandOpts {
  pub project_dir: PathBuf,
}

impl CommandOpts {
  pub fn from_matches(matches: &clap::ArgMatches<'_>) -> Self {
    Self { project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()) }
  }
}

pub fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
  clap::App::new(NAME)
    .about(
      "Starts the translation tool backend in a given project. This command should not be used \
      manually! It is reserved for internal use only by the translation tool itself.",
    )
    .arg(
      clap::Arg::with_name("project_dir")
        .value_name("PROJECT")
        .required(true)
        .help("Path to the project directory."),
    )
}

pub fn run(_global_opts: super::GlobalOpts, command_opts: CommandOpts) -> AnyResult<()> {
  let _project = Project::open(command_opts.project_dir).context("Failed to open the project")?;
  //
  Ok(())
}
