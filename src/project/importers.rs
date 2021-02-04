use crate::impl_prelude::*;

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read};

pub trait Importer: fmt::Debug {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed() -> Box<dyn Importer>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn file_extension(&self) -> &'static str;

  fn import(&mut self, reader: &mut dyn Read) -> AnyResult<()>;
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

  fn import(&mut self, _reader: &mut dyn Read) -> AnyResult<()> { todo!() }
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

  fn import(&mut self, _reader: &mut dyn Read) -> AnyResult<()> { todo!() }
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

  fn import(&mut self, _reader: &mut dyn Read) -> AnyResult<()> { todo!() }
}
