pub mod fragment_descriptions;
pub mod json_file_finder;
pub mod lang_label_extractor;

use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::{self, RcExt, Timestamp};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak as RcWeak};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ScanDbSerde {
  pub id: Uuid,
  #[serde(rename = "ctime")]
  pub creation_timestamp: Timestamp,
  pub game_version: RcString,
  pub game_files: IndexMap<RcString, ScanDbGameFileSerde>,
}

#[derive(Debug, Deserialize)]
pub struct ScanDbGameFileSerde {
  pub asset_root: RcString,
  pub fragments: IndexMap<RcString, ScanDbFragmentSerde>,
}

#[derive(Debug, Deserialize)]
pub struct ScanDbFragmentSerde {
  #[serde(rename = "luid")]
  pub lang_uid: i32,
  #[serde(rename = "desc")]
  pub description: Rc<Vec<RcString>>,
  pub text: Rc<HashMap<RcString, RcString>>,
  pub flags: Rc<HashSet<RcString>>,
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct ScanDbCreateOpts {
  pub game_version: RcString,
}

#[derive(Debug, Serialize)]
pub struct ScanDbMeta {
  pub id: Uuid,
  #[serde(rename = "ctime")]
  pub creation_timestamp: Timestamp,
  pub game_version: RcString,
}

#[derive(Debug, Serialize)]
pub struct ScanDb {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  db_file_path: PathBuf,
  #[serde(flatten)]
  meta: ScanDbMeta,
  game_files: RefCell<IndexMap<RcString, Rc<ScanDbGameFile>>>,
}

impl ScanDb {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline(always)]
  pub fn db_file_path(&self) -> &Path { &self.db_file_path }
  #[inline(always)]
  pub fn meta(&self) -> &ScanDbMeta { &self.meta }
  #[inline(always)]
  pub fn game_files(&self) -> Ref<IndexMap<RcString, Rc<ScanDbGameFile>>> {
    self.game_files.borrow()
  }

  fn new(db_file_path: PathBuf, meta: ScanDbMeta) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: Rc::new(Cell::new(false)),
      db_file_path,
      meta,
      game_files: RefCell::new(IndexMap::new()),
    })
  }

  pub fn create(db_file_path: PathBuf, opts: ScanDbCreateOpts) -> Rc<Self> {
    let creation_timestamp = utils::get_timestamp();
    let myself = Self::new(db_file_path, ScanDbMeta {
      id: utils::new_uuid(),
      creation_timestamp,
      game_version: opts.game_version,
    });
    myself.dirty_flag.set(true);
    myself
  }

  pub fn open(db_file_path: PathBuf) -> AnyResult<Rc<Self>> {
    let raw_data: ScanDbSerde = json::read_file(&db_file_path, &mut Vec::new())
      .with_context(|| format!("Failed to deserialize from JSON file {:?}", db_file_path))?;

    let myself = Self::new(db_file_path, ScanDbMeta {
      id: raw_data.id,
      creation_timestamp: raw_data.creation_timestamp,
      game_version: raw_data.game_version,
    });

    for (game_file_path, game_file_raw) in raw_data.game_files {
      let file = myself.new_game_file(ScanDbGameFileInitOpts {
        asset_root: game_file_raw.asset_root,
        path: game_file_path,
      })?;

      for (fragment_json_path, fragment_raw) in game_file_raw.fragments {
        file.new_fragment(ScanDbFragmentInitOpts {
          json_path: fragment_json_path,
          lang_uid: fragment_raw.lang_uid,
          description: fragment_raw.description,
          text: fragment_raw.text,
          flags: fragment_raw.flags,
        });
      }
    }

    Ok(myself)
  }

  pub fn write(&self) -> AnyResult<()> {
    if self.is_dirty() {
      self.write_force()?;
    }
    Ok(())
  }

  pub fn write_force(&self) -> AnyResult<()> {
    json::write_file(&self.db_file_path, self, json::UltimateFormatterConfig::default())
      .with_context(|| format!("Failed to serialize to JSON file {:?}", self.db_file_path))?;
    self.dirty_flag.set(false);
    Ok(())
  }

  pub fn reserve_additional_game_files(&self, additional_capacity: usize) {
    self.game_files.borrow_mut().reserve(additional_capacity);
  }

  pub fn new_game_file(
    self: &Rc<Self>,
    opts: ScanDbGameFileInitOpts,
  ) -> AnyResult<Rc<ScanDbGameFile>> {
    self.dirty_flag.set(true);
    let file = ScanDbGameFile::new(self, opts)?;
    let prev_file = self.game_files.borrow_mut().insert(file.path.share_rc(), file.share_rc());
    assert!(prev_file.is_none());
    Ok(file)
  }
}

