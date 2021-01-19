pub mod json;

use std::time::SystemTime;
use uuid::Uuid;

/// The One UUID generator, wrapped into a function for refactorability if
/// needed to switch to another UUID version.
#[inline(always)]
pub fn new_uuid() -> Uuid { Uuid::new_v4() }

pub type Timestamp = u64;

pub fn get_timestamp() -> u64 {
  SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs()
}

pub fn fast_concat(strings: &[&str]) -> String {
  let mut capacity = 0;
  for s in strings {
    capacity += s.len();
  }
  let mut result = String::with_capacity(capacity);
  for s in strings {
    result.push_str(s);
  }
  result
}

// Taken from <https://github.com/bluss/maplit/blob/04936f703da907bc4ffdaced121e4cfd5ecbaec6/src/lib.rs#L77-L93>
#[macro_export(local_inner_macros)]
macro_rules! hashset {
  (@single $($x:tt)*) => (());
  (@count $($rest:expr),*) => (<[()]>::len(&[$(hashset!(@single $rest)),*]));

  ($($key:expr,)+) => { hashset!($($key),+) };
  ($($key:expr),*) => {
    {
      let _cap = hashset!(@count $($key),*);
      let mut _set = ::std::collections::HashSet::with_capacity(_cap);
      $(let _ = _set.insert($key);)*
      _set
    }
  };
}
