// TODO: Catch panics, see <https://github.com/hyperium/hyper/blob/4c946af49cc7fbbc6bd4894283a654625c2ea383/src/ffi/macros.rs>.
#![allow(
  non_camel_case_types,
  clippy::not_unsafe_ptr_arg_deref,
  unreachable_patterns,
  missing_debug_implementations
)]

use crate::backend::transports::FfiChannelTransport;
use crate::backend::Backend;

use serde_json::Value as JsonValue;
use std::mem::ManuallyDrop;
use std::panic::{self, AssertUnwindSafe};
use std::process;
use std::ptr;
use std::slice;
use std::str;
use std::sync::mpsc;
use std::thread;

#[no_mangle]
pub static CROSSLOCALE_FFI_BRIDGE_VERSION: u32 = 3;

#[no_mangle]
pub static CROSSLOCALE_VERSION_PTR: &u8 = &crate::CRATE_VERSION.as_bytes()[0];
#[no_mangle]
pub static CROSSLOCALE_VERSION_LEN: usize = crate::CRATE_VERSION.len();
#[no_mangle]
pub static CROSSLOCALE_NICE_VERSION_PTR: &u8 = &crate::CRATE_NICE_VERSION.as_bytes()[0];
#[no_mangle]
pub static CROSSLOCALE_NICE_VERSION_LEN: usize = crate::CRATE_NICE_VERSION.len();
#[no_mangle]
pub static CROSSLOCALE_PROTOCOL_VERSION: u32 = crate::backend::PROTOCOL_VERSION;

#[no_mangle]
pub extern "C" fn crosslocale_init_logging() -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    crate::init_logging();
    crate::print_banner_message();
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum crosslocale_result {
  CROSSLOCALE_OK = 0,
  CROSSLOCALE_ERR_GENERIC_RUST_PANIC = 1,
  CROSSLOCALE_ERR_BACKEND_DISCONNECTED = 2,
  CROSSLOCALE_ERR_SPAWN_THREAD_FAILED = 4,
}
use crosslocale_result::*;

#[no_mangle]
pub extern "C" fn crosslocale_error_description(myself: crosslocale_result) -> *const u8 {
  let s: &'static str = match myself {
    CROSSLOCALE_OK => "this isn't actually an error\0",
    CROSSLOCALE_ERR_GENERIC_RUST_PANIC => "a generic Rust panic\0",
    CROSSLOCALE_ERR_BACKEND_DISCONNECTED => "the backend thread has disconnected\0",
    CROSSLOCALE_ERR_SPAWN_THREAD_FAILED => "failed to spawn the backend thread\0",
    _ => "unknown error\0",
  };
  s.as_ptr()
}

#[no_mangle]
pub extern "C" fn crosslocale_error_id_str(myself: crosslocale_result) -> *const u8 {
  macro_rules! lookup_error_id {
    ($var:expr, [$($name:path),+ $(,)?]) => {
      match $var {
        $($name => concat!(stringify!($name), "\0").as_ptr(),)+
        _ => return ptr::null(),
      }
    };
  }
  lookup_error_id!(myself, [
    CROSSLOCALE_OK,
    CROSSLOCALE_ERR_GENERIC_RUST_PANIC,
    CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    CROSSLOCALE_ERR_SPAWN_THREAD_FAILED,
  ])
}

// The Message type is based on this:
// <https://github.com/msgpack/msgpack-c/blob/c-4.0.0/include/msgpack/object.h#L27-L98>.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum crosslocale_message_type {
  CROSSLOCALE_MESSAGE_NIL = 0,
  CROSSLOCALE_MESSAGE_BOOL = 1,
  CROSSLOCALE_MESSAGE_I64 = 2,
  CROSSLOCALE_MESSAGE_F64 = 3,
  CROSSLOCALE_MESSAGE_STR = 4,
  CROSSLOCALE_MESSAGE_LIST = 5,
  CROSSLOCALE_MESSAGE_DICT = 6,
  CROSSLOCALE_MESSAGE_INVALID = -1,
}
use crosslocale_message_type::*;