#[derive(Debug)]
pub struct ScanDbGameFileInitOpts {
  pub path: RcString,
  pub asset_root: RcString,
}

#[derive(Debug, Serialize)]
pub struct ScanDbGameFile {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  scan_db: RcWeak<ScanDb>,
  asset_root: RcString,
  path: RcString,
  fragments: RefCell<IndexMap<RcString, Rc<ScanDbFragment>>>,
}

impl ScanDbGameFile {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn scan_db(&self) -> Rc<ScanDb> { self.scan_db.upgrade().unwrap() }
  #[inline(always)]
  pub fn asset_root(&self) -> &RcString { &self.asset_root }
  #[inline(always)]
  pub fn path(&self) -> &RcString { &self.path }
  #[inline(always)]
  pub fn fragments(&self) -> Ref<IndexMap<RcString, Rc<ScanDbFragment>>> {
    self.fragments.borrow()
  }

  fn new(scan_db: &Rc<ScanDb>, opts: ScanDbGameFileInitOpts) -> AnyResult<Rc<Self>> {
    ensure!(
      opts.path.starts_with(&*opts.asset_root),
      "Path to a game file ({:?}) must start with its asset root ({:?})",
      opts.path,
      opts.asset_root,
    );
    Ok(Rc::new(Self {
      dirty_flag: scan_db.dirty_flag.share_rc(),
      scan_db: Rc::downgrade(scan_db),
      asset_root: opts.asset_root,
      path: opts.path,
      fragments: RefCell::new(IndexMap::new()),
    }))
  }

  pub fn reserve_additional_fragments(&self, additional_capacity: usize) {
    self.fragments.borrow_mut().reserve(additional_capacity);
  }

  pub fn new_fragment(self: &Rc<Self>, opts: ScanDbFragmentInitOpts) -> Rc<ScanDbFragment> {
    self.dirty_flag.set(true);
    let scan_db = self.scan_db();
    let fragment = ScanDbFragment::new(&scan_db, self, opts);
    let prev_fragment =
      self.fragments.borrow_mut().insert(fragment.json_path.share_rc(), fragment.share_rc());
    assert!(prev_fragment.is_none());
    fragment
  }
}

#[derive(Debug)]
pub struct ScanDbFragmentInitOpts {
  pub json_path: RcString,
  pub lang_uid: i32,
  pub description: Rc<Vec<RcString>>,
  pub text: Rc<HashMap<RcString, RcString>>,
  pub flags: Rc<HashSet<RcString>>,
}

#[derive(Debug, Serialize)]
pub struct ScanDbFragment {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  scan_db: RcWeak<ScanDb>,
  #[serde(skip)]
  file: RcWeak<ScanDbGameFile>,
  #[serde(skip)]
  file_asset_root: RcString,
  #[serde(skip)]
  file_path: RcString,
  #[serde(skip)]
  json_path: RcString,
  #[serde(rename = "luid")]
  lang_uid: i32,
  #[serde(rename = "desc")]
  description: Rc<Vec<RcString>>,
  text: Rc<HashMap<RcString, RcString>>,
  flags: Rc<HashSet<RcString>>,
}

impl ScanDbFragment {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn scan_db(&self) -> Rc<ScanDb> { self.scan_db.upgrade().unwrap() }
  #[inline]
  pub fn file(&self) -> Rc<ScanDbGameFile> { self.file.upgrade().unwrap() }
  #[inline(always)]
  pub fn file_asset_root(&self) -> &RcString { &self.file_asset_root }
  #[inline(always)]
  pub fn file_path(&self) -> &RcString { &self.file_path }
  #[inline(always)]
  pub fn json_path(&self) -> &RcString { &self.json_path }
  #[inline(always)]
  pub fn lang_uid(&self) -> i32 { self.lang_uid }
  #[inline(always)]
  pub fn has_lang_uid(&self) -> bool { self.lang_uid != 0 }
  #[inline(always)]
  pub fn description(&self) -> &Rc<Vec<RcString>> { &self.description }
  #[inline(always)]
  pub fn text(&self) -> &Rc<HashMap<RcString, RcString>> { &self.text }
  #[inline(always)]
  pub fn flags(&self) -> &Rc<HashSet<RcString>> { &self.flags }

  fn new(
    scan_db: &Rc<ScanDb>,
    file: &Rc<ScanDbGameFile>,
    opts: ScanDbFragmentInitOpts,
  ) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: scan_db.dirty_flag.share_rc(),
      scan_db: Rc::downgrade(scan_db),
      file: file.share_rc_weak(),
      file_asset_root: file.asset_root.share_rc(),
      file_path: file.path.share_rc(),
      json_path: opts.json_path,
      lang_uid: opts.lang_uid,
      description: opts.description,
      text: opts.text,
      flags: opts.flags,
    })
  }
}
