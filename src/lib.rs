#![warn(missing_debug_implementations)]
#![allow(clippy::new_without_default)]
#![feature(try_blocks)]
// TODO: consider using feature(hash_raw_entry)

#[macro_use]
pub mod macros;

pub mod backend;
pub mod cc_ru_compat;
pub mod crosscode_markup;
pub mod ffi;
pub mod gettext_po;
pub mod impl_prelude;
pub mod localize_me;
pub mod logging;
pub mod progress;
pub mod project;
pub mod rc_string;
pub mod scan;
pub mod utils;

pub static CRATE_TITLE: &str = "CrossLocalE";
pub static CRATE_NAME: &str = env!("CARGO_PKG_NAME");
pub static CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub static CRATE_NICE_VERSION: &str = match option_env!("CARGO_PKG_NICE_VERSION") {
  Some(v) => v,
  None => CRATE_VERSION,
};
