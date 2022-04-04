use crate::impl_prelude::*;

use std::fmt;
use std::io::{self, Write};
use std::sync::mpsc;

assert_trait_is_object_safe!(Transport);
pub trait Transport: fmt::Debug + Send {
  fn recv(&self) -> AnyResult<String>;
  fn send(&self, text: String) -> AnyResult<()>;
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
  fn recv(&self) -> AnyResult<String> {
    let mut buf = String::new();
    let read_bytes = io::stdin().read_line(&mut buf).context("Failed to read from stdin")?;
    if read_bytes > 0 {
      Ok(buf)
    } else {
      Err(TransportDisconnectionError.into())
    }
  }

  fn send(&self, text: String) -> AnyResult<()> {
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
pub struct MpscChannelTransport {
  pub receiver: mpsc::Receiver<String>,
  pub sender: mpsc::Sender<String>,
}

impl Transport for MpscChannelTransport {
  fn recv(&self) -> AnyResult<String> {
    match self.receiver.recv() {
      Ok(v) => Ok(v),
      Err(mpsc::RecvError) => Err(TransportDisconnectionError.into()),
    }
  }

  fn send(&self, text: String) -> AnyResult<()> {
    match self.sender.send(text) {
      Ok(()) => Ok(()),
      Err(mpsc::SendError(_)) => Err(TransportDisconnectionError.into()),
    }
  }
}
