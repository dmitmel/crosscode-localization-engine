use crate::utils::json;

use serde_json::ser::Formatter;
use std::io;

#[derive(Debug)]
pub struct DumpCommandCommonOpts {
  pub compact_output: bool,
  pub indent: String,
  pub unbuffered: bool,
  pub wrap_array: bool,
}

impl DumpCommandCommonOpts {
  pub fn add_only_formatting_to_arg_parser(app: clap::Command) -> clap::Command {
    app
      .arg(
        clap::Arg::new("compact_output")
          .long("compat-output")
          .short('c')
          //
          .help(
            "Does exactly the same thing as jq's option of the same name: turns off pretty-\
            printing of the resulting JSON.",
          ),
      )
      .arg(
        clap::Arg::new("indent")
          .value_name("INDENT")
          .value_hint(clap::ValueHint::Other)
          .long("indent")
          .help("Selects what to use for indentation.")
          .possible_values(&["0", "1", "2", "3", "4", "5", "6", "7", "8", "tab"])
          .default_value("2"),
      )
  }

  pub fn add_to_arg_parser(app: clap::Command) -> clap::Command {
    Self::add_only_formatting_to_arg_parser(app)
      .arg(
        clap::Arg::new("unbuffered")
          .long("unbuffered")
          //
          .help(
            "Does exactly the same thing as the corresponding jq's option: flushes the output \
            stream after each JSON object is printed.",
          ),
      )
      .arg(
        clap::Arg::new("wrap_array")
          .long("wrap-array")
          .short('w')
          //
          .help(
            "Wrap the resulting JSON entries in a one big array. Alternatively, jq's --slurp \
            option can be used to achieve the same.",
          ),
      )
  }

  pub fn from_matches(matches: &clap::ArgMatches) -> Self {
    Self {
      compact_output: matches.is_present("compact_output"),
      indent: matches.value_of("indent").unwrap().to_owned(),
      unbuffered: matches.is_present("unbuffered"),
      wrap_array: matches.is_present("wrap_array"),
    }
  }

  pub fn ultimate_formatter_config(&self) -> json::UltimateFormatterConfig {
    json::UltimateFormatterConfig {
      compact: self.compact_output,
      indent: if self.compact_output {
        None
      } else {
        Some(match self.indent.as_str() {
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
    }
  }

  pub fn dump_stream_helper(&self) -> DumpStreamHelper {
    DumpStreamHelper {
      unbuffered: self.unbuffered,
      wrap_array: self.wrap_array,
      is_first_element: false,
    }
  }
}

#[derive(Debug)]
pub struct DumpStreamHelper {
  pub unbuffered: bool,
  pub wrap_array: bool,
  pub is_first_element: bool,
}

impl DumpStreamHelper {
  pub fn begin(
    &mut self,
    fmt: &mut json::UltimateFormatter,
    out: &mut (impl io::Write + ?Sized),
  ) -> io::Result<()> {
    if self.wrap_array {
      fmt.begin_array(out)?;
    }
    self.is_first_element = true;
    Ok(())
  }

  pub fn begin_value(
    &mut self,
    fmt: &mut json::UltimateFormatter,
    out: &mut (impl io::Write + ?Sized),
  ) -> io::Result<()> {
    if self.wrap_array {
      fmt.begin_array_value(out, self.is_first_element)?;
    }
    self.is_first_element = false;
    Ok(())
  }

  pub fn end_value(
    &mut self,
    fmt: &mut json::UltimateFormatter,
    out: &mut (impl io::Write + ?Sized),
  ) -> io::Result<()> {
    if self.wrap_array {
      fmt.end_array_value(out)?;
    } else {
      out.write_all(b"\n")?;
    }
    if self.unbuffered {
      out.flush()?;
    }
    Ok(())
  }

  pub fn end(
    &mut self,
    fmt: &mut json::UltimateFormatter,
    out: &mut (impl io::Write + ?Sized),
  ) -> io::Result<()> {
    if self.wrap_array {
      fmt.end_array(out)?;
      out.write_all(b"\n")?;
    }
    out.flush()?;
    Ok(())
  }
}

pub fn write_static_object_key(
  fmt: &mut json::UltimateFormatter,
  out: &mut (impl io::Write + ?Sized),
  first: bool,
  key: &'static str,
) -> io::Result<()> {
  fmt.begin_object_key(out, first)?;
  fmt.begin_string(out)?;
  fmt.write_string_fragment(out, key)?;
  fmt.end_string(out)?;
  fmt.end_object_key(out)?;
  Ok(())
}
