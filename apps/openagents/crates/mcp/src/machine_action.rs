//! OpenAgents #12033: `machine_action`, pruning a build cache or stopping a
//! process under the owner's grant, logged.
//!
//! The owner grants machine scopes to an agent ID at a terminal
//! (`openagents lease grant machine CHAT_ID --scope prune,stop`). This tool
//! acts for the chat the server speaks for, so its chat ID is the agent ID;
//! there is no argument to name another. `coder_lease::machine` checks the
//! grant, refuses what the build sweep would never touch, and logs every
//! action and refusal with the Chicago time. `log` and `grants` read that
//! record, which the owner reads with `openagents lease actions`.

use std::path::{Path, PathBuf};

use coder_lease::agent_builds::sweep::Layout;
use coder_lease::machine::{self, Action, Grants, Hands};
use serde_json::{Value, json};

use crate::tools::ToolDef;

pub const NAME: &str = "machine_action";

pub fn tool() -> ToolDef {
    ToolDef {
        name: NAME,
        description: "Act on this computer under the owner's grant, logged: prune a build cache (a Cargo target folder no build holds) or stop a process with SIGTERM. Each needs the owner's machine grant to this chat (`openagents lease grant machine CHAT_ID --scope prune,stop`) and a reason; without the grant it's refused, so hand the job to a chat or ask the owner. Deploys are never stopped. Every action and refusal goes in the log with the time, size or pid, and reason. Use action log to read the log and action grants to see what this chat holds.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["prune", "stop", "log", "grants"]
                },
                "path": {
                    "type": "string",
                    "description": "prune: the absolute path of the target folder, or a folder inside one."
                },
                "pid": {
                    "type": "integer",
                    "minimum": 2,
                    "description": "stop: the process to stop."
                },
                "reason": {
                    "type": "string",
                    "description": "prune and stop: why, in one line. It goes in the log."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "description": "log: how many recent entries (default 20)."
                }
            },
            "required": ["action"]
        }),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    action: String,
    path: Option<String>,
    pid: Option<u32>,
    reason: Option<String>,
    limit: Option<usize>,
}

/// Run the tool for the chat `chat_id` on this computer.
pub fn call(chat_id: Option<&str>, args: Value) -> Result<Value, String> {
    let (root, layout) = machine::here()?;
    call_at(&root, &layout, &machine::System, chat_id, args)
}

fn call_at(
    root: &Path,
    layout: &Layout,
    hands: &dyn Hands,
    chat_id: Option<&str>,
    args: Value,
) -> Result<Value, String> {
    let args: Args = serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))?;
    let action = match args.action.as_str() {
        "log" => {
            let entries = machine::read_log(root, args.limit.unwrap_or(20))
                .map_err(|e| format!("couldn't read the machine action log: {e}"))?;
            return Ok(json!({ "entries": entries }));
        }
        "grants" => {
            let grants = Grants::load(root)?;
            let now = coder_lease::now_ms();
            let scopes = chat_id.map_or_else(Vec::new, |chat| grants.scopes_of(chat, now));
            return Ok(json!({ "agent": chat_id, "scopes": scopes, "grants": grants.grants }));
        }
        "prune" => Action::Prune(PathBuf::from(
            args.path
                .ok_or("prune needs path: the target folder to remove")?,
        )),
        "stop" => Action::Stop(args.pid.ok_or("stop needs pid: the process to stop")?),
        other => {
            return Err(format!(
                "unknown action `{other}`: use prune, stop, log, or grants"
            ));
        }
    };
    let chat = chat_id.ok_or(
        "machine actions run only from an app chat, whose ID is the agent ID the owner grants",
    )?;
    let entry = machine::act(
        root,
        layout,
        hands,
        chat,
        &action,
        args.reason.as_deref().unwrap_or(""),
    )?;
    let line = entry.line();
    Ok(json!({ "outcome": entry.outcome, "entry": entry, "line": line }))
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use super::*;
    use coder_lease::agent_builds::sweep::Proc;
    use coder_lease::machine::{ProcInfo, Scope};

    struct Fixture;

    impl Hands for Fixture {
        fn builds(&self) -> Result<Vec<Proc>, String> {
            Ok(Vec::new())
        }
        fn process(&self, _: u32) -> Option<ProcInfo> {
            None
        }
        fn ancestors(&self) -> Vec<u32> {
            Vec::new()
        }
        fn terminate(&self, _: u32) -> std::io::Result<()> {
            Err(std::io::Error::other("fixtures never signal"))
        }
        fn remove(&self, path: &Path) -> std::io::Result<()> {
            std::fs::remove_dir_all(path)
        }
        fn cargo_locked(&self, _: &Path) -> bool {
            false
        }
        fn now(&self) -> SystemTime {
            SystemTime::now()
        }
        fn stamp(&self, _: SystemTime) -> String {
            "2026-10-17 9:30:00 PM CDT".to_owned()
        }
    }

    #[test]
    fn prunes_only_under_this_chats_grant_and_logs_it() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().canonicalize().unwrap();
        let root = home.join(".openagents/leases");
        let target = home.join("work/openagents-target-9");
        std::fs::create_dir_all(target.join("debug/deps")).unwrap();
        let layout = Layout::at(&home, &root);
        let prune = json!({"action": "prune", "path": target, "reason": "disk at 30 GiB"});

        let out = call_at(&root, &layout, &Fixture, Some("chat-a"), prune.clone()).unwrap();
        assert_eq!(out["outcome"], "refused");
        assert!(target.is_dir());
        assert!(call_at(&root, &layout, &Fixture, None, prune.clone()).is_err());

        machine::grant(&root, "chat-a", &[Scope::BuildPrune], None).unwrap();
        let out = call_at(&root, &layout, &Fixture, Some("chat-b"), prune.clone()).unwrap();
        assert_eq!(out["outcome"], "refused", "the grant is chat-a's alone");
        let out = call_at(&root, &layout, &Fixture, Some("chat-a"), prune).unwrap();
        assert_eq!(out["outcome"], "done", "{out}");
        assert!(!target.exists());

        let grants = call_at(
            &root,
            &layout,
            &Fixture,
            Some("chat-a"),
            json!({"action": "grants"}),
        )
        .unwrap();
        assert_eq!(grants["scopes"], json!(["build.prune"]));
        let log = call_at(
            &root,
            &layout,
            &Fixture,
            None,
            json!({"action": "log", "limit": 2}),
        )
        .unwrap();
        let entries = log["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1]["agent"], "chat-a");
        assert_eq!(entries[1]["outcome"], "done");
        assert_eq!(entries[1]["at"], "2026-10-17 9:30:00 PM CDT");
        assert!(
            call_at(
                &root,
                &layout,
                &Fixture,
                Some("chat-a"),
                json!({"action": "deploy"})
            )
            .is_err()
        );
    }
}
