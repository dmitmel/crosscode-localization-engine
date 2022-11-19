// Search for "completions" in <https://github.com/rust-lang/rustup/blob/44be718122ecff073bcb2dfd44c6b50ed84c7696/src/cli/rustup_mode.rs>.

use crate::impl_prelude::*;
use crate::progress::ProgressReporter;

use clap_complete::Shell;
use std::io::{self, Write};

#[derive(Debug)]
pub struct CompletionsCommand;

impl super::Command for CompletionsCommand {
  fn name(&self) -> &'static str { "completions" }

  fn create_arg_parser(&self, app: clap::Command) -> clap::Command {
    app
      .about("Generates completion scripts for various shells.")
      //
      .arg(
        clap::Arg::new("shell")
          .value_name("SHELL")
          .value_hint(clap::ValueHint::Other)
          .required(true)
          .value_parser(clap::value_parser!(Shell)),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let opt_shell = matches.get_one::<Shell>("shell").unwrap();

    let (mut arg_parser, _) = crate::cli::create_complete_arg_parser();
    let mut out = io::stdout();
    clap_complete::generate(*opt_shell, &mut arg_parser, env!("CARGO_BIN_NAME"), &mut out);
    out.write_all(b"\n")?;
    out.flush()?;

    Ok(())
  }
}
