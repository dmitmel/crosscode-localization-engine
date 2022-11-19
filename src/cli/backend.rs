use crate::backend::transports::StdioTransport;
use crate::backend::{self, Backend};
use crate::impl_prelude::*;
use crate::logging;
use crate::progress::ProgressReporter;

#[derive(Debug)]
pub struct BackendCommand;

impl super::Command for BackendCommand {
  fn name(&self) -> &'static str { "backend" }

  fn create_arg_parser(&self, app: clap::Command) -> clap::Command {
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
          .value_parser([backend::PROTOCOL_VERSION_STR.as_str()]),
      )
      .arg(
        clap::Arg::new("transport")
          .value_name("TRANSPORT")
          .value_hint(clap::ValueHint::Other)
          .required(true)
          .long("transport")
          .value_parser(["stdio"]),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    _matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let mut backend = Backend::new(Box::new(StdioTransport));
    logging::set_stdio_logger(None);
    backend.start()
  }
}
