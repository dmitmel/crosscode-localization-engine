// TODO: Something based on <https://github.com/getsentry/sentry-cli/blob/279bfdc218dd2d6b3c51edad59168c504c431e66/src/utils/logging.rs>

use crate::impl_prelude::*;

use once_cell::sync::Lazy;
use std::fmt;
use std::sync::RwLock;

#[derive(Debug)]
pub struct LoggerWrapper;

pub fn ensure_installed() {
  let err_if_already_installed = log::set_logger(&LoggerWrapper);
  if err_if_already_installed.is_ok() {
    *LOGGER_INSTANCE.write().unwrap() = Some(RealLogger { stdio_logger: None, listeners: vec![] });
    log::set_max_level(log::LevelFilter::max());
  } else if LOGGER_INSTANCE.read().unwrap().is_none() {
    panic!("A logger different from ours was installed, this shouldn't ever happen");
  }
}

// NOTE: The logger RwLock mustn't be poisoned, as this may take down the
// entire application if one thread crashes.
static LOGGER_INSTANCE: Lazy<RwLock<Option<RealLogger>>> = Lazy::new(|| RwLock::new(None));

#[inline]
fn with_logger_instance_read<T>(func: impl FnOnce(&RealLogger) -> T) -> T {
  let instance = LOGGER_INSTANCE.read().unwrap();
  let instance = instance.as_ref().unwrap();
  func(instance)
}

#[inline]
fn with_logger_instance_write<T>(func: impl FnOnce(&mut RealLogger) -> T) -> T {
  let mut instance = LOGGER_INSTANCE.write().unwrap();
  let instance = instance.as_mut().unwrap();
  func(instance)
}

#[derive(Debug)]
struct RealLogger {
  stdio_logger: Option<env_logger::Logger>,
  listeners: Vec<Listener>,
}

struct Listener {
  callback: Box<dyn Fn(&log::Record) + Send + Sync>,
  filter: env_logger::filter::Filter,
}

impl fmt::Debug for Listener {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    f.debug_struct("Listener")
      .field("callback", &(&*self.callback as *const _))
      .field("filter", &self.filter)
      .finish()
  }
}

impl log::Log for LoggerWrapper {
  fn enabled(&self, metadata: &log::Metadata) -> bool {
    with_logger_instance_read(|myself| {
      if let Some(stdio_logger) = &myself.stdio_logger {
        if stdio_logger.enabled(metadata) {
          return true;
        }
      }
      for listener in &myself.listeners {
        if listener.filter.enabled(metadata) {
          return true;
        }
      }
      false
    })
  }

  fn log(&self, record: &log::Record) {
    with_logger_instance_read(|myself| {
      if let Some(stdio_logger) = &myself.stdio_logger {
        stdio_logger.log(record);
      }
      for listener in &myself.listeners {
        if listener.filter.matches(record) {
          (listener.callback)(record);
        }
      }
    })
  }

  fn flush(&self) {
    with_logger_instance_read(|myself| {
      if let Some(stdio_logger) = &myself.stdio_logger {
        stdio_logger.flush();
      }
    })
  }
}

pub fn set_stdio_logger(logger: Option<env_logger::Logger>) {
  with_logger_instance_write(|myself| {
    myself.stdio_logger = logger;
  })
}

pub fn add_listener(
  filter: env_logger::filter::Filter,
  callback: Box<dyn Fn(&log::Record) + Send + Sync>,
) -> usize {
  with_logger_instance_write(|myself| {
    let id = &*callback as *const _ as *const () as usize;
    myself.listeners.push(Listener { callback, filter });
    id
  })
}

pub fn remove_listener(id: usize) {
  with_logger_instance_write(|myself| {
    myself.listeners.retain(|listener| {
      &*listener.callback as *const _ as *const () as usize == id //
    });
  })
}

pub fn print_banner_message() {
  info!("{}/{} v{}", crate::CRATE_TITLE, crate::CRATE_NAME, crate::CRATE_NICE_VERSION);
}

pub fn report_critical_error(mut error: AnyError) {
  error = error.context(format!(
    "CRITICAL ERROR in thread '{}'",
    std::thread::current().name().unwrap_or("<unnamed>"),
  ));
  if log::log_enabled!(log::Level::Error) {
    error!("{:?}", error);
  } else {
    eprintln!("ERROR: {:?}", error);
  }
}

pub fn report_error(mut error: AnyError) {
  error = error.context(format!(
    "non-critical error in thread '{}'",
    std::thread::current().name().unwrap_or("<unnamed>"),
  ));
  if log::log_enabled!(log::Level::Error) {
    warn!("{:?}", error);
  } else {
    eprintln!("WARN: {:?}", error);
  }
}
