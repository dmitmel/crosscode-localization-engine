use crate::backend::{self, Backend};
use crate::impl_prelude::*;

#[derive(Debug)]
pub struct BackendCommand;

impl super::Command for BackendCommand {
  fn name(&self) -> &'static str { "backend" }

  fn create_arg_parser<'a, 'b>(&self, app: clap::App<'a, 'b>) -> clap::App<'a, 'b> {
    app
      .about(
        "Starts the translation tool backend in a given project. This command should not be used \
        manually! It is reserved for internal use only by the translation tool itself.",
      )
      .arg(
        clap::Arg::with_name("protocol_version")
          .value_name("VERSION")
          .required(true)
          .long("protocol-version")
          .possible_value(&backend::PROTOCOL_VERSION_STR),
      )
      .arg(
        clap::Arg::with_name("transport")
          .value_name("TRANSPORT")
          .required(true)
          .long("transport")
          .possible_value("stdio"),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    _matches: &clap::ArgMatches<'_>,
  ) -> AnyResult<()> {
    Backend::new().start()
  }
}
