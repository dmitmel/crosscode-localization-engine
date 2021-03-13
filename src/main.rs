#![deny(missing_debug_implementations)]
#![allow(clippy::new_without_default)]
#![feature(try_blocks)]
// TODO: consider using feature(hash_raw_entry)

pub use crosslocale::*;

#[macro_use]
pub mod macros;

pub mod cli;

use crate::cli::Command;
use crate::impl_prelude::*;

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;

pub fn main() {
  let backtrace_var_name = OsStr::new("RUST_BACKTRACE");
  if env::var_os(backtrace_var_name).is_none() {
    env::set_var(backtrace_var_name, OsStr::new("1"));
  }

  if let Err(e) = try_main() {
    report_critical_error(e);
  }
}

pub fn try_main() -> AnyResult<()> {
  crate::init_logging();

  let mut arg_parser = cli::GlobalOpts::create_arg_parser();

  let all_commands: Vec<Box<dyn Command>> = cli::all_commands();
  let mut all_commands_map = HashMap::with_capacity(all_commands.len());
  for command in all_commands {
    arg_parser = arg_parser.subcommand(command.create_arg_parser(clap::App::new(command.name())));
    all_commands_map.insert(command.name(), command);
  }

  let matches = arg_parser.get_matches();
  let global_opts = cli::GlobalOpts::from_matches(&matches);

  log::set_max_level({
    let log_level_from_options =
      if global_opts.verbose { log::LevelFilter::Trace } else { log::LevelFilter::Debug };
    log::max_level().min(log_level_from_options)
  });

  let (command_name, command_matches) = matches.subcommand().unwrap();
  let command = all_commands_map.remove(command_name).unwrap();

  // Brace for impact.

  command
    .run(global_opts, command_matches)
    .with_context(|| format!("Failed to run command {:?}", command_name))
}
