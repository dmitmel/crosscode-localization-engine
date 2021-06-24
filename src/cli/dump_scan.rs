use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use crate::scan;
use crate::utils::json;

use serde_json::ser::Formatter;
use std::io::{self, Write};
use std::path::PathBuf;

#[derive(Debug)]
pub struct DumpScanCommand;

inventory::submit!(&DumpScanCommand as &dyn super::Command);

impl super::Command for DumpScanCommand {
  fn name(&self) -> &'static str { "dump-scan" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about(
        "Dumps a scan database to JSON in such a structure that is easy to process with jq (see \
        <https://stedolan.github.io/jq/>) on the command-line.",
      )
      .arg(
        clap::Arg::new("scan_db")
          .value_name("SCAN_DB_PATH")
          .value_hint(clap::ValueHint::FilePath)
          .required(true)
          .about("Path to a scan database to dump."),
      )
      .arg(
        clap::Arg::new("compact_output")
          .long("compat-output")
          .short('c')
          //
          .about(
            "Does exactly the same thing as jq's option of the same name: turns off pretty-\
            printing of the resulting JSON.",
          ),
      )
      .arg(
        clap::Arg::new("indent")
          .value_name("INDENT")
          .value_hint(clap::ValueHint::Other)
          .long("indent")
          .about("Selects what to use for indentation.")
          .possible_values(&["0", "1", "2", "3", "4", "5", "6", "7", "8", "tab"])
          .default_value("2"),
      )
      .arg(
        clap::Arg::new("unbuffered")
          .long("unbuffered")
          //
          .about(
            "Does exactly the same thing as the corresponding jq's option: flushes the output \
            stream after each JSON object is printed.",
          ),
      )
      .arg(
        clap::Arg::new("wrap_array")
          .long("wrap-array")
          .short('w')
          //
          .about(
            "Wrap the resulting JSON entries in a one big array. Alternatively, jq's --slurp \
            option can be used to achieve the same.",
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
    let opt_compact_output = matches.is_present("compact_output");
    let opt_indent = matches.value_of("indent").unwrap();
    let opt_unbuffered = matches.is_present("unbuffered");
    let opt_wrap_array = matches.is_present("wrap_array");

    let fmt = &mut json::UltimateFormatter::new(json::UltimateFormatterConfig {
      compact: opt_compact_output,
      indent: if opt_compact_output {
        None
      } else {
        Some(match opt_indent {
          "0" => "",
          "1" => " ",
          "2" => "  ",
          "3" => "   ",
          "4" => "    ",
          "5" => "     ",
          "6" => "      ",
          "7" => "       ",
          "8" => "        ",
          "tab" => "\t",
          _ => unreachable!(),
        })
      },
      ..Default::default()
    });
    let out = &mut io::stdout();

    let scan_db = scan::ScanDb::open(opt_scan_db).context("Failed to open the scan database")?;

    if opt_wrap_array {
      fmt.begin_array(out)?;
    }
    let mut first_fragment = true;

    for game_file in scan_db.game_files().values() {
      for fragment in game_file.fragments().values() {
        if opt_wrap_array {
          fmt.begin_array_value(out, first_fragment)?;
        }
        first_fragment = false;

        fn write_static_object_key(
          fmt: &mut json::UltimateFormatter,
          out: &mut impl Write,
          first: bool,
          key: &'static str,
        ) -> io::Result<()> {
          fmt.begin_object_key(out, first)?;
          {
            fmt.begin_string(out)?;
            fmt.write_string_fragment(out, key)?;
            fmt.end_string(out)?;
          }
          fmt.end_object_key(out)?;
          Ok(())
        }

        fmt.begin_object(out)?;

        {
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
              first = false;
              json::format_escaped_str(out, fmt, locale)?;
              fmt.end_object_key(out)?;

              fmt.begin_object_value(out)?;
              json::format_escaped_str(out, fmt, text)?;
              fmt.end_object_value(out)?;
            }
            fmt.end_object(out)?;
          }
          fmt.end_object_value(out)?;
        }

        fmt.end_object(out)?;

        if opt_wrap_array {
          fmt.end_array_value(out)?;
        } else {
          out.write_all(b"\n")?;
        }

        if opt_unbuffered {
          out.flush()?;
        }
      }
    }

    if opt_wrap_array {
      fmt.end_array(out)?;
    }

    Ok(())
  }
}
