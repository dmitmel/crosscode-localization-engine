use crate::impl_prelude::*;
use crate::utils::{self, ShareRc, ShareRcWeak, Timestamp};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak as RcWeak};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ScanDbSerde {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub game_version: String,
  // pub extracted_locales: Vec<String>,
  pub game_files: IndexMap<String, ScanDbGameFileSerde>,
}

#[derive(Debug, Deserialize)]
pub struct ScanDbGameFileSerde {
  pub is_lang_file: bool,
  pub fragments: IndexMap<String, ScanDbFragmentSerde>,
}

#[derive(Debug, Deserialize)]
pub struct ScanDbFragmentSerde {
  pub lang_uid: i32,
  pub description: Vec<String>,
  pub text: HashMap<String, String>,
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanDbCreateOpts {
  pub game_version: String,
}

#[derive(Debug, Serialize)]
pub struct ScanDbMeta {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub game_version: String,
  // TODO: extracted_locales
}

#[derive(Debug, Serialize)]
pub struct ScanDb {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  db_file_path: PathBuf,
  #[serde(flatten)]
  meta: ScanDbMeta,
  game_files: RefCell<IndexMap<Rc<String>, Rc<ScanDbGameFile>>>,
  #[serde(skip)]
  total_fragments_count: Cell<usize>,
}

impl ScanDb {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline(always)]
  pub fn db_file_path(&self) -> &Path { &self.db_file_path }
  #[inline(always)]
  pub fn meta(&self) -> &ScanDbMeta { &self.meta }
  #[inline(always)]
  pub fn game_files(&self) -> Ref<IndexMap<Rc<String>, Rc<ScanDbGameFile>>> {
    self.game_files.borrow()
  }
  #[inline(always)]
  pub fn total_fragments_count(&self) -> usize { self.total_fragments_count.get() }

