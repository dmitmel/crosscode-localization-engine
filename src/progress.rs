#![allow(missing_debug_implementations)]

use crate::impl_prelude::*;
use crate::rc_string::RcString;

use std::fmt::Write as FmtWrite;
use std::io::{self, Write};
use std::time::{Duration, Instant};

assert_trait_is_object_safe!(ProgressReporter);
pub trait ProgressReporter {
  fn begin_task(&mut self, total: usize) -> AnyResult<()>;
  fn end_task(&mut self) -> AnyResult<()>;
  fn set_task_info(&mut self, info: &RcString) -> AnyResult<()>;
  fn set_task_progress(&mut self, current: usize) -> AnyResult<()>;
  fn redraw(&mut self) -> AnyResult<()>;
}

pub struct NopProgressReporter;

impl ProgressReporter for NopProgressReporter {
  fn begin_task(&mut self, _total: usize) -> AnyResult<()> { Ok(()) }
  fn end_task(&mut self) -> AnyResult<()> { Ok(()) }
  fn set_task_info(&mut self, _info: &RcString) -> AnyResult<()> { Ok(()) }
  fn set_task_progress(&mut self, _current: usize) -> AnyResult<()> { Ok(()) }
  fn redraw(&mut self) -> AnyResult<()> { Ok(()) }
}

pub struct TuiProgresReporter {
  stream: io::Stderr,
  start_time: Option<Instant>,
  prev_redraw_time: Option<Instant>,
  current_task_info: RcString,
  current_item: usize,
  current_total: usize,
}

impl TuiProgresReporter {
  const REDRAW_INTERVAL: Duration = Duration::from_millis(40);

  pub fn new() -> Self {
    Self {
      stream: io::stderr(),
      start_time: None,
      prev_redraw_time: None,
      current_task_info: RcString::from(""),
      current_item: 0,
      current_total: 0,
    }
  }

  pub fn reset(&mut self) {
    self.start_time = None;
    self.prev_redraw_time = None;
    self.current_task_info = RcString::from("");
    self.current_item = 0;
    self.current_total = 0;
  }
}

// TODO: Make the progress bar compatible with the logger:
// https://github.com/getsentry/sentry-cli/blob/5769a14cb1a06703250042907e330876f5cada2d/src/utils/logging.rs
impl ProgressReporter for TuiProgresReporter {
  fn begin_task(&mut self, total: usize) -> AnyResult<()> {
    self.reset();
    self.current_total = total;
    self.start_time = Some(Instant::now());
    Ok(())
  }

  fn end_task(&mut self) -> AnyResult<()> {
    self.redraw()?;
    self.stream.write_all(b"\n")?;
    self.stream.flush()?;
    self.reset();
    Ok(())
  }

  fn set_task_info(&mut self, info: &RcString) -> AnyResult<()> {
    self.current_task_info = info.share_rc();
    Ok(())
  }

  fn set_task_progress(&mut self, current: usize) -> AnyResult<()> {
    self.current_item = current.min(self.current_total);

    if let Some(prev_redraw_time) = self.prev_redraw_time {
      let since_prev_redraw = prev_redraw_time.elapsed();
      if since_prev_redraw < Self::REDRAW_INTERVAL {
        return Ok(());
      }
    }

    self.redraw()
  }

  // TODO: Use unicode_width, see
  // <https://github.com/Aetf/unicode-truncate/blob/b1821b0af6801b81e1f1f900526748754f8cd44f/src/lib.rs>

