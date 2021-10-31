// Search for "completions" in <https://github.com/rust-lang/rustup/blob/44be718122ecff073bcb2dfd44c6b50ed84c7696/src/cli/rustup_mode.rs>.

use crate::impl_prelude::*;
use crate::progress::ProgressReporter;

use clap_generate::Shell;
use std::io::{self, Write};

#[derive(Debug)]
pub struct CompletionsCommand;

inventory::submit!(&CompletionsCommand as &dyn super::Command);

impl super::Command for CompletionsCommand {
  fn name(&self) -> &'static str { "completions" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about("Generates completion scripts for various shells.")
      //
      .arg(
        clap::Arg::new("shell")
          .value_name("SHELL")
          .value_hint(clap::ValueHint::Other)
          .required(true)
          .possible_values(&["bash", "elvish", "fish", "powershell", "zsh"]),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let opt_shell = matches.value_of("shell").unwrap();

    let (mut arg_parser, _) = crate::cli::create_complete_arg_parser();
    let shell = match opt_shell {
      "bash" => Shell::Bash,
      "elvish" => Shell::Elvish,
      "fish" => Shell::Fish,
      "powershell" => Shell::PowerShell,
      "zsh" => Shell::Zsh,
      _ => unreachable!(),
    };
    let mut out = io::stdout();
    clap_generate::generate(shell, &mut arg_parser, env!("CARGO_BIN_NAME"), &mut out);
    out.write_all(b"\n")?;
    out.flush()?;

    Ok(())
  }
}
