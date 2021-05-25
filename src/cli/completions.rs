// Search for "completions" in <https://github.com/rust-lang/rustup/blob/44be718122ecff073bcb2dfd44c6b50ed84c7696/src/cli/rustup_mode.rs>.

use crate::impl_prelude::*;
use crate::progress::ProgressReporter;

use clap_generate::generate;
use clap_generate::generators;
use std::io;

#[derive(Debug)]
pub struct CompletionsCommand;

impl super::Command for CompletionsCommand {
  fn name(&self) -> &'static str { "completions" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about("Generates completion scripts for various shells.")
      //
      .arg(
        clap::Arg::new("shell")
          .value_name("SHELL")
          .required(true)
          //
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
    let chosen_generator: fn(&mut clap::App, &'static str, &mut dyn io::Write) = match opt_shell {
      "bash" => generate::<generators::Bash, _>,
      "elvish" => generate::<generators::Elvish, _>,
      "fish" => generate::<generators::Fish, _>,
      "powershell" => generate::<generators::PowerShell, _>,
      "zsh" => generate::<generators::Zsh, _>,
      _ => unreachable!(),
    };
    chosen_generator(&mut arg_parser, env!("CARGO_BIN_NAME"), &mut io::stdout());

    Ok(())
  }
}
