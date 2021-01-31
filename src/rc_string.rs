#![allow(clippy::partialeq_ne_impl)]

use std::borrow::{Borrow, Cow};
use std::cmp::Ordering;
use std::convert::Infallible;
use std::fmt;
use std::hash::Hash;
use std::ops::Deref;
use std::rc::Rc;
use std::str::FromStr;

#[repr(transparent)]
pub struct RcString(Rc<String>);

impl RcString {
  #[inline(always)]
  pub fn as_str(&self) -> &str { &self.0 }
  #[inline(always)]
  pub fn as_string(&self) -> &String { &self.0 }
  #[inline(always)]
  pub fn as_rc(&self) -> &Rc<String> { &self.0 }
  #[inline(always)]
  pub fn into_rc(self) -> Rc<String> { self.0 }

  #[inline(always)]
  pub fn share_rc(&self) -> RcString { RcString(Rc::clone(&self.0)) }
  #[inline(always)]
  pub fn rc_clone_inner(&self) -> String { (*self.0).clone() }
}

impl From<char> for RcString {
  #[inline(always)]
  fn from(c: char) -> Self { Self(Rc::new(String::from(c))) }
}

impl From<&str> for RcString {
  #[inline(always)]
  fn from(s: &str) -> Self { Self(Rc::new(s.to_owned())) }
}

impl From<&mut str> for RcString {
  #[inline(always)]
  fn from(s: &mut str) -> Self { Self(Rc::new(s.to_owned())) }
}

impl From<String> for RcString {
  #[inline(always)]
  fn from(s: String) -> Self { Self(Rc::new(s)) }
}

impl From<&String> for RcString {
  #[inline(always)]
  fn from(s: &String) -> Self { Self(Rc::new(s.to_owned())) }
}

impl From<Box<String>> for RcString {
  #[inline(always)]
  fn from(s: Box<String>) -> Self { Self(Rc::new(*s)) }
}

impl From<Rc<String>> for RcString {
  #[inline(always)]
  fn from(s: Rc<String>) -> Self { Self(s) }
}

impl From<Cow<'_, str>> for RcString {
  #[inline(always)]
  fn from(s: Cow<'_, str>) -> Self { Self(Rc::new(s.into_owned())) }
}

impl Deref for RcString {
  type Target = String;
  #[inline(always)]
  fn deref(&self) -> &Self::Target { &self.0 }
}

impl Clone for RcString {
  #[inline(always)]
  fn clone(&self) -> Self { Self(self.0.clone()) }
  #[inline(always)]
  fn clone_from(&mut self, source: &Self) { self.0.clone_from(&source.0) }
}

impl Default for RcString {
  #[inline(always)]
  fn default() -> Self { Self(Rc::new(String::default())) }
}

impl PartialEq for RcString {
  #[inline(always)]
  fn eq(&self, other: &Self) -> bool { self.0.eq(&other.0) }
  #[inline(always)]
  fn ne(&self, other: &Self) -> bool { self.0.ne(&other.0) }
}

impl PartialEq<str> for RcString {
  #[inline(always)]
  fn eq(&self, other: &str) -> bool { (*self.0).eq(other) }
  #[inline(always)]
  fn ne(&self, other: &str) -> bool { (*self.0).ne(other) }
}

impl PartialEq<String> for RcString {
  #[inline(always)]
  fn eq(&self, other: &String) -> bool { (*self.0).eq(other) }
  #[inline(always)]
  fn ne(&self, other: &String) -> bool { (*self.0).ne(other) }
}

impl PartialEq<Rc<String>> for RcString {
  #[inline(always)]
  fn eq(&self, other: &Rc<String>) -> bool { self.0.eq(other) }
  #[inline(always)]
  fn ne(&self, other: &Rc<String>) -> bool { self.0.ne(other) }
}

impl Eq for RcString {
}

impl PartialOrd for RcString {
  #[inline(always)]
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> { self.0.partial_cmp(&other.0) }
  #[inline(always)]
  fn lt(&self, other: &Self) -> bool { self.0.lt(&other.0) }
  #[inline(always)]
  fn le(&self, other: &Self) -> bool { self.0.le(&other.0) }
  #[inline(always)]
  fn gt(&self, other: &Self) -> bool { self.0.gt(&other.0) }
  #[inline(always)]
  fn ge(&self, other: &Self) -> bool { self.0.ge(&other.0) }
}

