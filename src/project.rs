pub mod splitting_strategies;

use crate::utils::{is_default, Timestamp};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak as RcWeak};
use uuid::Uuid;

pub const META_FILE_PATH: &str = "crosslocale-project.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub game_version: String,
  pub original_locale: String,
  pub reference_locales: Vec<String>,
  pub translation_locale: String,
  pub splitting_strategy: String,
  pub translations_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationFileSerde {
  pub uuid: Uuid,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub project_meta_file: String,
  pub game_files: IndexMap<String, GameFileChunkSerde>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameFileChunkSerde {
  #[serde(default, skip_serializing_if = "is_default")]
  pub is_lang_file: bool,
  pub fragments: IndexMap<String, FragmentSerde>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FragmentSerde {
  #[serde(default, skip_serializing_if = "is_default")]
  pub lang_uid: i32,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub description: Vec<String>,
  pub original_text: String,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub reference_texts: HashMap<String, String>,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub flags: HashMap<String, bool>,
  pub translations: Vec<TranslationSerde>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub comments: Vec<CommentSerde>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationSerde {
  pub uuid: Uuid,
  pub author: String,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub text: String,
  #[serde(default, skip_serializing_if = "HashMap::is_empty")]
  pub flags: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommentSerde {
  pub uuid: Uuid,
  pub author: String,
  pub creation_timestamp: Timestamp,
  pub modification_timestamp: Timestamp,
  pub text: String,
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Project {
  root_dir: PathBuf,
  meta: ProjectMeta,

  translation_files: RefCell<IndexMap<Rc<String>, Rc<TranslationFile>>>,
  virtual_game_files: RefCell<IndexMap<Rc<String>, Rc<VirtualGameFile>>>,
}

impl Project {
  pub fn is_dirty(&self) -> bool { self.translation_files.borrow().values().any(|f| f.is_dirty()) }
  #[inline(always)]
  pub fn root_dir(&self) -> &Path { &self.root_dir }
  #[inline(always)]
  pub fn meta(&self) -> &ProjectMeta { &self.meta }
  #[inline(always)]
  pub fn translation_files(&self) -> Ref<IndexMap<Rc<String>, Rc<TranslationFile>>> {
    self.translation_files.borrow()
  }
  #[inline(always)]
  pub fn virtual_game_files(&self) -> Ref<IndexMap<Rc<String>, Rc<VirtualGameFile>>> {
    self.virtual_game_files.borrow()
  }
}

#[derive(Debug)]
pub struct TranslationFile {
  dirty_flag: Rc<Cell<bool>>,
  project: RcWeak<Project>,

  uuid: Uuid,
  creation_timestamp: Timestamp,
  modification_timestamp: Timestamp,
  // project_meta_file: String, // TODO
  relative_path: String,
  fs_path: PathBuf,

  game_file_chunks: RefCell<IndexMap<Rc<String>, Rc<GameFileChunk>>>,
}

impl TranslationFile {
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
  pub fn relative_path(&self) -> &String { &self.relative_path }
  #[inline(always)]
  pub fn fs_path(&self) -> &Path { &self.fs_path }
  #[inline(always)]
  pub fn game_file_chunks(&self) -> Ref<IndexMap<Rc<String>, Rc<GameFileChunk>>> {
    self.game_file_chunks.borrow()
  }
}

#[derive(Debug)]
pub struct GameFileChunk {
  dirty_flag: Rc<Cell<bool>>,
  project: RcWeak<Project>,
  translation_file: RcWeak<TranslationFile>,

  path: Rc<String>,
  is_lang_file: bool,

  fragments: RefCell<IndexMap<Rc<String>, Rc<Fragment>>>,
}

impl GameFileChunk {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline]
  pub fn translation_file(&self) -> Rc<TranslationFile> {
    self.translation_file.upgrade().unwrap()
  }
  #[inline(always)]
  pub fn path(&self) -> &Rc<String> { &self.path }
  #[inline(always)]
  pub fn is_lang_file(&self) -> bool { self.is_lang_file }
  #[inline(always)]
  pub fn fragments(&self) -> Ref<IndexMap<Rc<String>, Rc<Fragment>>> { self.fragments.borrow() }
}

#[derive(Debug)]
pub struct Fragment {
  dirty_flag: Rc<Cell<bool>>,
  project: RcWeak<Project>,
  translation_file: RcWeak<TranslationFile>,
  game_file_chunk: RcWeak<GameFileChunk>,

  file_path: Rc<String>,
  json_path: Rc<String>,
  lang_uid: i32,
  description: Vec<String>,
  original_text: String,
  reference_texts: HashMap<String, String>,
  flags: RefCell<HashMap<String, bool>>,

  translations: RefCell<Vec<Rc<Translation>>>,
  comments: RefCell<Vec<Rc<Comment>>>,
}

impl Fragment {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline]
  pub fn translation_file(&self) -> Rc<TranslationFile> {
    self.translation_file.upgrade().unwrap()
  }
  #[inline]
  pub fn game_file_chunk(&self) -> Rc<GameFileChunk> { self.game_file_chunk.upgrade().unwrap() }
  #[inline(always)]
  pub fn file_path(&self) -> &Rc<String> { &self.file_path }
  #[inline(always)]
  pub fn json_path(&self) -> &Rc<String> { &self.json_path }
  #[inline(always)]
  pub fn lang_uid(&self) -> i32 { self.lang_uid }
  #[inline(always)]
  pub fn description(&self) -> &[String] { &self.description }
  #[inline(always)]
  pub fn original_text(&self) -> &str { &self.original_text }
  #[inline(always)]
  pub fn reference_texts(&self) -> &HashMap<String, String> { &self.reference_texts }
  #[inline(always)]
  pub fn flags(&self) -> Ref<HashMap<String, bool>> { self.flags.borrow() }
  #[inline(always)]
  pub fn translations(&self) -> Ref<Vec<Rc<Translation>>> { self.translations.borrow() }
  #[inline(always)]
  pub fn comments(&self) -> Ref<Vec<Rc<Comment>>> { self.comments.borrow() }
}

#[derive(Debug)]
pub struct Translation {
  dirty_flag: Rc<Cell<bool>>,
  project: RcWeak<Project>,
  translation_file: RcWeak<TranslationFile>,
  game_file_chunk: RcWeak<GameFileChunk>,
  fragment: RcWeak<Fragment>,

  uuid: Uuid,
  author: String,
  creation_timestamp: Timestamp,
  modification_timestamp: Cell<Timestamp>,
  text: RefCell<String>,
  flags: RefCell<HashMap<String, bool>>,
}

impl Translation {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline]
  pub fn translation_file(&self) -> Rc<TranslationFile> {
    self.translation_file.upgrade().unwrap()
  }
  #[inline]
  pub fn game_file_chunk(&self) -> Rc<GameFileChunk> { self.game_file_chunk.upgrade().unwrap() }
  #[inline]
  pub fn fragment(&self) -> Rc<Fragment> { self.fragment.upgrade().unwrap() }
  #[inline(always)]
  pub fn uuid(&self) -> Uuid { self.uuid }
  #[inline(always)]
  pub fn author(&self) -> &str { &self.author }
  #[inline(always)]
  pub fn creation_timestamp(&self) -> Timestamp { self.creation_timestamp }
  #[inline(always)]
  pub fn modification_timestamp(&self) -> Timestamp { self.modification_timestamp.get() }
  #[inline(always)]
  pub fn text(&self) -> Ref<String> { self.text.borrow() }
}

#[derive(Debug)]
pub struct Comment {
  dirty_flag: Rc<Cell<bool>>,
  project: RcWeak<Project>,
  translation_file: RcWeak<TranslationFile>,
  game_file_chunk: RcWeak<GameFileChunk>,
  fragment: RcWeak<Fragment>,

  uuid: Uuid,
  author: String,
  creation_timestamp: Timestamp,
  modification_timestamp: Cell<Timestamp>,
  text: RefCell<String>,
}

impl Comment {
  #[inline(always)]
  pub fn is_dirty(&self) -> bool { self.dirty_flag.get() }
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline]
  pub fn translation_file(&self) -> Rc<TranslationFile> {
    self.translation_file.upgrade().unwrap()
  }
  #[inline]
  pub fn game_file_chunk(&self) -> Rc<GameFileChunk> { self.game_file_chunk.upgrade().unwrap() }
  #[inline]
  pub fn fragment(&self) -> Rc<Fragment> { self.fragment.upgrade().unwrap() }
  #[inline(always)]
  pub fn uuid(&self) -> Uuid { self.uuid }
  #[inline(always)]
  pub fn author(&self) -> &str { &self.author }
  #[inline(always)]
  pub fn creation_timestamp(&self) -> Timestamp { self.creation_timestamp }
  #[inline(always)]
  pub fn modification_timestamp(&self) -> Timestamp { self.modification_timestamp.get() }
  #[inline(always)]
  pub fn text(&self) -> Ref<String> { self.text.borrow() }
}

#[derive(Debug)]
pub struct VirtualGameFile {
  project: RcWeak<Project>,

  path: Rc<String>,
  is_lang_file: bool,

  fragments: RefCell<IndexMap<Rc<String>, Rc<Fragment>>>,
}

impl VirtualGameFile {
  #[inline]
  pub fn project(&self) -> Rc<Project> { self.project.upgrade().unwrap() }
  #[inline(always)]
  pub fn path(&self) -> &Rc<String> { &self.path }
  #[inline(always)]
  pub fn is_lang_file(&self) -> bool { self.is_lang_file }
  #[inline(always)]
  pub fn fragments(&self) -> Ref<IndexMap<Rc<String>, Rc<Fragment>>> { self.fragments.borrow() }
}
