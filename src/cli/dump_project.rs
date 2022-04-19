use super::dump_common as common;
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

      crate::json_fmt_helper!(wrap_object, fmt, out, {
        fmt.write_static_object_key(out, true, "id")?;
        let bytes = utils::encode_compact_uuid(&fragment.id());
        fmt.write_escaped_string_object_value(out, str::from_utf8(&bytes).unwrap())?;

        fmt.write_static_object_key(out, false, "tr_file_id")?;
        let bytes = utils::encode_compact_uuid(&tr_file.id());
        fmt.write_escaped_string_object_value(out, str::from_utf8(&bytes).unwrap())?;

        fmt.write_static_object_key(out, false, "tr_file_path")?;
        fmt.write_escaped_string_object_value(out, tr_file.relative_path())?;

        fmt.write_static_object_key(out, false, "file_asset_root")?;
        fmt.write_escaped_string_object_value(out, game_file.asset_root())?;

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

        fmt.write_static_object_key(out, false, "original_text")?;
        fmt.write_escaped_string_object_value(out, fragment.original_text())?;

        fmt.write_static_object_key(out, false, "reference_texts")?;
        crate::json_fmt_helper!(wrap_object_value, fmt, out, {
          crate::json_fmt_helper!(wrap_object, fmt, out, {
            for (i, (locale, text)) in fragment.reference_texts().iter().enumerate() {
              crate::json_fmt_helper!(wrap_object_key, fmt, out, i == 0, {
                fmt.write_escaped_string(out, locale)?;
              });
              crate::json_fmt_helper!(wrap_object_value, fmt, out, {
                fmt.write_escaped_string(out, text)?;
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

        fmt.write_static_object_key(out, false, "translations")?;
        crate::json_fmt_helper!(wrap_object_value, fmt, out, {
          crate::json_fmt_helper!(wrap_array, fmt, out, {
            for (i, translation) in fragment.translations().iter().enumerate() {
              crate::json_fmt_helper!(wrap_array_value, fmt, out, i == 0, {
                crate::json_fmt_helper!(wrap_object, fmt, out, {
                  fmt.write_static_object_key(out, true, "id")?;
                  let bytes = utils::encode_compact_uuid(&translation.id());
                  fmt.write_escaped_string_object_value(out, str::from_utf8(&bytes).unwrap())?;

                  fmt.write_static_object_key(out, false, "author_username")?;
                  fmt.write_escaped_string_object_value(out, translation.author_username())?;

                  fmt.write_static_object_key(out, false, "editor_username")?;
                  fmt.write_escaped_string_object_value(out, &translation.editor_username())?;

                  fmt.write_static_object_key(out, false, "creation_timestamp")?;
                  crate::json_fmt_helper!(wrap_object_value, fmt, out, {
                    fmt.write_i64(out, translation.creation_timestamp())?;
                  });

                  fmt.write_static_object_key(out, false, "modification_timestamp")?;
                  crate::json_fmt_helper!(wrap_object_value, fmt, out, {
                    fmt.write_i64(out, translation.modification_timestamp())?;
                  });

                  fmt.write_static_object_key(out, false, "text")?;
                  fmt.write_escaped_string_object_value(out, &translation.text())?;

                  fmt.write_static_object_key(out, false, "flags")?;
                  crate::json_fmt_helper!(wrap_object_value, fmt, out, {
                    crate::json_fmt_helper!(wrap_array, fmt, out, {
                      for (i, flag) in translation.flags().iter().enumerate() {
                        crate::json_fmt_helper!(wrap_array_value, fmt, out, i == 0, {
                          fmt.write_escaped_string(out, flag)?;
                        });
                      }
                    });
                  });
                });
              });
            }
          });
        });

        fmt.write_static_object_key(out, false, "comments")?;
        crate::json_fmt_helper!(wrap_object_value, fmt, out, {
          crate::json_fmt_helper!(wrap_array, fmt, out, {
            for (i, comment) in fragment.comments().iter().enumerate() {
              crate::json_fmt_helper!(wrap_array_value, fmt, out, i == 0, {
                crate::json_fmt_helper!(wrap_object, fmt, out, {
                  fmt.write_static_object_key(out, true, "id")?;
                  let bytes = utils::encode_compact_uuid(&comment.id());
                  fmt.write_escaped_string_object_value(out, str::from_utf8(&bytes).unwrap())?;

                  fmt.write_static_object_key(out, false, "author_username")?;
                  fmt.write_escaped_string_object_value(out, comment.author_username())?;

                  fmt.write_static_object_key(out, false, "editor_username")?;
                  fmt.write_escaped_string_object_value(out, &comment.editor_username())?;

                  fmt.write_static_object_key(out, false, "creation_timestamp")?;
                  crate::json_fmt_helper!(wrap_object_value, fmt, out, {
                    fmt.write_i64(out, comment.creation_timestamp())?;
                  });

                  fmt.write_static_object_key(out, false, "modification_timestamp")?;
                  crate::json_fmt_helper!(wrap_object_value, fmt, out, {
                    fmt.write_i64(out, comment.modification_timestamp())?;
                  });

                  fmt.write_static_object_key(out, false, "text")?;
                  fmt.write_escaped_string_object_value(out, &comment.text())?;
                });
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
