use crate::backend::transports::StdioTransport;
use crate::backend::{self, Backend};
use crate::impl_prelude::*;
use crate::progress::ProgressReporter;

#[derive(Debug)]
pub struct BackendCommand;

inventory::submit!(&BackendCommand as &dyn super::Command);

impl super::Command for BackendCommand {
  fn name(&self) -> &'static str { "backend" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about(
        "Starts the translation tool backend in a given project. This command should not be used \
        manually! It is reserved for internal use only by the translation tool itself.",
      )
      .arg(
        clap::Arg::new("protocol_version")
          .value_name("VERSION")
          .value_hint(clap::ValueHint::Other)
          .required(true)
          .long("protocol-version")
          .possible_value(&backend::PROTOCOL_VERSION_STR),
      )
      .arg(
        clap::Arg::new("transport")
          .value_name("TRANSPORT")
          .value_hint(clap::ValueHint::Other)
          .required(true)
          .long("transport")
          .possible_value("stdio"),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    _matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    Backend::new(Box::new(StdioTransport)).start()
  }
}
