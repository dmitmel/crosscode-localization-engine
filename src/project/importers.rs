use crate::cc_ru_compat;
use crate::impl_prelude::*;
use crate::localize_me;
use crate::rc_string::RcString;
use crate::utils::Timestamp;

use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportedFragment {
  pub file_path: RcString,
  pub json_path: RcString,
  pub original_text: RcString,
  pub translations: Vec<ImportedTranslation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportedTranslation {
  pub author_username: Option<RcString>,
  pub creation_timestamp: Option<Timestamp>,
  pub text: RcString,
  pub flags: HashSet<RcString>,
}

pub trait Importer: fmt::Debug {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed() -> Box<dyn Importer>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn file_extension(&self) -> &'static str;

  fn import(
    &mut self,
    input: &str,
    imported_fragments: &mut Vec<ImportedFragment>,
  ) -> AnyResult<()>;
}

macro_rules! importers_map {
  ($($impl:ident,)+) => { importers_map![$($impl),+]; };
  ($($impl:ident),*) => {
    pub const IMPORTERS_IDS: &'static [&'static str] = &[$($impl::ID),+];
    lazy_static! {
      pub static ref IMPORTERS_MAP: HashMap<&'static str, fn() -> Box<dyn Importer>> = {
        let _cap = count_exprs!($($impl),*);
        // Don't ask me why the compiler requires the following type
        // annotation.
        let mut _map: HashMap<_, fn() -> _> = HashMap::with_capacity(_cap);
        $(let _ = _map.insert($impl::ID, $impl::new_boxed);)*
        _map
      };
    }
  };
}

importers_map![LocalizeMeTrPackImporter, CcRuChapterFragmentsImporter, GettextPoImporter];

pub fn create_by_id(id: &str) -> AnyResult<Box<dyn Importer>> {
  let constructor: &fn() -> Box<dyn Importer> =
    IMPORTERS_MAP.get(id).ok_or_else(|| format_err!("no such importer {:?}", id))?;
  Ok(constructor())
}

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

  fn import(
    &mut self,
    input: &str,
    imported_fragments: &mut Vec<ImportedFragment>,
  ) -> AnyResult<()> {
    let tr_pack: localize_me::TrPackSerde = serde_json::from_str(input)?;
    for (lm_file_dict_path, tr_pack_entry) in tr_pack.entries {
      let (lm_file_path, json_path) = match localize_me::parse_file_dict_path(&lm_file_dict_path) {
        Some(v) => v,
        None => {
          warn!("Invalid Localize Me file_dict_path_str: {:?}", lm_file_dict_path);
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
          creation_timestamp: None,
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

  fn import(
    &mut self,
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
            creation_timestamp: Some(t.timestamp),
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

  fn import(
    &mut self,
    _input: &str,
    _imported_fragments: &mut Vec<ImportedFragment>,
  ) -> AnyResult<()> {
    todo!()
  }
}
