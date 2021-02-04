use super::{Fragment, ProjectMeta};
use crate::gettext_po;
use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::{self, Timestamp};

use lazy_static::lazy_static;
use serde_json::ser::Formatter;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ExporterConfig {
  pub compact: bool,
}

pub trait Exporter: fmt::Debug {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed(config: ExporterConfig) -> Box<dyn Exporter>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn file_extension(&self) -> &'static str;

  fn export(
    &mut self,
    project_meta: &ProjectMeta,
    fragments: &[Rc<Fragment>],
    writer: &mut dyn Write,
  ) -> AnyResult<()>;
}

macro_rules! exporters_map {
  ($($impl:ident,)+) => { exporters_map![$($impl),+]; };
  ($($impl:ident),*) => {
    pub const EXPORTERS_IDS: &'static [&'static str] = &[$($impl::ID),+];
    lazy_static! {
      pub static ref EXPORTERS_MAP: HashMap<
        &'static str,
        fn(config: ExporterConfig) -> Box<dyn Exporter>,
      > = {
        let _cap = count_exprs!($($impl),*);
        // Don't ask me why the compiler requires the following type
        // annotation.
        let mut _map: HashMap<_, fn(config: ExporterConfig) -> _> = HashMap::with_capacity(_cap);
        $(let _ = _map.insert($impl::ID, $impl::new_boxed);)*
        _map
      };
    }
  };
}

exporters_map![LocalizeMeTrPackExporter, GettextPoExporter];

pub fn create(id: &str, config: ExporterConfig) -> AnyResult<Box<dyn Exporter>> {
  let constructor: &fn(config: ExporterConfig) -> Box<dyn Exporter> =
    EXPORTERS_MAP.get(id).ok_or_else(|| format_err!("no such exporter {:?}", id))?;
  Ok(constructor(config))
}

#[derive(Debug)]
pub struct LocalizeMeTrPackExporter {
  json_fmt: json::UltimateFormatter<'static>,
}

impl LocalizeMeTrPackExporter {
  pub const ID: &'static str = "lm-tr-pack";
}

impl Exporter for LocalizeMeTrPackExporter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed(config: ExporterConfig) -> Box<dyn Exporter>
  where
    Self: Sized,
  {
    Box::new(Self {
      json_fmt: json::UltimateFormatter::new(json::UltimateFormatterConfig {
        indent: if config.compact { None } else { Some(json::DEFAULT_INDENT) },
        ..Default::default()
      }),
    })
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  #[inline(always)]
  fn file_extension(&self) -> &'static str { "json" }

  fn export(
    &mut self,
    _project_meta: &ProjectMeta,
    fragments: &[Rc<Fragment>],
    writer: &mut dyn Write,
  ) -> AnyResult<()> {
    let fmt = &mut self.json_fmt;

    fmt.begin_object(writer)?;
    let mut is_first_entry = true;
    for fragment in fragments {
      let translation_text = match fragment.get_best_translation() {
        Some(tr) => tr.text().share_rc(),
        None => RcString::from(""),
      };

      let localize_me_file_path =
        fragment.file_path.strip_prefix("data/").unwrap_or(&fragment.file_path);

      fmt.begin_object_key(writer, is_first_entry)?;
      is_first_entry = false;
      {
        fmt.begin_string(writer)?;
        json::format_escaped_str_contents(writer, fmt, &localize_me_file_path)?;
        fmt.write_string_fragment(writer, "/")?;
        json::format_escaped_str_contents(writer, fmt, &fragment.file_path)?;
        fmt.end_string(writer)?;
      }
      fmt.end_object_key(writer)?;

      fmt.begin_object_value(writer)?;
      {
        fmt.begin_object(writer)?;

        {
          fmt.begin_object_key(writer, true)?;
          {
            fmt.begin_string(writer)?;
            fmt.write_string_fragment(writer, "orig")?;
            fmt.end_string(writer)?;
          }
          fmt.end_object_key(writer)?;
          fmt.begin_object_value(writer)?;
          {
            json::format_escaped_str(writer, fmt, &fragment.original_text)?;
          }
          fmt.end_object_value(writer)?;

          fmt.begin_object_key(writer, false)?;
          {
            fmt.begin_string(writer)?;
            fmt.write_string_fragment(writer, "text")?;
            fmt.end_string(writer)?;
          }
          fmt.end_object_key(writer)?;
          fmt.begin_object_value(writer)?;
          {
            json::format_escaped_str(writer, fmt, &translation_text)?;
          }
          fmt.end_object_value(writer)?;
        }

        fmt.end_object(writer)?;
      }
      fmt.end_object_value(writer)?;
    }
    fmt.end_object(writer)?;

    writer.write_all(b"\n")?;
    Ok(())
  }
}

