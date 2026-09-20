//! The terminal's lifecycle: what a shell changes about the terminal on
//! the way in, and the guard that changes it back on every way out.
//!
//! A full-screen shell puts the terminal in raw mode, switches to the
//! alternate screen, and recolors and reshapes the cursor. Each of those is
//! a [`Step`], and [`Guard::enter`] applies them in order. Whatever it
//! applied, it undoes in reverse when it drops — after a later step fails
//! to apply, after the shell returns, and while a panic unwinds. A panic
//! hook installed with [`Guard::arm_panic_hook`] restores the same steps
//! before the panic prints, so the message lands on a sane terminal rather
//! than on the alternate screen in raw mode.
//!
//! The steps are applied through a [`Console`], so the ordering and the
//! failure paths are testable without a terminal; [`Stdout`] is the console
//! a real shell uses.

use std::io::{self, Write};
use std::sync::{Arc, Mutex, Once};

use crossterm::cursor::{SetCursorStyle, Show};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};

/// OSC 12 paints the terminal's hardware cursor the ladder's full amber;
/// OSC 112 hands the terminal's own color back on exit.
pub const CURSOR_COLOR_SET: &str = "\x1b]12;#FFB000\x07";
/// The sequence that resets the cursor color `CURSOR_COLOR_SET` painted.
pub const CURSOR_COLOR_RESET: &str = "\x1b]112\x07";

/// One reversible change to the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// Raw mode: keys arrive unbuffered and unechoed.
    RawMode,
    /// The alternate screen, so the shell's frames do not scroll the
    /// user's own scrollback.
    AlternateScreen,
    /// A blinking block cursor; undoing it restores the user's shape and
    /// makes the cursor visible again in case a frame hid it.
    CursorStyle,
    /// The cursor painted amber; undoing it hands the color back.
    CursorColor,
}

/// The steps a full-screen shell takes, in the order they apply.
pub const FULL_SCREEN: [Step; 4] = [
    Step::RawMode,
    Step::AlternateScreen,
    Step::CursorStyle,
    Step::CursorColor,
];

/// Where the steps land. A real console writes to the terminal; a test
/// console records what happened and fails on demand.
pub trait Console {
    /// Applies `step`.
    fn apply(&mut self, step: Step) -> io::Result<()>;
    /// Undoes `step`.
    fn undo(&mut self, step: Step) -> io::Result<()>;
}

/// The process's standard output as a [`Console`].
#[derive(Default)]
pub struct Stdout;

impl Console for Stdout {
    fn apply(&mut self, step: Step) -> io::Result<()> {
        let mut out = io::stdout();
        match step {
            Step::RawMode => enable_raw_mode(),
            Step::AlternateScreen => execute!(out, EnterAlternateScreen),
            Step::CursorStyle => execute!(out, SetCursorStyle::BlinkingBlock),
            Step::CursorColor => {
                out.write_all(CURSOR_COLOR_SET.as_bytes())?;
                out.flush()
            }
        }
    }

    fn undo(&mut self, step: Step) -> io::Result<()> {
        let mut out = io::stdout();
        match step {
            Step::RawMode => disable_raw_mode(),
            Step::AlternateScreen => execute!(out, LeaveAlternateScreen),
            Step::CursorStyle => execute!(out, SetCursorStyle::DefaultUserShape, Show),
            Step::CursorColor => {
                out.write_all(CURSOR_COLOR_RESET.as_bytes())?;
                out.flush()
            }
        }
    }
}

/// The steps a guard has applied and not yet undone, shared with the
/// panic hook so whichever runs first restores them and the other finds
/// nothing left to do.
type Applied = Arc<Mutex<Vec<Step>>>;

/// Undoes `applied` in reverse order on `console`, emptying the list as
/// it goes. Every step is attempted; the first error is what comes back.
fn restore<C: Console>(console: &mut C, applied: &Applied) -> io::Result<()> {
    let steps: Vec<Step> = match applied.lock() {
        Ok(mut applied) => std::mem::take(&mut *applied),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    };
    let mut first_error = None;
    for step in steps.into_iter().rev() {
        if let Err(error) = console.undo(step)
            && first_error.is_none()
        {
            first_error = Some(error);
        }
    }
    first_error.map_or(Ok(()), Err)
}

/// A terminal held in a shell's mode. Dropping it hands the terminal back.
pub struct Guard<C: Console> {
    console: C,
    applied: Applied,
}

impl<C: Console> Guard<C> {
    /// Applies `steps` in order. A step that fails leaves nothing behind:
    /// the steps before it are undone in reverse before the error returns.
    pub fn enter(mut console: C, steps: &[Step]) -> io::Result<Self> {
        let applied: Applied = Arc::default();
        for &step in steps {
            if let Err(error) = console.apply(step) {
                let _ = restore(&mut console, &applied);
                return Err(error);
            }
            applied.lock().expect("no other holder yet").push(step);
        }
        Ok(Self { console, applied })
    }

    /// The steps in force, in the order they applied.
    pub fn applied(&self) -> Vec<Step> {
        self.applied
            .lock()
            .map(|steps| steps.clone())
            .unwrap_or_default()
    }

    /// Hands the terminal back now and reports how that went. Dropping the
    /// guard does the same, but cannot report an error.
    pub fn restore(mut self) -> io::Result<()> {
        restore(&mut self.console, &self.applied)
    }
}

