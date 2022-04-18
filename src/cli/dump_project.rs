use super::dump_common::{self as common, write_static_object_key};
use crate::impl_prelude::*;
use crate::progress::ProgressReporter;
use crate::project;
use crate::utils;
use crate::utils::json;

use serde_json::ser::Formatter;
use std::io;
use std::path::PathBuf;
use std::rc::Rc;
use std::str;

#[derive(Debug)]
pub struct DumpProjectCommand;

impl super::Command for DumpProjectCommand {
  fn name(&self) -> &'static str { "dump-project" }

  fn create_arg_parser<'help>(&self, app: clap::Command<'help>) -> clap::Command<'help> {
    common::DumpCommandCommonOpts::add_to_arg_parser(
      app
        .about(
          "Dumps a project to JSON in a structure that is easy to process with jq (see \
          <https://stedolan.github.io/jq/>) on the command-line.",
        )
        .arg(
          clap::Arg::new("project_dir")
            .value_name("PROJECT")
            .value_hint(clap::ValueHint::DirPath)
            .allow_invalid_utf8(true)
            .required(true)
            .help("Path to the project directory."),
        ),
    )
  }

  fn run(
    &self,
    _global_opts: super::GlobalOpts,
    matches: &clap::ArgMatches,
    _progress: Box<dyn ProgressReporter>,
  ) -> AnyResult<()> {
    let opt_project_dir = PathBuf::from(matches.value_of_os("project_dir").unwrap());
    let common_opt = common::DumpCommandCommonOpts::from_matches(matches);

    let mut fmt = json::UltimateFormatter::new(common_opt.ultimate_formatter_config());
    let mut out = io::stdout();
    let mut stream_helper = common_opt.dump_stream_helper();

    let project = project::Project::open(opt_project_dir).context("Failed to open the projet")?;

    match dump_the_project(project, &mut fmt, &mut out, &mut stream_helper) {
      Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {}
      r => r?,
    };

    Ok(())
  }
}

pub fn dump_the_project(
  project: Rc<project::Project>,
  fmt: &mut json::UltimateFormatter,
  out: &mut (impl io::Write + ?Sized),
  stream_helper: &mut common::DumpStreamHelper,
) -> io::Result<()> {
  stream_helper.begin(fmt, out)?;

  for game_file in project.virtual_game_files().values() {
    for fragment in game_file.fragments().values() {
      let tr_file = fragment.tr_file();
      stream_helper.begin_value(fmt, out)?;
      {
        fmt.begin_object(out)?;

        write_static_object_key(fmt, out, true, "id")?;
        fmt.begin_object_value(out)?;
        {
          let bytes = utils::encode_compact_uuid(&fragment.id());
          json::format_escaped_str(out, fmt, str::from_utf8(&bytes).unwrap())?;
        }
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "tr_file_id")?;
        fmt.begin_object_value(out)?;
        {
          let bytes = utils::encode_compact_uuid(&tr_file.id());
          json::format_escaped_str(out, fmt, str::from_utf8(&bytes).unwrap())?;
        }
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "tr_file_path")?;
        fmt.begin_object_value(out)?;
        json::format_escaped_str(out, fmt, tr_file.relative_path())?;
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "file_asset_root")?;
        fmt.begin_object_value(out)?;
        json::format_escaped_str(out, fmt, game_file.asset_root())?;
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

        write_static_object_key(fmt, out, false, "original_text")?;
        fmt.begin_object_value(out)?;
        json::format_escaped_str(out, fmt, fragment.original_text())?;
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "reference_texts")?;
        fmt.begin_object_value(out)?;
        {
          fmt.begin_object(out)?;
          let mut first = true;
          for (locale, text) in fragment.reference_texts().iter() {
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

        write_static_object_key(fmt, out, false, "translations")?;
        fmt.begin_object_value(out)?;
        {
          fmt.begin_array(out)?;
          let mut first = true;
          for translation in fragment.translations().iter() {
            fmt.begin_array_value(out, first)?;
            {
              fmt.begin_object(out)?;

              write_static_object_key(fmt, out, true, "id")?;
              fmt.begin_object_value(out)?;
              {
                let bytes = utils::encode_compact_uuid(&translation.id());
                json::format_escaped_str(out, fmt, str::from_utf8(&bytes).unwrap())?;
              }
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "author_username")?;
              fmt.begin_object_value(out)?;
              json::format_escaped_str(out, fmt, translation.author_username())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "editor_username")?;
              fmt.begin_object_value(out)?;
              json::format_escaped_str(out, fmt, &translation.editor_username())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "creation_timestamp")?;
              fmt.begin_object_value(out)?;
              fmt.write_i64(out, translation.creation_timestamp())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "modification_timestamp")?;
              fmt.begin_object_value(out)?;
              fmt.write_i64(out, translation.modification_timestamp())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "text")?;
              fmt.begin_object_value(out)?;
              json::format_escaped_str(out, fmt, &translation.text())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "flags")?;
              fmt.begin_object_value(out)?;
              {
                fmt.begin_array(out)?;
                let mut first = true;
                for flag in translation.flags().iter() {
                  fmt.begin_array_value(out, first)?;
                  json::format_escaped_str(out, fmt, flag)?;
                  fmt.end_array_value(out)?;
                  first = false;
                }
                fmt.end_array(out)?;
              }
              fmt.end_object_value(out)?;

              fmt.end_object(out)?;
            }
            fmt.end_array_value(out)?;
            first = false;
          }
          fmt.end_array(out)?;
        }
        fmt.end_object_value(out)?;

        write_static_object_key(fmt, out, false, "comments")?;
        fmt.begin_object_value(out)?;
        {
          fmt.begin_array(out)?;
          let mut first = true;
          for comment in fragment.comments().iter() {
            fmt.begin_array_value(out, first)?;
            {
              fmt.begin_object(out)?;

              write_static_object_key(fmt, out, true, "id")?;
              fmt.begin_object_value(out)?;
              {
                let bytes = utils::encode_compact_uuid(&comment.id());
                json::format_escaped_str(out, fmt, str::from_utf8(&bytes).unwrap())?;
              }
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "author_username")?;
              fmt.begin_object_value(out)?;
              json::format_escaped_str(out, fmt, comment.author_username())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "editor_username")?;
              fmt.begin_object_value(out)?;
              json::format_escaped_str(out, fmt, &comment.editor_username())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "creation_timestamp")?;
              fmt.begin_object_value(out)?;
              fmt.write_i64(out, comment.creation_timestamp())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "modification_timestamp")?;
              fmt.begin_object_value(out)?;
              fmt.write_i64(out, comment.modification_timestamp())?;
              fmt.end_object_value(out)?;

              write_static_object_key(fmt, out, false, "text")?;
              fmt.begin_object_value(out)?;
              json::format_escaped_str(out, fmt, &comment.text())?;
              fmt.end_object_value(out)?;

              fmt.end_object(out)?;
            }
            fmt.end_array_value(out)?;
            first = false;
          }
          fmt.end_array(out)?;
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
