use crate::cli;
use crate::impl_prelude::*;

pub fn run(_common_opts: cli::CommonOpts, command_opts: cli::ExportCommandOpts) -> AnyResult<()> {
  println!("{:?}", command_opts);
  Ok(())
}