impl<C: Console> Drop for Guard<C> {
    fn drop(&mut self) {
        let _ = restore(&mut self.console, &self.applied);
    }
}

/// The steps the panic hook restores: the live guard's, or none.
static PANIC_STEPS: Mutex<Option<Applied>> = Mutex::new(None);
static PANIC_HOOK: Once = Once::new();

impl Guard<Stdout> {
    /// Enters [`FULL_SCREEN`] on standard output and arms the panic hook.
    pub fn full_screen() -> io::Result<Self> {
        let guard = Self::enter(Stdout, &FULL_SCREEN)?;
        guard.arm_panic_hook();
        Ok(guard)
    }

    /// Makes a panic restore this guard's steps before it prints. The hook
    /// wraps the one already installed, so the panic message and backtrace
    /// print as they would have, on a terminal the reader can read them
    /// on. Installing is idempotent; a later guard replaces the steps the
    /// hook watches.
    pub fn arm_panic_hook(&self) {
        if let Ok(mut slot) = PANIC_STEPS.lock() {
            *slot = Some(Arc::clone(&self.applied));
        }
        PANIC_HOOK.call_once(|| {
            let previous = std::panic::take_hook();
            std::panic::set_hook(Box::new(move |info| {
                let applied = PANIC_STEPS.lock().ok().and_then(|slot| slot.clone());
                if let Some(applied) = applied {
                    let _ = restore(&mut Stdout, &applied);
                }
                previous(info);
            }));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A console that logs every call and fails to apply one chosen step.
    #[derive(Default)]
    struct Fake {
        log: Arc<Mutex<Vec<String>>>,
        fail_on: Option<Step>,
    }

    impl Fake {
        fn new(fail_on: Option<Step>) -> (Self, Arc<Mutex<Vec<String>>>) {
            let log = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    log: Arc::clone(&log),
                    fail_on,
                },
                log,
            )
        }
    }

    impl Console for Fake {
        fn apply(&mut self, step: Step) -> io::Result<()> {
            if self.fail_on == Some(step) {
                return Err(io::Error::other(format!("{step:?} refused")));
            }
            self.log.lock().unwrap().push(format!("apply {step:?}"));
            Ok(())
        }

        fn undo(&mut self, step: Step) -> io::Result<()> {
            self.log.lock().unwrap().push(format!("undo {step:?}"));
            Ok(())
        }
    }

    fn entries(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }

    #[test]
    fn a_failed_step_undoes_the_ones_before_it_in_reverse() {
        let (console, log) = Fake::new(Some(Step::CursorStyle));
        let error = Guard::enter(console, &FULL_SCREEN)
            .err()
            .expect("the third step fails");
        assert_eq!(error.to_string(), "CursorStyle refused");
        assert_eq!(
            entries(&log),
            [
                "apply RawMode",
                "apply AlternateScreen",
                "undo AlternateScreen",
                "undo RawMode",
            ]
        );
    }

    #[test]
    fn a_failed_first_step_leaves_nothing_to_undo() {
        let (console, log) = Fake::new(Some(Step::RawMode));
        assert!(Guard::enter(console, &FULL_SCREEN).is_err());
        assert!(entries(&log).is_empty());
    }

    #[test]
    fn dropping_the_guard_restores_everything_in_reverse() {
        let (console, log) = Fake::new(None);
        {
            let guard = Guard::enter(console, &FULL_SCREEN).unwrap();
            assert_eq!(guard.applied(), FULL_SCREEN);
        }
        assert_eq!(
            entries(&log),
            [
                "apply RawMode",
                "apply AlternateScreen",
                "apply CursorStyle",
                "apply CursorColor",
                "undo CursorColor",
                "undo CursorStyle",
                "undo AlternateScreen",
                "undo RawMode",
            ]
        );
    }

    #[test]
    fn restore_reports_and_drop_does_not_repeat_it() {
        let (console, log) = Fake::new(None);
        let guard = Guard::enter(console, &[Step::RawMode]).unwrap();
        guard.restore().unwrap();
        assert_eq!(entries(&log), ["apply RawMode", "undo RawMode"]);
    }

    #[test]
    fn an_unwinding_panic_still_restores() {
        let (console, log) = Fake::new(None);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = Guard::enter(console, &FULL_SCREEN).unwrap();
            panic!("the draw loop fell over");
        }));
        assert!(outcome.is_err());
        let log = entries(&log);
        assert_eq!(log.len(), 8);
        assert_eq!(
            log[4..],
            [
                "undo CursorColor",
                "undo CursorStyle",
                "undo AlternateScreen",
                "undo RawMode"
            ]
        );
    }

    #[test]
    fn the_hook_and_the_guard_share_one_list_so_neither_restores_twice() {
        let (console, log) = Fake::new(None);
        let guard = Guard::enter(console, &FULL_SCREEN).unwrap();
        let applied = Arc::clone(&guard.applied);
        // The hook's side: restore through another console first.
        let (mut other, other_log) = Fake::new(None);
        restore(&mut other, &applied).unwrap();
        assert_eq!(entries(&other_log).len(), 4);
        assert!(guard.applied().is_empty());
        drop(guard);
        // The guard found the list empty and undid nothing more.
        assert_eq!(entries(&log).len(), 4);
    }
}
