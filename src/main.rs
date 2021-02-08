#![deny(missing_debug_implementations)]
#![allow(clippy::new_without_default)]
#![feature(try_blocks)]
// TODO: consider using feature(hash_raw_entry)

#[macro_use]
pub mod macros;

pub mod backend;
pub mod cc_ru_compat;
pub mod cli;
pub mod gettext_po;
pub mod impl_prelude;
pub mod localize_me;
pub mod project;
pub mod rc_string;
pub mod scan;
pub mod utils;

use crate::cli::Command;
use crate::impl_prelude::*;

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;

pub const CRATE_TITLE: &str = "CrossLocalE";
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn main() {
  let backtrace_var_name = OsStr::new("RUST_BACKTRACE");
  if env::var_os(backtrace_var_name).is_none() {
    env::set_var(backtrace_var_name, OsStr::new("1"));
  }

  if let Err(e) = try_main() {
    report_critical_error(e);
  }
}

pub fn report_critical_error(mut error: AnyError) {
  error = error.context(format!(
    "CRITICAL ERROR in thread '{}'",
    std::thread::current().name().unwrap_or("<unnamed>"),
  ));
  if log::log_enabled!(log::Level::Error) {
    error!("{:?}", error);
  } else {
    eprintln!("ERROR: {:?}", error);
  }
}

pub fn report_error(mut error: AnyError) {
  error = error.context(format!(
    "non-critical error in thread '{}'",
    std::thread::current().name().unwrap_or("<unnamed>"),
  ));
  if log::log_enabled!(log::Level::Error) {
    warn!("{:?}", error);
  } else {
    eprintln!("WARN: {:?}", error);
  }
}

pub fn try_main() -> AnyResult<()> {
  env_logger::init_from_env(env_logger::Env::default().default_filter_or(
    // The logging level of `env_logger` can't be changed once the logger has
    // been installed, so instead let's by default allow all logging levels on
    // the `env_logger` side, we will lower the logging level later on
    // ourselves on the `log` side.
    "trace",
  ));

  let mut arg_parser = cli::GlobalOpts::create_arg_parser();

  let all_commands: Vec<Box<dyn Command>> = cli::all_commands();
  let mut all_commands_map = HashMap::with_capacity(all_commands.len());
  for command in all_commands {
    arg_parser.p.add_subcommand(command.create_arg_parser(clap::App::new(command.name())));
    all_commands_map.insert(command.name(), command);
  }

  let matches = arg_parser.get_matches();
  let global_opts = cli::GlobalOpts::from_matches(&matches);

  log::set_max_level({
    let log_level_from_options =
      if global_opts.verbose { log::LevelFilter::Trace } else { log::LevelFilter::Debug };
    log::max_level().min(log_level_from_options)
  });

  let (command_name, command_matches) = matches.subcommand();
  let command = all_commands_map.remove(command_name).unwrap();
  let command_matches = command_matches.unwrap();

  // Brace for impact.
  info!("{}/{} v{}", CRATE_TITLE, CRATE_NAME, CRATE_VERSION);

  command
    .run(global_opts, command_matches)
    .with_context(|| format!("Failed to run command {:?}", command_name))
}
