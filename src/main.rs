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

use crate::impl_prelude::*;

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

  if let Err(err) = try_main().context("CRITICAL ERROR") {
    if log::log_enabled!(log::Level::Error) {
      error!("{:?}", err);
    } else {
      eprintln!("ERROR: {:?}", err);
    }
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

  let (global_opts, command_opts) =
    cli::parse_opts().context("Failed to parse command-line arguments")?;

  log::set_max_level({
    let log_level_from_options =
      if global_opts.verbose { log::LevelFilter::Trace } else { log::LevelFilter::Debug };
    log::max_level().min(log_level_from_options)
  });

  // Brace for impact.
  info!("{}/{} v{}", CRATE_TITLE, CRATE_NAME, CRATE_VERSION);

  match command_opts {
    cli::CommandOpts::Scan(cmd_opts) => cli::scan::run(global_opts, *cmd_opts),
    cli::CommandOpts::CreateProject(cmd_opts) => cli::create_project::run(global_opts, *cmd_opts),
    cli::CommandOpts::ParsePo(cmd_opts) => cli::parse_po::run(global_opts, *cmd_opts),
    cli::CommandOpts::Export(cmd_opts) => cli::export::run(global_opts, *cmd_opts),
    cli::CommandOpts::Import(cmd_opts) => cli::import::run(global_opts, *cmd_opts),
    cli::CommandOpts::Status(cmd_opts) => cli::status::run(global_opts, *cmd_opts),
    cli::CommandOpts::Backend(cmd_opts) => cli::backend::run(global_opts, *cmd_opts),
  }
}
