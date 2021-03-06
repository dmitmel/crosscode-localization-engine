// TODO: Catch panics, see <https://github.com/hyperium/hyper/blob/4c946af49cc7fbbc6bd4894283a654625c2ea383/src/ffi/macros.rs>.
#![allow(non_camel_case_types, clippy::not_unsafe_ptr_arg_deref)]

use crate::backend::transports::MpscChannelTransport;
use crate::backend::Backend;

use std::mem::ManuallyDrop;
use std::panic::{self, AssertUnwindSafe};
use std::process;
use std::slice;
use std::sync::mpsc;
use std::thread;

#[no_mangle]
pub static CROSSLOCALE_FFI_BRIDGE_VERSION: u32 = 2;

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
pub extern "C" fn crosslocale_init_logging() -> crosslocale_result_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    crate::init_logging();
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_message_free(
  buf: *mut u8,
  len: usize,
  cap: usize,
) -> crosslocale_result_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    drop(unsafe { String::from_raw_parts(buf, len, cap) });
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[derive(Debug)]
pub struct crosslocale_backend_t {
  message_sender: Option<mpsc::Sender<String>>,
  message_receiver: mpsc::Receiver<String>,
  backend_thread: thread::JoinHandle<()>,
}

pub type crosslocale_result_t = u32;
#[no_mangle]
pub static CROSSLOCALE_OK: crosslocale_result_t = 0;
#[no_mangle]
pub static CROSSLOCALE_ERR_GENERIC_RUST_PANIC: crosslocale_result_t = 1;
#[no_mangle]
pub static CROSSLOCALE_ERR_BACKEND_DISCONNECTED: crosslocale_result_t = 2;
#[no_mangle]
pub static CROSSLOCALE_ERR_NON_UTF8_STRING: crosslocale_result_t = 3;
#[no_mangle]
pub static CROSSLOCALE_ERR_SPAWN_THREAD_FAILED: crosslocale_result_t = 4;

#[no_mangle]
pub extern "C" fn crosslocale_backend_new(
  out: *mut *mut crosslocale_backend_t,
) -> crosslocale_result_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let (incoming_send, incoming_recv) = mpsc::channel::<String>();
    let (outgoing_send, outgoing_recv) = mpsc::channel::<String>();

    let backend_thread =
      match thread::Builder::new().name(std::any::type_name::<Backend>().to_owned()).spawn(
        move || match panic::catch_unwind(AssertUnwindSafe(move || {
          let mut backend = Backend::new(Box::new(MpscChannelTransport {
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

    let ffi_backend = Box::into_raw(Box::new(crosslocale_backend_t {
      message_sender: Some(incoming_send),
      message_receiver: outgoing_recv,
      backend_thread,
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
  myself: *mut crosslocale_backend_t,
) -> crosslocale_result_t {
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
  myself: *const crosslocale_backend_t,
  out_message: *mut *mut u8,
  out_message_len: *mut usize,
  out_message_cap: *mut usize,
) -> crosslocale_result_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &*myself };
    let message_string = match myself.message_receiver.recv() {
      Ok(v) => v,
      Err(mpsc::RecvError) => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    };
    let (message, message_len, message_cap) = {
      let mut string = ManuallyDrop::new(message_string);
      (string.as_mut_ptr(), string.len(), string.capacity())
    };
    unsafe {
      *out_message = message;
      *out_message_len = message_len;
      *out_message_cap = message_cap;
    }
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_send_message(
  myself: *const crosslocale_backend_t,
  message: *const u8,
  message_len: usize,
) -> crosslocale_result_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &*myself };
    let message_slice = unsafe { slice::from_raw_parts(message, message_len) };
    let message_vec = message_slice.to_owned();
    let message_string = match String::from_utf8(message_vec) {
      Ok(s) => s,
      Err(_) => return CROSSLOCALE_ERR_NON_UTF8_STRING,
    };
    match try_option!({ myself.message_sender.as_ref()?.send(message_string).ok()? }) {
      Some(()) => {}
      None => return CROSSLOCALE_ERR_BACKEND_DISCONNECTED,
    }
    CROSSLOCALE_OK
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_close(
  myself: *mut crosslocale_backend_t,
) -> crosslocale_result_t {
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
  myself: *mut crosslocale_backend_t,
  out: *mut bool,
) -> crosslocale_result_t {
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
