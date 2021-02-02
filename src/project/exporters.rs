use super::Fragment;
use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils::json;

use lazy_static::lazy_static;
use serde_json::ser::Formatter;
use std::collections::HashMap;
use std::io::Write;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct ExporterConfig {
  pub compact: bool,
}

pub trait Exporter: std::fmt::Debug {
  fn id_static() -> &'static str
  where
    Self: Sized;

  fn new_boxed(config: ExporterConfig) -> Box<dyn Exporter>
  where
    Self: Sized;

  fn id(&self) -> &'static str;

  fn export(&mut self, fragments: &[Rc<Fragment>], output: &mut dyn Write) -> AnyResult<()>;
}

macro_rules! exporters_map {
  ($($strat:ident,)+) => { exporters_map![$($strat),+]; };
  ($($strat:ident),*) => {
    pub const EXPORTERS_IDS: &'static [&'static str] = &[$($strat::ID),+];
    lazy_static! {
      pub static ref EXPORTERS_MAP: HashMap<
        &'static str,
        fn(config: ExporterConfig) -> Box<dyn Exporter>,
      > = {
        let _cap = count_exprs!($($strat),*);
        // Don't ask me why the compiler requires the following type
        // annotation.
        let mut _map: HashMap<_, fn(config: ExporterConfig) -> _> = HashMap::with_capacity(_cap);
        $(let _ = _map.insert($strat::ID, $strat::new_boxed);)*
        _map
      };
    }
  };
}

exporters_map![LocalizeMeTrPack, GettextPo];

pub fn create(id: &str, config: ExporterConfig) -> AnyResult<Box<dyn Exporter>> {
  let constructor: &fn(config: ExporterConfig) -> Box<dyn Exporter> =
    EXPORTERS_MAP.get(id).ok_or_else(|| format_err!("no such exporter '{}'", id))?;
  Ok(constructor(config))
}

#[derive(Debug)]
pub struct LocalizeMeTrPack {
  json_fmt: json::UltimateFormatter<'static>,
}

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
  fn new_boxed(config: ExporterConfig) -> Box<dyn Exporter>
  where
    Self: Sized,
  {
    Box::new(Self {
      json_fmt: json::UltimateFormatter::new(json::UltimateFormatterConfig {
        indent: if config.compact { None } else { Some(b"  ") },
        ..Default::default()
      }),
    })
  }

  #[inline(always)]
  fn id(&self) -> &'static str { Self::ID }

  fn export(&mut self, fragments: &[Rc<Fragment>], writer: &mut dyn Write) -> AnyResult<()> {
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
  fn new_boxed(_config: ExporterConfig) -> Box<dyn Exporter>
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
