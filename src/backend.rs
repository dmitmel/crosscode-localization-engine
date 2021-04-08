pub mod transports;

use self::transports::Transport;
use crate::impl_prelude::*;
use crate::project::Project;
use crate::rc_string::{MaybeStaticStr, RcString};
use crate::utils::{self, RcExt, Timestamp};

use once_cell::sync::Lazy;
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
  },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ResponseMessageType {
  #[serde(rename = "ok", skip_deserializing)]
  Ok,
  #[serde(rename = "Backend/info", skip_deserializing)]
  BackendInfo { implementation_name: MaybeStaticStr, implementation_version: MaybeStaticStr },
  #[serde(rename = "Project/open", skip_deserializing)]
  ProjectOpen { project_id: u32 },
  #[serde(rename = "Project/get_meta", skip_deserializing)]
  ProjectGetMeta {
    root_dir: PathBuf,
    id: Uuid,
    creation_timestamp: Timestamp,
    modification_timestamp: Timestamp,
    game_version: RcString,
    original_locale: RcString,
    reference_locales: Rc<Vec<RcString>>,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct ListedFragment {
  pub id: Uuid,
  #[serde(rename = "json")]
  pub json_path: RcString,
  #[serde(skip_serializing_if = "utils::is_default", rename = "luid")]
  pub lang_uid: i32,
  #[serde(skip_serializing_if = "Vec::is_empty", rename = "desc")]
  pub description: Rc<Vec<RcString>>,
  #[serde(rename = "orig")]
  pub original_text: RcString,
  // #[serde(rename = "refs", skip_serializing_if = "HashMap::is_empty")]
  // pub reference_texts: Rc<HashMap<RcString, RcString>>,
  #[serde(skip_serializing_if = "HashSet::is_empty")]
  pub flags: Rc<HashSet<RcString>>,
  #[serde(skip_serializing_if = "Vec::is_empty", rename = "tr")]
  pub translations: Vec<ListedTranslation>,
  #[serde(skip_serializing_if = "Vec::is_empty", rename = "cm")]
  pub comments: Vec<ListedComment>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListedTranslation {
  pub id: Uuid,
  #[serde(rename = "author")]
  pub author_username: RcString,
  #[serde(rename = "editor")]
  pub editor_username: RcString,
  #[serde(rename = "ctime")]
  pub creation_timestamp: Timestamp,
  #[serde(rename = "mtime")]
  pub modification_timestamp: Timestamp,
  pub text: RcString,
  #[serde(skip_serializing_if = "HashSet::is_empty")]
  pub flags: Rc<HashSet<RcString>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ListedComment {
  pub id: Uuid,
  #[serde(rename = "author")]
  pub author_username: RcString,
  #[serde(rename = "editor")]
  pub editor_username: RcString,
  #[serde(rename = "ctime")]
  pub creation_timestamp: Timestamp,
  #[serde(rename = "mtime")]
  pub modification_timestamp: Timestamp,
  pub text: RcString,
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

      RequestMessageType::VirtualGameFileListFragments { project_id, file_path, start, end } => {
        let project = match self.projects.get(project_id) {
          Some(v) => v,
          None => backend_nice_error!("project ID not found"),
        };
        let virt_file = match project.get_virtual_game_file(file_path) {
          Some(v) => v,
          None => backend_nice_error!("virtual game file not found"),
        };
        let all_fragments = virt_file.fragments();
        let (start, end) = Self::validate_range(all_fragments.len(), (*start, *end))?;
        let mut listed_fragments = Vec::with_capacity(end.checked_sub(start).unwrap());

        for i in start..end {
          let (_, f) = all_fragments.get_index(i).unwrap();
          listed_fragments.push(ListedFragment {
            id: f.id(),
            json_path: f.json_path().share_rc(),
            lang_uid: f.lang_uid(),
            description: f.description().share_rc(),
            original_text: f.original_text().share_rc(),
            // reference_texts: f.reference_texts().share_rc(),
            flags: f.flags().share_rc(),

            translations: f
              .translations()
              .iter()
              .map(|t| ListedTranslation {
                id: t.id(),
                author_username: t.author_username().share_rc(),
                editor_username: t.editor_username().share_rc(),
                creation_timestamp: t.creation_timestamp(),
                modification_timestamp: t.modification_timestamp(),
                text: t.text().share_rc(),
                flags: t.flags().share_rc(),
              })
              .collect(),

            comments: f
              .comments()
              .iter()
              .map(|t| ListedComment {
                id: t.id(),
                author_username: t.author_username().share_rc(),
                editor_username: t.editor_username().share_rc(),
                creation_timestamp: t.creation_timestamp(),
                modification_timestamp: t.modification_timestamp(),
                text: t.text().share_rc(),
              })
              .collect(),
          });
        }

        Ok(ResponseMessageType::VirtualGameFileListFragments { fragments: listed_fragments })
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
