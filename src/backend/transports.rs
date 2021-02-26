use crate::impl_prelude::*;

use std::fmt;
use std::io::{self, Write};

assert_trait_is_object_safe!(Transport);
pub trait Transport: fmt::Debug {
  fn recv(&mut self) -> AnyResult<String>;
  fn send(&mut self, bytes: &[u8]) -> AnyResult<()>;
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
  fn recv(&mut self) -> AnyResult<String> {
    let mut buf = String::new();
    let read_bytes = io::stdin().read_line(&mut buf).context("Failed to read from stdin")?;
    if read_bytes > 0 {
      Ok(buf)
    } else {
      Err(TransportDisconnectionError.into())
    }
  }

  fn send(&mut self, bytes: &[u8]) -> AnyResult<()> {
    let mut stdout = io::stdout();
    match try_io_result!({
      stdout.write_all(bytes)?;
      stdout.write_all(b"\n")?;
    }) {
      Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Err(TransportDisconnectionError.into()),
      result => Ok(result.context("Failed to serialize to stdout")?),
    }
  }
}
