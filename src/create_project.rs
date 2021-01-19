use crate::cli;
use crate::impl_prelude::*;

pub fn run(
  common_opts: &cli::CommonOpts,
  command_opts: &cli::CreateProjectCommandOpts,
) -> AnyResult<()> {
  dbg!(common_opts, command_opts);
  Ok(())
}
