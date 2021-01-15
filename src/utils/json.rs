use std::borrow::Cow;

pub use serde_json::Value;

pub fn type_name_of(v: &Value) -> &'static str {
  match v {
    Value::Null => "null",
    Value::Bool(_) => "bool",
    Value::Number(_) => "number",
    Value::String(_) => "string",
    Value::Array(_) => "array",
    Value::Object(_) => "object",
  }
}

pub trait ValueExt {
  fn entries_iter(&self) -> Option<ValueEntriesIter>;
}

impl ValueExt for Value {
  #[inline(always)]
  fn entries_iter(&self) -> Option<ValueEntriesIter> { ValueEntriesIter::new(self) }
}

#[allow(missing_debug_implementations)]
pub enum ValueEntriesIter<'a> {
  Array { iter: std::slice::Iter<'a, Value>, counter: usize },
  Object { iter: serde_json::map::Iter<'a> },
}

impl<'a> ValueEntriesIter<'a> {
  fn new(value: &'a Value) -> Option<Self> {
    Some(match value {
      Value::Array(vec) => Self::Array { iter: vec.iter(), counter: 0 },
      Value::Object(map) => Self::Object { iter: map.iter() },
      _ => return None,
    })
  }
}

impl<'a> Iterator for ValueEntriesIter<'a> {
  type Item = (Cow<'a, str>, &'a Value);

  fn next(&mut self) -> Option<Self::Item> {
    match self {
      Self::Array { iter, counter, .. } => {
        let (k, v) = (counter.to_string(), iter.next()?);
        *counter += 1;
        Some((Cow::Owned(k), v))
      }
      Self::Object { iter, .. } => {
        let (k, v) = iter.next()?;
        Some((Cow::Borrowed(k), v))
      }
    }
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    match self {
      Self::Array { iter, .. } => iter.size_hint(),
      Self::Object { iter, .. } => iter.size_hint(),
    }
  }

  fn count(self) -> usize {
    match self {
      Self::Array { iter, .. } => iter.count(),
      Self::Object { iter, .. } => iter.count(),
    }
  }
}
