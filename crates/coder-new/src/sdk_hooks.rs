//! Coder's hook sets on the Coder SDK (`docs/coder/design/coder-sdk.md`):
//! the same built-ins registered once and run by whichever backend takes
//! the turn, Claude Code (#11496) or Coder's own loop (#11497).

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use coder_sdk::{Budget, Registry};

/// The built-in hooks a session runs: the session worktree's fence
/// (#11485) and the floors on every turn, and on a
/// full turn the token savers' squeezer, registered before them in the
/// rewrite chain (the floors judge the final input whatever the order).
#[must_use]
pub fn registry(cwd: &Path, full: bool) -> Registry {
    let mut registry = Registry::new();
    let mut hooks = vec![
        crate::checkout_guard::sdk_hook(cwd.to_owned()),
        crate::floors::sdk_hook(cwd.to_owned()),
    ];
    if full {
        hooks.extend(crate::token_savers::sdk_hooks());
    }
    for hook in hooks {
        // Built-in IDs are distinct, and the floors fall back to ask.
        let _ = registry.register(hook);
    }
    registry
}

/// The Claude Code backend for a session in `cwd`: [`registry`], with the
/// floors' asks answered on the approval card where someone can answer.
#[must_use]
pub fn claude(cwd: &Path, full: bool) -> coder_sdk::claude::Claude {
    let mut registry = registry(cwd, full);
    if full {
        system_one(&mut registry, cwd, Harness::ClaudeCode);
    }
    coder_sdk::claude::Claude::new(registry, Budget::default())
        .expect("every hook event is native on Claude Code")
        .asking(crate::floors::asker(
            cwd.to_owned(),
            crate::ui::claude_rail_label,
            None,
        ))
}

/// The Coder loop's hooks for a run in `cwd` (#11497): the floors, the
/// same table as on Claude Code, asking on the approval card until the
/// run is cancelled.
#[must_use]
pub fn coder_loop(cwd: &Path, cancel: &Arc<AtomicBool>) -> coder_sdk::Hooks {
    let mut registry = registry(cwd, false);
    system_one(&mut registry, cwd, Harness::CoderLoop);
    coder_sdk::Hooks::new(
        registry,
        Budget::default(),
        coder_sdk::HookBackend::CoderLoop,
    )
    .expect("the floors fire natively on the Coder loop")
    .asking(crate::floors::asker(
        cwd.to_owned(),
        || "Coder".to_owned(),
        Some(Arc::clone(cancel)),
    ))
}

/// Where a session's System One hooks run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harness {
    ClaudeCode,
    CoderLoop,
}

/// How a System One hook runs until its A/B arm has measured it: off,
/// recording what it would do (`shadow`), or acting (`on`). Set per hook
/// with its variable; turning one on admits what it sends to the
/// Decisions API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Off,
    Shadow,
    On,
}

/// The monitor's switch (#11488).
pub const MONITOR_VAR: &str = "CODER_MONITOR";
/// The router's switch (#11491).
pub const ROUTE_VAR: &str = "CODER_ROUTE";

/// The mode `var` sets: `shadow` or `on`, else off.
#[must_use]
pub fn mode(var: &str) -> Mode {
    match std::env::var(var)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "shadow" => Mode::Shadow,
        "on" | "1" | "true" => Mode::On,
        _ => Mode::Off,
    }
}

/// Registers the System One decision hooks that are switched on: the
/// monitor on both harnesses, the router in Claude Code chats. A hook is
/// registered only when the session's admission covers what it sends; the
/// switch is that admission.
pub fn system_one(registry: &mut Registry, cwd: &Path, harness: Harness) {
    system_one_with(registry, cwd, harness, mode(MONITOR_VAR), mode(ROUTE_VAR));
}

