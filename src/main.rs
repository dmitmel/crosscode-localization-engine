#![deny(missing_debug_implementations)]
#![allow(clippy::new_without_default)]

pub mod cli;
pub mod impl_prelude;
pub mod scan;

use crate::impl_prelude::*;

pub const CRATE_TITLE: &str = "CrossLocalE";
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn main() {
  if let Err(err) = try_main() {
    if log::log_enabled!(log::Level::Error) {
      error!("{:?}", err);
    } else {
      eprintln!("ERROR: {:?}", err);
    }
  }
}

pub fn try_main() -> AnyResult<()> {
  env_logger::init_from_env(env_logger::Env::default().default_filter_or("debug"));
  let cli::Opts { common_opts, command_opts } =
    cli::parse_opts().context("Failed to parse command-line arguments")?;
  match command_opts {
    cli::CommandOpts::Scan(command_opts) => scan::run(&common_opts, &command_opts),
  }
}
