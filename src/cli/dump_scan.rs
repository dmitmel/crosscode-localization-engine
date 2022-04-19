use super::dump_common as common;
use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use crate::scan;
use crate::utils::json;

use serde_json::ser::Formatter;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;

#[derive(Debug)]
pub struct DumpScanCommand;

impl super::Command for DumpScanCommand {
  fn name(&self) -> &'static str { "dump-scan" }

  fn create_arg_parser<'help>(&self, app: clap::Command<'help>) -> clap::Command<'help> {
    common::DumpCommandCommonOpts::add_to_arg_parser(
      app
        .about(
          "Dumps a scan database to JSON in a structure that is easy to process with jq (see \
          <https://stedolan.github.io/jq/>) on the command-line.",
        )
        .arg(
          clap::Arg::new("scan_db")
            .value_name("SCAN_DB_PATH")
            .value_hint(clap::ValueHint::FilePath)
            .allow_invalid_utf8(true)
            .required(true)
            .help("Path to a scan database to dump."),
        ),
    )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let opt_scan_db = PathBuf::from(matches.value_of_os("scan_db").unwrap());
    let common_opt = common::DumpCommandCommonOpts::from_matches(matches);

    let mut fmt = json::UltimateFormatter::new(common_opt.ultimate_formatter_config());
    let mut out = io::stdout();
    let mut stream_helper = common_opt.dump_stream_helper();

    let scan_db = scan::ScanDb::open(opt_scan_db).context("Failed to open the scan database")?;

    match dump_the_scan_db(scan_db, &mut fmt, &mut out, &mut stream_helper) {
      Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
      r => r?,
    };

    Ok(())
  }
}

pub fn dump_the_scan_db(
  scan_db: Rc<scan::ScanDb>,
  fmt: &mut json::UltimateFormatter,
  out: &mut (impl io::Write + ?Sized),
  stream_helper: &mut common::DumpStreamHelper,
) -> io::Result<()> {
  stream_helper.begin(fmt, out)?;

  for game_file in scan_db.game_files().values() {
    for fragment in game_file.fragments().values() {
      stream_helper.begin_value(fmt, out)?;

      crate::json_fmt_helper!(wrap_object, fmt, out, {
        fmt.write_static_object_key(out, true, "file_asset_root")?;
        fmt.write_escaped_string_object_value(out, fragment.file_asset_root())?;

        fmt.write_static_object_key(out, false, "file_path")?;
        fmt.write_escaped_string_object_value(out, fragment.file_path())?;

        fmt.write_static_object_key(out, false, "json_path")?;
        fmt.write_escaped_string_object_value(out, fragment.json_path())?;

        fmt.write_static_object_key(out, false, "lang_uid")?;
        crate::json_fmt_helper!(wrap_object_value, fmt, out, {
          fmt.write_i32(out, fragment.lang_uid())?;
        });

        fmt.write_static_object_key(out, false, "description")?;
        crate::json_fmt_helper!(wrap_object_value, fmt, out, {
          crate::json_fmt_helper!(wrap_array, fmt, out, {
            for (i, line) in fragment.description().iter().enumerate() {
              crate::json_fmt_helper!(wrap_array_value, fmt, out, i == 0, {
                fmt.write_escaped_string(out, line)?;
              });
            }
          });
        });

        fmt.write_static_object_key(out, false, "flags")?;
        crate::json_fmt_helper!(wrap_object_value, fmt, out, {
          crate::json_fmt_helper!(wrap_array, fmt, out, {
            for (i, flag) in fragment.flags().iter().enumerate() {
              crate::json_fmt_helper!(wrap_array_value, fmt, out, i == 0, {
                fmt.write_escaped_string(out, flag)?;
              });
            }
          });
        });

        fmt.write_static_object_key(out, false, "text")?;
        crate::json_fmt_helper!(wrap_object_value, fmt, out, {
          crate::json_fmt_helper!(wrap_object, fmt, out, {
            for (i, (locale, text)) in fragment.text().iter().enumerate() {
              crate::json_fmt_helper!(wrap_object_key, fmt, out, i == 0, {
                fmt.write_escaped_string(out, locale)?;
              });
              crate::json_fmt_helper!(wrap_object_value, fmt, out, {
                fmt.write_escaped_string(out, text)?;
              });
            }
          });
        });
      });

      stream_helper.end_value(fmt, out)?;
    }
  }

  stream_helper.end(fmt, out)?;

  Ok(())
}