/// [`system_one`] with the monitor's and the router's modes given.
pub fn system_one_with(
    registry: &mut Registry,
    cwd: &Path,
    harness: Harness,
    monitor: Mode,
    route: Mode,
) {
    let mut admission = coder_sdk::Admission::none();
    let mut hooks = Vec::new();
    let decider: Arc<dyn coder_sdk::decide::Decider> = Arc::new(coder_sdk::decide::Api);
    let recorder = coder_sdk::decision::default_recorder();
    if monitor != Mode::Off {
        admission = admission.admitting(coder_sdk::system_one::monitor::disclosure());
        hooks.push(coder_sdk::DecisionHook {
            shadow: monitor == Mode::Shadow,
            ..coder_sdk::system_one::monitor::hook(
                harness == Harness::ClaudeCode,
                Some(probe(cwd)),
                decider.clone(),
                recorder.clone(),
            )
        });
    }
    if route != Mode::Off && harness == Harness::ClaudeCode {
        admission = admission.admitting(coder_sdk::system_one::route::disclosure());
        hooks.push(coder_sdk::DecisionHook {
            shadow: route == Mode::Shadow,
            ..coder_sdk::system_one::route::hook(None, decider, recorder)
        });
    }
    for hook in hooks {
        // A refusal is recorded by the hook kind itself.
        let _ = hook.register(registry, &admission);
    }
}

/// The monitor's `fetch_evidence` probe: the working tree's changes, as
/// `git status --short` and `git diff --stat` print them.
fn probe(cwd: &Path) -> coder_sdk::system_one::monitor::Probe {
    let cwd = cwd.to_owned();
    Arc::new(move || {
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&cwd)
                .args(args)
                .stderr(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        };
        let status = run(&["status", "--short"])?;
        let stat = run(&["diff", "--stat"]).unwrap_or_default();
        let found = format!("{status}\n{stat}").trim().to_owned();
        Some(if found.is_empty() {
            "no changes in the working tree".to_owned()
        } else {
            found
        })
    })
}

/// The floors for a session nobody can answer from inside (a Codex run's
/// command hooks, Coder's tools on stdio): the same table, with an `ask`
/// rule held and a `deny` rule refused, in the words the floors' Codex
/// hook always used ([`crate::floors::hook_decision`]). A call is judged by
/// its plain name, so `mcp__coder__openagents_cli` is `openagents_cli`.
#[must_use]
pub fn held_floors(cwd: std::path::PathBuf) -> coder_sdk::Hook {
    let server = format!("mcp__{}__", crate::claude_tools::SERVER);
    coder_sdk::Hook::floor(
        "floors",
        coder_sdk::Event::PreToolUse,
        coder_sdk::Permission::ask("Coder could not check this step."),
        move |call: &coder_sdk::Call| {
            let tool = call.tool.as_deref().unwrap_or_default();
            let plain = tool.strip_prefix(&server).unwrap_or(tool);
            let Some(floor) = crate::floors::classify(plain, &call.input) else {
                return Ok(coder_sdk::HookOutcome::none());
            };
            Ok(match crate::floors::rule(&cwd, floor.ability) {
                crate::risk_policy::Rule::Allow => coder_sdk::HookOutcome::none(),
                crate::risk_policy::Rule::Deny => {
                    coder_sdk::HookOutcome::deny(&crate::floors::refused(&floor))
                }
                crate::risk_policy::Rule::Ask => {
                    coder_sdk::HookOutcome::deny(&crate::floors::held(&floor))
                }
            })
        },
    )
    .pinned(coder_sdk::Versions {
        question_set: None,
        policy: Some(crate::floors::FLOORS_POLICY.to_owned()),
        model: None,
    })
}

/// The hooks a Codex run gets on its command tier (#11500): the floors
/// ([`held_floors`]), then the person's and the project's command hooks
/// (`~/.openagents/hooks.json`, `.openagents/hooks.json`), composed by the
/// SDK's rules, so no command hook gets past a floor. Also the problems
/// reading the command hooks, if any.
#[must_use]
pub fn codex_registry(cwd: &Path) -> (Registry, Vec<String>) {
    let mut registry = Registry::new();
    let _ = registry.register(held_floors(cwd.to_owned()));
    let (hooks, mut problems) =
        coder_sdk::command::load(cwd, coder_sdk::command::home().as_deref());
    let session = std::env::var(hook_log::RUN_ID_VAR).unwrap_or_default();
    problems.extend(coder_sdk::command::register(
        &mut registry,
        &hooks,
        cwd,
        &session,
    ));
    (registry, problems)
}

