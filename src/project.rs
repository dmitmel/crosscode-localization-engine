pub mod exporters;
pub mod importers;
pub mod splitters;

use self::splitters::Splitter;
use crate::impl_prelude::*;
use crate::rc_string::RcString;
use crate::utils::json;
use crate::utils::{self, RcExt, Timestamp};

use indexmap::IndexMap;
use lazy_static::lazy_static;
use once_cell::unsync::OnceCell;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak as RcWeak};
use uuid::Uuid;

lazy_static! {
  pub static ref META_FILE_NAME: &'static Path = Path::new("crosslocale-project.json");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetaSerde {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub game_version: RcString,
  pub original_locale: RcString,
  pub reference_locales: Vec<RcString>,
  pub translation_locale: RcString,
  pub translations_dir: RcString,
  pub splitter: RcString,
  pub translation_files: Vec<RcString>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TrFileSerde {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  // pub project_meta_file: RcString,
  pub game_file_chunks: IndexMap<RcString, GameFileChunkSerde>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GameFileChunkSerde {
  pub fragments: IndexMap<RcString, FragmentSerde>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FragmentSerde {
  #[serde(default)]
  pub lang_uid: i32,
  #[serde(default)]
  pub description: Vec<RcString>,
  #[serde(with = "utils::serde::MultilineStringHelper")]
  pub original_text: RcString,
  // #[serde(default)]
  // pub reference_texts: HashMap<RcString, Vec<RcString>>,
  #[serde(default)]
  pub flags: HashMap<RcString, bool>,
  pub translations: Vec<TranslationSerde>,
  #[serde(default)]
  pub comments: Vec<CommentSerde>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranslationSerde {
  pub uuid: Uuid,
  pub author: RcString,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  #[serde(with = "utils::serde::MultilineStringHelper")]
  pub text: RcString,
  #[serde(default)]
  pub flags: HashMap<RcString, bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommentSerde {
  pub uuid: Uuid,
  pub author: RcString,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  #[serde(with = "utils::serde::MultilineStringHelper")]
  pub text: RcString,
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Serialize)]
pub struct ProjectMeta {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  project: RcWeak<Project>,

  uuid: Uuid,
  creation_timestamp: Timestamp,
  modification_timestamp: Cell<Timestamp>, // TODO
  game_version: RcString,
  original_locale: RcString,
  reference_locales: Vec<RcString>,
  translation_locale: RcString,
  translations_dir: RcString,
  splitter: RefCell<Box<dyn Splitter>>,

  // HACK: Don't ask.
  #[serde(
    rename = "translation_files",
    serialize_with = "ProjectMeta::serialize_translation_files_link"
  )]
  translation_files_link: RcWeak<Project>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMetaInitOpts {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub game_version: RcString,
  pub original_locale: RcString,
  pub reference_locales: Vec<RcString>,
  pub translation_locale: RcString,
  pub translations_dir: RcString,
  pub splitter: RcString,
}

impl ProjectMeta {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline(always)]
  pub fn uuid(&self) -> Uuid { self.uuid }
  #[inline(always)]
  pub fn creation_timestamp(&self) -> Timestamp { self.creation_timestamp }
  #[inline(always)]
  pub fn modification_timestamp(&self) -> Timestamp { self.modification_timestamp.get() }
  #[inline(always)]
  pub fn game_version(&self) -> &RcString { &self.game_version }
  #[inline(always)]
  pub fn original_locale(&self) -> &RcString { &self.original_locale }
  #[inline(always)]
  pub fn reference_locales(&self) -> &[RcString] { &self.reference_locales }
  #[inline(always)]
  pub fn translation_locale(&self) -> &RcString { &self.translation_locale }
  #[inline(always)]
  pub fn translations_dir(&self) -> &RcString { &self.translations_dir }
  #[inline(always)]
  pub fn splitter(&self) -> Ref<Box<dyn Splitter>> { self.splitter.borrow() }
  #[inline(always)]
  pub fn splitter_mut(&self) -> RefMut<Box<dyn Splitter>> { self.splitter.borrow_mut() }

  fn new(project: &Rc<Project>, opts: ProjectMetaInitOpts) -> AnyResult<Self> {
    Ok(Self {
      dirty_flag: Rc::new(Cell::new(true)),
      project: project.share_rc_weak(),

      uuid: opts.uuid,
      creation_timestamp: opts.creation_timestamp,
      modification_timestamp: Cell::new(opts.modification_timestamp),
      game_version: opts.game_version,
      original_locale: opts.original_locale,
      reference_locales: opts.reference_locales,
      translation_locale: opts.translation_locale,
      translations_dir: opts.translations_dir,

      splitter: RefCell::new(
        splitters::create_by_id(&opts.splitter).context("Failed to create the splitter")?,
      ),
      translation_files_link: project.share_rc_weak(),
    })
  }

  pub fn fs_path(&self) -> PathBuf { self.project().root_dir.join(*META_FILE_NAME) }

  pub fn write(&self) -> AnyResult<()> {
    if self.is_dirty() {
      self.write_force()?;
    }
    Ok(())
  }

  pub fn write_force(&self) -> AnyResult<()> {
    let fs_path = self.fs_path();
    json::write_file(&fs_path, self, json::UltimateFormatterConfig::default())
      .with_context(|| format!("Failed to serialize to JSON file {:?}", fs_path))?;
    self.dirty_flag.set(false);
    Ok(())
  }

  fn serialize_translation_files_link<S>(
    value: &RcWeak<Project>,
    serializer: S,
  ) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let project = value.upgrade().unwrap();
    let tr_files = project.tr_files.borrow();
    let tr_file_paths: Vec<&RcString> = tr_files.keys().collect();
    tr_file_paths.serialize(serializer)
  }
}

#[derive(Debug)]
pub struct Project {
  root_dir: PathBuf,
  meta: OnceCell<ProjectMeta>,

  tr_files: RefCell<IndexMap<RcString, Rc<TrFile>>>,
  virtual_game_files: RefCell<IndexMap<RcString, Rc<VirtualGameFile>>>,
}

impl Project {
  #[inline(always)]
  pub fn root_dir(&self) -> &Path { &self.root_dir }
  #[inline(always)]
  pub fn meta(&self) -> &ProjectMeta { self.meta.get().unwrap() }
  #[inline(always)]
  pub fn tr_files(&self) -> Ref<IndexMap<RcString, Rc<TrFile>>> { self.tr_files.borrow() }
  #[inline(always)]
  pub fn virtual_game_files(&self) -> Ref<IndexMap<RcString, Rc<VirtualGameFile>>> {
    self.virtual_game_files.borrow()
  }

  pub fn create(root_dir: PathBuf, opts: ProjectMetaInitOpts) -> AnyResult<Rc<Self>> {
    let myself = Rc::new(Self {
      root_dir,
      meta: OnceCell::new(),

      tr_files: RefCell::new(IndexMap::new()),
      virtual_game_files: RefCell::new(IndexMap::new()),
    });

    myself.meta.set(ProjectMeta::new(&myself, opts)?).unwrap();
    Ok(myself)
  }

  pub fn open(root_dir: PathBuf) -> AnyResult<Rc<Self>> {
    let meta_file_path = root_dir.join(*META_FILE_NAME);
    let meta_raw: ProjectMetaSerde = json::read_file(&meta_file_path, &mut Vec::new())
      .with_context(|| format!("Failed to deserialize from JSON file {:?}", meta_file_path))?;

    let myself = Self::create(root_dir, ProjectMetaInitOpts {
      uuid: meta_raw.uuid,
      creation_timestamp: meta_raw.creation_timestamp,
      modification_timestamp: meta_raw.modification_timestamp,
      game_version: meta_raw.game_version,
      original_locale: meta_raw.original_locale,
      reference_locales: meta_raw.reference_locales,
      translation_locale: meta_raw.translation_locale,
      translations_dir: meta_raw.translations_dir,
      splitter: meta_raw.splitter,
    })?;
    myself.meta().dirty_flag.set(false);

    for tr_file_relative_path in meta_raw.translation_files {
      let tr_file_fs_path = {
        let path =
          myself.root_dir.join(&myself.meta().translations_dir).join(&tr_file_relative_path);
        let mut path = path.into_os_string();
        path.push(".json");
        PathBuf::from(path)
      };

      let tr_file_raw: TrFileSerde = json::read_file(&tr_file_fs_path, &mut Vec::new())
        .with_context(|| format!("Failed to deserialize from JSON file {:?}", tr_file_fs_path))?;
      let tr_file = myself.new_tr_file(TrFileInitOpts {
        uuid: tr_file_raw.uuid,
        creation_timestamp: tr_file_raw.creation_timestamp,
        modification_timestamp: tr_file_raw.modification_timestamp,
        relative_path: tr_file_relative_path,
      });

      for (game_file_path, game_file_chunk_raw) in tr_file_raw.game_file_chunks {
        let game_file_chunk =
          tr_file.new_game_file_chunk(GameFileChunkInitOpts { path: game_file_path.share_rc() });

        for (fragment_json_path, fragment_raw) in game_file_chunk_raw.fragments {
          let _fragment = game_file_chunk.new_fragment(FragmentInitOpts {
            file_path: game_file_path.share_rc(),
            json_path: fragment_json_path,
            lang_uid: fragment_raw.lang_uid,
            description: fragment_raw.description,
            original_text: fragment_raw.original_text,
            flags: fragment_raw.flags,
          });

          for _translation_raw in fragment_raw.translations {
            // TODO
          }

          for _comment_raw in fragment_raw.comments {
            // TODO
          }
        }
      }

      tr_file.dirty_flag.set(false);
    }

    Ok(myself)
  }

  pub fn get_tr_file(&self, path: &str) -> Option<Rc<TrFile>> {
    self.tr_files.borrow().get(path).cloned()
  }

  pub fn new_tr_file(self: &Rc<Self>, opts: TrFileInitOpts) -> Rc<TrFile> {
    let file = TrFile::new(self, opts);
    let prev_file =
      self.tr_files.borrow_mut().insert(file.relative_path.share_rc(), file.share_rc());
    assert!(prev_file.is_none());
    file
  }

  pub fn reserve_additional_tr_files(&self, additional_capacity: usize) {
    self.tr_files.borrow_mut().reserve(additional_capacity);
  }

  pub fn get_virtual_game_file(&self, path: &str) -> Option<Rc<VirtualGameFile>> {
    self.virtual_game_files.borrow().get(path).cloned()
  }

  fn new_virtual_game_file(self: &Rc<Self>, path: RcString) -> Rc<VirtualGameFile> {
    let file = VirtualGameFile::new(self, path);
    let prev_file =
      self.virtual_game_files.borrow_mut().insert(file.path.share_rc(), file.share_rc());
    assert!(prev_file.is_none());
    file
  }

  pub fn reserve_additional_virtual_game_files(&self, additional_capacity: usize) {
    self.virtual_game_files.borrow_mut().reserve(additional_capacity);
  }

  pub fn is_dirty(&self) -> bool {
    self.meta().is_dirty() || self.tr_files.borrow().values().any(|f| f.is_dirty())
  }

  pub fn write(&self) -> AnyResult<()> {
    let tr_files = self.tr_files.borrow();
    let total_files_count = 1 + tr_files.len();
    let mut file_index = 1;

    trace!("[{}/{}] Writing the meta file", file_index, total_files_count);
    self.meta().write().context("Failed to write Meta")?;

    for (path, tr_file) in tr_files.iter() {
      file_index += 1;
      trace!("[{}/{}] Writing translation file {:?}", file_index, total_files_count, path);
      tr_file.write().with_context(|| format!("Failed to write TrFile {:?}", path))?;
    }

    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrFileInitOpts {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub relative_path: RcString,
}

#[derive(Debug, Serialize)]
pub struct TrFile {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  project: RcWeak<Project>,

  uuid: Uuid,
  creation_timestamp: Timestamp,
  modification_timestamp: Timestamp,
  // project_meta_file: RcString, // TODO
  #[serde(skip)]
  relative_path: RcString,

  game_file_chunks: RefCell<IndexMap<RcString, Rc<GameFileChunk>>>,
}

impl TrFile {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline(always)]
  pub fn uuid(&self) -> Uuid { self.uuid }
  #[inline(always)]
  pub fn creation_timestamp(&self) -> Timestamp { self.creation_timestamp }
  #[inline(always)]
  pub fn modification_timestamp(&self) -> Timestamp { self.modification_timestamp }
  #[inline(always)]
  pub fn relative_path(&self) -> &RcString { &self.relative_path }
  #[inline(always)]
  pub fn game_file_chunks(&self) -> Ref<IndexMap<RcString, Rc<GameFileChunk>>> {
    self.game_file_chunks.borrow()
  }

  #[inline(always)]
  pub fn mark_dirty(&self) { self.dirty_flag.set(true); }

  fn new(project: &Rc<Project>, opts: TrFileInitOpts) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: Rc::new(Cell::new(false)),
      project: project.share_rc_weak(),

      uuid: opts.uuid,
      creation_timestamp: opts.creation_timestamp,
      modification_timestamp: opts.modification_timestamp,
      relative_path: opts.relative_path,

      game_file_chunks: RefCell::new(IndexMap::new()),
    })
  }

  pub fn get_game_file_chunk(&self, path: &str) -> Option<Rc<GameFileChunk>> {
    self.game_file_chunks.borrow().get(path).cloned()
  }

  pub fn new_game_file_chunk(self: &Rc<Self>, opts: GameFileChunkInitOpts) -> Rc<GameFileChunk> {
    self.dirty_flag.set(true);
    let project = self.project();
    let virt_file = project
      .get_virtual_game_file(&opts.path)
      .unwrap_or_else(|| project.new_virtual_game_file(opts.path.share_rc()));
    let chunk = GameFileChunk::new(&self.project(), self, virt_file, opts);
    let prev_chunk =
      self.game_file_chunks.borrow_mut().insert(chunk.path.share_rc(), chunk.share_rc());
    assert!(prev_chunk.is_none());
    chunk
  }

  pub fn reserve_additional_game_file_chunks(&self, additional_capacity: usize) {
    self.game_file_chunks.borrow_mut().reserve(additional_capacity);
  }

  pub fn fs_path(&self) -> PathBuf {
    let project = self.project();
    let path = project.root_dir.join(&project.meta().translations_dir).join(&self.relative_path);
    let mut path = path.into_os_string();
    path.push(".json");
    PathBuf::from(path)
  }

  pub fn write(&self) -> AnyResult<()> {
    if self.is_dirty() {
      self.write_force()?;
    }
    Ok(())
  }

  pub fn write_force(&self) -> AnyResult<()> {
    let fs_path = self.fs_path();
    utils::create_dir_recursively(fs_path.parent().unwrap())
      .with_context(|| format!("Failed to create the parent directories for {:?}", fs_path))?;
    json::write_file(&fs_path, self, json::UltimateFormatterConfig::default())
      .with_context(|| format!("Failed to serialize to JSON file {:?}", fs_path))?;
    self.dirty_flag.set(false);
    Ok(())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameFileChunkInitOpts {
  pub path: RcString,
}

#[derive(Debug, Serialize)]
pub struct GameFileChunk {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  project: RcWeak<Project>,
  #[serde(skip)]
  tr_file: RcWeak<TrFile>,
  #[serde(skip)]
  virtual_game_file: Rc<VirtualGameFile>,

  path: RcString,

  fragments: RefCell<IndexMap<RcString, Rc<Fragment>>>,
}

impl GameFileChunk {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline]
  pub fn tr_file(&self) -> Rc<TrFile> { self.tr_file.upgrade().unwrap() }
  #[inline]
  pub fn virtual_game_file(&self) -> &Rc<VirtualGameFile> { &self.virtual_game_file }
  #[inline(always)]
  pub fn path(&self) -> &RcString { &self.path }
  #[inline(always)]
  pub fn fragments(&self) -> Ref<IndexMap<RcString, Rc<Fragment>>> { self.fragments.borrow() }

  fn new(
    project: &Rc<Project>,
    tr_file: &Rc<TrFile>,
    virtual_game_file: Rc<VirtualGameFile>,
    opts: GameFileChunkInitOpts,
  ) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: tr_file.dirty_flag.share_rc(),
      project: project.share_rc_weak(),
      tr_file: tr_file.share_rc_weak(),
      virtual_game_file,

      path: opts.path,

      fragments: RefCell::new(IndexMap::new()),
    })
  }

  pub fn get_fragment(&self, json_path: &str) -> Option<Rc<Fragment>> {
    self.fragments.borrow().get(json_path).cloned()
  }

  pub fn new_fragment(self: &Rc<Self>, opts: FragmentInitOpts) -> Rc<Fragment> {
    self.dirty_flag.set(true);
    let fragment = Fragment::new(&self.project(), &self.tr_file(), self, opts);
    let prev_fragment =
      self.fragments.borrow_mut().insert(fragment.json_path.share_rc(), fragment.share_rc());
    assert!(prev_fragment.is_none());

    let virt_fragments = &self.virtual_game_file.fragments;
    let prev_virt_fragment =
      virt_fragments.borrow_mut().insert(fragment.json_path.share_rc(), fragment.share_rc());
    assert!(prev_virt_fragment.is_none());

    fragment
  }

  pub fn reserve_additional_fragments(&self, additional_capacity: usize) {
    self.fragments.borrow_mut().reserve(additional_capacity);
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentInitOpts {
  pub file_path: RcString,
  pub json_path: RcString,
  pub lang_uid: i32,
  pub description: Vec<RcString>,
  pub original_text: RcString,
  // pub reference_texts: HashMap<RcString, RcString>,
  pub flags: HashMap<RcString, bool>,
}

#[derive(Debug, Serialize)]
pub struct Fragment {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  project: RcWeak<Project>,
  #[serde(skip)]
  tr_file: RcWeak<TrFile>,
  #[serde(skip)]
  game_file_chunk: RcWeak<GameFileChunk>,

  #[serde(skip)]
  file_path: RcString,
  json_path: RcString,
  #[serde(default, skip_serializing_if = "utils::is_default")]
  lang_uid: i32,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  description: Vec<RcString>,
  #[serde(with = "utils::serde::MultilineStringHelper")]
  original_text: RcString,
  // #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  // reference_texts: HashMap<RcString, RcString>,
  #[serde(default, skip_serializing_if = "utils::serde::is_refcell_hashmap_empty")]
  flags: RefCell<HashMap<RcString, bool>>,

  translations: RefCell<Vec<Rc<Translation>>>,
  #[serde(default, skip_serializing_if = "utils::serde::is_refcell_vec_empty")]
  comments: RefCell<Vec<Rc<Comment>>>,
}

impl Fragment {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline]
  pub fn tr_file(&self) -> Rc<TrFile> { self.tr_file.upgrade().unwrap() }
  #[inline]
  pub fn game_file_chunk(&self) -> Rc<GameFileChunk> { self.game_file_chunk.upgrade().unwrap() }
  #[inline(always)]
  pub fn file_path(&self) -> &RcString { &self.file_path }
  #[inline(always)]
  pub fn json_path(&self) -> &RcString { &self.json_path }
  #[inline(always)]
  pub fn lang_uid(&self) -> i32 { self.lang_uid }
  #[inline(always)]
  pub fn description(&self) -> &[RcString] { &self.description }
  #[inline(always)]
  pub fn original_text(&self) -> &RcString { &self.original_text }
  // #[inline(always)]
  // pub fn reference_texts(&self) -> &HashMap<RcString, RcString> { &self.reference_texts }
  #[inline(always)]
  pub fn flags(&self) -> Ref<HashMap<RcString, bool>> { self.flags.borrow() }
  #[inline(always)]
  pub fn translations(&self) -> Ref<Vec<Rc<Translation>>> { self.translations.borrow() }
  #[inline(always)]
  pub fn comments(&self) -> Ref<Vec<Rc<Comment>>> { self.comments.borrow() }

  fn new(
    project: &Rc<Project>,
    tr_file: &Rc<TrFile>,
    game_file_chunk: &Rc<GameFileChunk>,
    opts: FragmentInitOpts,
  ) -> Rc<Self> {
    Rc::new(Self {
      dirty_flag: tr_file.dirty_flag.share_rc(),
      project: project.share_rc_weak(),
      tr_file: tr_file.share_rc_weak(),
      game_file_chunk: game_file_chunk.share_rc_weak(),

      file_path: game_file_chunk.path.share_rc(),
      json_path: opts.json_path,
      lang_uid: opts.lang_uid,
      description: opts.description,
      original_text: opts.original_text,
      // reference_texts: opts.reference_texts,
      flags: RefCell::new(opts.flags),

      translations: RefCell::new(Vec::new()),
      comments: RefCell::new(Vec::new()),
    })
  }

  pub fn get_best_translation(&self) -> Option<Rc<Translation>> {
    self
      .translations
      .borrow()
      .iter()
      .max_by_key(|f| f.creation_timestamp.max(f.modification_timestamp.get()))
      .map(|f| f.share_rc())
  }

  pub fn reserve_additional_translations(&self, additional_capacity: usize) {
    self.translations.borrow_mut().reserve(additional_capacity);
  }

  pub fn reserve_additional_comments(&self, additional_capacity: usize) {
    self.translations.borrow_mut().reserve(additional_capacity);
  }
}

