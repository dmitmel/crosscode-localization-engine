#![deny(missing_debug_implementations)]
#![allow(clippy::new_without_default)]

pub mod cli;
pub mod impl_prelude;

use crate::impl_prelude::*;

pub const CRATE_TITLE: &str = "CrossLocalE";
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn main() {
  if let Err(err) = try_main() {
    if log::log_enabled!(log::Level::Error) {
      error!("{:?}", err);
    } else {
      eprintln!("ERROR: ${:?}", err);
    }
  }
}

pub fn try_main() -> AnyResult<()> {
  let args = cli::parse_args().context("Failed to parse command-line arguments")?;
  println!("{:?}", args);
  Ok(())
}
