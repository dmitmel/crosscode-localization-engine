// TODO: Catch panics, see <https://github.com/hyperium/hyper/blob/4c946af49cc7fbbc6bd4894283a654625c2ea383/src/ffi/macros.rs>.
#![allow(non_camel_case_types, clippy::not_unsafe_ptr_arg_deref)]

use crate::backend::transports::MpscChannelTransport;
use crate::backend::Backend;
use crate::impl_prelude::*;

use std::mem::ManuallyDrop;
use std::os::raw::c_void;
use std::ptr;
use std::slice;
use std::sync::mpsc;
use std::thread;

#[no_mangle]
pub extern "C" fn crosslocale_init_logging() { crate::init_logging(); }

#[no_mangle]
pub extern "C" fn crosslocale_message_free(buf: *mut u8, len: usize, cap: usize) {
  drop(unsafe { String::from_raw_parts(buf, len, cap) });
}

#[derive(Debug)]
pub struct crosslocale_backend_t {
  message_sender: mpsc::Sender<String>,
  message_receiver: mpsc::Receiver<String>,
  backend_thread: thread::JoinHandle<AnyResult<()>>,
  message_callback: unsafe extern "C" fn(
    user_data: *mut c_void,
    message: *mut u8,
    message_len: usize,
    message_cap: usize,
  ),
  message_callback_user_data: *mut c_void,
}

/// cbindgen:ignore
extern "C" fn message_callback_nop(
  _user_data: *mut c_void,
  _message: *mut u8,
  _message_len: usize,
  _message_cap: usize,
) {
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_new() -> *mut crosslocale_backend_t {
  let (incoming_send, incoming_recv) = mpsc::channel::<String>();
  let (outgoing_send, outgoing_recv) = mpsc::channel::<String>();

  let backend_thread = thread::Builder::new()
    .name(std::any::type_name::<Backend>().to_owned())
    .spawn(move || {
      let mut backend = Backend::new(Box::new(MpscChannelTransport {
        receiver: incoming_recv,
        sender: outgoing_send,
      }));
      backend.start()
    })
    .unwrap();

  Box::into_raw(Box::new(crosslocale_backend_t {
    message_sender: incoming_send,
    message_receiver: outgoing_recv,
    backend_thread,
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
  callback: Option<
    unsafe extern "C" fn(
      user_data: *mut c_void,
      message: *mut u8,
      message_len: usize,
      message_cap: usize,
    ),
  >,
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
  myself: *mut crosslocale_backend_t,
  out_message: *mut *mut u8,
  out_message_len: *mut usize,
  out_message_cap: *mut usize,
) {
  let myself = unsafe { &*myself };
  let message_string = myself.message_receiver.recv().unwrap();
  let (message, message_len, message_cap) = {
    let mut string = ManuallyDrop::new(message_string);
    (string.as_mut_ptr(), string.len(), string.capacity())
  };
  unsafe {
    *out_message = message;
    *out_message_len = message_len;
    *out_message_cap = message_cap;
  }
}

#[no_mangle]
pub extern "C" fn crosslocale_backend_send_message(
  myself: *mut crosslocale_backend_t,
  message: *const u8,
  message_len: usize,
) {
  let myself = unsafe { &*myself };
  let message_slice = unsafe { slice::from_raw_parts(message, message_len) };
  let message_vec = message_slice.to_owned();
  let message_string = String::from_utf8(message_vec).unwrap();
  myself.message_sender.send(message_string).unwrap();
}
