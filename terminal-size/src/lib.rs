//! A substitute for the [`terminal_size`](https://docs.rs/terminal_size/0.2.2/terminal_size/)
//! crate, but with less dependencies and a more straightforward API. The code
//! was copied from <https://github.com/eminence/terminal-size/blob/v0.1.16/src/unix.rs>
//! and <https://github.com/eminence/terminal-size/blob/v0.1.16/src/windows.rs>.
//!
//! **NOTE**: The API of this crate only has source-level compatibility
//! with where it is used in clap: <https://github.com/clap-rs/clap/blob/v4.0.26/src/output/help_template.rs#L987-L989>.

#[inline]
pub fn terminal_size() -> Option<((u16,), (u16,))> {
  terminal_size_simple().map(|(w, h)| ((w,), (h,)))
}

pub fn terminal_size_simple() -> Option<(u16, u16)> {
  use std::mem;

  #[cfg(unix)]
  unsafe {
    let fd = libc::STDERR_FILENO;
    if libc::isatty(fd) != 1 {
      return None;
    }
    let mut winsize: libc::winsize = mem::zeroed();
    if libc::ioctl(fd, libc::TIOCGWINSZ, &mut winsize) != 0 {
      return None;
    }
    if !(winsize.ws_col > 0 && winsize.ws_row > 0) {
      return None;
    }
    Some((winsize.ws_col, winsize.ws_row))
  }

  #[cfg(windows)]
  unsafe {
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_ERROR_HANDLE;
    use winapi::um::wincon::{GetConsoleScreenBufferInfo, CONSOLE_SCREEN_BUFFER_INFO};

    let handle = GetStdHandle(STD_ERROR_HANDLE);
    if handle == INVALID_HANDLE_VALUE {
      return None;
    }
    let mut csbi: CONSOLE_SCREEN_BUFFER_INFO = mem::zeroed();
    if GetConsoleScreenBufferInfo(handle, &mut csbi) == 0 {
      return None;
    }
    let w = (csbi.srWindow.Right - csbi.srWindow.Left + 1) as u16;
    let h = (csbi.srWindow.Bottom - csbi.srWindow.Top + 1) as u16;
    Some((w, h))
  }
}
