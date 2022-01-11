use super::{Fragment, ProjectMeta, Translation};
use crate::gettext_po;
use crate::impl_prelude::*;
use crate::localize_me;
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::{self, RcExt, Timestamp};

use once_cell::sync::Lazy;
use serde_json::ser::Formatter;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::{self, Write};
use std::rc::Rc;
use uuid::Uuid;

#[derive(Debug)]
pub struct ExporterConfig {
  pub compact: bool,
}

// assert_trait_is_object_safe!(ExportedProjectMeta);
// pub trait ExportedProjectMeta: fmt::Debug {
//   #[inline(always)]
//   fn id(&self) -> Option<Uuid> { None }
//   #[inline(always)]
//   fn creation_timestamp(&self) -> Option<Timestamp> { None }
//   #[inline(always)]
//   fn modification_timestamp(&self) -> Option<Timestamp> { None }
//   #[inline(always)]
//   fn game_version(&self) -> Option<RcString> { None }
//   #[inline(always)]
//   fn original_locale(&self) -> Option<RcString> { None }
//   #[inline(always)]
//   fn reference_locales(&self) -> Option<Rc<HashSet<RcString>>> { None }
//   #[inline(always)]
//   fn translation_locale(&self) -> Option<RcString> { None }
// }

// impl ExportedProjectMeta for ProjectMeta {
//   #[inline(always)]
//   fn id(&self) -> Option<Uuid> { Some(self.id()) }
//   #[inline(always)]
//   fn creation_timestamp(&self) -> Option<Timestamp> { Some(self.creation_timestamp()) }
//   #[inline(always)]
//   fn modification_timestamp(&self) -> Option<Timestamp> { Some(self.modification_timestamp()) }
//   #[inline(always)]
//   fn game_version(&self) -> Option<RcString> { Some(self.game_version().share_rc()) }
//   #[inline(always)]
//   fn original_locale(&self) -> Option<RcString> { Some(self.original_locale().share_rc()) }
//   #[inline(always)]
//   fn reference_locales(&self) -> Option<Rc<HashSet<RcString>>> {
//     Some(self.reference_locales().share_rc())
//   }
//   #[inline(always)]
//   fn translation_locale(&self) -> Option<RcString> { Some(self.translation_locale().share_rc()) }
// }

#[derive(Debug)]
pub struct ExportedProjectMeta {
  pub id: Option<Uuid>,
  pub creation_timestamp: Option<Timestamp>,
  pub modification_timestamp: Option<Timestamp>,
  pub game_version: Option<RcString>,
  pub original_locale: Option<RcString>,
  pub reference_locales: Option<Rc<HashSet<RcString>>>,
  pub translation_locale: Option<RcString>,
}

impl ExportedProjectMeta {
  pub fn new(real_project_meta: &ProjectMeta) -> Self {
    let m = real_project_meta;
    Self {
      id: Some(m.id()),
      creation_timestamp: Some(m.creation_timestamp()),
      modification_timestamp: Some(m.modification_timestamp()),
      game_version: Some(m.game_version().share_rc()),
      original_locale: Some(m.original_locale().share_rc()),
      reference_locales: Some(m.reference_locales().share_rc()),
      translation_locale: Some(m.translation_locale().share_rc()),
    }
  }
}

#[derive(Debug)]
pub struct ExportedFragment {
  pub id: Option<Uuid>,
  pub file_path: RcString,
  pub json_path: RcString,
  pub lang_uid: Option<i32>,
  pub description: Option<Rc<Vec<RcString>>>,
  pub original_text: RcString,
  pub reference_texts: Option<Rc<HashMap<RcString, RcString>>>,
  pub flags: Option<Rc<HashSet<RcString>>>,
  pub best_translation: Option<ExportedTranslation>,
  pub translations: Vec<ExportedTranslation>,
}

