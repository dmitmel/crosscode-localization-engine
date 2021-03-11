#![deny(missing_debug_implementations)]
#![allow(clippy::new_without_default)]
#![feature(try_blocks)]
// TODO: consider using feature(hash_raw_entry)

#[macro_use]
pub mod macros;

pub mod backend;
pub mod cc_ru_compat;
pub mod ffi;
pub mod gettext_po;
pub mod impl_prelude;
pub mod localize_me;
pub mod project;
pub mod rc_string;
pub mod scan;
pub mod utils;

use crate::impl_prelude::*;

pub const CRATE_TITLE: &str = "CrossLocalE";
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn init_logging() -> bool {
  let set_logger_result: Result<(), log::SetLoggerError> =
    env_logger::try_init_from_env(env_logger::Env::default().default_filter_or(
      // The logging level of `env_logger` can't be changed once the logger has
      // been installed, so instead let's by default allow all logging levels
      // on the `env_logger` side, we will lower the logging level later on
      // ourselves on the `log` side.
      "trace",
    ));
  let other_logger_already_installed = set_logger_result.is_err();
  info!("{}/{} v{}", CRATE_TITLE, CRATE_NAME, CRATE_VERSION);
  !other_logger_already_installed
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
