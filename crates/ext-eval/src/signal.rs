//! Stopping a run: `SIGINT` and `SIGTERM`, or a caller's own flag.
//!
//! The first signal asks the runner to stop: it stops every live child
//! (`SIGTERM` to its process group, then `SIGKILL` after a short grace),
//! marks the unfinished runs cancelled, and returns, so the command exits
//! `130` for `SIGINT` or `143` for `SIGTERM`. A second signal exits at
//! once with the same code.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};

static SIGNAL: AtomicI32 = AtomicI32::new(0);
static COUNT: AtomicUsize = AtomicUsize::new(0);

/// A stop request a runner polls.
#[derive(Clone, Debug, Default)]
pub struct Cancel {
    flag: Arc<AtomicBool>,
    signals: bool,
}

impl Cancel {
    /// A stop request only its holder sets.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A stop request `SIGINT` and `SIGTERM` set too. Installs the process
    /// handlers once.
    #[must_use]
    pub fn on_signals() -> Self {
        install();
        Self {
            flag: Arc::new(AtomicBool::new(false)),
            signals: true,
        }
    }

    /// Asks the runner to stop.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Whether a stop was asked for.
    #[must_use]
    pub fn cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst) || (self.signals && SIGNAL.load(Ordering::SeqCst) != 0)
    }

    /// The exit code a signal asks for: `130` for `SIGINT`, `143` for
    /// `SIGTERM`, `None` without a signal.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        if !self.signals {
            return None;
        }
        match SIGNAL.load(Ordering::SeqCst) {
            0 => None,
            signal => Some(128 + signal),
        }
    }
}

extern "C" fn handle(signal: libc::c_int) {
    SIGNAL.store(signal, Ordering::SeqCst);
    if COUNT.fetch_add(1, Ordering::SeqCst) >= 1 {
        // SAFETY: `_exit` is async-signal-safe and takes no pointers.
        unsafe { libc::_exit(128 + signal) };
    }
}

fn install() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let handler = handle as extern "C" fn(libc::c_int) as libc::sighandler_t;
        // SAFETY: `handle` only touches atomics and calls `_exit`, both
        // async-signal-safe; `signal` installs it for two standard signals.
        unsafe {
            libc::signal(libc::SIGINT, handler);
            libc::signal(libc::SIGTERM, handler);
        }
    });
}
