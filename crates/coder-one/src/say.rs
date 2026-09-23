//! Where the host's progress lines go.
//!
//! An episode prints its progress, such as `survey ▸ 40 files judged`, to
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
}