impl PartialOrd<str> for RcString {
  #[inline(always)]
  fn partial_cmp(&self, other: &str) -> Option<Ordering> { (**self.0).partial_cmp(other) }
  #[inline(always)]
  fn lt(&self, other: &str) -> bool { (**self.0).lt(other) }
  #[inline(always)]
  fn le(&self, other: &str) -> bool { (**self.0).le(other) }
  #[inline(always)]
  fn gt(&self, other: &str) -> bool { (**self.0).gt(other) }
  #[inline(always)]
  fn ge(&self, other: &str) -> bool { (**self.0).ge(other) }
}

impl PartialOrd<String> for RcString {
  #[inline(always)]
  fn partial_cmp(&self, other: &String) -> Option<Ordering> { (*self.0).partial_cmp(other) }
  #[inline(always)]
  fn lt(&self, other: &String) -> bool { (*self.0).lt(other) }
  #[inline(always)]
  fn le(&self, other: &String) -> bool { (*self.0).le(other) }
  #[inline(always)]
  fn gt(&self, other: &String) -> bool { (*self.0).gt(other) }
  #[inline(always)]
  fn ge(&self, other: &String) -> bool { (*self.0).ge(other) }
}

impl PartialOrd<Rc<String>> for RcString {
  #[inline(always)]
  fn partial_cmp(&self, other: &Rc<String>) -> Option<Ordering> { self.0.partial_cmp(other) }
  #[inline(always)]
  fn lt(&self, other: &Rc<String>) -> bool { self.0.lt(other) }
  #[inline(always)]
  fn le(&self, other: &Rc<String>) -> bool { self.0.le(other) }
  #[inline(always)]
  fn gt(&self, other: &Rc<String>) -> bool { self.0.gt(other) }
  #[inline(always)]
  fn ge(&self, other: &Rc<String>) -> bool { self.0.ge(other) }
}

impl Ord for RcString {
  #[inline(always)]
  fn cmp(&self, other: &Self) -> Ordering { self.0.cmp(&other.0) }

  #[inline(always)]
  fn max(self, other: Self) -> Self
  where
    Self: Sized,
  {
    Self(self.0.max(other.0))
  }

  #[inline(always)]
  fn min(self, other: Self) -> Self
  where
    Self: Sized,
  {
    Self(self.0.min(other.0))
  }

  #[inline(always)]
  fn clamp(self, min: Self, max: Self) -> Self
  where
    Self: Sized,
  {
    Self(self.0.clamp(min.0, max.0))
  }
}

impl Hash for RcString {
  #[inline(always)]
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.0.hash(state) }
}

impl fmt::Display for RcString {
  #[inline(always)]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

impl fmt::Debug for RcString {
  #[inline(always)]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

impl fmt::Pointer for RcString {
  #[inline(always)]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}

impl Borrow<str> for RcString {
  #[inline(always)]
  fn borrow(&self) -> &str { &self.0 }
}

impl Borrow<String> for RcString {
  #[inline(always)]
  fn borrow(&self) -> &String { &self.0 }
}

impl Borrow<Rc<String>> for RcString {
  #[inline(always)]
  fn borrow(&self) -> &Rc<String> { &self.0 }
}

impl AsRef<str> for RcString {
  #[inline(always)]
  fn as_ref(&self) -> &str { &self.0 }
}

impl AsRef<String> for RcString {
  #[inline(always)]
  fn as_ref(&self) -> &String { &self.0 }
}

impl AsRef<Rc<String>> for RcString {
  #[inline(always)]
  fn as_ref(&self) -> &Rc<String> { &self.0 }
}

impl AsRef<[u8]> for RcString {
  #[inline(always)]
  fn as_ref(&self) -> &[u8] { (*self.0).as_ref() }
}

impl Unpin for RcString {
}

impl FromStr for RcString {
  type Err = Infallible;
  #[inline(always)]
  fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self::from(s)) }
}

impl serde::Serialize for RcString {
  #[inline(always)]
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    self.as_str().serialize(serializer)
  }
}

impl<'de> serde::Deserialize<'de> for RcString {
  #[inline(always)]
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    String::deserialize(deserializer).map(Self::from)
  }
}