#[derive(Debug)]
pub struct GettextPoExporter;

impl GettextPoExporter {
  pub const ID: &'static str = "po";
}

impl Exporter for GettextPoExporter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed(_config: ExporterConfig) -> Box<dyn Exporter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  #[inline(always)]
  fn file_extension(&self) -> &'static str { "po" }

  #[allow(clippy::write_with_newline)]
  fn export(
    &mut self,
    project_meta: &ProjectMeta,
    fragments: &[Rc<Fragment>],
    writer: &mut dyn Write,
  ) -> AnyResult<()> {
    fn write_po_string(writer: &mut dyn io::Write, text: &str) -> io::Result<()> {
      writer.write_all(b"\"")?;
      let mut buf = String::new();
      gettext_po::escape_str(text, &mut buf);
      writer.write_all(buf.as_bytes())?;
      writer.write_all(b"\"\n")?;
      Ok(())
    }

    fn write_po_comment(
      writer: &mut dyn Write,
      prefix: &'static str,
      text: &str,
    ) -> io::Result<()> {
      for line in text.lines() {
        writer.write_all(prefix.as_bytes())?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
      }
      Ok(())
    }

    fn format_po_timestamp(timestamp: Timestamp) -> impl fmt::Display {
      time::OffsetDateTime::from_unix_timestamp(timestamp).lazy_format("%Y-%m-%d %H:%M")
    }

    writer.write_all(b"msgid \"\"\n")?;
    writer.write_all(b"msgstr \"\"\n")?;
    write_po_string(
      writer,
      &format!("Project-Id-Version: crosscode {}\n", project_meta.game_version),
    )?;
    write_po_string(writer, "Report-Msgid-Bugs-To: \n")?;
    write_po_string(
      writer,
      &format!(
        "POT-Creation-Date: {}+0000\n",
        format_po_timestamp(project_meta.creation_timestamp)
      ),
    )?;
    write_po_string(
      writer,
      &format!(
        "PO-Revision-Date: {}+0000\n",
        format_po_timestamp(project_meta.modification_timestamp.get())
      ),
    )?;
    write_po_string(writer, "Last-Translator: \n")?;
    write_po_string(writer, "Language-Team: \n")?;
    write_po_string(writer, &format!("Language: {}\n", project_meta.translation_locale))?;
    write_po_string(writer, "MIME-Version: 1.0\n")?;
    write_po_string(writer, "Content-Type: text/plain; charset=UTF-8\n")?;
    write_po_string(writer, "Content-Transfer-Encoding: 8bit\n")?;
    write_po_string(writer, "Plural-Forms: \n")?;
    write_po_string(
      writer,
      &format!("X-Generator: {} {}\n", crate::CRATE_NAME, crate::CRATE_VERSION),
    )?;

    for fragment in fragments {
      // The empty msgid is reserved only for the very first entry in a po file
      // containing metadata.
      if fragment.original_text.is_empty() {
        continue;
      }

      let translation_text = match fragment.get_best_translation() {
        Some(tr) => tr.text().share_rc(),
        None => RcString::from(""),
      };

      let location_line =
        format!("{} {} #{}", fragment.file_path, fragment.json_path, fragment.lang_uid);

      writer.write_all(b"\n")?;

      write_po_comment(writer, "#. ", &location_line)?;
      for line in &fragment.description {
        write_po_comment(writer, "#. ", line)?;
      }
      write_po_comment(writer, "#: ", &{
        let mut buf = String::new();
        gettext_po::encode_reference_comment_as_uri_for_weblate(&location_line, &mut buf);
        buf
      })?;

      writer.write_all(b"msgctxt ")?;
      write_po_string(
        writer,
        &utils::fast_concat(&[&fragment.file_path, " ", &fragment.json_path]),
      )?;
      writer.write_all(b"msgid ")?;
      write_po_string(writer, &fragment.original_text)?;
      writer.write_all(b"msgstr ")?;
      write_po_string(writer, &translation_text)?;
    }

    Ok(())
  }
}