#[derive(Debug, Serialize)]
pub struct Translation {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  fragment: RcWeak<Fragment>,

  uuid: Uuid,
  author: RcString,
  creation_timestamp: Timestamp,
  modification_timestamp: Cell<Timestamp>,
  text: RefCell<RcString>,
  #[serde(default, skip_serializing_if = "utils::serde::is_refcell_hashmap_empty")]
  flags: RefCell<HashMap<RcString, bool>>,
}

impl Translation {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn fragment(&self) -> Rc<Fragment> { self.fragment.upgrade().unwrap() }
  #[inline(always)]
  pub fn uuid(&self) -> Uuid { self.uuid }
  #[inline(always)]
  pub fn author(&self) -> &RcString { &self.author }
  #[inline(always)]
  pub fn creation_timestamp(&self) -> Timestamp { self.creation_timestamp }
  #[inline(always)]
  pub fn modification_timestamp(&self) -> Timestamp { self.modification_timestamp.get() }
  #[inline(always)]
  pub fn text(&self) -> Ref<RcString> { self.text.borrow() }
}

#[derive(Debug, Serialize)]
pub struct Comment {
  #[serde(skip)]
  dirty_flag: Rc<Cell<bool>>,
  #[serde(skip)]
  fragment: RcWeak<Fragment>,

