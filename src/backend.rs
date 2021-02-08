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

  pub fn error_response(&self, message: MaybeStaticStr) -> ErrorResponseMessage {
    ErrorResponseMessage { id: self.id().unwrap_or(0), message }
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
  pub fn error_response(&self, message: MaybeStaticStr) -> ErrorResponseMessage {
    ErrorResponseMessage { id: self.id, message }
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
  next_project_id: u32,
  projects: HashMap<u32, Rc<Project>>,
}

impl Backend {
  pub fn new() -> Self {
    Self {
      handshake_received: false,
      next_project_id: 1,
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
        let in_msg = serde_json::from_str::<Message>(&buf)
          .context("Failed to deserialize a message from stdin")?;
        let out_msg = self.process_message(in_msg)?;
        serde_json::to_writer(&mut stdout, &out_msg)
          .context("Failed to serialize a message to stdout")?;
        stdout.write_all(b"\n").context("Failed to write to stdout")?;
        if let Message::ErrorResponse(ErrorResponseMessage { message, .. }) = out_msg {
          bail!("{}", message);
        }
      })
      .with_context(|| format!("Failed to process message with index {}", message_index))?;
      message_index = message_index.wrapping_add(1);
    }
    Ok(())
  }

  #[allow(clippy::unnecessary_wraps)]
  fn process_message(&mut self, message: Message) -> AnyResult<Message> {
    let request_msg = match message {
      Message::Request(v) => v,
      _ => {
        return Ok(
          message.error_response("the backend currently can't receive responses".into()).into(),
        );
      }
    };

    if !self.handshake_received {
      match request_msg.data {
        RequestMessageType::Handshake { protocol_version: PROTOCOL_VERSION } => {}
        RequestMessageType::Handshake { protocol_version: _ } => {
          return Ok(request_msg.error_response("unsupported protocol version".into()).into());
        }
        _ => {
          return Ok(request_msg.error_response("expected a handshake message".into()).into());
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
        self.next_project_id = self.next_project_id.max(1);
        let project_id = self.next_project_id;
        self.next_project_id = self.next_project_id.wrapping_add(1);

        let project = Project::open(project_dir.clone())
          .with_context(|| format!("Failed to open project in {:?}", project_dir))?;
        self.projects.insert(project_id, project);
        Ok(request_msg.response(ResponseMessageType::ProjectOpen { id: project_id }).into())
      }

      RequestMessageType::ProjectClose { id: project_id } => {
        match self.projects.remove(&project_id) {
          Some(_project) => Ok(request_msg.response(ResponseMessageType::Ok {}).into()),
          None => Ok(request_msg.error_response("project ID not found".into()).into()),
        }
      }

      _ => Ok(request_msg.error_response("unimplemented".into()).into()),
    }
  }
}