/// The host a Codex run's hook bridge asks ([`coder_sdk::bridge`]):
/// [`codex_registry`] on a socket, kept for the run. None when the socket
/// can't be made; the bridge then falls back to the floors alone.
#[must_use]
pub fn codex_host(cwd: &Path) -> Option<coder_sdk::bridge::Host> {
    let (registry, _problems) = codex_registry(cwd);
    let runner = coder_sdk::Runner::new(registry, Budget::default());
    coder_sdk::bridge::Host::start(Arc::new(std::sync::Mutex::new(runner))).ok()
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn codex_shell(cwd: &Path, command: &str) -> Value {
        json!({
            "session_id": "019a", "turn_id": "1", "cwd": cwd, "model": "gpt-6.1-sol",
            "permission_mode": "bypassPermissions", "hook_event_name": "PreToolUse",
            "tool_name": "shell",
            "tool_input": {"command": command.split_whitespace().collect::<Vec<_>>()},
            "tool_use_id": "call_1",
        })
    }

    /// A Codex run's bridge answers from the floors and the project's
    /// command hooks together, and a command hook's allow never lifts a
    /// floor.
    #[test]
    fn codex_runs_get_the_floors_and_the_projects_command_hooks() {
        let _lock = crate::approval::test_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(repo.path())
            .status()
            .unwrap();
        std::fs::create_dir_all(repo.path().join(".openagents")).unwrap();
        // The project's hook: no `rm -rf`, and allow everything else.
        std::fs::write(
            repo.path().join(".openagents/hooks.json"),
            json!({"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command":
                "if grep -q '\"rm\",\"-rf\"' ; then echo 'This project keeps its build folder.' >&2; exit 2; fi; \
                 printf '%s' '{\"hookSpecificOutput\":{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"allow\"}}'"
            }]}]}})
            .to_string(),
        )
        .unwrap();
        let (registry, problems) = codex_registry(repo.path());
        assert!(problems.is_empty(), "{problems:?}");
        let mut runner = coder_sdk::Runner::new(registry, Budget::default());
        let codex = coder_sdk::bridge::Format::Codex;
        let ask = |runner: &mut coder_sdk::Runner, command: &str| {
            coder_sdk::bridge::answer(runner, codex, &codex_shell(repo.path(), command))
        };
        let removed = ask(&mut runner, "rm -rf target").unwrap();
        assert_eq!(removed["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            removed["hookSpecificOutput"]["permissionDecisionReason"],
            "This project keeps its build folder."
        );
        // The project's allow alone is dropped: Codex fails it open.
        assert_eq!(ask(&mut runner, "ls"), None);
        // A floored call: the project hook allows it, the floors hold it.
        let held = ask(&mut runner, "openagents wallet send x").unwrap();
        assert_eq!(held["hookSpecificOutput"]["permissionDecision"], "deny");
        let reason = held["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap();
        // The same words the floors' own Codex hook gives.
        let alone =
            crate::floors::hook_decision(&codex_shell(repo.path(), "openagents wallet send x"))
                .unwrap();
        assert_eq!(
            reason,
            alone["hookSpecificOutput"]["permissionDecisionReason"]
        );
    }

    /// With its switch on, the monitor is registered on both harnesses and
    /// the router on Claude Code only, each pinned in the hook set; off,
    /// neither is.
    #[test]
    fn the_switches_register_the_system_one_hooks() {
        let dir = tempfile::tempdir().unwrap();
        let registry = |harness, monitor, route| {
            let mut registry = Registry::new();
            system_one_with(&mut registry, dir.path(), harness, monitor, route);
            registry
        };
        let ids = |registry: &Registry| {
            registry
                .hooks()
                .map(|hook| hook.id.clone())
                .collect::<Vec<_>>()
        };
        assert!(ids(&registry(Harness::ClaudeCode, Mode::Off, Mode::Off)).is_empty());
        let both = registry(Harness::ClaudeCode, Mode::Shadow, Mode::On);
        assert_eq!(ids(&both), ["route", "monitor"]);
        assert_eq!(
            ids(&registry(Harness::CoderLoop, Mode::Shadow, Mode::On)),
            ["monitor"]
        );
        let set = coder_sdk::hook_set(&both);
        let pinned: Vec<&str> = set["hooks"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|hook| hook["question_set"].as_str())
            .collect();
        assert_eq!(pinned, ["route-v1", "monitor-v1"]);
        // The probe reads the working tree.
        assert!(probe(dir.path())().is_none(), "not a checkout");
    }
}