impl ExportedFragment {
  pub fn new(real_fragment: &Fragment) -> Self {
    let f = real_fragment;
    Self {
      id: Some(f.id()),
      file_path: f.file_path().share_rc(),
      json_path: f.json_path().share_rc(),
      lang_uid: Some(f.lang_uid()),
      description: Some(f.description().share_rc()),
      original_text: f.original_text().share_rc(),
      reference_texts: Some(f.reference_texts().share_rc()),
      flags: Some(f.flags().share_rc()),
      best_translation: f.get_best_translation().map(|t| ExportedTranslation::new(&t)),
      translations: f.translations().iter().map(|t| ExportedTranslation::new(t)).collect(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct ExportedTranslation {
  pub id: Option<Uuid>,
  pub author_username: Option<RcString>,
  pub editor_username: Option<RcString>,
  pub creation_timestamp: Option<Timestamp>,
  pub modification_timestamp: Option<Timestamp>,
  pub text: RcString,
  pub flags: Option<Rc<HashSet<RcString>>>,
}

impl ExportedTranslation {
  pub fn new(real_translation: &Translation) -> Self {
    let t = real_translation;
    Self {
      id: Some(t.id()),
      author_username: Some(t.author_username().share_rc()),
      editor_username: Some(t.editor_username().share_rc()),
      creation_timestamp: Some(t.creation_timestamp()),
      modification_timestamp: Some(t.modification_timestamp()),
      text: t.text().share_rc(),
      flags: Some(t.flags().share_rc()),
    }
  }
}

pub type ExporterDeclaration = utils::StrategyDeclaration<ExporterConfig, Box<dyn Exporter>>;

assert_trait_is_object_safe!(Exporter);
pub trait Exporter: fmt::Debug + Send + Sync {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed(config: ExporterConfig) -> Box<dyn Exporter>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn file_extension(&self) -> &'static str;

  fn supports_multiple_translations_for_fragments(&self) -> bool;

  fn export(
    &mut self,
    project_meta: &ExportedProjectMeta,
    fragments: &[ExportedFragment],
    writer: &mut dyn Write,
  ) -> AnyResult<()>;

  fn declaration() -> ExporterDeclaration
  where
    Self: Sized,
  {
    ExporterDeclaration { id: Self::id_static(), ctor: |config| Ok(Self::new_boxed(config)) }
  }
}

inventory::collect!(ExporterDeclaration);

pub static REGISTRY: Lazy<utils::StrategicalRegistry<ExporterConfig, Box<dyn Exporter>>> =
  Lazy::new(utils::StrategicalRegistry::new);

#[derive(Debug)]
pub struct LocalizeMeTrPackExporter {
  json_fmt: json::UltimateFormatter,
}
inventory::submit!(LocalizeMeTrPackExporter::declaration());

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
        compact: config.compact,
        indent: if config.compact { None } else { Some(json::DEFAULT_INDENT) },
        ..Default::default()
      }),
    })
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  #[inline(always)]
  fn file_extension(&self) -> &'static str { "json" }

  #[inline(always)]
  fn supports_multiple_translations_for_fragments(&self) -> bool { false }

  fn export(
    &mut self,
    _project_meta: &ExportedProjectMeta,
    fragments: &[ExportedFragment],
    writer: &mut dyn Write,
  ) -> AnyResult<()> {
    let fmt = &mut self.json_fmt;

    fmt.begin_object(writer)?;
    let mut is_first_entry = true;
    for fragment in fragments {
      let translation_text = match &fragment.best_translation {
        Some(tr) => tr.text.share_rc(),
        None => RcString::from(""),
      };

      let localize_me_file_path = localize_me::serialize_file_path(&fragment.file_path);

      fmt.begin_object_key(writer, is_first_entry)?;
      is_first_entry = false;
      {
        fmt.begin_string(writer)?;
        json::format_escaped_str_contents(writer, fmt, localize_me_file_path)?;
        fmt.write_string_fragment(writer, "/")?;
        json::format_escaped_str_contents(writer, fmt, &fragment.json_path)?;
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
inventory::submit!(GettextPoExporter::declaration());

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

  #[inline(always)]
  fn supports_multiple_translations_for_fragments(&self) -> bool { false }

  #[allow(clippy::write_with_newline)]
  fn export(
    &mut self,
    project_meta: &ExportedProjectMeta,
    fragments: &[ExportedFragment],
    writer: &mut dyn Write,
  ) -> AnyResult<()> {
    fn write_po_string(writer: &mut dyn io::Write, text: &str) -> io::Result<()> {
      let resplit_text: Vec<&str> = utils::LinesWithEndings::new(text).collect();
      if resplit_text.len() != 1 {
        writer.write_all(b"\"\"\n")?;
      }
      for substr in resplit_text {
        writer.write_all(b"\"")?;
        let mut buf = String::new();
        gettext_po::escape_str(substr, &mut buf);
        writer.write_all(buf.as_bytes())?;
        writer.write_all(b"\"\n")?;
      }
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

    let metadata_block = utils::fast_concat_cow(&[
      if let Some(game_version) = &project_meta.game_version {
        format!("Project-Id-Version: crosscode {}\n", game_version).into()
      } else {
        "Project-Id-Version: crosscode\n".into()
      },
      "Report-Msgid-Bugs-To: \n".into(),
      if let Some(creation_timestamp) = project_meta.creation_timestamp {
        format!("POT-Creation-Date: {}+0000\n", format_po_timestamp(creation_timestamp)).into()
      } else {
        "POT-Creation-Date: \n".into()
      },
      if let Some(modification_timestamp) = project_meta.modification_timestamp {
        format!("POT-Revision-Date: {}+0000\n", format_po_timestamp(modification_timestamp)).into()
      } else {
        "POT-Revision-Date: \n".into()
      },
      "Last-Translator: \n".into(),
      "Language-Team: \n".into(),
      if let Some(translation_locale) = &project_meta.translation_locale {
        format!("Language: {}\n", translation_locale).into()
      } else {
        "Language: \n".into()
      },
      "MIME-Version: 1.0\n".into(),
      "Content-Type: text/plain; charset=UTF-8\n".into(),
      "Content-Transfer-Encoding: 8bit\n".into(),
      "Plural-Forms: \n".into(),
      format!("X-Generator: {} {}\n", crate::CRATE_NAME, crate::CRATE_NICE_VERSION).into(),
    ]);

    writer.write_all(b"msgid ")?;
    write_po_string(writer, "")?;
    writer.write_all(b"msgstr ")?;
    write_po_string(writer, &metadata_block)?;

    for fragment in fragments {
      // The empty msgid is reserved only for the very first entry in a po file
      // containing metadata.
      if fragment.original_text.is_empty() {
        continue;
      }

      let translation_text = match &fragment.best_translation {
        Some(tr) => tr.text.as_str(),
        None => "",
      };

      let location_line = format!(
        "{} {} #{}",
        fragment.file_path,
        fragment.json_path,
        fragment.lang_uid.unwrap_or(0)
      );

      writer.write_all(b"\n")?;

      write_po_comment(writer, "#. ", &location_line)?;
      if let Some(description) = &fragment.description {
        for line in description.iter() {
          write_po_comment(writer, "#. ", line)?;
        }
      }
      write_po_comment(writer, "#: ", &{
        let mut buf = String::new();
        gettext_po::encode_reference_comment_as_uri_for_weblate(&location_line, &mut buf);
        buf
      })?;

      writer.write_all(b"msgctxt ")?;
      write_po_string(
        writer,
        &utils::fast_concat(&[&fragment.file_path, "//", &fragment.json_path]),
      )?;
      writer.write_all(b"msgid ")?;
      write_po_string(writer, &fragment.original_text)?;
      writer.write_all(b"msgstr ")?;
      write_po_string(writer, translation_text)?;
    }

    Ok(())
  }
}
