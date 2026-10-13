//! Coder One's delegations on the Coder SDK (#11509): a delegated Claude
//! Code or Codex session runs the SDK's hook bridge before every tool, and
//! the bridge asks this process's host, whose registry holds the floors.
//! Nobody answers an episode from inside, so a floored call (a production
//! deploy, a merge, a secret, a payment, a DNS change) is held for the
//! person; with the host gone the bridge's fallback holds it the same way.
//!
//! The bridge program is this binary's [`COMMAND`] subcommand ([`main`]).

use std::sync::{Arc, Mutex};

use coder_delegate::delegate::{Cli, HookBridge};

/// The subcommand that runs the bridge: `coder-one coder-hook --format
/// claude|codex`.
pub const COMMAND: &str = coder_sdk::bridge::COMMAND;

/// A delegated session's hooks: the floors, held.
#[must_use]
pub fn registry() -> coder_sdk::Registry {
    let mut registry = coder_sdk::Registry::new();
    registry
        .register(coder_sdk::floors::held_hook())
        .expect("one hook registers");
    registry
}

/// The host answering a delegated session's bridge, kept while it runs.
pub struct Installed {
    _host: coder_sdk::bridge::Host,
}

/// Starts the host for `cli`'s sessions and installs the bridge (this
/// program's [`COMMAND`]) as the CLI's own hook. `None`, with the CLI
/// left unchanged, when the host or this program's path can't be had.
pub fn install(cli: &mut Cli) -> Option<Installed> {
    let program = std::env::current_exe().ok()?;
    install_with(cli, program)
}

/// [`install`] with the bridge program named.
pub fn install_with(cli: &mut Cli, program: std::path::PathBuf) -> Option<Installed> {
    let runner = coder_sdk::Runner::new(registry(), coder_sdk::Budget::default());
    let host = coder_sdk::bridge::Host::start(Arc::new(Mutex::new(runner))).ok()?;
    cli.hooks = Some(HookBridge {
        program,
        args: vec![COMMAND.to_owned()],
        socket: host.path().to_path_buf(),
    });
    Some(Installed { _host: host })
}

/// The bridge program: reads one hook input, asks the host, prints the
/// reply; with no host, the held floors answer alone.
#[must_use]
pub fn main(args: &[String]) -> i32 {
    coder_sdk::bridge::main(args, coder_sdk::floors::held_decision)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::Duration;

    use coder_delegate::delegate::Agent;
    use coder_delegate::policy::ExecutorHost;
    use serde_json::{Value, json};

    use super::*;

    /// A Coder One delegation, built by the executor every episode uses
    /// and started through `CliSession` on a stand-in Claude Code, carries
    /// the bridge; the stand-in's hook call, answered by the session's
    /// host as the bridge program answers it, holds a merge, and the
    /// fallback holds it with no host at all.
    #[tokio::test]
    #[cfg(unix)]
    async fn a_delegated_session_runs_the_floors_through_the_bridge() {
        use coder_delegate::session::Session as _;
        let dir = tempfile::tempdir().unwrap();
        let recorded = dir.path().join("recorded");
        std::fs::create_dir_all(&recorded).unwrap();
        // A stand-in Claude Code that records its settings and the socket
        // it was given, and ends its turn.
        let claude = dir.path().join("claude");
        std::fs::write(
            &claude,
            format!(
                r#"#!/bin/sh
while [ $# -gt 0 ]; do
  if [ "$1" = --settings ]; then printf '%s' "$2" > '{dir}/settings.json'; fi
  shift
done
printf '%s' "$CODER_HOOK_SOCKET" > '{dir}/socket'
cat > /dev/null
echo '{{"type":"system","subtype":"init","session_id":"s","model":"stand-in"}}'
echo '{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"done","session_id":"s"}}'
"#,
                dir = recorded.display()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&claude).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&claude, permissions).unwrap();
        let policy: coder_delegate::policy::ExecutorPolicy = serde_json::from_value(json!({
            "agent": "claude-code", "version": null, "model": "stand-in", "effort": null,
            "tools": null, "prompt_cache_ttl": null, "deadline_sec": 30,
        }))
        .unwrap();
        let mut cli = coder_delegate::policy::executor(
            &policy,
            ExecutorHost {
                binary: Some(claude.clone()),
                credential: coder_delegate::delegate::Credential::OauthToken,
                workdir: dir.path().to_path_buf(),
                artifacts: dir.path().join("artifacts"),
                artifacts_label: "artifacts".into(),
                env: Vec::new(),
            },
        );
        assert_eq!(cli.agent, Agent::ClaudeCode);
        let _host = install_with(&mut cli, Path::new("/usr/local/bin/coder-one").into()).unwrap();
        let wrap = |command: std::process::Command| Ok::<_, String>(command);
        let mut session =
            coder_delegate::adapter::CliSession::new(&cli, claude, &wrap, "delegate-1");
        let briefing = coder_delegate::delegate::Briefing {
            text: "Merge it.".into(),
            cap: 1000,
            included: Vec::new(),
            omitted: Vec::new(),
        };
        session.start(&briefing).await.unwrap();
        let mut now = 0;
        while now < 20_000 && session.advance(now).await {
            now += 100;
        }
        let _ = session.observe();
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(recorded.join("settings.json")).unwrap())
                .unwrap();
        assert_eq!(
            settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
            "/usr/local/bin/coder-one coder-hook --format claude"
        );
        let socket = std::fs::read_to_string(recorded.join("socket")).unwrap();
        let merge = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": "gh pr merge 5 --squash"}});
        let reply = coder_sdk::bridge::ask(
            Path::new(&socket),
            coder_sdk::bridge::Format::Claude,
            &merge,
            Duration::from_secs(10),
        )
        .unwrap()
        .unwrap();
        assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            reply["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .starts_with("Held for the person")
        );
        let plain = json!({"hook_event_name": "PreToolUse", "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}});
        assert_eq!(
            coder_sdk::bridge::ask(
                Path::new(&socket),
                coder_sdk::bridge::Format::Claude,
                &plain,
                Duration::from_secs(10)
            )
            .unwrap(),
            None
        );
        // With no host, the bridge's fallback holds the merge alone.
        let alone = coder_sdk::floors::held_decision(&merge).unwrap();
        assert_eq!(alone["hookSpecificOutput"]["permissionDecision"], "deny");
    }
}
