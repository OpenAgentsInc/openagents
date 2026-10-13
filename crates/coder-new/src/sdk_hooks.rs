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
    coder_sdk::claude::Claude::new(registry(cwd, full), Budget::default())
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
    coder_sdk::Hooks::new(
        registry(cwd, false),
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
}
