use crate::impl_prelude::*;

use std::fmt;
use std::io::{self, Write};

assert_trait_is_object_safe!(Transport);
pub trait Transport: fmt::Debug {
  fn recv(&mut self) -> AnyResult<Option<String>>;
  fn send(&mut self, bytes: &[u8]) -> AnyResult<Option<()>>;
}

#[derive(Debug)]
pub struct StdioTransport;

impl Transport for StdioTransport {
  fn recv(&mut self) -> AnyResult<Option<String>> {
    let mut buf = String::new();
    let read_bytes = io::stdin().read_line(&mut buf).context("Failed to read from stdin")?;
    Ok(if read_bytes > 0 { Some(buf) } else { None })
  }

  fn send(&mut self, bytes: &[u8]) -> AnyResult<Option<()>> {
    let mut stdout = io::stdout();
    match try_io_result!({
      stdout.write_all(bytes)?;
      stdout.write_all(b"\n")?;
    }) {
      Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(None),
      result => Ok(Some(result.context("Failed to serialize to stdout")?)),
    }
  }
}
