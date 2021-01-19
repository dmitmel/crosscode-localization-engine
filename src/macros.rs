#[macro_export(local_inner_macros)]
macro_rules! replace_with_single_token {
  ($($x:tt)*) => {
    ()
  };
}

#[macro_export(local_inner_macros)]
macro_rules! count_exprs {
  ($($rest:expr),*) => {
    <[()]>::len(&[$(replace_with_single_token!($rest)),*])
  };
}

// Taken from <https://github.com/bluss/maplit/blob/04936f703da907bc4ffdaced121e4cfd5ecbaec6/src/lib.rs#L77-L93>
#[macro_export(local_inner_macros)]
macro_rules! hashset {
  ($($key:expr,)+) => { hashset!($($key),+) };
  ($($key:expr),*) => {
    {
      let _cap = count_exprs!($($key),*);
      let mut _set = ::std::collections::HashSet::with_capacity(_cap);
      $(let _ = _set.insert($key);)*
      _set
    }
  };
}
