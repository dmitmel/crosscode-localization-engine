use crate::impl_prelude::*;

use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::io::{self, Write};

pub const PROTOCOL_VERSION: u32 = 0;

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IncomingMessage {
  Handshake { protocol_version: u32 },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutgoingMessage {
  HandshakeResponse { protocol_version: u32 },
  Error { message: Cow<'static, str> },
}

#[derive(Debug)]
pub struct Backend {
  handshake_received: bool,
}

impl Backend {
  pub fn new() -> Self { Self { handshake_received: false } }

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
        let in_msg = serde_json::from_str::<IncomingMessage>(&buf)
          .context("Failed to deserialize an IncomingMessage")?;
        let out_msg = self.process_message(in_msg)?;
        serde_json::to_writer(&mut stdout, &out_msg).context("Failed to serialize to stdout")?;
        stdout.write_all(b"\n").context("Failed to write to stdout")?;
        if let OutgoingMessage::Error { message } = out_msg {
          bail!("{}", message);
        }
      })
      .with_context(|| format!("Failed to process message #{}", message_index))?;
      message_index = message_index.wrapping_add(1);
    }
    Ok(())
  }

  #[allow(clippy::unnecessary_wraps)]
  fn process_message(&mut self, message: IncomingMessage) -> AnyResult<OutgoingMessage> {
    if !self.handshake_received {
      let protocol_version = match message {
        IncomingMessage::Handshake { protocol_version } => protocol_version,
        _ => return Ok(OutgoingMessage::Error { message: "expected a handshake message".into() }),
      };
      if protocol_version != PROTOCOL_VERSION {
        return Ok(OutgoingMessage::Error { message: "unsupported protocol version".into() });
      }

      self.handshake_received = true;
      return Ok(OutgoingMessage::HandshakeResponse { protocol_version });
    }

    Ok(OutgoingMessage::Error { message: "unimplemented".into() })
  }
}