  // NOTE: push_str is technically faster because non-ASCII characters are
  // already encoded in a static string
  #[allow(clippy::single_char_add_str)]
  fn redraw(&mut self) -> AnyResult<()> {
    let current = self.current_item;
    let total = self.current_total;

    let start_time = match self.start_time {
      Some(v) => v,
      None => return Ok(()),
    };
    let elapsed: Duration = start_time.elapsed();

    let term_width = terminal_size::terminal_size().map(|(w, _h)| w.0 as usize).unwrap_or(80);

    let rate = current as f64 / elapsed.as_secs_f64();

    // wget --limit-rate=50k https://cdn.openbsd.org/pub/OpenBSD/OpenSSH/portable/openssh-7.9p1.tar.gz -O /dev/null 2>&1
    // wget --limit-rate=50k https://cdn.openbsd.org/pub/OpenBSD/OpenSSH/portable/openssh-7.9p1.tar.gz -O /dev/null 2>&1 | cat
    // openssh-7.9p1.tar.gz                                 100%[=======================================================================================================>]   1,49M  51,4KB/s    in 30s     //
    // [===================================                                                                 ] 35%  99 items/s  ETA 0.7s
    // 1,00 B 0:00:02 [1,01 B/s] [================================>                                                                                                                         ] 20% ETA 0:00:08

    let mut left_str = String::with_capacity(term_width / 2);

    let info_max_width = term_width / 4;
    let info_real_width = self.current_task_info.chars().count();
    if info_real_width <= info_max_width {
      left_str.push_str(&self.current_task_info);
      left_str.push_str(&" ".repeat(info_max_width - info_real_width));
    } else {
      let mut min_index = self.current_task_info.len();
      let mut char_indices_iter = self.current_task_info.char_indices().rev();
      for _ in 0..info_max_width {
        match char_indices_iter.next() {
          Some((index, _)) => min_index = index,
          None => break,
        }
      }
      left_str.push_str(&self.current_task_info[min_index..]);
    }

    left_str.push_str("  ");
    let done_percent = if total != 0 { (100 * current / total).clamp(0, 100) } else { 100 };
    write!(left_str, "{:3}%", done_percent).unwrap();
    left_str.push_str("[");

    let mut right_str = String::with_capacity(1);
    right_str.push_str("]");
    right_str.push_str("  ");

    let total_num_str = total.to_string();
    let total_num_width = total_num_str.len(); // correct because digits are ASCII-only
    let current_num_str = current.to_string();
    let current_num_width = current_num_str.len(); // correct because digits are ASCII-only
    let rate_num_str = (rate as usize).to_string();
    let rate_num_width = current_num_str.len(); // you get the idea
    right_str.push_str(&" ".repeat(total_num_width.saturating_sub(current_num_width)));
    right_str.push_str(&current_num_str);
    right_str.push_str("/");
    right_str.push_str(&total_num_str);
    right_str.push_str("  ");
    right_str.push_str(&" ".repeat(total_num_width.saturating_sub(rate_num_width)));
    right_str.push_str(&rate_num_str);
    right_str.push_str("/s ");

    let elapsed_seconds = elapsed.as_secs() % 60;
    let elapsed_minutes = (elapsed.as_secs() / 60) % 60;
    let elapsed_hours = ((elapsed.as_secs() / 60) / 60) % 60;
    write!(right_str, " {:02}:{:02}:{:02} ", elapsed_hours, elapsed_minutes, elapsed_seconds)
      .unwrap();

    let total_bar_width = term_width - left_str.chars().count() - right_str.chars().count();
    let mut filled_bar_width = if total != 0 {
      (total_bar_width * current / total).clamp(0, total_bar_width)
    } else {
      total_bar_width
    };
    let mut bar_str = String::with_capacity(total_bar_width);
    bar_str.push_str(&"=".repeat(filled_bar_width));
    if current < total {
      bar_str.push_str(">");
      filled_bar_width += 1;
    }
    bar_str.push_str(&".".repeat(total_bar_width - filled_bar_width));
    debug_assert_eq!(bar_str.len(), bar_str.capacity());

    self.stream.write_all(left_str.as_bytes())?;
    self.stream.write_all(bar_str.as_bytes())?;
    self.stream.write_all(right_str.as_bytes())?;
    self.stream.write_all(b"\r")?;
    self.stream.flush()?;

    self.prev_redraw_time = Some(Instant::now());
    Ok(())
  }
}