/// cbindgen:field-names=[type, as]
#[repr(C)]
#[derive(Clone, Copy)]
pub struct crosslocale_message {
  type_: crosslocale_message_type,
  as_: crosslocale_message_inner,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union crosslocale_message_inner {
  pub value_bool: bool,
  pub value_i64: i64,
  pub value_f64: f64,
  pub value_str: crosslocale_message_str,
  pub value_list: crosslocale_message_list,
  pub value_dict: crosslocale_message_dict,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct crosslocale_message_str {
  pub len: usize,
  pub ptr: *mut u8,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct crosslocale_message_list {
  pub len: usize,
  pub ptr: *mut crosslocale_message,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct crosslocale_message_dict {
  pub len: usize,
  pub keys: *mut crosslocale_message_str,
  pub values: *mut crosslocale_message,
}

unsafe fn free_message(myself: &mut crosslocale_message) {
  unsafe fn free_string(s: &mut crosslocale_message_str) {
    let data: Box<[u8]> = Box::from_raw(slice::from_raw_parts_mut(s.ptr, s.len));
    drop(data);
  }

  match myself.type_ {
    CROSSLOCALE_MESSAGE_STR => {
      let mut myself = myself.as_.value_str;
      free_string(&mut myself);
    }

    CROSSLOCALE_MESSAGE_LIST => {
      let myself = myself.as_.value_list;

      let mut values: Box<[crosslocale_message]> =
        Box::from_raw(slice::from_raw_parts_mut(myself.ptr, myself.len));
      for value in values.iter_mut() {
        free_message(value);
      }
      drop(values);
    }

    CROSSLOCALE_MESSAGE_DICT => {
      let myself = myself.as_.value_dict;

      let mut keys: Box<[crosslocale_message_str]> =
        Box::from_raw(slice::from_raw_parts_mut(myself.keys, myself.len));
      for key in keys.iter_mut() {
        free_string(key);
      }
      drop(keys);

      let mut values: Box<[crosslocale_message]> =
        Box::from_raw(slice::from_raw_parts_mut(myself.values, myself.len));
      for value in values.iter_mut() {
        free_message(value);
      }
      drop(values);
    }

    _ => {}
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_message_free(
  myself: *mut crosslocale_message,
) -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    unsafe { free_message(&mut *myself) };
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

/// cbindgen:ignore
const MESSAGE_NIL_VALUE: crosslocale_message = crosslocale_message {
  type_: CROSSLOCALE_MESSAGE_NIL,
  as_: crosslocale_message_inner { value_bool: false },
};

unsafe fn wrap_json_value(json_value: JsonValue) -> Option<crosslocale_message> {
  Some(match json_value {
    JsonValue::Null => MESSAGE_NIL_VALUE,

    JsonValue::Bool(b) => crosslocale_message {
      type_: CROSSLOCALE_MESSAGE_BOOL,
      as_: crosslocale_message_inner { value_bool: b },
    },

    JsonValue::Number(n) => {
      if let Some(n) = n.as_i64() {
        crosslocale_message {
          type_: CROSSLOCALE_MESSAGE_I64,
          as_: crosslocale_message_inner { value_i64: n },
        }
      } else if let Some(n) = n.as_f64() {
        crosslocale_message {
          type_: CROSSLOCALE_MESSAGE_F64,
          as_: crosslocale_message_inner { value_f64: n },
        }
      } else {
        return None;
      }
    }

    JsonValue::String(s) => {
      let s: Box<[u8]> = s.into_boxed_str().into_boxed_bytes();
      let mut s = ManuallyDrop::new(s);
      crosslocale_message {
        type_: CROSSLOCALE_MESSAGE_STR,
        as_: crosslocale_message_inner {
          value_str: crosslocale_message_str { len: s.len(), ptr: s.as_mut_ptr() },
        },
      }
    }

    JsonValue::Array(array) => {
      let mut list: Vec<crosslocale_message> = Vec::with_capacity(array.len());
      for v in array.into_iter() {
        if let Some(v) = wrap_json_value(v) {
          list.push(v);
        }
      }
      let list: Box<[crosslocale_message]> = list.into_boxed_slice();
      let mut list = ManuallyDrop::new(list);
      crosslocale_message {
        type_: CROSSLOCALE_MESSAGE_LIST,
        as_: crosslocale_message_inner {
          value_list: crosslocale_message_list { len: list.len(), ptr: list.as_mut_ptr() },
        },
      }
    }

    JsonValue::Object(map) => {
      let mut keys: Vec<crosslocale_message_str> = Vec::with_capacity(map.len());
      let mut values: Vec<crosslocale_message> = Vec::with_capacity(map.len());
      for (k, v) in map.into_iter() {
        if let Some(v) = wrap_json_value(v) {
          let k: Box<[u8]> = k.into_boxed_str().into_boxed_bytes();
          let mut k = ManuallyDrop::new(k);
          keys.push(crosslocale_message_str { len: k.len(), ptr: k.as_mut_ptr() });
          values.push(v);
        }
      }
      let mut keys = ManuallyDrop::new(keys.into_boxed_slice());
      let mut values = ManuallyDrop::new(values.into_boxed_slice());
      crosslocale_message {
        type_: CROSSLOCALE_MESSAGE_DICT,
        as_: crosslocale_message_inner {
          value_dict: crosslocale_message_dict {
            len: keys.len(),
            keys: keys.as_mut_ptr(),
            values: values.as_mut_ptr(),
          },
        },
      }
    }
  })
}

unsafe fn unwrap_json_value(message: &crosslocale_message) -> JsonValue {
  match message.type_ {
    CROSSLOCALE_MESSAGE_NIL => JsonValue::Null,
    CROSSLOCALE_MESSAGE_BOOL => JsonValue::Bool(message.as_.value_bool),
    CROSSLOCALE_MESSAGE_I64 => JsonValue::from(message.as_.value_i64),
    CROSSLOCALE_MESSAGE_F64 => JsonValue::from(message.as_.value_f64),

    CROSSLOCALE_MESSAGE_STR => JsonValue::String({
      let s: &[u8] = slice::from_raw_parts(message.as_.value_str.ptr, message.as_.value_str.len);
      str::from_utf8_unchecked(s).to_owned()
    }),

    CROSSLOCALE_MESSAGE_LIST => JsonValue::Array({
      let list: &[crosslocale_message] =
        slice::from_raw_parts(message.as_.value_list.ptr, message.as_.value_list.len);
      list.iter().map(|v| unwrap_json_value(v)).collect::<Vec<JsonValue>>()
    }),

    CROSSLOCALE_MESSAGE_DICT => JsonValue::Object({
      let keys: &[crosslocale_message_str] =
        slice::from_raw_parts(message.as_.value_dict.keys, message.as_.value_dict.len);
      let values: &[crosslocale_message] =
        slice::from_raw_parts(message.as_.value_dict.values, message.as_.value_dict.len);
      let mut map = serde_json::Map::with_capacity(keys.len());
      for i in 0..keys.len().min(values.len()) {
        let (k, v) = (keys[i], values[i]);
        let k: &[u8] = slice::from_raw_parts(k.ptr, k.len);
        map.insert(str::from_utf8_unchecked(k).to_owned(), unwrap_json_value(&v));
      }
      map
    }),

    CROSSLOCALE_MESSAGE_INVALID => panic!("encountered an explicitly invalid value"),
    _ => panic!("encountered an unknown value type: {:?}", message.type_ as u32),
  }
}

pub struct crosslocale_backend {
  message_sender: Option<mpsc::Sender<JsonValue>>,
  message_receiver: mpsc::Receiver<JsonValue>,
  _backend_thread: thread::JoinHandle<()>,
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_new(
  out: *mut *mut crosslocale_backend,
) -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let (incoming_send, incoming_recv) = mpsc::channel::<JsonValue>();
    let (outgoing_send, outgoing_recv) = mpsc::channel::<JsonValue>();

    let backend_thread =
      match thread::Builder::new().name(std::any::type_name::<Backend>().to_owned()).spawn(
        move || match panic::catch_unwind(AssertUnwindSafe(move || {
          let mut backend = Backend::new(Box::new(FfiChannelTransport {
            receiver: incoming_recv,
            sender: outgoing_send,
          }));
          if let Err(e) = backend.start() {
            crate::report_critical_error(e);
            process::abort();
          }
        })) {
          Ok(v) => v,
          Err(_) => process::abort(),
        },
      ) {
        Ok(v) => v,
        Err(_) => return CROSSLOCALE_ERR_SPAWN_THREAD_FAILED,
      };

    let ffi_backend = Box::into_raw(Box::new(crosslocale_backend {
      message_sender: Some(incoming_send),
      message_receiver: outgoing_recv,
      _backend_thread: backend_thread,
    }));
    unsafe { *out = ffi_backend };

    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_free(
  myself: *mut crosslocale_backend,
) -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    drop(unsafe { Box::from_raw(myself) });
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_recv_message(
  myself: *const crosslocale_backend,
  out_message: *mut crosslocale_message,
) -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &*myself };
    let message: JsonValue = match myself.message_receiver.recv() {
      Ok(v) => v,
      Err(mpsc::RecvError) => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    };
    unsafe { *out_message = wrap_json_value(message).unwrap_or(MESSAGE_NIL_VALUE) };
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_send_message(
  myself: *const crosslocale_backend,
  message: *const crosslocale_message,
) -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &*myself };
    let message = unsafe { unwrap_json_value(&*message) };
    let message_sender = match myself.message_sender.as_ref() {
      Some(v) => v,
      None => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    };
    match message_sender.send(message) {
      Ok(()) => {}
      Err(mpsc::SendError(_)) => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    }
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_close(
  myself: *mut crosslocale_backend,
) -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &mut *myself };
    myself.message_sender = None;
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_is_closed(
  myself: *mut crosslocale_backend,
  out: *mut bool,
) -> crosslocale_result {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &mut *myself };
    unsafe {
      *out = myself.message_sender.is_none();
    }
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}