  uuid: Uuid,
  author: RcString,
  creation_timestamp: Timestamp,
  modification_timestamp: Cell<Timestamp>,
  text: RefCell<RcString>,
}

impl Comment {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn fragment(&self) -> Rc<Fragment> { self.fragment.upgrade().unwrap() }
  #[inline(always)]
  pub fn uuid(&self) -> Uuid { self.uuid }
  #[inline(always)]
  pub fn author(&self) -> &RcString { &self.author }
  #[inline(always)]
  pub fn creation_timestamp(&self) -> Timestamp { self.creation_timestamp }
  #[inline(always)]
  pub fn modification_timestamp(&self) -> Timestamp { self.modification_timestamp.get() }
  #[inline(always)]
  pub fn text(&self) -> Ref<RcString> { self.text.borrow() }
}

#[derive(Debug)]
pub struct VirtualGameFile {
  project: RcWeak<Project>,

  path: RcString,

  fragments: RefCell<IndexMap<RcString, Rc<Fragment>>>,
}

impl VirtualGameFile {
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline(always)]
  pub fn path(&self) -> &RcString { &self.path }
  #[inline(always)]
  pub fn fragments(&self) -> Ref<IndexMap<RcString, Rc<Fragment>>> { self.fragments.borrow() }

  fn new(project: &Rc<Project>, path: RcString) -> Rc<Self> {
    Rc::new(Self {
      project: project.share_rc_weak(),

      path,

      fragments: RefCell::new(IndexMap::new()),
    })
  }
}
