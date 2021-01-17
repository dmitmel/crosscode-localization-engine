pub mod json;

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
