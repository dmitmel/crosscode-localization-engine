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
