#![deny(missing_debug_implementations)]
#![allow(clippy::new_without_default)]
#![feature(try_blocks)]
// TODO: consider using feature(hash_raw_entry)

pub use crosslocale::*;

#[macro_use]
pub mod macros;

pub mod cli;

use crate::cli::ProgressMode;
use crate::impl_prelude::*;

use std::env;
use std::ffi::OsStr;

pub fn main() {
  let backtrace_var_name = OsStr::new("RUST_BACKTRACE");
  if env::var_os(backtrace_var_name).is_none() {
    env::set_var(backtrace_var_name, OsStr::new("1"));
  }

  if let Err(e) = try_main() {
    report_critical_error(e);
  }
}

pub fn try_main() -> AnyResult<()> {
  crate::init_logging();

  let (arg_parser, mut all_commands_map) = cli::create_complete_arg_parser();
  let matches = arg_parser.get_matches();
  let global_opts = cli::GlobalOpts::from_matches(&matches);

  if !global_opts.no_banner_message {
    print_banner_message();
  }
  log::set_max_level({
    let log_level_from_options =
      if global_opts.verbose { log::LevelFilter::Trace } else { log::LevelFilter::Debug };
    log::max_level().min(log_level_from_options)
  });

  let enable_fancy_progress_bar = match global_opts.progress_mode {
    ProgressMode::Auto => atty::is(atty::Stream::Stderr),
    ProgressMode::Always => true,
    ProgressMode::Never => false,
  };
  let progress_reporter: Box<dyn progress::ProgressReporter> = if enable_fancy_progress_bar {
    Box::new(progress::TuiProgresReporter::new())
  } else {
    Box::new(progress::NopProgressReporter)
  };

  // {
  //   use self::progress::ProgressReporter as _;

  //   let p = &mut progress_reporter;
  //   p.set_task_info(&rc_string::RcString::from("test"))?;

  //   // p.begin_task()?;
  //   // p.set_task_progress(0, 0)?;
  //   // p.end_task()?;

  //   p.begin_task()?;
  //   p.set_task_progress(0, 1)?;
  //   p.set_task_progress(1, 1)?;
  //   p.end_task()?;

  //   p.begin_task()?;
  //   p.set_task_progress(0, 2)?;
  //   p.set_task_progress(1, 2)?;
  //   p.set_task_progress(2, 2)?;
  //   p.end_task()?;

  //   p.begin_task()?;
  //   p.set_task_progress(0, 3)?;
  //   p.set_task_progress(1, 3)?;
  //   p.set_task_progress(2, 3)?;
  //   p.set_task_progress(3, 3)?;
  //   p.end_task()?;

  //   p.begin_task()?;
  //   let total = 100;
  //   for i in 0..=total {
  //     p.set_task_info(&rc_string::RcString::from(format!("test тест test тест test тест {}", i)))?;
  //     p.set_task_progress(i, total)?;
  //     std::thread::sleep(std::time::Duration::from_secs(1) / total as u32);
  //   }
  //   p.end_task()?;

  //   // return Ok(());
  // }

  if let Some(dir) = &global_opts.cd {
    trace!("cd {:?}", dir);
    env::set_current_dir(dir)
      .with_context(|| format!("Failed to change the working directory to {:?}", dir))?;
  }

  let (command_name, command_matches) = matches.subcommand().unwrap();
  let command = all_commands_map.remove(command_name).unwrap();

  // Brace for impact.

  command
    .run(global_opts, command_matches, progress_reporter)
    .with_context(|| format!("Failed to run command {:?}", command_name))
}
