use crate::impl_prelude::*;

use serde_json::Value as JsonValue;
use std::fmt;
use std::io::{self, Write};
use std::sync::mpsc;

#[derive(Debug)]
pub enum TransportedValue {
  Parsed(serde_json::Result<JsonValue>),
  Json(String),
}

assert_trait_is_object_safe!(Transport);
pub trait Transport: fmt::Debug {
  fn uses_parsed_values(&self) -> bool;
  fn recv(&mut self) -> AnyResult<TransportedValue>;
  fn send(&mut self, text: TransportedValue) -> AnyResult<()>;
}

#[derive(Debug)]
pub struct TransportDisconnectionError;

impl fmt::Display for TransportDisconnectionError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Transport has disconnected")
  }
}

impl StdError for TransportDisconnectionError {
}

#[derive(Debug)]
pub struct StdioTransport;

impl Transport for StdioTransport {
  #[inline(always)]
  fn uses_parsed_values(&self) -> bool { false }

  fn recv(&mut self) -> AnyResult<TransportedValue> {
    let mut buf = String::new();
    let read_bytes = io::stdin().read_line(&mut buf).context("Failed to read from stdin")?;
    if read_bytes > 0 {
      Ok(TransportedValue::Json(buf))
    } else {
      Err(TransportDisconnectionError.into())
    }
  }

  fn send(&mut self, value: TransportedValue) -> AnyResult<()> {
    let text = match value {
      TransportedValue::Json(v) => v,
      _ => unreachable!(),
    };
    let mut stdout = io::stdout();
    match try_io_result!({
      stdout.write_all(text.as_bytes())?;
      stdout.write_all(b"\n")?;
    }) {
      Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Err(TransportDisconnectionError.into()),
      result => Ok(result.context("Failed to serialize to stdout")?),
    }
  }
}

#[derive(Debug)]
pub struct FfiChannelTransport {
  pub receiver: mpsc::Receiver<JsonValue>,
  pub sender: mpsc::Sender<JsonValue>,
}

impl Transport for FfiChannelTransport {
  #[inline(always)]
  fn uses_parsed_values(&self) -> bool { true }

  fn recv(&mut self) -> AnyResult<TransportedValue> {
    match self.receiver.recv() {
      Ok(v) => Ok(TransportedValue::Parsed(Ok(v))),
      Err(mpsc::RecvError) => Err(TransportDisconnectionError.into()),
    }
  }

  fn send(&mut self, value: TransportedValue) -> AnyResult<()> {
    let value = match value {
      TransportedValue::Parsed(Ok(v)) => v,
      _ => unreachable!(),
    };
    match self.sender.send(value) {
      Ok(()) => Ok(()),
      Err(mpsc::SendError(_)) => Err(TransportDisconnectionError.into()),
    }
  }
}
