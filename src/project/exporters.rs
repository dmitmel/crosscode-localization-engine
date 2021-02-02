use super::Fragment;
use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils::json;

use lazy_static::lazy_static;
use serde_json::ser::Formatter;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

pub trait Exporter: std::fmt::Debug {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed() -> Box<dyn Exporter>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn export(&mut self, fragments: &[Rc<Fragment>], output: &mut dyn Write) -> AnyResult<()>;
}

impl<'de> serde::Deserialize<'de> for Box<dyn Exporter> {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let id = <&str>::deserialize(deserializer)?;
    create_by_id(id).map_err(|_| serde::de::Error::unknown_variant(id, EXPORTERS_IDS))
  }
}

impl serde::Serialize for Box<dyn Exporter> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    serializer.serialize_str(self.id())
  }
}

macro_rules! exporters_map {
  ($($strat:ident,)+) => { exporters_map![$($strat),+]; };
  ($($strat:ident),*) => {
    pub const EXPORTERS_IDS: &'static [&'static str] = &[$($strat::ID),+];
    lazy_static! {
      pub static ref EXPORTERS_MAP: HashMap<
        &'static str,
        fn() -> Box<dyn Exporter>,
      > = {
        let _cap = count_exprs!($($strat),*);
        // Don't ask me why the compiler requires the following type
        // annotation.
        let mut _map: HashMap<_, fn() -> _> = HashMap::with_capacity(_cap);
        $(let _ = _map.insert($strat::ID, $strat::new_boxed);)*
        _map
      };
    }
  };
}

exporters_map![LocalizeMeTrPack, GettextPo];

pub fn create_by_id(id: &str) -> AnyResult<Box<dyn Exporter>> {
  let constructor: &fn() -> Box<dyn Exporter> =
    EXPORTERS_MAP.get(id).ok_or_else(|| format_err!("no such exporter '{}'", id))?;
  Ok(constructor())
}

#[derive(Debug)]
pub struct LocalizeMeTrPack;

impl LocalizeMeTrPack {
  pub const ID: &'static str = "lm-tr-pack";
}

impl Exporter for LocalizeMeTrPack {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Exporter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  fn export(&mut self, fragments: &[Rc<Fragment>], writer: &mut dyn Write) -> AnyResult<()> {
    let mut serializer = serde_json::ser::PrettyFormatter::new();

    serializer.begin_object(writer)?;
    let mut is_first_entry = true;
    for fragment in fragments {
      let translation_text = match fragment.get_best_translation() {
        Some(tr) => tr.text().share_rc(),
        None => RcString::from(""),
      };

      let localize_me_file_path =
        fragment.file_path.strip_prefix("data/").unwrap_or(&fragment.file_path);

      serializer.begin_object_key(writer, is_first_entry)?;
      is_first_entry = false;
      {
        serializer.begin_string(writer)?;
        json::format_escaped_str_contents(writer, &mut serializer, &localize_me_file_path)?;
        serializer.write_string_fragment(writer, "/")?;
        json::format_escaped_str_contents(writer, &mut serializer, &fragment.file_path)?;
        serializer.end_string(writer)?;
      }
      serializer.end_object_key(writer)?;

      serializer.begin_object_value(writer)?;
      {
        serializer.begin_object(writer)?;

        {
          serializer.begin_object_key(writer, true)?;
          {
            serializer.begin_string(writer)?;
            serializer.write_string_fragment(writer, "orig")?;
            serializer.end_string(writer)?;
          }
          serializer.end_object_key(writer)?;
          serializer.begin_object_value(writer)?;
          {
            json::format_escaped_str(writer, &mut serializer, &fragment.original_text)?;
          }
          serializer.end_object_value(writer)?;

          serializer.begin_object_key(writer, false)?;
          {
            serializer.begin_string(writer)?;
            serializer.write_string_fragment(writer, "text")?;
            serializer.end_string(writer)?;
          }
          serializer.end_object_key(writer)?;
          serializer.begin_object_value(writer)?;
          {
            json::format_escaped_str(writer, &mut serializer, &translation_text)?;
          }
          serializer.end_object_value(writer)?;
        }

        serializer.end_object(writer)?;
      }
      serializer.end_object_value(writer)?;
    }
    serializer.end_object(writer)?;

    Ok(())
  }
}

#[derive(Debug)]
pub struct GettextPo;

impl GettextPo {
  pub const ID: &'static str = "po";
}

impl Exporter for GettextPo {
  #[inline(always)]
  fn id_static() -> &'static str
  where
    Self: Sized,
  {
    Self::ID
  }

  #[inline(always)]
  fn new_boxed() -> Box<dyn Exporter>
  where
    Self: Sized,
  {
    Box::new(Self)
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  fn export(&mut self, _fragments: &[Rc<Fragment>], _output: &mut dyn Write) -> AnyResult<()> {
    //
    todo!()
  }
}
