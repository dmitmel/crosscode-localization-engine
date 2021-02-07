use crate::backend::Backend;
use crate::impl_prelude::*;

pub const NAME: &str = "backend";

#[derive(Debug)]
pub struct CommandOpts {}

impl CommandOpts {
  pub fn from_matches(_matches: &clap::ArgMatches<'_>) -> Self { Self {} }
}

pub fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
  clap::App::new(NAME).about(
    "Starts the translation tool backend in a given project. This command should not be used \
      manually! It is reserved for internal use only by the translation tool itself.",
  )
}

pub fn run(_global_opts: super::GlobalOpts, _command_opts: CommandOpts) -> AnyResult<()> {
  Backend::new().start()
}
