pub mod transports;

use self::transports::Transport;
use crate::impl_prelude::*;
use crate::project::{Comment, Fragment, Project, Translation};
use crate::rc_string::{MaybeStaticStr, RcString};
use crate::utils::{self, RcExt, Timestamp};

use once_cell::sync::Lazy;
use serde::ser::SerializeSeq as _;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 0;
pub static PROTOCOL_VERSION_STR: Lazy<String> = Lazy::new(|| PROTOCOL_VERSION.to_string());

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
  #[serde(rename = "req")]
  Request(RequestMessage),
  #[serde(rename = "res")]
  Response(ResponseMessage),
  #[serde(rename = "err")]
  ErrorResponse(ErrorResponseMessage),
  // Notification(NotificationMessage),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestMessage {
  id: u32,
  data: RequestMessageType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseMessage {
  id: u32,
  data: ResponseMessageType,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorResponseMessage {
  id: Option<u32>,
  message: MaybeStaticStr,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum RequestMessageType {
  #[serde(rename = "Backend/info", skip_serializing)]
  BackendInfo {},
  #[serde(rename = "Project/open", skip_serializing)]
  ProjectOpen { dir: PathBuf },
  #[serde(rename = "Project/close", skip_serializing)]
  ProjectClose { project_id: u32 },
  #[serde(rename = "Project/get_meta", skip_serializing)]
  ProjectGetMeta { project_id: u32 },
  #[serde(rename = "Project/list_tr_files", skip_serializing)]
  ProjectListTrFiles { project_id: u32 },
  #[serde(rename = "Project/list_virtual_game_files", skip_serializing)]
  ProjectListVirtualGameFiles { project_id: u32 },
  #[serde(rename = "VirtualGameFile/list_fragments", skip_serializing)]
  VirtualGameFileListFragments {
    project_id: u32,
    file_path: String,
    start: Option<usize>,
    end: Option<usize>,
    select_fields: Rc<FieldsSelection>,
  },
  #[serde(rename = "VirtualGameFile/get_fragment", skip_serializing)]
  VirtualGameFileGetFragment {
    project_id: u32,
    file_path: String,
    json_path: String,
    select_fields: Rc<FieldsSelection>,
  },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ResponseMessageType {
  #[serde(rename = "ok", skip_deserializing)]
  Ok,
  #[serde(rename = "Backend/info", skip_deserializing)]
  BackendInfo {
    implementation_name: MaybeStaticStr,
    implementation_version: MaybeStaticStr,
    implementation_nice_version: MaybeStaticStr,
  },
  #[serde(rename = "Project/open", skip_deserializing)]
  ProjectOpen { project_id: u32 },
  #[serde(rename = "Project/get_meta", skip_deserializing)]
  ProjectGetMeta {
    root_dir: PathBuf,
    #[serde(with = "utils::serde::CompactUuidHelper")]
    id: Uuid,
    creation_timestamp: Timestamp,
    modification_timestamp: Timestamp,
    game_version: RcString,
    original_locale: RcString,
    reference_locales: Rc<HashSet<RcString>>,
    translation_locale: RcString,
    translations_dir: RcString,
    splitter: MaybeStaticStr,
  },
  #[serde(rename = "Project/list_tr_files", skip_deserializing)]
  ProjectListTrFiles { paths: Vec<RcString> },
  #[serde(rename = "Project/list_virtual_game_files", skip_deserializing)]
  ProjectListVirtualGameFiles { paths: Vec<RcString> },
  #[serde(rename = "VirtualGameFile/list_fragments", skip_deserializing)]
  VirtualGameFileListFragments { fragments: Vec<ListedFragment> },
  #[serde(rename = "VirtualGameFile/get_fragment", skip_deserializing)]
  VirtualGameFileGetFragment { fragment: ListedFragment },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FieldsSelection {
  #[serde(default)]
  fragments: Vec<FragmentField>,
  #[serde(default)]
  translations: Vec<TranslationField>,
  #[serde(default)]
  comments: Vec<CommentField>,
}

macro_rules! backend_fields_enum {
  ({$($tt:tt)+}) => { backend_fields_enum! { $($tt)+ } };

  (
    $(#[$enum_meta:meta])* $visibility:vis enum $enum_name:ident {
      $($(#[$variant_meta:meta])* $field_name:ident),+ $(,)?
    }
  ) => {
    #[derive(Debug, Clone, Copy, Deserialize, Serialize)]
    #[allow(non_camel_case_types)]
    $(#[$enum_meta])*
    $visibility enum $enum_name {
      $($(#[$variant_meta])* $field_name,)+
    }

    impl $enum_name {
      $visibility const ALL: &'static [Self] = &[$(Self::$field_name),+];
    }
  };
}

backend_fields_enum!({
  pub enum FragmentField {
    id,
    tr_file_path,
    game_file_path,
    json_path,
    lang_uid,
    description,
    original_text,
    reference_texts,
    flags,
    translations,
    comments,
  }
});

#[derive(Debug, Clone)]
pub struct ListedFragment {
  pub fragment: Rc<Fragment>,
  pub select_fields: Rc<FieldsSelection>,
}

impl serde::Serialize for ListedFragment {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let fields = &self.select_fields.fragments;
    let mut seq = serializer.serialize_seq(Some(fields.len()))?;
    for field in fields.iter() {
      use FragmentField as F;
      let f = &self.fragment;
      match field {
        F::id => seq.serialize_element(&utils::serde::CompactUuidSerializer(f.id()))?,
        F::tr_file_path => seq.serialize_element(f.tr_file().relative_path())?,
        F::game_file_path => seq.serialize_element(f.virtual_game_file().path())?,
        F::json_path => seq.serialize_element(f.json_path())?,
        F::lang_uid => seq.serialize_element(&f.lang_uid())?,
        F::description => seq.serialize_element(f.description())?,
        F::original_text => seq.serialize_element(f.original_text())?,
        F::reference_texts => seq.serialize_element(&*f.reference_texts())?,
        F::flags => seq.serialize_element(&*f.flags())?,
        F::translations => seq.serialize_element(&utils::serde::SerializeSeqIterator::new(
          f.translations().iter().map(|t| ListedTranslation {
            translation: t.share_rc(),
            select_fields: self.select_fields.share_rc(),
          }),
        ))?,
        F::comments => seq.serialize_element(&utils::serde::SerializeSeqIterator::new(
          f.comments().iter().map(|c| ListedComment {
            comment: c.share_rc(),
            select_fields: self.select_fields.share_rc(),
          }),
        ))?,
      }
    }
    seq.end()
  }
}

backend_fields_enum!({
  pub enum TranslationField {
    id,
    author_username,
    editor_username,
    creation_timestamp,
    modification_timestamp,
    text,
    flags,
  }
});

#[derive(Debug, Clone)]
pub struct ListedTranslation {
  pub translation: Rc<Translation>,
  pub select_fields: Rc<FieldsSelection>,
}

impl serde::Serialize for ListedTranslation {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let fields = &self.select_fields.translations;
    let mut seq = serializer.serialize_seq(Some(fields.len()))?;
    for field in fields.iter() {
      use TranslationField as F;
      let t = &self.translation;
      match field {
        F::id => seq.serialize_element(&utils::serde::CompactUuidSerializer(t.id()))?,
        F::author_username => seq.serialize_element(t.author_username())?,
        F::editor_username => seq.serialize_element(&*t.editor_username())?,
        F::creation_timestamp => seq.serialize_element(&t.creation_timestamp())?,
        F::modification_timestamp => seq.serialize_element(&t.modification_timestamp())?,
        F::text => seq.serialize_element(&*t.text())?,
        F::flags => seq.serialize_element(&*t.flags())?,
      }
    }
    seq.end()
  }
}

backend_fields_enum!({
  pub enum CommentField {
    id,
    author_username,
    editor_username,
    creation_timestamp,
    modification_timestamp,
    text,
  }
});

#[derive(Debug, Clone)]
pub struct ListedComment {
  pub comment: Rc<Comment>,
  pub select_fields: Rc<FieldsSelection>,
}

impl serde::Serialize for ListedComment {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let fields = &self.select_fields.comments;
    let mut seq = serializer.serialize_seq(Some(fields.len()))?;
    for field in fields.iter() {
      use CommentField as F;
      let c = &self.comment;
      match field {
        F::id => seq.serialize_element(&utils::serde::CompactUuidSerializer(c.id()))?,
        F::author_username => seq.serialize_element(c.author_username())?,
        F::editor_username => seq.serialize_element(&*c.editor_username())?,
        F::creation_timestamp => seq.serialize_element(&c.creation_timestamp())?,
        F::modification_timestamp => seq.serialize_element(&c.modification_timestamp())?,
        F::text => seq.serialize_element(&*c.text())?,
      }
    }
    seq.end()
  }
}

macro_rules! backend_nice_error {
  ($message:expr $(, $source:expr)? $(,)?) => {{
    let message = $message;
    #[allow(unused_mut)]
    let mut err = BackendNiceError::from(message);
    $(err.source = Some($source);)?
    return Err(AnyError::from(err));
  }};
}

#[derive(Debug)]
pub struct Backend {
  transport: Box<dyn Transport>,
  project_id_alloc: IdAllocator,
  projects: HashMap<u32, Rc<Project>>,
}

impl Backend {
  pub fn new(transport: Box<dyn Transport>) -> Self {
    Self {
      transport,
      project_id_alloc: IdAllocator::new(),
      // I assume that at least one project will be opened because otherwise
      // (without opening a project) the backend is pretty much useless
      projects: HashMap::with_capacity(1),
    }
  }

  pub fn start(&mut self) -> AnyResult<()> {
    let mut message_index: u32 = 0;
    loop {
      let result = try_any_result!({
        let buf = self.transport.recv().context("Failed to receive message from the transport")?;

        let in_msg: serde_json::Result<Message> = match serde_json::from_str::<Message>(&buf) {
          Err(e) if !e.is_io() => {
            warn!("Failed to deserialize message(index={}): {}", message_index, e);
            Err(e)
          }
          result => Ok(result.context("Failed to deserialize message")?),
        };

        let out_msg: Message = match in_msg {
          Ok(in_msg) => {
            let in_msg_id = match &in_msg {
              Message::Request(RequestMessage { id, .. }) => Some(*id),
              _ => None,
            };
            match self.process_message(in_msg) {
              Ok(out_msg) => out_msg,
              Err(e) => {
                let e: BackendNiceError = e.downcast()?;
                if let Some(e2) = e.source {
                  crate::report_error(e2);
                }
                Message::ErrorResponse(ErrorResponseMessage { id: in_msg_id, message: e.message })
              }
            }
          }
          Err(e) => Message::ErrorResponse(ErrorResponseMessage {
            id: None,
            message: Cow::Owned(e.to_string()),
          }),
        };

        let mut buf = Vec::new();
        serde_json::to_writer(&mut buf, &out_msg).context("Failed to serialize")?;
        // Safe because serde_json doesn't emit invalid UTF-8, and besides JSON
        // files are required to be encoded as UTF-8 by the specification. See
        // <https://tools.ietf.org/html/rfc8259#section-8.1>.
        let buf = unsafe { String::from_utf8_unchecked(buf) };

        self.transport.send(buf).context("Failed to send message to the transport")?;
      })
      .with_context(|| format!("Failed to process message(index={})", message_index));

      match result {
        Err(e) if e.is::<transports::TransportDisconnectionError>() => {
          warn!("The frontend has disconnected, exiting cleanly");
          break;
        }
        _ => {}
      }

      message_index = message_index.wrapping_add(1);
    }

    Ok(())
  }

  #[allow(clippy::unnecessary_wraps)]
  fn process_message(&mut self, message: Message) -> AnyResult<Message> {
    Ok(match message {
      Message::Request(request_msg) => Message::Response(ResponseMessage {
        id: request_msg.id,
        data: self.process_request(request_msg.data)?,
      }),
      Message::Response(_) | Message::ErrorResponse(_) => {
        backend_nice_error!("the backend currently can't receive responses");
      }
    })
  }

  #[allow(clippy::unnecessary_wraps)]
  fn process_request(&mut self, message: RequestMessageType) -> AnyResult<ResponseMessageType> {
    match &message {
      RequestMessageType::BackendInfo {} => Ok(ResponseMessageType::BackendInfo {
        implementation_name: Cow::Borrowed(crate::CRATE_NAME),
        implementation_version: Cow::Borrowed(crate::CRATE_VERSION),
        implementation_nice_version: Cow::Borrowed(crate::CRATE_NICE_VERSION),
      }),

      RequestMessageType::ProjectOpen { dir: project_dir } => {
        let project = match Project::open(project_dir.clone())
          .with_context(|| format!("Failed to open project in {:?}", project_dir))
        {
          Ok(v) => v,
          Err(e) => backend_nice_error!("failed to open project", e),
        };
        let project_id = self.project_id_alloc.next().unwrap();
        self.projects.insert(project_id, project);
        Ok(ResponseMessageType::ProjectOpen { project_id })
      }

      RequestMessageType::ProjectClose { project_id } => match self.projects.remove(project_id) {
        Some(_project) => Ok(ResponseMessageType::Ok),
        None => backend_nice_error!("project ID not found"),
      },

      RequestMessageType::ProjectGetMeta { project_id } => {
        let project = match self.projects.get(project_id) {
          Some(v) => v,
          None => backend_nice_error!("project ID not found"),
        };
        let meta = project.meta();
        Ok(ResponseMessageType::ProjectGetMeta {
          root_dir: project.root_dir().to_owned(),
          id: meta.id(),
          creation_timestamp: meta.creation_timestamp(),
          modification_timestamp: meta.modification_timestamp(),
          game_version: meta.game_version().share_rc(),
          original_locale: meta.original_locale().share_rc(),
          reference_locales: meta.reference_locales().share_rc(),
          translation_locale: meta.translation_locale().share_rc(),
          translations_dir: meta.translations_dir().share_rc(),
          splitter: Cow::Borrowed(meta.splitter().id()),
        })
      }

      RequestMessageType::ProjectListTrFiles { project_id } => {
        let project = match self.projects.get(project_id) {
          Some(v) => v,
          None => backend_nice_error!("project ID not found"),
        };
        let paths: Vec<RcString> = project.tr_files().keys().cloned().collect();
        Ok(ResponseMessageType::ProjectListTrFiles { paths })
      }

      RequestMessageType::ProjectListVirtualGameFiles { project_id } => {
        let project = match self.projects.get(project_id) {
          Some(v) => v,
          None => backend_nice_error!("project ID not found"),
        };
        let paths: Vec<RcString> = project.virtual_game_files().keys().cloned().collect();
        Ok(ResponseMessageType::ProjectListVirtualGameFiles { paths })
      }

      RequestMessageType::VirtualGameFileListFragments {
        project_id,
        file_path,
        start,
        end,
        select_fields,
      } => {
        let project = match self.projects.get(project_id) {
          Some(v) => v,
          None => backend_nice_error!("project ID not found"),
        };
        let game_file = match project.get_virtual_game_file(file_path) {
          Some(v) => v,
          None => backend_nice_error!("virtual game file not found"),
        };
        let all_fragments = game_file.fragments();
        let (start, end) = Self::validate_range(all_fragments.len(), (*start, *end))?;
        let mut listed_fragments = Vec::with_capacity(end.checked_sub(start).unwrap());

        for i in start..end {
          let (_, f) = all_fragments.get_index(i).unwrap();
          listed_fragments.push(ListedFragment {
            fragment: f.share_rc(),
            select_fields: select_fields.share_rc(),
          });
        }

        Ok(ResponseMessageType::VirtualGameFileListFragments { fragments: listed_fragments })
      }

      RequestMessageType::VirtualGameFileGetFragment {
        project_id,
        file_path,
        json_path,
        select_fields,
      } => {
        let project = match self.projects.get(project_id) {
          Some(v) => v,
          None => backend_nice_error!("project ID not found"),
        };
        let game_file = match project.get_virtual_game_file(file_path) {
          Some(v) => v,
          None => backend_nice_error!("virtual game file not found"),
        };
        let f = match game_file.get_fragment(json_path) {
          Some(v) => v,
          None => backend_nice_error!("virtual game file not found"),
        };

        Ok(ResponseMessageType::VirtualGameFileGetFragment {
          fragment: ListedFragment {
            fragment: f.share_rc(),
            select_fields: select_fields.share_rc(),
          },
        })
      }
    }
  }

  /// Based on <https://github.com/rust-lang/rust/blob/0c341226ad3780c11b1f29f6da8172b1d653f9ef/library/core/src/slice/index.rs#L514-L548>.
  fn validate_range(
    len: usize,
    range: (Option<usize>, Option<usize>),
  ) -> AnyResult<(usize, usize)> {
    let (start, end) = range;
    let (start, end) = (start.unwrap_or(0), end.unwrap_or(len));
    if start > end {
      backend_nice_error!("start > end");
    }
    if end > len {
      backend_nice_error!("end > len");
    }
    Ok((start, end))
  }
}

#[derive(Debug, Clone)]
pub struct IdAllocator {
  /// `u32`s are used because JS only has 32-bit integers. And 64-bit floats,
  /// but those aren't really convenient for storing IDs.
  current_id: u32,
  pub only_nonzero: bool,
  pub wrap_around: bool,
}

impl IdAllocator {
  pub fn new() -> Self { Self { current_id: 0, only_nonzero: true, wrap_around: true } }
}

impl Iterator for IdAllocator {
  type Item = u32;
  fn next(&mut self) -> Option<Self::Item> {
    // Clever branchless hack. Will take a max value with 1 when `only_nonzero`
    // is true, will not affect `self.next_id` otherwise.
    let id = self.current_id.max(self.only_nonzero as u32);
    let (next_id, overflow) = id.overflowing_add(1);
    if overflow && !self.wrap_around {
      None
    } else {
      self.current_id = next_id;
      Some(id)
    }
  }
}

#[derive(Debug)]
pub struct BackendNiceError {
  pub message: MaybeStaticStr,
  pub source: Option<AnyError>,
}

impl From<&'static str> for BackendNiceError {
  fn from(message: &'static str) -> Self { Self { message: Cow::Borrowed(message), source: None } }
}

impl From<String> for BackendNiceError {
  fn from(message: String) -> Self { Self { message: Cow::Owned(message), source: None } }
}

impl fmt::Display for BackendNiceError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "BackendError (you are most likely seeing this error because of a bug; it should've been \
      sent over the backend protocol and handled by the frontend): {}",
      self.message,
    )
  }
}

impl StdError for BackendNiceError {
  #[inline]
  fn source(&self) -> Option<&(dyn StdError + 'static)> {
    self.source.as_ref().map(AnyError::as_ref)
  }
}
