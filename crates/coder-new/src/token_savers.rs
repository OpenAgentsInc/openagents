//! Token savers (#11494): Coder's opinionated cost shortcuts, one built-in
//! plugin on by default that a person can turn off on the Plugins screen.
//!
//! Each shortcut is a lever in [`LEVERS`] (`docs/coder/design/system-one-hooks.md`)
//! and installs its hooks into a Claude Code session only through
//! [`install`], so turning the plugin off removes every one of them from
//! new turns. Whether the plugin is on is a process-wide flag: the app sets
//! it when it loads its settings and when the person toggles it
//! ([`set_enabled`]); a headless run that never loads them reads the saved
//! `bundled-plugins.json`, and a missing or unreadable file means on.

use std::sync::atomic::{AtomicU8, Ordering};

use claude_agent_sdk::{HookEvent, QueryOptions};

/// The plugin's ID.
pub const PLUGIN: &str = "token-savers";

/// One shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lever {
    /// #11486: a noisy build, test, or lint command's output is trimmed to
    /// the lines that decide the next step before the agent reads it.
    Squeeze,
    /// #11489: a script the session never read is held for a read when the
    /// decision rates running it unread as risky.
    Gate,
}

/// Every lever, with the sentence the Plugins screen shows for it.
pub const LEVERS: &[(Lever, &str)] = &[
    (
        Lever::Squeeze,
        "Long build and test output is trimmed to its errors before the agent reads it; the full output is saved to a file.",
    ),
    (
        Lever::Gate,
        "A script the agent has not read is held for a read first when running it blind looks risky.",
    ),
];

/// Unset, off, or on.
static STATE: AtomicU8 = AtomicU8::new(UNSET);
const UNSET: u8 = 0;
const OFF: u8 = 1;
const ON: u8 = 2;

/// Records whether the plugin is on, for every later turn in this process.
pub fn set_enabled(enabled: bool) {
    STATE.store(if enabled { ON } else { OFF }, Ordering::Relaxed);
}

/// Whether the plugin is on.
#[must_use]
pub fn enabled() -> bool {
    match STATE.load(Ordering::Relaxed) {
        ON => true,
        OFF => false,
        // A test never reads the person's own settings.
        _ if cfg!(test) => true,
        _ => crate::home::data_dir()
            .is_none_or(|dir| crate::bundled_settings::saved_enabled(&dir, PLUGIN)),
    }
}

/// `options` with `lever`'s hooks when the plugin is on.
#[must_use]
pub fn install(options: QueryOptions, lever: Lever) -> QueryOptions {
    install_if(options, lever, enabled())
}

fn install_if(mut options: QueryOptions, lever: Lever, on: bool) -> QueryOptions {
    if !on {
        return options;
    }
    match lever {
        Lever::Squeeze => {
            if let Ok(this) = std::env::current_exe() {
                options = options.hook(
                    HookEvent::PreToolUse,
                    squeeze::claude_hook(squeeze::invoker(&this, &[squeeze::CODER_SUBCOMMAND])),
                );
            }
        }
        Lever::Gate => {
            options = options.hook(HookEvent::PreToolUse, crate::gate::claude_hook());
        }
    }
    options
}

/// The Plugins screen's description.
pub const DESCRIPTION: &str = "Shortcuts that cut what the agent reads and pays for. Long build and \
     test output is trimmed to its errors before the agent reads it, with the full output saved \
     to a file. Turn off to send everything through unchanged.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_installs_nothing_and_on_installs_the_squeezer() {
        let options = install_if(QueryOptions::new(), Lever::Squeeze, false);
        assert!(options.hooks.get(&HookEvent::PreToolUse).is_none());
        let options = install_if(QueryOptions::new(), Lever::Squeeze, true);
        let hooks = options.hooks.get(&HookEvent::PreToolUse).unwrap();
        assert_eq!(hooks[0].matcher.as_deref(), Some("Bash"));
    }
}
