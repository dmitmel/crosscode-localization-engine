use crate::gettext_po::{self, ParsedMessage};
use crate::impl_prelude::*;
use crate::utils;
use crate::utils::json;
use crate::utils::parsing::ParsingError;

use std::borrow::Cow;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

#[derive(Debug)]
pub struct ParsePoCommand;

impl super::Command for ParsePoCommand {
  fn name(&self) -> &'static str { "parse-po" }

  fn create_arg_parser<'help>(&self, app: clap::App<'help>) -> clap::App<'help> {
    app
      .about("Debug command for testing the gettext po parser.")
      .setting(clap::AppSettings::Hidden)
      .arg(clap::Arg::new("file").value_name("FILE"))
      .arg(clap::Arg::new("json").short('J').long("json"))
  }

  fn run(&self, _global_opts: super::GlobalOpts, matches: &clap::ArgMatches) -> AnyResult<()> {
    let opt_file = matches.value_of("file").map(PathBuf::from);
    let opt_json = matches.is_present("json");

    let (src, filename): (String, Cow<str>) = match &opt_file {
      Some(file) => (fs::read_to_string(file)?, file.to_string_lossy()),
      None => {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf)?;
        (buf, "<stdin>".into())
      }
    };

    let iter = gettext_po::parse(&src).filter_map(
      |message: Result<ParsedMessage, ParsingError>| -> Option<ParsedMessage> {
        match message {
          Ok(message) => Some(message),
          Err(e) => {
            error!("{}", e.nice_formatter(&filename, &src));
            None
          }
        }
      },
    );
    if opt_json {
      print_messages_json(iter)
    } else {
      print_messages_po(iter)
    }
  }
}

fn print_messages_json<'src>(iter: impl Iterator<Item = ParsedMessage<'src>>) -> AnyResult<()> {
  for message in iter {
    let mut message_obj = json::Map::new();

    let mut add_comments = |name: &'static str, comments: &[Cow<str>]| {
      if !comments.is_empty() {
        message_obj.insert(
          name.to_owned(),
          json::Value::Array(
            comments.iter().map(|s| json::Value::String(s.clone().into_owned())).collect(),
          ),
        );
      }
    };

    add_comments("translator_comments", &message.translator_comments);
    add_comments("automatic_comments", &message.automatic_comments);
    add_comments("reference_comments", &message.reference_comments);
    add_comments("flags_comments", &message.flags_comments);

    let mut add_section = |name: &'static str, strings: &[Cow<str>]| {
      let joined_string = utils::fast_concat_cow(strings);
      if !joined_string.is_empty() {
        message_obj.insert(name.to_owned(), json::Value::String(joined_string));
      }
    };

    add_section("prev_msgctxt", &message.prev_msgctxt);
    add_section("prev_msgid", &message.prev_msgid);
    add_section("msgctxt", &message.msgctxt);
    add_section("msgid", &message.msgid);
    add_section("msgstr", &message.msgstr);

    let message_obj = json::Value::Object(message_obj);

    let mut stdout = io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &message_obj)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
  }

  Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn print_messages_po<'src>(iter: impl Iterator<Item = ParsedMessage<'src>>) -> AnyResult<()> {
  let mut is_first_message = true;
  for message in iter {
    if is_first_message {
      is_first_message = false;
    } else {
      println!();
    }

    let print_comments = |prefix: &'static str, comments: &[Cow<str>]| {
      for comment in comments {
        println!("{}{}", prefix, comment);
      }
    };

    print_comments("#", &message.translator_comments);
    print_comments("#.", &message.automatic_comments);
    print_comments("#:", &message.reference_comments);
    print_comments("#,", &message.flags_comments);

    let print_section =
      |prefix: &'static str, keyword: &'static str, text_strings: &[Cow<str>]| {
        if text_strings.is_empty() {
          return;
        }
        let mut joined_string = String::new();
        let text_strings = resplit_po_string(text_strings, &mut joined_string);

        fn quote_string(string: &str) -> String { serde_json::to_string(string).unwrap() }

        println!(
          "{}{} {}",
          prefix,
          keyword,
          if text_strings.len() == 1 { quote_string(&text_strings[0]) } else { "\"\"".to_owned() }
        );

        if text_strings.len() > 1 {
          for string in text_strings {
            println!("{}{}", prefix, quote_string(string));
          }
        }
      };

    print_section("#| ", "msgctxt", &message.prev_msgctxt);
    print_section("#| ", "msgid", &message.prev_msgid);
    print_section("", "msgctxt", &message.msgctxt);
    print_section("", "msgid", &message.msgid);
    print_section("", "msgstr", &message.msgstr);
  }

  Ok(())
}

fn resplit_po_string<'a>(strings: &[Cow<str>], out_joined_string: &'a mut String) -> Vec<&'a str> {
  *out_joined_string = utils::fast_concat_cow(strings);
  utils::LinesWithEndings::new(out_joined_string).collect()
}
