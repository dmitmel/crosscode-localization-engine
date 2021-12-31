use super::dump_common::{self as common, write_static_object_key};
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

inventory::submit!(&DumpScanCommand as &dyn super::Command);

impl super::Command for DumpScanCommand {
  fn name(&self) -> &'static str { "dump-scan" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
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
            .setting(clap::ArgSettings::AllowInvalidUtf8)
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
      {
        fmt.begin_object(out)?;

        write_static_object_key(fmt, out, true, "file_asset_root")?;
        fmt.begin_object_value(out)?;
        json::format_escaped_str(out, fmt, fragment.file_asset_root())?;
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "file_path")?;
        fmt.begin_object_value(out)?;
        json::format_escaped_str(out, fmt, fragment.file_path())?;
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "json_path")?;
        fmt.begin_object_value(out)?;
        json::format_escaped_str(out, fmt, fragment.json_path())?;
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "lang_uid")?;
        fmt.begin_object_value(out)?;
        fmt.write_i32(out, fragment.lang_uid())?;
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "description")?;
        fmt.begin_object_value(out)?;
        {
          fmt.begin_array(out)?;
          let mut first = true;
          for line in fragment.description().iter() {
            fmt.begin_array_value(out, first)?;
            json::format_escaped_str(out, fmt, line)?;
            fmt.end_array_value(out)?;
            first = false;
          }
          fmt.end_array(out)?;
        }
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "flags")?;
        fmt.begin_object_value(out)?;
        {
          fmt.begin_array(out)?;
          let mut first = true;
          for flag in fragment.flags().iter() {
            fmt.begin_array_value(out, first)?;
            json::format_escaped_str(out, fmt, flag)?;
            fmt.end_array_value(out)?;
            first = false;
          }
          fmt.end_array(out)?;
        }
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "text")?;
        fmt.begin_object_value(out)?;
        {
          fmt.begin_object(out)?;
          let mut first = true;
          for (locale, text) in fragment.text().iter() {
            fmt.begin_object_key(out, first)?;
            json::format_escaped_str(out, fmt, locale)?;
            fmt.end_object_key(out)?;
            fmt.begin_object_value(out)?;
            json::format_escaped_str(out, fmt, text)?;
            fmt.end_object_value(out)?;
            first = false;
          }
          fmt.end_object(out)?;
        }
        fmt.end_object_value(out)?;

        fmt.end_object(out)?;
      }
      stream_helper.end_value(fmt, out)?;
    }
  }

  stream_helper.end(fmt, out)?;

  Ok(())
}
