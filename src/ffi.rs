// TODO: Catch panics, see <https://github.com/hyperium/hyper/blob/4c946af49cc7fbbc6bd4894283a654625c2ea383/src/ffi/macros.rs>.
#![allow(non_camel_case_types, clippy::not_unsafe_ptr_arg_deref)]

use crate::backend::transports::MpscChannelTransport;
use crate::backend::Backend;
use crate::impl_prelude::*;

use std::mem::ManuallyDrop;
use std::panic::{self, AssertUnwindSafe};
use std::process;
use std::slice;
use std::sync::mpsc;
use std::thread;

#[no_mangle]
pub extern "C" fn crosslocale_init_logging() -> crosslocale_error_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    crate::init_logging();
    CROSSLOCALE_SUCCESS
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
) -> crosslocale_error_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    drop(unsafe { String::from_raw_parts(buf, len, cap) });
    CROSSLOCALE_SUCCESS
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[derive(Debug)]
pub struct crosslocale_backend_t {
  message_sender: mpsc::Sender<String>,
  message_receiver: mpsc::Receiver<String>,
  backend_thread: thread::JoinHandle<AnyResult<()>>,
}

pub type crosslocale_error_t = u32;
#[no_mangle]
pub static CROSSLOCALE_SUCCESS: crosslocale_error_t = 0;
#[no_mangle]
pub static CROSSLOCALE_GENERIC_RUST_PANIC: crosslocale_error_t = 1;
#[no_mangle]
pub static CROSSLOCALE_ERROR_MESSAGE_SENDER_DISCONNECTED: crosslocale_error_t = 2;
#[no_mangle]
pub static CROSSLOCALE_ERROR_MESSAGE_RECEIVER_DISCONNECTED: crosslocale_error_t = 3;
#[no_mangle]
pub static CROSSLOCALE_NON_UTF8_STRING: crosslocale_error_t = 4;
#[no_mangle]
pub static CROSSLOCALE_SPAWN_THREAD_ERROR: crosslocale_error_t = 5;

#[no_mangle]
pub extern "C" fn crosslocale_backend_new(
  out: *mut *mut crosslocale_backend_t,
) -> crosslocale_error_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let (incoming_send, incoming_recv) = mpsc::channel::<String>();
    let (outgoing_send, outgoing_recv) = mpsc::channel::<String>();

    let backend_thread = match thread::Builder::new()
      .name(std::any::type_name::<Backend>().to_owned())
      .spawn(move || {
        let mut backend = Backend::new(Box::new(MpscChannelTransport {
          receiver: incoming_recv,
          sender: outgoing_send,
        }));
        backend.start()
      }) {
      Ok(v) => v,
      Err(_) => return CROSSLOCALE_SPAWN_THREAD_ERROR,
    };

    let ffi_backend = Box::into_raw(Box::new(crosslocale_backend_t {
      message_sender: incoming_send,
      message_receiver: outgoing_recv,
      backend_thread,
    }));
    unsafe { *out = ffi_backend };

    CROSSLOCALE_SUCCESS
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_free(
  myself: *mut crosslocale_backend_t,
) -> crosslocale_error_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    drop(unsafe { Box::from_raw(myself) });
    CROSSLOCALE_SUCCESS
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_recv_message(
  myself: *mut crosslocale_backend_t,
  out_message: *mut *mut u8,
  out_message_len: *mut usize,
  out_message_cap: *mut usize,
) -> crosslocale_error_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &*myself };
    let message_string = match myself.message_receiver.recv() {
      Ok(v) => v,
      Err(mpsc::RecvError) => return CROSSLOCALE_ERROR_MESSAGE_RECEIVER_DISCONNECTED,
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
    CROSSLOCALE_SUCCESS
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_send_message(
  myself: *mut crosslocale_backend_t,
  message: *const u8,
  message_len: usize,
) -> crosslocale_error_t {
  match panic::catch_unwind(AssertUnwindSafe(move || {
    let myself = unsafe { &*myself };
    let message_slice = unsafe { slice::from_raw_parts(message, message_len) };
    let message_vec = message_slice.to_owned();
    let message_string = match String::from_utf8(message_vec) {
      Ok(s) => s,
      Err(_) => return CROSSLOCALE_NON_UTF8_STRING,
    };
    match myself.message_sender.send(message_string) {
      Ok(()) => {}
      Err(mpsc::SendError(_)) => return CROSSLOCALE_ERROR_MESSAGE_SENDER_DISCONNECTED,
    }
    CROSSLOCALE_SUCCESS
  })) {
    Ok(v) => v,
    Err(_) => process::abort(),
  }
}
