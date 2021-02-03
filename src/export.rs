use crate::cli;
use crate::impl_prelude::*;
use crate::project::Project;

pub fn run(_common_opts: cli::CommonOpts, command_opts: cli::ExportCommandOpts) -> AnyResult<()> {
  info!(
    "Exporting a translation project in '{}' as '{}' into '{}'",
    command_opts.project_dir.display(),
    command_opts.format,
    command_opts.output.display(),
  );

  let _project = Project::open(command_opts.project_dir).context("Failed to open the project")?;

  Ok(())
}
