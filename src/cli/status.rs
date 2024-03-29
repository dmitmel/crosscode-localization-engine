use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use crate::project::Project;
use crate::rc_string::RcString;

use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug)]
pub struct StatusCommand;

impl super::Command for StatusCommand {
  fn name(&self) -> &'static str { "status" }

  fn create_arg_parser(&self, app: clap::Command) -> clap::Command {
    app
      .about("Displays general information about the project, such as translation progress.")
      //
      .arg(
        clap::Arg::new("project_dir")
          .value_name("PROJECT")
          .value_hint(clap::ValueHint::DirPath)
          .value_parser(clap::value_parser!(PathBuf))
          .required(true)
          .help("Path to the project directory."),
      )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let opt_project_dir = matches.get_one::<PathBuf>("project_dir").unwrap();

    let project = Project::open(opt_project_dir.clone()).context("Failed to open the project")?;

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
}
