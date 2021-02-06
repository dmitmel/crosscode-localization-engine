use crate::impl_prelude::*;
use crate::project::Project;
use crate::rc_string::RcString;

use std::collections::HashMap;
use std::path::PathBuf;

pub const NAME: &str = "status";

#[derive(Debug)]
pub struct CommandOpts {
  pub project_dir: PathBuf,
}

impl CommandOpts {
  pub fn from_matches(matches: &clap::ArgMatches<'_>) -> Self {
    Self { project_dir: PathBuf::from(matches.value_of_os("project_dir").unwrap()) }
  }
}

pub fn create_arg_parser<'a, 'b>() -> clap::App<'a, 'b> {
  clap::App::new(NAME)
    .about("Displays general information about the project, such as translation progress.")
    .arg(
      clap::Arg::with_name("project_dir")
        .value_name("PROJECT")
        .required(true)
        .help("Path to the project directory."),
    )
}

pub fn run(_global_opts: super::GlobalOpts, command_opts: CommandOpts) -> AnyResult<()> {
  let project = Project::open(command_opts.project_dir).context("Failed to open the project")?;

  let mut total_fragments: u64 = 0;
  let mut unique_fragments = HashMap::<RcString, u64>::new();
  let mut translated_fragments: u64 = 0;
  let mut total_translations: u64 = 0;

  for game_file in project.virtual_game_files().values() {
    for fragment in game_file.fragments().values() {
      total_fragments += 1;
      translated_fragments += (!fragment.translations().is_empty()) as u64;
      *unique_fragments.entry(fragment.original_text().share_rc()).or_insert(0) += 1;
      total_translations += fragment.translations().len() as u64;
    }
  }

  info!("       Total fragments: {:>6}", total_fragments);
  let unique_percent = unique_fragments.len() as f64 / total_fragments as f64 * 100.0;
  info!("      Unique fragments: {:>6}  {:.02}%", unique_fragments.len(), unique_percent);
  let translated_percent = translated_fragments as f64 / total_fragments as f64 * 100.0;
  info!("  Translated fragments: {:>6}  {:.02}%", translated_fragments, translated_percent);
  let untranslated_fragments = total_fragments - translated_fragments;
  let untranslated_percent = untranslated_fragments as f64 / total_fragments as f64 * 100.0;
  info!("Untranslated fragments: {:>6}  {:.02}%", untranslated_fragments, untranslated_percent);

  info!("    Total translations: {:>6}", total_translations);

  Ok(())
}
