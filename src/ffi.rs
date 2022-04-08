// TODO: Catch panics, see <https://github.com/hyperium/hyper/blob/4c946af49cc7fbbc6bd4894283a654625c2ea383/src/ffi/macros.rs>.
#![allow(non_camel_case_types, unreachable_patterns, missing_debug_implementations)]

use crate::backend::transports::MpscChannelTransport;
use crate::backend::Backend;

use std::mem::ManuallyDrop;
use std::panic::{self, AssertUnwindSafe};
use std::process;
use std::ptr;
use std::slice;
use std::str;
use std::sync::mpsc;
use std::thread;

#[no_mangle]
pub static CROSSLOCALE_FFI_BRIDGE_VERSION: u32 = 5;

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

macro_rules! abort_on_caught_panic {
  ($body:block) => {
    match panic::catch_unwind(AssertUnwindSafe(move || $body)) {
      Ok(v) => v,
      Err(_) => process::abort(),
    }
  };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub enum crosslocale_result {
  CROSSLOCALE_OK = 0,
  CROSSLOCALE_ERR_GENERIC_RUST_PANIC = 1,
  CROSSLOCALE_ERR_BACKEND_DISCONNECTED = 2,
  CROSSLOCALE_ERR_NON_UTF8_STRING = 3,
  CROSSLOCALE_ERR_SPAWN_THREAD_FAILED = 4,
}
use crosslocale_result::*;

#[no_mangle]
pub extern "C" fn crosslocale_error_describe(myself: crosslocale_result) -> *const u8 {
  let s: &'static str = match myself {
    CROSSLOCALE_OK => "this isn't actually an error\0",
    CROSSLOCALE_ERR_GENERIC_RUST_PANIC => "a generic Rust panic\0",
    CROSSLOCALE_ERR_BACKEND_DISCONNECTED => "the backend thread has disconnected\0",
    CROSSLOCALE_ERR_NON_UTF8_STRING => "a provided string wasn't properly utf8-encoded\0",
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

pub struct crosslocale_backend {
  message_sender: Option<mpsc::Sender<String>>,
  message_receiver: mpsc::Receiver<String>,
  _backend_thread: thread::JoinHandle<()>,
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_new(
  out: *mut *mut crosslocale_backend,
) -> crosslocale_result {
  abort_on_caught_panic!({
    let (incoming_send, incoming_recv) = mpsc::channel::<String>();
    let (outgoing_send, outgoing_recv) = mpsc::channel::<String>();

    let thread_body = move || {
      abort_on_caught_panic!({
        let mut backend = Backend::new(Box::new(MpscChannelTransport {
          receiver: incoming_recv,
          sender: outgoing_send,
        }));
        if let Err(e) = backend.start() {
          crate::report_critical_error!(e);
          process::abort();
        }
      })
    };

    let backend_thread = match thread::Builder::new()
      .name(std::any::type_name::<Backend>().to_owned())
      .spawn(thread_body)
    {
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
  })
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_free(
  myself: *mut crosslocale_backend,
) -> crosslocale_result {
  abort_on_caught_panic!({
    drop(unsafe { Box::from_raw(myself) });
    CROSSLOCALE_OK
  })
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_recv_message(
  myself: *const crosslocale_backend,
  out_message: *mut *mut u8,
  out_message_len: *mut usize,
) -> crosslocale_result {
  abort_on_caught_panic!({
    let myself = unsafe { &*myself };
    let message: String = match myself.message_receiver.recv() {
      Ok(v) => v,
      Err(mpsc::RecvError) => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    };
    let message: Box<[u8]> = message.into_boxed_str().into_boxed_bytes();
    let mut message = ManuallyDrop::new(message);
    unsafe {
      *out_message = message.as_mut_ptr();
      *out_message_len = message.len();
    }
    CROSSLOCALE_OK
  })
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_send_message(
  myself: *const crosslocale_backend,
  message: *const u8,
  message_len: usize,
) -> crosslocale_result {
  abort_on_caught_panic!({
    let myself = unsafe { &*myself };
    let message: &[u8] = unsafe { slice::from_raw_parts(message, message_len) };
    let message: String = match str::from_utf8(message) {
      Ok(s) => s.to_owned(),
      Err(_) => return CROSSLOCALE_ERR_NON_UTF8_STRING,
    };
    let message_sender = match myself.message_sender.as_ref() {
      Some(v) => v,
      None => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    };
    match message_sender.send(message) {
      Ok(()) => {}
      Err(mpsc::SendError(_)) => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    }
    CROSSLOCALE_OK
  })
}

#[no_mangle]
pub extern "C" fn crosslocale_message_free(ptr: *mut u8, len: usize) -> crosslocale_result {
  abort_on_caught_panic!({
    let data: Box<[u8]> = unsafe { Box::from_raw(slice::from_raw_parts_mut(ptr, len)) };
    drop(data);
    CROSSLOCALE_OK
  })
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_close(
  myself: *mut crosslocale_backend,
) -> crosslocale_result {
  abort_on_caught_panic!({
    let myself = unsafe { &mut *myself };
    myself.message_sender = None;
    CROSSLOCALE_OK
  })
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_is_closed(
  myself: *mut crosslocale_backend,
  out: *mut bool,
) -> crosslocale_result {
  abort_on_caught_panic!({
    let myself = unsafe { &mut *myself };
    unsafe { *out = myself.message_sender.is_none() };
    CROSSLOCALE_OK
  })
}
