use crate::impl_prelude::*;
use crate::project::Project;
use crate::rc_string::MaybeStaticStr;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::rc::Rc;

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

impl Message {
  pub fn id(&self) -> Option<u32> {
    match self {
      Self::Request(RequestMessage { id, .. }) => Some(*id),
      Self::Response(ResponseMessage { id, .. }) => Some(*id),
      Self::ErrorResponse(ErrorResponseMessage { id, .. }) => Some(*id),
    }
  }

  pub fn error_response(&self, message: impl Into<MaybeStaticStr>) -> ErrorResponseMessage {
    ErrorResponseMessage { id: self.id().unwrap_or(0), message: message.into() }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestMessage {
  id: u32,
  data: RequestMessageType,
}

impl From<RequestMessage> for Message {
  #[inline(always)]
  fn from(v: RequestMessage) -> Self { Self::Request(v) }
}

impl RequestMessage {
  #[inline(always)]
  pub fn response(&self, data: ResponseMessageType) -> ResponseMessage {
    ResponseMessage { id: self.id, data }
  }

  #[inline(always)]
  pub fn error_response(&self, message: impl Into<MaybeStaticStr>) -> ErrorResponseMessage {
    ErrorResponseMessage { id: self.id, message: message.into() }
  }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponseMessage {
  id: u32,
  data: ResponseMessageType,
}

impl From<ResponseMessage> for Message {
  #[inline(always)]
  fn from(v: ResponseMessage) -> Self { Self::Response(v) }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ErrorResponseMessage {
  id: u32,
  message: MaybeStaticStr,
}

impl From<ErrorResponseMessage> for Message {
  #[inline(always)]
  fn from(v: ErrorResponseMessage) -> Self { Self::ErrorResponse(v) }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum RequestMessageType {
  #[serde(rename = "handshake")]
  Handshake { protocol_version: u32 },
  #[serde(rename = "Project/open")]
  ProjectOpen { dir: PathBuf },
  #[serde(rename = "Project/close")]
  ProjectClose { id: u32 },
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
  ProjectOpen { id: u32 },
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
        fn handle_broken_pipe<T: Default>(result: io::Result<T>) -> io::Result<T> {
          match result {
            Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
              warn!("The frontend has disconnected, exiting cleanly. Caused by: {}", e);
              Ok(T::default())
            }
            _ => result,
          }
        }

        let mut buf = String::new();
        let read_bytes =
          handle_broken_pipe(stdin.read_line(&mut buf)).context("Failed to read from stdin")?;
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
          Ok(in_msg) => self.process_message(in_msg)?,
          Err(e) => ErrorResponseMessage { id: 0, message: e.to_string().into() }.into(),
        };

        handle_broken_pipe(
          try {
            serde_json::to_writer(&mut stdout, &out_msg)?;
            stdout.write_all(b"\n")?;
          },
        )
        .context("Failed to serialize to stdout")?;
      })
      .with_context(|| format!("Failed to process message(index={})", message_index))?;

      message_index = message_index.wrapping_add(1);
    }
    Ok(())
  }

  #[allow(clippy::unnecessary_wraps)]
  fn process_message(&mut self, message: Message) -> AnyResult<Message> {
    let request_msg = match message {
      Message::Request(v) => v,
      _ => {
        return Ok(message.error_response("the backend currently can't receive responses").into());
      }
    };

    if !self.handshake_received {
      match request_msg.data {
        RequestMessageType::Handshake { protocol_version: PROTOCOL_VERSION } => {}
        RequestMessageType::Handshake { protocol_version: _ } => {
          return Ok(request_msg.error_response("unsupported protocol version").into());
        }
        _ => {
          return Ok(request_msg.error_response("expected a handshake message").into());
        }
      };

      self.handshake_received = true;
      return Ok(
        request_msg
          .response(ResponseMessageType::Handshake {
            protocol_version: PROTOCOL_VERSION,
            implementation_name: crate::CRATE_NAME.into(),
            implementation_version: crate::CRATE_VERSION.into(),
          })
          .into(),
      );
    }

    match &request_msg.data {
      RequestMessageType::ProjectOpen { dir: project_dir } => {
        let project = match Project::open(project_dir.clone())
          .with_context(|| format!("Failed to open project in {:?}", project_dir))
        {
          Ok(v) => v,
          Err(e) => {
            crate::report_error(e);
            return Ok(request_msg.error_response("failed to open project").into());
          }
        };
        let project_id = self.project_id_alloc.next().unwrap();
        self.projects.insert(project_id, project);
        Ok(request_msg.response(ResponseMessageType::ProjectOpen { id: project_id }).into())
      }

      RequestMessageType::ProjectClose { id: project_id } => {
        match self.projects.remove(&project_id) {
          Some(_project) => Ok(request_msg.response(ResponseMessageType::Ok {}).into()),
          None => Ok(request_msg.error_response("project ID not found").into()),
        }
      }

      _ => Ok(request_msg.error_response("unimplemented").into()),
    }
  }
}

#[derive(Debug, Clone)]
pub struct IdAllocator {
  // `u32`s are used because JS only has 32-bit integers. And 64-bit floats,
  // but those aren't really convenient for storing IDs.
  current_id: u32,
  only_nonzero: bool,
  wrap_around: bool,
}

impl IdAllocator {
  pub fn new() -> Self { Self { current_id: 0, only_nonzero: true, wrap_around: true } }
  #[inline(always)]
  pub fn set_only_nonzero(&mut self, only_nonzero: bool) { self.only_nonzero = only_nonzero; }
  #[inline(always)]
  pub fn set_wrap_around(&mut self, wrap_around: bool) { self.wrap_around = wrap_around; }
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
