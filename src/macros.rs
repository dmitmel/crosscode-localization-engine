#[macro_export]
macro_rules! replace_with_single_token {
  ($($x:tt)*) => {
    ()
  };
}

#[macro_export]
macro_rules! count_exprs {
  ($($rest:expr),*) => {
    <[()]>::len(&[$(replace_with_single_token!($rest)),*])
  };
}

/// Taken from <https://github.com/bluss/maplit/blob/04936f703da907bc4ffdaced121e4cfd5ecbaec6/src/lib.rs#L77-L93>
#[macro_export]
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

/// Based on <https://github.com/bluss/maplit/blob/04936f703da907bc4ffdaced121e4cfd5ecbaec6/src/lib.rs#L46-L61>.
#[macro_export]
macro_rules! hashmap {
  ($(($key:expr, $value:expr),)+) => { hashmap!($(($key, $value)),+) };
  ($(($key:expr, $value:expr)),*) => {
    {
      let _cap = count_exprs!($($key),*);
      let mut _map = ::std::collections::HashMap::with_capacity(_cap);
      $(let _ = _map.insert($key, $value);)*
      _map
    }
  };
}

#[macro_export]
macro_rules! try_any_result {
  ($block:block) => {{
    let result: AnyResult<_> = try { $block };
    result
  }};
}

#[macro_export]
macro_rules! try_io_result {
  ($block:block) => {{
    let result: std::io::Result<_> = try { $block };
    result
  }};
}

#[macro_export]
macro_rules! try_option {
  ($block:block) => {{
    let result: Option<_> = try { $block };
    result
  }};
}

#[macro_export]
macro_rules! assert_trait_is_object_safe {
  ($($trait:tt)+) => {
    #[doc(hidden)]
    const _: ::std::marker::PhantomData<dyn $($trait)+> = ::std::marker::PhantomData;
  };
}
