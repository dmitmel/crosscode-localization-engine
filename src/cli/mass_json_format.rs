use super::dump_common;
use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::serde as serde_utils;
use crate::utils::RcExt;

use std::convert::TryFrom;
use std::fs;
use std::io::{self, Read, Seek, Write};
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug)]
pub struct MassJsonFormatCommand;

inventory::submit!(&MassJsonFormatCommand as &dyn super::Command);

impl super::Command for MassJsonFormatCommand {
  fn name(&self) -> &'static str { "mass-json-format" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    dump_common::DumpCommandCommonOpts::add_only_formatting_to_arg_parser(
      app
        .about(
          "Utility command for quickly formatting or minifying a ton of JSON files. Intended for \
          personal use by Dima as an aid for working on the CrossCode version archive.",
        )
        .setting(clap::AppSettings::Hidden)
        .arg(
          clap::Arg::new("inputs")
            .value_name("INPUT_PATH")
            .value_hint(clap::ValueHint::AnyPath)
            .multiple_values(true)
            .required(true)
            .conflicts_with("inputs_file")
            .about(
              "Files to format. Directories may be passed as well, in which case all .json files \
              contained within the directory will be formatted recursively.",
            ),
        )
        .arg(
          clap::Arg::new("inputs_file")
            .value_name("PATH")
            .value_hint(clap::ValueHint::FilePath)
            .short('I')
            .long("read-inputs")
            .about(
              "Read paths to input files from a file. If there are other paths specified via \
              command-line arguments, then those will be used instead and the inputs file will be \
              ignored.",
            ),
        )
        .arg(
          clap::Arg::new("output")
            .value_name("PATH")
            .value_hint(clap::ValueHint::AnyPath)
            .short('o')
            .long("output")
            .about("Path to the destination file or directory."),
        )
        .arg(
          clap::Arg::new("in_place")
            .short('i')
            .long("in-place")
            //
            .about("Format files in-place."),
        )
        .group(clap::ArgGroup::new("write_mode").arg("output").arg("in_place").required(true)),
    )
  }

  fn run(
    &self,
    global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    mut progress: Box<dyn ProgressReporter>,
  ) -> anyhow::Result<()> {
    let opt_inputs: Vec<_> = matches
      .values_of_os("inputs")
      .map_or_else(Vec::new, |values| values.map(PathBuf::from).collect());
    let opt_inputs_file = matches.value_of_os("inputs_file").map(PathBuf::from);
    let opt_output = matches.value_of_os("output").map(PathBuf::from);
    let opt_in_place = matches.is_present("in_place");
    let dump_common_opt = dump_common::DumpCommandCommonOpts::from_matches(matches);

    let inputs = super::import::collect_input_files(&opt_inputs, &opt_inputs_file, "json")?;
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

    let json_config = dump_common_opt.ultimate_formatter_config();
    let opt_output = opt_output.map(Rc::new);

    let all_inputs_len = inputs.len();
    progress.begin_task(all_inputs_len)?;
    let mut errors_count: usize = 0;
    for (i, (input_entry_arg, input_entry)) in inputs.into_iter().enumerate() {
      let input_path = Rc::new(input_entry.into_path());
      progress.set_task_info(&RcString::from(input_path.to_string_lossy()))?;
      progress.set_task_progress(i)?;

      let output_path = match &opt_output {
        Some(opt_output) if treat_output_as_regular_file => opt_output.share_rc(),
        Some(opt_output) => {
          let input_rel_path = input_path
            .strip_prefix(input_entry_arg.parent().unwrap_or(&*input_entry_arg))
            .unwrap();
          Rc::new(opt_output.join(input_rel_path))
        }
        None => input_path.share_rc(),
      };

      if let Err(e) = try_any_result!({
        let mut input_file = fs::OpenOptions::new()
          .read(true)
          .write(opt_in_place)
          .truncate(false)
          .open(&*input_path)?;

        let mut input_non_mmap_bytes: Vec<u8>;
        let mut input_rw_mmap: Option<memmap2::MmapMut> = None;
        let input_ro_mmap: memmap2::Mmap;
        let use_mmap = global_opts.mmap_preference.should_actually_use(&input_path);
        let input_bytes: &[u8] = if !use_mmap {
          input_non_mmap_bytes = Vec::with_capacity(
            // See <https://github.com/rust-lang/rust/blob/1.55.0/library/std/src/fs.rs#L201-L207>.
            input_file.metadata().map_or(0, |m| m.len() as usize + 1),
          );
          input_file.read_to_end(&mut input_non_mmap_bytes)?;
          &input_non_mmap_bytes
        } else if opt_in_place {
          input_rw_mmap =
            Some(unsafe { memmap2::MmapOptions::new().populate().map_mut(&input_file)? });
          input_rw_mmap.as_ref().unwrap()
        } else {
          input_ro_mmap = unsafe { memmap2::MmapOptions::new().populate().map(&input_file)? };
          &input_ro_mmap
        };

        let mut output_bytes: Vec<u8> = Vec::with_capacity(input_bytes.len());
        let mut deserializer = serde_json::Deserializer::from_slice(input_bytes);
        let mut serializer = serde_json::Serializer::with_formatter(
          &mut output_bytes,
          json::UltimateFormatter::new(json_config.clone()),
        );
        serde_utils::OnTheFlyConverter::convert(&mut serializer, &mut deserializer)?;
        deserializer.end()?;
        if !output_bytes.ends_with(b"\n") {
          output_bytes.push(b'\n');
        }

        if let Err(e) = try_any_result!({
          let mut output_file = if !opt_in_place {
            if let Some(entry_output_dir) = output_path.parent() {
              fs::create_dir_all(entry_output_dir)
                .with_context(|| format!("Failed to create directory {:?}", entry_output_dir))?;
            }
            fs::OpenOptions::new()
              .read(use_mmap) // Apparently required for mmaping?
              .write(true)
              .create(true)
              .truncate(false)
              .open(&*output_path)?
          } else {
            input_file.seek(io::SeekFrom::Start(0))?;
            input_file
          };

          output_file.set_len(u64::try_from(output_bytes.len()).unwrap())?;
          if use_mmap {
            let mut output_mmap = if !opt_in_place {
              unsafe { memmap2::MmapOptions::new().map_mut(&output_file)? }
            } else {
              input_rw_mmap.unwrap()
            };
            output_mmap.copy_from_slice(&output_bytes);
            // output_mmap.flush_async()?;
          } else {
            output_file.write_all(&output_bytes)?;
          }
          output_file.flush()?;
        }) {
          Err(if !opt_in_place {
            e.context(format!("Error while writing to output file {:?}", output_path))
          } else {
            e.context("Failed to write")
          })?
        }
      }) {
        crate::report_error(e.context(format!("Failed to format file {:?}", input_path)));
        errors_count += 1;
      }
    }

    progress.set_task_progress(all_inputs_len)?;
    progress.end_task()?;

    if errors_count > 0 {
      bail!("Failed to format {} files, see logs above", errors_count);
    }
    info!("Successfully formatted {} files", all_inputs_len);
    Ok(())
  }
}
