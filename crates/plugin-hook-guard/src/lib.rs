//! Hook guard: a plugin whose Wasm runs as Coder SDK hooks (#11499,
//! `docs/coder/design/coder-sdk.md`, "Plugin hooks").
//!
//! The host hands each firing the normalized event as the packet's input,
//! `{event, tool, input}` (only the fields the hook's entry discloses), and
//! reads the reply as a hook outcome: `permission` (`deny` or `ask` with a
//! reason or step; a plugin's `allow` never counts), `context`, and
//! `rewrite`, which counts only when the person turned it on. A refusal is
//! no opinion.
//!
//! Operations:
//!
//! - `guard` (`PreToolUse` on `Bash`): denies a force push and deleting
//!   the whole disk;
//! - `remind` (`PostToolUse`): adds one line of context after a command;
//! - `dry_run` (`PreToolUse` on `Bash`): rewrites `deploy ...` to add
//!   `--dry-run`, the rewrite the person must turn on;
//! - `risky` (`PreToolUse` on `Bash`): asks the host's `decide` capability
//!   whether the command could delete or overwrite data, and asks the
//!   person when it probably could.
//!
//! Build: `cargo build -p plugin-hook-guard --target wasm32-unknown-unknown
//! --profile guest`, then copy `plugin_hook_guard.wasm` to
//! `wasm/hook-guard.wasm` and restate its digest in each `hooks/*.json`.

use plugin_pdk::Request;
use plugin_pdk::guest::{Host, Refusal};
use serde_json::{Value, json};

plugin_pdk::export_guest!(handle);

/// The host capability that asks a decision model.
const DECIDE: &str = "decide";

/// The probability above which `risky` asks the person.
const RISKY: f64 = 0.8;

/// The guest's one entry point.
///
/// # Errors
///
/// An unknown operation is unsupported.
pub fn handle(request: &Request, host: &mut dyn Host) -> Result<Value, Refusal> {
    let command = request.input["input"]["command"]
        .as_str()
        .unwrap_or_default();
    match request.operation.as_str() {
        "guard" => Ok(guard(command)),
        "remind" => Ok(json!({"context": {
            "source": "hook-guard",
            "text": "Hook guard: check the command's output before you go on.",
            "priority": 0,
        }})),
        "dry_run" => Ok(command
            .strip_prefix("deploy")
            .filter(|_| !command.contains("--dry-run"))
            .map_or_else(
                || json!({}),
                |rest| json!({"rewrite": {"command": format!("deploy --dry-run{rest}")}}),
            )),
        "risky" => risky(command, host),
        _ => Err(Refusal::unsupported("operation")),
    }
}

fn guard(command: &str) -> Value {
    let words: Vec<&str> = command.split_whitespace().collect();
    let pushes = words.windows(2).any(|pair| pair == ["git", "push"]);
    let forced = words
        .iter()
        .any(|word| matches!(*word, "--force" | "-f" | "--force-with-lease"));
    let disk = words
        .windows(3)
        .any(|w| w[0] == "rm" && w[1].starts_with("-r") && w[2] == "/");
    let reason = if pushes && forced {
        "Hook guard: no force pushes here. Push a new commit instead."
    } else if disk {
        "Hook guard: that would delete the whole disk."
    } else {
        return json!({});
    };
    json!({"permission": {"permission": "deny", "reason": reason}})
}

fn risky(command: &str, host: &mut dyn Host) -> Result<Value, Refusal> {
    if command.is_empty() {
        return Ok(json!({}));
    }
    // The host's `decide` capability, on the one import; unsupported
    // (no decision granted) is a refusal, which is no opinion.
    let answer = host
        .call(&json!({"v": 1, "capability": DECIDE, "args": {
            "input": command,
            "question": "Could running this shell command delete or overwrite data that isn't easy to get back?",
        }}))
        .map_err(|_| Refusal::refused("no decision"))?;
    let probability = answer["probability"].as_f64().unwrap_or(0.0);
    Ok(if probability > RISKY {
        json!({"permission": {"permission": "ask", "step": "Hook guard: this command could delete data. Confirm it first."}})
    } else {
        json!({})
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Answering(f64);

    impl Host for Answering {
        fn call(&mut self, request: &Value) -> Result<Value, i32> {
            assert_eq!(request["capability"], "decide");
            Ok(json!({"probability": self.0}))
        }
    }

    fn request(operation: &str, command: &str) -> Request {
        plugin_pdk::guest::MemoryHost::request(
            operation,
            json!({"event": "PreToolUse", "tool": "Bash", "input": {"command": command}}),
        )
    }

    #[test]
    fn guard_denies_force_pushes_and_the_disk() {
        let mut host = Answering(0.0);
        let deny = handle(&request("guard", "git push --force origin main"), &mut host).unwrap();
        assert_eq!(deny["permission"]["permission"], "deny");
        assert_eq!(
            handle(&request("guard", "git push origin main"), &mut host).unwrap(),
            json!({})
        );
        let disk = handle(&request("guard", "rm -rf /"), &mut host).unwrap();
        assert_eq!(disk["permission"]["permission"], "deny");
    }

    #[test]
    fn dry_run_and_risky() {
        let mut host = Answering(0.95);
        let rewrite = handle(&request("dry_run", "deploy --prod"), &mut host).unwrap();
        assert_eq!(rewrite["rewrite"]["command"], "deploy --dry-run --prod");
        let ask = handle(
            &request("risky", "dd if=/dev/zero of=/dev/disk2"),
            &mut host,
        )
        .unwrap();
        assert_eq!(ask["permission"]["permission"], "ask");
        let mut calm = Answering(0.1);
        assert_eq!(
            handle(&request("risky", "ls"), &mut calm).unwrap(),
            json!({})
        );
    }
}
