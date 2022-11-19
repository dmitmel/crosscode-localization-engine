use super::dump_common;
use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::{self, RcExt};

use std::convert::TryFrom;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

#[derive(Debug)]
pub struct MassJsonFormatCommand;

impl super::Command for MassJsonFormatCommand {
  fn name(&self) -> &'static str { "mass-json-format" }

  fn create_arg_parser(&self, app: clap::Command) -> clap::Command {
    dump_common::DumpCommandCommonOpts::add_only_formatting_to_arg_parser(
      app
        .about(
          "Utility command for quickly formatting or minifying a ton of JSON files. Intended for \
          personal use by Dima as an aid for working on the CrossCode version archive.",
        )
        .hide(true)
        .arg(
          clap::Arg::new("inputs")
            .value_name("INPUT_PATH")
            .value_hint(clap::ValueHint::AnyPath)
            .value_parser(clap::value_parser!(PathBuf))
            .action(clap::ArgAction::Append)
            .help(
              "Files to format. Directories may be passed as well, in which case all .json files \
              contained within the directory will be formatted recursively.",
            ),
        )
        .arg(
          clap::Arg::new("inputs_file")
            .value_name("PATH")
            .value_hint(clap::ValueHint::FilePath)
            .value_parser(clap::value_parser!(PathBuf))
            .short('I')
            .long("read-inputs")
            .help(
              "Read paths to input files from a file. If there are other paths specified via \
              command-line arguments, then those will be used instead and the inputs file will be \
              ignored.",
            ),
        )
        .arg(
          clap::Arg::new("output")
            .value_name("PATH")
            .value_hint(clap::ValueHint::AnyPath)
            .value_parser(clap::value_parser!(PathBuf))
            .short('o')
            .long("output")
            .help("Path to the destination file or directory."),
        )
        .arg(
          clap::Arg::new("in_place")
            .action(clap::ArgAction::SetTrue)
            .short('i')
            .long("in-place")
            .help("Format files in-place."),
        )
        .arg(
          clap::Arg::new("pipe")
            .action(clap::ArgAction::SetTrue)
            .short('P')
            .long("pipe")
            .help("Use the program as a filter in shell pipes."),
        )
        .group(
          clap::ArgGroup::new("read_mode")
            .arg("inputs")
            .arg("inputs_file")
            .arg("pipe")
            .required(true),
        )
        .group(
          clap::ArgGroup::new("write_mode")
            .arg("output")
            .arg("in_place")
            .arg("pipe")
            .required(true),
        )
        .arg(
          clap::Arg::new("jobs")
            .short('j')
            .long("jobs")
            .help(
              "The number of parallel worker threads allocated for formatting. Zero means using \
              as many threads as there are CPU cores available.",
            )
            .value_parser(clap::value_parser!(usize))
            .default_value("0"),
        ),
    )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    mut progress: Box<dyn ProgressReporter>,
  ) -> anyhow::Result<()> {
    let opt_inputs: Vec<_> = matches.get_many::<PathBuf>("inputs").unwrap().cloned().collect();
    let opt_inputs_file = matches.get_one::<PathBuf>("inputs_file");
    let opt_output = matches.get_one::<PathBuf>("output");
    let _opt_in_place = matches.get_flag("in_place");
    let opt_pipe = matches.get_flag("pipe");
    let dump_common_opt =
      dump_common::DumpCommandCommonOpts::from_matches_only_formatting(matches);
    let opt_jobs = *matches.get_one::<usize>("jobs").unwrap();

    let json_config = dump_common_opt.ultimate_formatter_config();

    if opt_pipe {
      let mut input_bytes = Vec::new();
      let (mut stdin, mut stdout) = (io::stdin(), io::stdout());
      stdin.read_to_end(&mut input_bytes)?;
      let output_bytes = format_buffer(&input_bytes, json_config)?;
      stdout.write_all(&output_bytes)?;
      stdout.flush()?;
      return Ok(());
    }

    let inputs = super::import::collect_input_files(&opt_inputs, opt_inputs_file, "json")?;
    if inputs.is_empty() {
      warn!("Found no files to format!");
      return Ok(());
    }

    let treat_output_as_regular_file = match inputs.as_slice() {
      // Only a single entry, but we still must check that it's not just from a
      // directory with 1 file
      [(_, entry)] => entry.depth() == 0 && !entry.file_type().is_dir(),
      // Many entries
      _ => false,
    };

    // The implementation of multithreading is based on the code in the scan
    // command, see the comments there.
    let pool: threadpool::ThreadPool = {
      let mut builder = threadpool::Builder::new();
      if opt_jobs != 0 {
        builder = builder.num_threads(opt_jobs);
      }
      builder.build()
    };

    #[derive(Debug)]
    struct TaskResult {
      task_index: usize,
      input_entry: walkdir::DirEntry,
      success: bool,
    }

    let (task_results_tx, task_results_rx) = mpsc::channel::<Box<TaskResult>>();

    let all_inputs_len = inputs.len();
    progress.begin_task(all_inputs_len)?;
    progress.set_task_info(&RcString::from("<Starting...>"))?;
    progress.set_task_progress(0)?;

    let mut errors_count: usize = 0;
    for (task_index, (input_entry_arg, input_entry)) in inputs.into_iter().enumerate() {
      let task_results_tx = task_results_tx.clone();
      let opt_output = opt_output.cloned();
      let json_config = json_config.clone();
      let input_entry_arg: PathBuf = input_entry_arg.rc_clone_inner();

      pool.execute(move || {
        let input_path = input_entry.path();
        let constructed_output_path: PathBuf;
        let output_path: Option<&Path> = match &opt_output {
          Some(opt_output) if treat_output_as_regular_file => Some(opt_output),
          Some(opt_output) => {
            let input_rel_path = input_path
              .strip_prefix(input_entry_arg.parent().unwrap_or(&*input_entry_arg))
              .unwrap();
            constructed_output_path = opt_output.join(input_rel_path);
            Some(&constructed_output_path)
          }
          None => None,
        };

        let mut success = true;
        if let Err(e) = try_any_result!({
          let mut input_file = fs::OpenOptions::new()
            .read(true)
            .write(output_path.is_none())
            .truncate(false)
            .open(input_path)?;

          let mut input_bytes =
            Vec::with_capacity(utils::buffer_capacity_for_reading_file(&input_file));
          input_file.read_to_end(&mut input_bytes)?;

          let output_bytes: Vec<u8> = format_buffer(&input_bytes, json_config)?;

          try_any_result!({
            let mut output_file = if let Some(output_path) = output_path {
              if let Some(output_dir) = output_path.parent() {
                fs::create_dir_all(output_dir)
                  .with_context(|| format!("Failed to create directory {:?}", output_dir))?;
              }
              fs::OpenOptions::new().write(true).create(true).truncate(true).open(output_path)?
            } else {
              input_file.seek(io::SeekFrom::Start(0))?;
              input_file
            };

            // Not every file can be truncated, e.g. /dev/stdout, and even if
            // set_len errors errors on a legitimate file, it can be safely
            // ignoredd because of truncate(true) when opening it. This call
            // does speed up I/O though because the OS gets a chance to
            // pre-allocate the entire file, and it is essential for in-place
            // writing.
            let _ = output_file.set_len(u64::try_from(output_bytes.len()).unwrap());
            output_file.write_all(&output_bytes)?;
            output_file.flush()?;
          })
          .map_err(|e| {
            if let Some(output_path) = output_path {
              e.context(format!("Error while writing to output file {:?}", output_path))
            } else {
              e.context("Failed to write")
            }
          })?;
        }) {
          crate::report_error!(e.context(format!("Failed to format file {:?}", input_path)));
          success = false;
        }

        task_results_tx.send(Box::new(TaskResult { task_index, input_entry, success })).unwrap();
      });
    }

    drop(task_results_tx);

    let mut sorted_results = Vec::<Option<Box<TaskResult>>>::with_capacity(all_inputs_len);
    for _ in 0..all_inputs_len {
      sorted_results.push(None);
    }

    for (i, task_result) in task_results_rx.into_iter().enumerate() {
      progress.set_task_info(&RcString::from(task_result.input_entry.path().to_string_lossy()))?;
      progress.set_task_progress(i + 1)?;
      if !task_result.success {
        errors_count += 1;
      }
      let i = task_result.task_index;
      sorted_results[i] = Some(task_result);
    }

    progress.set_task_progress(all_inputs_len)?;
    pool.join();
    progress.end_task()?;

    if errors_count > 0 {
      bail!("Failed to format {} files, see logs above", errors_count);
    }
    info!("Successfully formatted {} files", all_inputs_len);
    Ok(())
  }
}

fn format_buffer(
  input_bytes: &[u8],
  json_config: json::UltimateFormatterConfig,
) -> AnyResult<Vec<u8>> {
  let mut output_bytes: Vec<u8> = Vec::with_capacity(input_bytes.len());
  let mut deserializer = serde_json::Deserializer::from_slice(input_bytes);
  let mut serializer = serde_json::Serializer::with_formatter(
    &mut output_bytes,
    json::UltimateFormatter::new(json_config),
  );
  serde_transcode::transcode(&mut deserializer, &mut serializer)?;
  deserializer.end()?;
  if !output_bytes.ends_with(b"\n") {
    output_bytes.push(b'\n');
  }
  Ok(output_bytes)
}
