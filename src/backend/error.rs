use crate::impl_prelude::*;
use crate::rc_string::MaybeStaticStr;

use std::borrow::Cow;
use std::fmt;

#[macro_export]
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
