use crate::impl_prelude::*;
use crate::project::Project;
use crate::rc_string::{MaybeStaticStr, RcString};
use crate::utils::Timestamp;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::error::Error as StdError;
use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;
use std::rc::Rc;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 0;

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
  id: u32,
  message: MaybeStaticStr,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum RequestMessageType {
  #[serde(rename = "handshake")]
  Handshake { protocol_version: u32 },
  #[serde(rename = "Project/open")]
  ProjectOpen { dir: PathBuf },
  #[serde(rename = "Project/close")]
  ProjectClose { project_id: u32 },
  #[serde(rename = "Project/meta/get")]
  ProjectMetaGet { project_id: u32 },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum ResponseMessageType {
  #[serde(rename = "ok")]
  Ok,
  #[serde(rename = "handshake")]
  Handshake {
    protocol_version: u32,
    implementation_name: MaybeStaticStr,
    implementation_version: MaybeStaticStr,
  },
  #[serde(rename = "Project/open")]
  ProjectOpen { project_id: u32 },
  #[serde(rename = "Project/meta/get")]
  ProjectMetaGet {
    root_dir: PathBuf,
    id: Uuid,
    creation_timestamp: Timestamp,
    modification_timestamp: Timestamp,
    game_version: RcString,
    original_locale: RcString,
    reference_locales: Vec<RcString>,
    translation_locale: RcString,
    translations_dir: RcString,
    splitter: MaybeStaticStr,
  },
}

macro_rules! backend_nice_error {
  ($expr:expr $(,)?) => {
    return Err(AnyError::from(BackendNiceError::from($expr)));
  };
}

#[derive(Debug)]
pub struct Backend {
  handshake_received: bool,
  project_id_alloc: IdAllocator,
  projects: HashMap<u32, Rc<Project>>,
}

impl Backend {
  pub fn new() -> Self {
    Self {
      handshake_received: false,
      project_id_alloc: IdAllocator::new(),
      // I assume that at least one project will be opened because otherwise
      // (without opening a project) the backend is pretty much useless
      projects: HashMap::with_capacity(1),
    }
  }

  pub fn start(&mut self) -> AnyResult<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut message_index: u32 = 0;
    loop {
      try_any_result!({
        let mut buf = String::new();
        let read_bytes = stdin.read_line(&mut buf).context("Failed to read from stdin")?;
        if read_bytes == 0 {
          break;
        }

        let in_msg: serde_json::Result<Message> = match serde_json::from_str::<Message>(&buf) {
          Err(e) if !e.is_io() => {
            warn!("Failed to deserialize message(index={}) from stdin: {}", message_index, e);
            Err(e)
          }
          result => Ok(result.context("Failed to deserialize from stdin")?),
        };

        let out_msg: Message = match in_msg {
          Ok(in_msg) => {
            let in_msg_id = match &in_msg {
              Message::Request(RequestMessage { id, .. }) => *id,
              Message::Response(ResponseMessage { id, .. }) => *id,
              Message::ErrorResponse(ErrorResponseMessage { id, .. }) => *id,
            };
            match self.process_message(in_msg) {
              Ok(out_msg) => out_msg,
              Err(e) => Message::ErrorResponse(ErrorResponseMessage {
                id: in_msg_id,
                message: e.downcast::<BackendNiceError>()?.message,
              }),
            }
          }
          Err(e) => {
            Message::ErrorResponse(ErrorResponseMessage { id: 0, message: e.to_string().into() })
          }
        };

        match try_io_result!({
          serde_json::to_writer(&mut stdout, &out_msg)?;
          stdout.write_all(b"\n")?;
        }) {
          Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
            warn!("The frontend has disconnected, exiting cleanly. Caused by: {}", e);
            break;
          }
          result => result.context("Failed to serialize to stdout")?,
        }
      })
      .with_context(|| format!("Failed to process message(index={})", message_index))?;

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
    if !self.handshake_received {
      match message {
        RequestMessageType::Handshake { protocol_version: PROTOCOL_VERSION } => {}
        RequestMessageType::Handshake { protocol_version: _ } => {
          backend_nice_error!("unsupported protocol version");
        }
        _ => {
          backend_nice_error!("expected a handshake message");
        }
      };

      self.handshake_received = true;
      return Ok(ResponseMessageType::Handshake {
        protocol_version: PROTOCOL_VERSION,
        implementation_name: crate::CRATE_NAME.into(),
        implementation_version: crate::CRATE_VERSION.into(),
      });
    }

    match &message {
      RequestMessageType::Handshake { .. } => unimplemented!(),

      RequestMessageType::ProjectOpen { dir: project_dir } => {
        let project = match Project::open(project_dir.clone())
          .with_context(|| format!("Failed to open project in {:?}", project_dir))
        {
          Ok(v) => v,
          Err(e) => {
            crate::report_error(e);
            backend_nice_error!("failed to open project");
          }
        };
        let project_id = self.project_id_alloc.next().unwrap();
        self.projects.insert(project_id, project);
        Ok(ResponseMessageType::ProjectOpen { project_id })
      }

      RequestMessageType::ProjectClose { project_id } => match self.projects.remove(&project_id) {
        Some(_project) => Ok(ResponseMessageType::Ok),
        None => backend_nice_error!("project ID not found"),
      },

      RequestMessageType::ProjectMetaGet { project_id } => match self.projects.get(&project_id) {
        Some(project) => Ok({
          let meta = project.meta();
          ResponseMessageType::ProjectMetaGet {
            root_dir: project.root_dir().to_owned(),
            id: meta.id(),
            creation_timestamp: meta.creation_timestamp(),
            modification_timestamp: meta.modification_timestamp(),
            game_version: meta.game_version().share_rc(),
            original_locale: meta.original_locale().share_rc(),
            reference_locales: meta.reference_locales().to_owned(),
            translation_locale: meta.translation_locale().share_rc(),
            translations_dir: meta.translations_dir().share_rc(),
            splitter: Cow::Borrowed(meta.splitter().id()),
          }
        }),
        None => backend_nice_error!("project ID not found"),
      },
      // _ => backend_nice_error!("unimplemented"),
    }
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
}

impl From<&'static str> for BackendNiceError {
  fn from(message: &'static str) -> Self { Self { message: Cow::Borrowed(message) } }
}

impl From<String> for BackendNiceError {
  fn from(message: String) -> Self { Self { message: Cow::Owned(message) } }
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
  #[inline(always)]
  fn source(&self) -> Option<&(dyn StdError + 'static)> { None }
}
