//! OpenAgents #12056: `upgrade_host`, upgrade this computer's desktop app
//! from `main` with no owner steps.
//!
//! The tool starts `openagents host-upgrade --detach` from this binary: it
//! builds `main` in the shared target, checks the result, and hands it to the
//! app, which swaps it in at a quiet moment and reopens on the same chat
//! while agents under keepers keep running. Only a chat granted the
//! `host.upgrade` scope may start one (the command checks and logs it), and
//! `status` reads the log. Like `build_status`, it acts on this computer,
//! not through the engine.

use std::time::Duration;

use serde_json::{Value, json};

use crate::tools::ToolDef;

pub const NAME: &str = "upgrade_host";

const LIMIT: Duration = Duration::from_secs(60);

pub fn tool() -> ToolDef {
    ToolDef {
        name: NAME,
        description: "Upgrade the desktop app on this computer to the latest main: build it in the shared target, check it, and restart the app into it at a quiet moment (the owner isn't typing or on a call) with a notice first. Running chats keep going, and the app reopens on the same chat. A failed build or check leaves the running app alone and says why. Needs the host.upgrade scope. Returns at once; pass status: true for the waiting upgrade and the last results.",
        input_schema: json!({
            "type": "object",
            "properties": {
                "reason": {
                    "type": "string",
                    "description": "Why, for the upgrade log (e.g. the commit or issue you need)."
                },
                "force": {
                    "type": "boolean",
                    "description": "Rebuild even when main is what's running."
                },
                "status": {
                    "type": "boolean",
                    "description": "Only report the upgrade waiting in the app and the last results."
                }
            }
        }),
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    reason: String,
    #[serde(default)]
    force: bool,
    #[serde(default)]
    status: bool,
}

/// Run the tool for the chat `agent`.
pub fn call(args: Value, agent: Option<&str>) -> Result<Value, String> {
    let args: Args = if args.is_null() {
        Args {
            reason: String::new(),
            force: false,
            status: false,
        }
    } else {
        serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))?
    };
    let exe = std::env::current_exe().map_err(|e| format!("couldn't find this app: {e}"))?;
    let mut command = std::process::Command::new(exe);
    command.arg("host-upgrade");
    if args.status {
        command.arg("--status");
    } else {
        let Some(agent) = agent else {
            return Err(
                "upgrade_host runs only from a chat, which needs the host.upgrade scope".into(),
            );
        };
        if args.reason.trim().is_empty() {
            return Err("say why in reason; it goes in the upgrade log".into());
        }
        command.args(["--detach", "--agent", agent, "--reason", args.reason.trim()]);
        if args.force {
            command.arg("--force");
        }
    }
    let output = run(command)?;
    if output.0 {
        Ok(json!({ "result": output.1.trim() }))
    } else {
        Err(output.1.trim().to_owned())
    }
}

/// Success and the combined output, within [`LIMIT`].
fn run(mut command: std::process::Command) -> Result<(bool, String), String> {
    let mut child = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("couldn't start the upgrade: {e}"))?;
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > LIMIT => {
                let _ = child.kill();
                return Err(format!(
                    "the upgrade didn't answer within {} s",
                    LIMIT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(e.to_string()),
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("couldn't read the upgrade's answer: {e}"))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(stderr.trim().trim_start_matches("Error: "));
    }
    Ok((output.status.success(), text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_chat_with_a_reason_starts_one() {
        let err = call(json!({ "reason": "pick up #12056" }), None).unwrap_err();
        assert!(err.contains("host.upgrade"), "{err}");
        let err = call(json!({}), Some("9fa93200")).unwrap_err();
        assert!(err.contains("reason"), "{err}");
        assert!(call(json!({ "slot": 1 }), Some("9fa93200")).is_err());
    }
}
