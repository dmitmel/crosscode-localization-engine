use crate::impl_prelude::*;
use crate::utils::{self, ShareRc, ShareRcWeak, Timestamp};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::rc::{Rc, Weak as RcWeak};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ScanDbSerde {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub game_version: String,
  // pub extracted_locales: Vec<String>,
  pub files: IndexMap<String, ScanDbFileSerde>,
}

#[derive(Debug, Deserialize)]
pub struct ScanDbFileSerde {
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
  db_file_path: PathBuf,
  #[serde(flatten)]
  meta: ScanDbMeta,
  files: RefCell<IndexMap<Rc<String>, Rc<ScanDbFile>>>,
  #[serde(skip)]
  total_fragments_count: Cell<usize>,
}

impl ScanDb {
  #[inline(always)]
  pub fn meta(&self) -> &ScanDbMeta { &self.meta }
  #[inline(always)]
  pub fn files(&self) -> Ref<IndexMap<Rc<String>, Rc<ScanDbFile>>> { self.files.borrow() }
  #[inline(always)]
  pub fn total_fragments_count(&self) -> usize { self.total_fragments_count.get() }

  fn new(db_file_path: PathBuf, meta: ScanDbMeta) -> Rc<Self> {
    Rc::new(Self {
      db_file_path,
      meta,
      files: RefCell::new(IndexMap::new()),
      total_fragments_count: Cell::new(0),
    })
  }

  pub fn create(db_file_path: PathBuf, opts: ScanDbCreateOpts) -> Rc<Self> {
    let creation_timestamp = utils::get_timestamp();
    let uuid = utils::new_uuid();
    Self::new(
      db_file_path,
      ScanDbMeta { uuid, creation_timestamp, game_version: opts.game_version },
    )
  }

  pub fn open(db_file_path: PathBuf) -> AnyResult<Rc<Self>> {
    let json_bytes = fs::read(&db_file_path)
      .with_context(|| format!("Failed to read file '{}'", db_file_path.display()))?;
    let serde_data = serde_json::from_slice::<ScanDbSerde>(&json_bytes)
      .with_context(|| format!("Failed to parse JSON file '{}'", db_file_path.display()))?;

    let myself = Self::new(
      db_file_path,
      ScanDbMeta {
        uuid: serde_data.uuid,
        creation_timestamp: serde_data.creation_timestamp,
        game_version: serde_data.game_version,
      },
    );

    for (file_serde_path, file_serde_data) in serde_data.files {
      let file = myself.new_file(ScanDbFileInitOpts {
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
    let mut writer = io::BufWriter::new(
      fs::File::create(&self.db_file_path)
        .with_context(|| format!("Failed to open file '{}'", self.db_file_path.display()))?,
    );
    serde_json::to_writer_pretty(&mut writer, self).with_context(|| {
      format!("Failed to write JSON into file '{}'", self.db_file_path.display())
    })?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
  }

  pub fn reserve_additional_files(&self, additional_capacity: usize) {
    self.files.borrow_mut().reserve(additional_capacity);
  }

  pub fn new_file(self: &Rc<Self>, file_init_opts: ScanDbFileInitOpts) -> Rc<ScanDbFile> {
    let file = ScanDbFile::new(file_init_opts, self.share_rc_weak());
    self.files.borrow_mut().insert(file.path.share_rc(), file.share_rc());
    file
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanDbFileInitOpts {
  // TODO: split `path` into `asset_root` and `relative_path`
  pub path: String,
  pub is_lang_file: bool,
}

#[derive(Debug, Serialize)]
pub struct ScanDbFile {
  #[serde(skip)]
  scan_db: RcWeak<ScanDb>,
  #[serde(skip)]
  path: Rc<String>,
  is_lang_file: bool,
  fragments: RefCell<IndexMap<Rc<String>, Rc<ScanDbFragment>>>,
}

impl ScanDbFile {
  #[inline(always)]
  pub fn path(&self) -> &Rc<String> { &self.path }
  #[inline(always)]
  pub fn is_lang_file(&self) -> bool { self.is_lang_file }
  #[inline(always)]
  pub fn fragments(&self) -> Ref<IndexMap<Rc<String>, Rc<ScanDbFragment>>> {
    self.fragments.borrow()
  }

  fn new(file_init_opts: ScanDbFileInitOpts, scan_db: RcWeak<ScanDb>) -> Rc<Self> {
    Rc::new(Self {
      scan_db,
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
    let fragment =
      ScanDbFragment::new(fragment_init_opts, self.scan_db.share_rc_weak(), self.share_rc_weak());
    self.fragments.borrow_mut().insert(fragment.json_path.share_rc(), fragment.share_rc());
    self.scan_db.upgrade().unwrap().total_fragments_count.update(|c| c + 1);
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
  scan_db: RcWeak<ScanDb>,
  #[serde(skip)]
  file: RcWeak<ScanDbFile>,
  #[serde(skip)]
  json_path: Rc<String>,
  lang_uid: i32,
  description: Vec<String>,
  text: HashMap<String, String>,
}

impl ScanDbFragment {
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
    scan_db: RcWeak<ScanDb>,
    file: RcWeak<ScanDbFile>,
  ) -> Rc<Self> {
    Rc::new(Self {
      scan_db,
      file,
      json_path: Rc::new(fragment_init_opts.json_path),
      lang_uid: fragment_init_opts.lang_uid,
      description: fragment_init_opts.description,
      text: fragment_init_opts.text,
    })
  }
}
