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
    /// #11490: the first touch of a directory adds the instruction files
    /// Claude Code would not load for it.
    Context,
    /// #11487: an issue-run agent that stops without passing its final
    /// checks is sent back with their verdict, unless it says it is blocked
    /// or asks a question. The gate needs the session's checks, so the
    /// session builds it and installs it through [`install_stop_gate`].
    StopGate,
    /// #11491: each later chat message is routed: it continues the
    /// session, Coder answers it from what it knows, or new unrelated work
    /// starts a fresh session with a fresh briefing.
    Route,
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
    (
        Lever::Context,
        "A folder's own instructions are added the first time the agent works in it, when they matter for the task.",
    ),
    (
        Lever::StopGate,
        "An agent that stops on an issue before its final checks pass is sent back with what failed, instead of being told up front to verify everything.",
    ),
    (
        Lever::Route,
        "A later message that only asks what Coder already knows is answered without the agent, and one that starts unrelated work starts a fresh session instead of re-reading the old one.",
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

/// The squeezer as a Coder SDK hook (#11496), run through this binary's
/// squeeze subcommand, when the plugin is on. A session puts it in the
/// same registry as the floors, so the floors judge the command it wraps.
#[must_use]
pub fn sdk_hooks() -> Vec<coder_sdk::Hook> {
    if enabled() {
        squeeze_hook().into_iter().collect()
    } else {
        Vec::new()
    }
}

/// The squeezer for the Coder loop (#11520): the same rewrite as on Claude
/// Code's Bash, for the loop's `Run`, when the plugin is on.
#[must_use]
pub fn coder_loop_hooks() -> Vec<coder_sdk::Hook> {
    if !enabled() {
        return Vec::new();
    }
    let Some(this) = std::env::current_exe().ok() else {
        return Vec::new();
    };
    let invoker = squeeze::invoker(&this, &[squeeze::CODER_SUBCOMMAND]);
    vec![coder_loop_squeeze(invoker)]
}

/// The Coder loop's squeeze hook over `invoker`: the loop's `Run` read as
/// the Bash call the squeezer knows.
fn coder_loop_squeeze(invoker: String) -> coder_sdk::Hook {
    coder_sdk::Hook::new(
        "squeeze",
        coder_sdk::Event::PreToolUse,
        coder_sdk::Source::BuiltIn,
        move |call: &coder_sdk::Call| {
            let input = serde_json::json!({"tool_name": "Bash", "tool_input": call.input});
            Ok(match squeeze::hook_output(&input, &invoker) {
                Some(specific) => coder_sdk::HookOutcome::rewrite(specific["updatedInput"].clone()),
                None => coder_sdk::HookOutcome::none(),
            })
        },
    )
    .matching("Run")
}

fn squeeze_hook() -> Option<coder_sdk::Hook> {
    Some(squeeze::sdk_hook(invoker()?))
}

/// The squeezer's command line, through this binary.
fn invoker() -> Option<String> {
    let this = std::env::current_exe().ok()?;
    Some(squeeze::invoker(&this, &[squeeze::CODER_SUBCOMMAND]))
}

/// What a harness that runs commands as it asked them (Codex, #11525) is
/// told up front, when the plugin is on: run noisy commands through the
/// squeezer, so its hook needn't send one back.
#[must_use]
pub fn squeeze_instructions() -> Option<String> {
    if !enabled() || squeeze::off() {
        return None;
    }
    Some(format!(
        "Run each build, test, or lint command (cargo test, cargo build, npm test, pytest, go test, make and the like) through Coder's squeezer, as `{} '<command>'`: it trims long output to the lines that decide your next step and names a file with the full output; read that file only for a line the trimmed output left out. Coder sends such a command back when it isn't run that way.\n",
        invoker()?
    ))
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
            let mut registry = coder_sdk::Registry::new();
            if let Some(hook) = squeeze_hook()
                && registry.register(hook).is_ok()
                && let Ok(backend) =
                    coder_sdk::claude::Claude::new(registry, coder_sdk::Budget::default())
            {
                options = backend.install(options, coder_sdk::claude::PreTool::Hook);
            }
        }
        Lever::Gate => {
            options = options.hook(HookEvent::PreToolUse, crate::gate::claude_hook());
        }
        Lever::Context => {
            let cwd = options.cwd.clone().unwrap_or_default();
            for (event, matcher) in crate::context_hook::hooks(cwd) {
                options = options.hook(event, matcher);
            }
        }
        // Built per session: see [`install_stop_gate`].
        Lever::StopGate => {}
        // Registered with the floors in the chat's own registry: see
        // [`crate::sdk_hooks::system_one`] and [`crate::sdk_hooks::route_mode`].
        Lever::Route => {}
    }
    options
}

/// `options` with `gate`'s `Stop` hook when the plugin is on (#11487), and
/// whether it went in.
#[must_use]
pub fn install_stop_gate(
    options: QueryOptions,
    gate: std::sync::Arc<briefed_agent::stop_gate::StopGate>,
) -> (QueryOptions, bool) {
    install_stop_gate_if(options, gate, enabled())
}

fn install_stop_gate_if(
    options: QueryOptions,
    gate: std::sync::Arc<briefed_agent::stop_gate::StopGate>,
    on: bool,
) -> (QueryOptions, bool) {
    if on {
        (options.hook(HookEvent::Stop, gate.hook()), true)
    } else {
        (options, false)
    }
}

/// The Plugins screen's description.
pub const DESCRIPTION: &str = "Shortcuts that cut what the agent reads and pays for. On Claude Code \
     and Coder's own loop, long build and test output is trimmed to its errors before the agent \
     reads it, with the full output saved to a file. Turn off to send everything through \
     unchanged.";

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

    #[test]
    fn the_route_follows_the_plugin_unless_its_switch_says_otherwise() {
        // In tests the plugin is on and `CODER_ROUTE` is unset.
        assert_eq!(crate::sdk_hooks::route_mode(), crate::sdk_hooks::Mode::On);
    }

    #[test]
    fn the_stop_gate_goes_in_only_when_the_plugin_is_on() {
        use briefed_agent::stop_gate::{Check, Classify, StopGate, Tree};
        use std::sync::Arc;
        let check: Check = Arc::new(|| Box::pin(async { serde_json::json!({"status": "pass"}) }));
        let tree: Tree = Arc::new(|| Box::pin(async { None }));
        let classify: Classify = Arc::new(|_| Box::pin(async { Err("offline".to_owned()) }));
        let gate = Arc::new(StopGate::new(
            Arc::default(),
            Arc::default(),
            classify,
            check,
            tree,
            None,
        ));
        let (options, on) = install_stop_gate_if(QueryOptions::new(), gate.clone(), false);
        assert!(!on && options.hooks.get(&HookEvent::Stop).is_none());
        let (options, on) = install_stop_gate_if(QueryOptions::new(), gate, true);
        assert!(on && options.hooks.get(&HookEvent::Stop).unwrap().len() == 1);
    }
}
