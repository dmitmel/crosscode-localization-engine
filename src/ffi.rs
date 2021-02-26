// TODO: Catch panics, see <https://github.com/hyperium/hyper/blob/4c946af49cc7fbbc6bd4894283a654625c2ea383/src/ffi/macros.rs>.
#![allow(non_camel_case_types, clippy::not_unsafe_ptr_arg_deref)]

// use crate::backend::Backend;

use std::os::raw::c_void;
use std::ptr;

#[no_mangle]
pub extern "C" fn crosslocale_init_logging() { crate::init_logging(); }

#[derive(Debug)]
pub struct crosslocale_backend_t {
  message_callback: extern "C" fn(user_data: *mut c_void, message: crosslocale_message_t),
  message_callback_user_data: *mut c_void,
}

pub type crosslocale_message_t = u32;

/// cbindgen:ignore
extern "C" fn message_callback_nop(_user_data: *mut c_void, _message: crosslocale_message_t) {
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_new() -> *mut crosslocale_backend_t {
  Box::into_raw(Box::new(crosslocale_backend_t {
    message_callback: message_callback_nop,
    message_callback_user_data: ptr::null_mut(),
  }))
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_free(myself: *mut crosslocale_backend_t) {
  drop(unsafe { Box::from_raw(myself) });
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_set_message_callback(
  myself: *mut crosslocale_backend_t,
  callback: Option<extern "C" fn(user_data: *mut c_void, message: crosslocale_message_t)>,
  user_data: *mut c_void,
) {
  let mut myself = unsafe { &mut *myself };
  if let Some(callback) = callback {
    myself.message_callback = callback;
    myself.message_callback_user_data = user_data;
  } else {
    myself.message_callback = message_callback_nop;
    myself.message_callback_user_data = ptr::null_mut();
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_recv_message(
  _myself: *mut crosslocale_backend_t,
) -> *mut crosslocale_message_t {
  ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_send_message(
  myself: *mut crosslocale_backend_t,
  message: u32,
) {
  let myself = unsafe { &*myself };
  (myself.message_callback)(myself.message_callback_user_data, message);
}
