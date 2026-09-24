//! Where the host's progress lines go.
//!
//! An episode prints its progress, such as `survey ▸ Jev rated 40 files`, to
//! standard output, where a person watching a run reads it. A host that
//! owns standard output itself, such as Coder Terminal drawing a screen or
//! `coder -p` writing only the reply there, takes the lines instead:
//! [`capture`] routes every line said on the current thread to a sink of
//! its own until the returned guard drops.
//!
//! The sink is per thread because a Coder One episode runs on one thread:
//! its futures are not `Send`, so a host runs them on a current-thread
//! runtime, and every line they say is said on that thread.

use std::cell::RefCell;

/// Receives one progress line, without its trailing newline.
pub type Sink = Box<dyn Fn(&str)>;

thread_local! {
    static SINK: RefCell<Option<Sink>> = const { RefCell::new(None) };
}

/// Says one progress line: to the thread's sink when a host captured this
/// thread, and to standard output otherwise.
pub fn line(text: &str) {
    let captured = SINK.with(|sink| match sink.borrow().as_ref() {
        Some(sink) => {
            sink(text);
            true
        }
        None => false,
    });
    if !captured {
        println!("{text}");
    }
}

/// Routes this thread's progress lines to `sink` until the guard drops.
#[must_use = "the capture ends when the guard drops"]
pub fn capture(sink: Sink) -> Captured {
    let previous = SINK.with(|slot| slot.borrow_mut().replace(sink));
    Captured { previous }
}

/// Holds a capture open; dropping it restores what was there before.
pub struct Captured {
    previous: Option<Sink>,
}

impl Drop for Captured {
    fn drop(&mut self) {
        let previous = self.previous.take();
        SINK.with(|slot| *slot.borrow_mut() = previous);
    }
}

/// Writes `n` with commas between thousands, such as `8,061`, for a
/// progress line.
#[must_use]
pub fn count(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Writes `ms` milliseconds as seconds with one decimal, such as `2.8 s`.
#[must_use]
pub fn seconds(ms: u128) -> String {
    format!("{:.1} s", ms as f64 / 1000.0)
}

/// Says a formatted progress line through [`line`].
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::say::line(&format!($($arg)*))
    };
}

pub(crate) use say;

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn a_capture_takes_the_lines_and_gives_them_back_when_it_ends() {
        let heard = Rc::new(RefCell::new(Vec::new()));
        {
            let into = heard.clone();
            let _captured = capture(Box::new(move |text| {
                into.borrow_mut().push(text.to_string())
            }));
            say!("survey ▸ {} files", 40);
        }
        assert_eq!(*heard.borrow(), vec!["survey ▸ 40 files".to_string()]);
        assert!(SINK.with(|sink| sink.borrow().is_none()));
    }

    #[test]
    fn counts_carry_commas_between_thousands() {
        assert_eq!(count(0), "0");
        assert_eq!(count(999), "999");
        assert_eq!(count(8_061), "8,061");
        assert_eq!(count(1_234_567), "1,234,567");
        assert_eq!(seconds(2_763), "2.8 s");
    }
}
