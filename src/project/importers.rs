use crate::cc_ru_compat;
use crate::gettext_po;
use crate::impl_prelude::*;
use crate::localize_me;
use crate::rc_string::RcString;
use crate::utils;
use crate::utils::Timestamp;

use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::fmt;
use std::path::Path;

#[derive(Debug)]
pub struct ImportedFragment {
  pub file_path: RcString,
  pub json_path: RcString,
  pub original_text: RcString,
  pub translations: Vec<ImportedTranslation>,
}

#[derive(Debug)]
pub struct ImportedTranslation {
  pub author_username: Option<RcString>,
  pub editor_username: Option<RcString>,
  pub creation_timestamp: Option<Timestamp>,
  pub modification_timestamp: Option<Timestamp>,
  pub text: RcString,
  pub flags: HashSet<RcString>,
}

pub type ImporterDeclaration = utils::StrategyDeclaration<(), Box<dyn Importer>>;

assert_trait_is_object_safe!(Importer);
pub trait Importer: fmt::Debug + Send + Sync {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed() -> Box<dyn Importer>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn file_extension(&self) -> &'static str;

  fn supports_multiple_translations_for_fragments(&self) -> bool;

  fn import(
    &mut self,
    file_path: &Path,
    input: &str,
    imported_fragments: &mut Vec<ImportedFragment>,
  ) -> AnyResult<()>;

  fn declaration() -> ImporterDeclaration
  where
    Self: Sized,
  {
    ImporterDeclaration { id: Self::id_static(), ctor: |_| Ok(Self::new_boxed()) }
  }
}

pub static REGISTRY: Lazy<utils::StrategicalRegistry<(), Box<dyn Importer>>> = Lazy::new(|| {
  utils::StrategicalRegistry::new(&[
    LocalizeMeTrPackImporter::declaration(),
    CcRuChapterFragmentsImporter::declaration(),
    GettextPoImporter::declaration(),
  ])
});

#[derive(Debug)]
pub struct LocalizeMeTrPackImporter;

impl LocalizeMeTrPackImporter {
  pub const ID: &'static str = "lm-tr-pack";
}

impl Importer for LocalizeMeTrPackImporter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Importer>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  #[inline(always)]
  fn file_extension(&self) -> &'static str { "json" }

  #[inline(always)]
  fn supports_multiple_translations_for_fragments(&self) -> bool { false }

  fn import(
    &mut self,
    file_path: &Path,
    input: &str,
    imported_fragments: &mut Vec<ImportedFragment>,
  ) -> AnyResult<()> {
    let tr_pack: localize_me::TrPackSerde = serde_json::from_str(input)?;
    for (lm_file_dict_path, tr_pack_entry) in tr_pack.entries {
      let (lm_file_path, json_path) = match localize_me::parse_file_dict_path(&lm_file_dict_path) {
        Some(v) => v,
        None => {
          warn!("TrPack {:?}: Invalid file_dict_path_str: {:?}", file_path, lm_file_dict_path);
          continue;
        }
      };
      let file_path = localize_me::deserialize_file_path(lm_file_path);

      imported_fragments.push(ImportedFragment {
        file_path: RcString::from(file_path),
        json_path: RcString::from(json_path),
        original_text: RcString::from(tr_pack_entry.orig),
        translations: vec![ImportedTranslation {
          author_username: None,
          editor_username: None,
          creation_timestamp: None,
          modification_timestamp: None,
          text: RcString::from(tr_pack_entry.text),
          flags: HashSet::new(),
        }],
      });
    }

    Ok(())
  }
}

#[derive(Debug)]
pub struct CcRuChapterFragmentsImporter;

impl CcRuChapterFragmentsImporter {
  pub const ID: &'static str = "cc-ru-chapter-fragments";
}

impl Importer for CcRuChapterFragmentsImporter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Importer>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  #[inline(always)]
  fn file_extension(&self) -> &'static str { "json" }

  #[inline(always)]
  fn supports_multiple_translations_for_fragments(&self) -> bool { false }

  fn import(
    &mut self,
    _file_path: &Path,
    input: &str,
    imported_fragments: &mut Vec<ImportedFragment>,
  ) -> AnyResult<()> {
    let chapter_fragments: cc_ru_compat::ChapterFragmentsFileSerde = serde_json::from_str(input)?;
    for fragment in chapter_fragments.fragments {
      imported_fragments.push(ImportedFragment {
        file_path: RcString::from(fragment.original.file),
        json_path: RcString::from(fragment.original.json_path),
        original_text: RcString::from(fragment.original.text),

        translations: fragment
          .translations
          .into_iter()
          .map(|t| ImportedTranslation {
            author_username: Some(RcString::from(t.author_username)),
            editor_username: None,
            creation_timestamp: Some(t.timestamp),
            modification_timestamp: Some(t.timestamp),
            text: RcString::from(t.text),
            flags: {
              let mut flags = HashSet::with_capacity(t.flags.len());
              for (k, v) in t.flags {
                if v {
                  flags.insert(RcString::from(k));
                }
              }
              flags
            },
          })
          .collect(),
      });
    }
    Ok(())
  }
}

#[derive(Debug)]
pub struct GettextPoImporter;

impl GettextPoImporter {
  pub const ID: &'static str = "po";
}

impl Importer for GettextPoImporter {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Importer>
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

  fn import(
    &mut self,
    file_path: &Path,
    input: &str,
    imported_fragments: &mut Vec<ImportedFragment>,
  ) -> AnyResult<()> {
    for (i, message) in gettext_po::parse(input).enumerate() {
      let message = match message {
        Ok(v) => v,
        Err(e) => {
          bail!("{}", e.nice_formatter(&file_path.file_name().unwrap().to_string_lossy(), input))
        }
      };
      let msgctxt = utils::concat_strings(&message.msgctxt);
      let msgid = utils::concat_strings(&message.msgid);
      let msgstr = utils::concat_strings(&message.msgstr);
      if msgid.is_empty() || msgctxt.is_empty() {
        continue;
      }

      let (file_path, json_path) = match msgctxt.find("//") {
        Some(msgctxt_sep_index) => {
          (&msgctxt[..msgctxt_sep_index], &msgctxt[msgctxt_sep_index + 2..])
        }
        None => {
          warn!(
            "PO message #{} in {:?}: Invalid file_dict_path_str: {:?}",
            i + 1,
            file_path,
            msgctxt,
          );
          continue;
        }
      };

      imported_fragments.push(ImportedFragment {
        file_path: RcString::from(file_path),
        json_path: RcString::from(json_path),
        original_text: RcString::from(msgid),
        translations: if !msgstr.is_empty() {
          vec![ImportedTranslation {
            author_username: None,
            editor_username: None,
            creation_timestamp: None,
            modification_timestamp: None,
            text: RcString::from(msgstr),
            flags: HashSet::new(),
          }]
        } else {
          Vec::new()
        },
      });
    }
    Ok(())
  }
}
