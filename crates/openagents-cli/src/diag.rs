//! `--verbose` and `--no-color`: two process-wide output settings.
//!
//! Both were flags the parser accepted and nothing read. A flag that parses
//! and changes nothing is worse than a missing one, because the missing one
//! errors and the declared one silently lies, so each is stored here once and
//! consulted at the places that can act on it.
//!
//! ## Verbose
//!
//! `-v` prints the request line, the response status, and — when the server
//! refused — the server's own message, all on stderr so a `--json` body piped
//! to `jq` stays parseable. The HTTP clients call [`request`] before sending
//! and [`response`] after, which is why the diagnostic covers every command
//! family rather than the one that remembered to log.
//!
//! ## Colour
//!
//! `--no-color` turns off every colour the coder session draws and exports
//! `NO_COLOR=1`, which is the convention a delegated child harness reads. The
//! non-interactive command families print no escape sequences at all, so there
//! is nothing there for it to suppress; that is a property of those printers,
//! not a claim this flag makes about them.

use std::sync::atomic::{AtomicBool, Ordering};

static VERBOSE: AtomicBool = AtomicBool::new(false);
static COLOR: AtomicBool = AtomicBool::new(true);

/// Turn the request trace on. Called once from `cli::run`.
pub fn set_verbose(on: bool) {
    VERBOSE.store(on, Ordering::Relaxed);
}

pub fn verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

/// Turn colour off, and tell anything this process spawns.
///
/// `NO_COLOR` is set rather than merely read so a delegated child harness —
/// which is a separate process with its own idea of styling — inherits the
/// reader's choice.
pub fn set_color(on: bool) {
    COLOR.store(on, Ordering::Relaxed);
    if !on {
        std::env::set_var("NO_COLOR", "1");
    }
}

/// Whether the coder session may draw in colour.
///
/// `NO_COLOR` in the environment counts, so the flag and the convention agree.
pub fn color() -> bool {
    COLOR.load(Ordering::Relaxed)
        && std::env::var("NO_COLOR")
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
}

/// Note a request about to be sent.
pub fn request(method: &str, url: &str) {
    if verbose() {
        eprintln!("oa: > {} {}", method, url);
    }
}

/// Note the status a request came back with.
pub fn response(status: u16, url: &str) {
    if verbose() {
        eprintln!("oa: < {} {}", status, url);
    }
}

/// Note a refusal the server explained.
///
/// Printed separately from [`response`] because the status alone does not say
/// which of six things the server objected to.
pub fn refused(status: u16, message: &str) {
    if verbose() {
        eprintln!("oa: ! {} {}", status, message);
    }
}

/// Note a request that never completed.
pub fn transport(url: &str, why: &str) {
    if verbose() {
        eprintln!("oa: ! {} did not complete: {}", url, why);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbose_is_off_until_it_is_asked_for() {
        set_verbose(false);
        assert!(!verbose());
        set_verbose(true);
        assert!(verbose());
        set_verbose(false);
    }

    #[test]
    fn no_color_is_exported_for_children() {
        std::env::remove_var("NO_COLOR");
        set_color(true);
        assert!(color());
        set_color(false);
        assert!(!color());
        assert_eq!(std::env::var("NO_COLOR").as_deref(), Ok("1"));
        // Leave the process as the other tests expect to find it.
        std::env::remove_var("NO_COLOR");
        set_color(true);
    }
}