  fn new(db_file_path: PathBuf, meta: ScanDbMeta) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: Rc::new(Cell::new(false)),
      db_file_path,
      meta,
      game_files: RefCell::new(IndexMap::new()),
      total_fragments_count: Cell::new(0),
    })
  }

  pub fn create(db_file_path: PathBuf, opts: ScanDbCreateOpts) -> Rc<Self> {
    let creation_timestamp = utils::get_timestamp();
    let uuid = utils::new_uuid();
    let myself = Self::new(db_file_path, ScanDbMeta {
      uuid,
      creation_timestamp,
      game_version: opts.game_version,
    });
    myself.dirty_flag.set(true);
    myself
  }

  pub fn open(db_file_path: PathBuf) -> AnyResult<Rc<Self>> {
    let serde_data: ScanDbSerde = utils::json::read_file(&db_file_path, &mut Vec::new())
      .with_context(|| {
        format!("Failed to deserialize from JSON file '{}'", db_file_path.display())
      })?;

    let myself = Self::new(db_file_path, ScanDbMeta {
      uuid: serde_data.uuid,
      creation_timestamp: serde_data.creation_timestamp,
      game_version: serde_data.game_version,
    });

    for (file_serde_path, file_serde_data) in serde_data.game_files {
      let file = myself.new_game_file(ScanDbGameFileInitOpts {
        path: file_serde_path,
        is_lang_file: file_serde_data.is_lang_file,
      });

      for (fragment_serde_json_path, fragment_serde_data) in file_serde_data.fragments {
        file.new_fragment(ScanDbFragmentInitOpts {
          json_path: fragment_serde_json_path,
          lang_uid: fragment_serde_data.lang_uid,
          description: fragment_serde_data.description,
          text: fragment_serde_data.text,
        });
      }
    }

    Ok(myself)
  }

  pub fn write(&self) -> AnyResult<()> {
    if self.is_dirty() {
      self.write_force()?;
      self.dirty_flag.set(false);
    }
    Ok(())
  }

  pub fn write_force(&self) -> AnyResult<()> {
    utils::json::write_file(&self.db_file_path, self).with_context(|| {
      format!("Failed to serialize to JSON file '{}'", self.db_file_path.display())
    })
  }

  pub fn reserve_additional_game_files(&self, additional_capacity: usize) {
    self.game_files.borrow_mut().reserve(additional_capacity);
  }

  pub fn new_game_file(
    self: &Rc<Self>,
    file_init_opts: ScanDbGameFileInitOpts,
  ) -> Rc<ScanDbGameFile> {
    self.dirty_flag.set(true);
    let file = ScanDbGameFile::new(file_init_opts, &self);
    self.game_files.borrow_mut().insert(file.path.share_rc(), file.share_rc());
    file
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanDbGameFileInitOpts {
  // TODO: split `path` into `asset_root` and `relative_path`
  pub path: String,
  pub is_lang_file: bool,
}

#[derive(Debug, Serialize)]
pub struct ScanDbGameFile {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  scan_db: RcWeak<ScanDb>,
  #[serde(skip)]
  path: Rc<String>,
  is_lang_file: bool,
  fragments: RefCell<IndexMap<Rc<String>, Rc<ScanDbFragment>>>,
}

impl ScanDbGameFile {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn scan_db(&self) -> Rc<ScanDb> { self.scan_db.upgrade().unwrap() }
  #[inline(always)]
  pub fn path(&self) -> &Rc<String> { &self.path }
  #[inline(always)]
  pub fn is_lang_file(&self) -> bool { self.is_lang_file }
  #[inline(always)]
  pub fn fragments(&self) -> Ref<IndexMap<Rc<String>, Rc<ScanDbFragment>>> {
    self.fragments.borrow()
  }

  fn new(file_init_opts: ScanDbGameFileInitOpts, scan_db: &Rc<ScanDb>) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: scan_db.dirty_flag.share_rc(),
      scan_db: Rc::downgrade(scan_db),
      path: Rc::new(file_init_opts.path),
      is_lang_file: file_init_opts.is_lang_file,
      fragments: RefCell::new(IndexMap::new()),
    })
  }

  pub fn reserve_additional_fragments(&self, additional_capacity: usize) {
    self.fragments.borrow_mut().reserve(additional_capacity);
  }

  pub fn new_fragment(
    self: &Rc<Self>,
    fragment_init_opts: ScanDbFragmentInitOpts,
  ) -> Rc<ScanDbFragment> {
    self.dirty_flag.set(true);
    let scan_db = self.scan_db();
    let fragment = ScanDbFragment::new(fragment_init_opts, &scan_db, self);
    self.fragments.borrow_mut().insert(fragment.json_path.share_rc(), fragment.share_rc());
    scan_db.total_fragments_count.update(|c| c + 1);
    fragment
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanDbFragmentInitOpts {
  pub json_path: String,
  pub lang_uid: i32,
  pub description: Vec<String>,
  pub text: HashMap<String, String>,
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
  file_path: Rc<String>,
  #[serde(skip)]
  json_path: Rc<String>,
  lang_uid: i32,
  description: Vec<String>,
  text: HashMap<String, String>,
}

impl ScanDbFragment {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn scan_db(&self) -> Rc<ScanDb> { self.scan_db.upgrade().unwrap() }
  #[inline]
  pub fn file(&self) -> Rc<ScanDbGameFile> { self.file.upgrade().unwrap() }
  #[inline(always)]
  pub fn file_path(&self) -> &Rc<String> { &self.file_path }
  #[inline(always)]
  pub fn json_path(&self) -> &Rc<String> { &self.json_path }
  #[inline(always)]
  pub fn lang_uid(&self) -> i32 { self.lang_uid }
  #[inline(always)]
  pub fn has_lang_uid(&self) -> bool { self.lang_uid != 0 }
  #[inline(always)]
  pub fn description(&self) -> &[String] { &self.description }
  #[inline(always)]
  pub fn text(&self) -> &HashMap<String, String> { &self.text }

  fn new(
    fragment_init_opts: ScanDbFragmentInitOpts,
    scan_db: &Rc<ScanDb>,
    file: &Rc<ScanDbGameFile>,
  ) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: scan_db.dirty_flag.share_rc(),
      scan_db: Rc::downgrade(scan_db),
      file: file.share_rc_weak(),
      file_path: file.path.share_rc(),
      json_path: Rc::new(fragment_init_opts.json_path),
      lang_uid: fragment_init_opts.lang_uid,
      description: fragment_init_opts.description,
      text: fragment_init_opts.text,
    })
  }
}
