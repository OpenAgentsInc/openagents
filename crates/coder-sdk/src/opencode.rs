//! OpenCode as a Coder harness (#11507): its hook seat.
//!
//! OpenCode runs plugins in its own process, and a plugin's
//! `tool.execute.before` runs before every tool, built-in or MCP, with the
//! tool's exact arguments. Throwing there stops the tool: OpenCode records
//! the thrown message as the tool's error and the tool never runs (seen on
//! OpenCode 1.18.26: a held `rm a.txt` left the file in place). Changing
//! `output.args` there changes what the tool runs with.
//!
//! So Coder writes one small plugin ([`PLUGIN_SOURCE`]) into a
//! configuration directory of its own, named to OpenCode by
//! `OPENCODE_CONFIG_DIR` (OpenCode still reads the person's own
//! configuration as well). The plugin hands each call to the hook bridge,
//! `coder coder-hook --format opencode`, which asks the session's host
//! ([`crate::bridge`]) or, with no host, the floors alone. That is the
//! command tier for `PreToolUse`; every other event is only seen in the
//! stream.
//!
//! The bridge reads OpenCode's call as the Claude Code hook input the
//! registry knows ([`to_claude`]: `bash` is `Bash`, `filePath` is
//! `file_path`, ...), and answers in OpenCode's terms ([`reply`]): a
//! denial's reason, or the rewritten arguments. Nobody can answer a
//! question from inside OpenCode's tool call, so an ask is held, as on
//! Codex.

use serde_json::{Map, Value, json};

/// The variable naming the program the plugin runs (`coder`).
pub const PROGRAM_VAR: &str = "CODER_HOOK_PROGRAM";

/// The plugin's file name in the configuration directory's `plugin/`.
pub const PLUGIN_FILE: &str = "coder-guard.js";

/// The MCP server name Coder's tools have in OpenCode, which names their
/// tools `coder_<tool>`.
pub const CODER_SERVER: &str = "coder";

/// Coder's OpenCode plugin. It fails closed: when the bridge can't run or
/// its answer can't be read, the tool does not run.
pub const PLUGIN_SOURCE: &str = r#"// Coder's guard for OpenCode. Coder writes this file; edits are replaced.
// Every tool call is checked by Coder before it runs (#11507).
import { spawn } from "node:child_process";

const program = process.env.CODER_HOOK_PROGRAM;
const unchecked = "Coder could not check this step, so it did not run.";

function ask(call) {
  return new Promise((resolve) => {
    if (!program) {
      resolve(null);
      return;
    }
    let out = "";
    let child;
    try {
      child = spawn(program, ["coder-hook", "--format", "opencode"], {
        stdio: ["pipe", "pipe", "ignore"],
      });
    } catch {
      resolve(null);
      return;
    }
    child.stdout.on("data", (chunk) => {
      out += chunk;
    });
    child.on("error", () => resolve(null));
    child.on("close", (code) => resolve(code === 0 ? out : null));
    child.stdin.end(JSON.stringify(call));
  });
}

export const CoderGuard = async ({ directory }) => ({
  "tool.execute.before": async (input, output) => {
    const text = await ask({
      tool: input.tool,
      sessionID: input.sessionID,
      callID: input.callID,
      args: output.args ?? {},
      cwd: directory,
    });
    if (text === null) throw new Error(unchecked);
    const trimmed = text.trim();
    if (!trimmed) return;
    let reply;
    try {
      reply = JSON.parse(trimmed);
    } catch {
      throw new Error(unchecked);
    }
    if (typeof reply.deny === "string") throw new Error(reply.deny);
    if (reply.args && typeof reply.args === "object") {
      for (const key of Object.keys(output.args ?? {})) delete output.args[key];
      Object.assign(output.args, reply.args);
    }
  },
});
"#;

/// Writes Coder's plugin ([`PLUGIN_SOURCE`]) into the OpenCode
/// configuration directory `dir` (`dir/plugin/`), the one named to OpenCode
/// by `OPENCODE_CONFIG_DIR`; an unchanged plugin is left as it is.
///
/// # Errors
/// The directory or the file can't be written.
pub fn write_plugin(dir: &std::path::Path) -> std::io::Result<()> {
    let plugins = dir.join("plugin");
    std::fs::create_dir_all(&plugins)?;
    let path = plugins.join(PLUGIN_FILE);
    if std::fs::read_to_string(&path).ok().as_deref() != Some(PLUGIN_SOURCE) {
        std::fs::write(&path, PLUGIN_SOURCE)?;
    }
    Ok(())
}

/// OpenCode's tool name as Claude Code names the same tool, so the floors,
/// the worktree fence and command hooks written for Claude Code read it.
/// Coder's own MCP tools (`coder_agent`) become `mcp__coder__agent`.
#[must_use]
pub fn tool_name(tool: &str) -> String {
    let named = match tool {
        "bash" => "Bash",
        "read" => "Read",
        "write" => "Write",
        "edit" => "Edit",
        "multiedit" => "MultiEdit",
        "patch" | "apply_patch" => "apply_patch",
        "glob" => "Glob",
        "grep" => "Grep",
        "list" => "LS",
        "webfetch" => "WebFetch",
        "websearch" => "WebSearch",
        "task" => "Task",
        "todowrite" => "TodoWrite",
        "todoread" => "TodoRead",
        other => {
            return match other.strip_prefix(&format!("{CODER_SERVER}_")) {
                Some(rest) if !rest.is_empty() => format!("mcp__{CODER_SERVER}__{rest}"),
                _ => other.to_owned(),
            };
        }
    };
    named.to_owned()
}

/// Field names OpenCode and Claude Code spell differently: (OpenCode,
/// Claude Code).
const FIELDS: [(&str, &str); 4] = [
    ("filePath", "file_path"),
    ("oldString", "old_string"),
    ("newString", "new_string"),
    ("replaceAll", "replace_all"),
];

fn renamed(args: &Value, patch: bool, to_claude: bool) -> Value {
    let Value::Object(fields) = args else {
        return args.clone();
    };
    let mut out = Map::new();
    for (key, value) in fields {
        let key = if patch {
            match (key.as_str(), to_claude) {
                ("patchText", true) => "input".to_owned(),
                ("input", false) => "patchText".to_owned(),
                _ => key.clone(),
            }
        } else {
            FIELDS
                .iter()
                .find_map(|(open, claude)| {
                    let (from, to) = if to_claude {
                        (open, claude)
                    } else {
                        (claude, open)
                    };
                    (key == from).then(|| (*to).to_owned())
                })
                .unwrap_or_else(|| key.clone())
        };
        let value = match value {
            // MultiEdit's edits carry the same fields.
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|item| renamed(item, false, to_claude))
                    .collect(),
            ),
            other => other.clone(),
        };
        out.insert(key, value);
    }
    Value::Object(out)
}

/// OpenCode's arguments for `tool` as Claude Code's tool input.
#[must_use]
pub fn args_to_claude(tool: &str, args: &Value) -> Value {
    renamed(args, matches!(tool, "patch" | "apply_patch"), true)
}

/// A rewritten Claude Code tool input back as OpenCode's arguments.
#[must_use]
pub fn args_from_claude(tool: &str, input: &Value) -> Value {
    renamed(input, matches!(tool, "patch" | "apply_patch"), false)
}

/// The plugin's call (`{tool, sessionID, callID, args, cwd}`) as Claude
/// Code's `PreToolUse` hook input.
#[must_use]
pub fn to_claude(call: &Value) -> Value {
    let tool = call["tool"].as_str().unwrap_or_default();
    json!({
        "session_id": call["sessionID"],
        "cwd": call["cwd"],
        "permission_mode": "bypassPermissions",
        "hook_event_name": "PreToolUse",
        "tool_name": tool_name(tool),
        "tool_input": args_to_claude(tool, &call["args"]),
        "tool_use_id": call["callID"],
    })
}

/// The bridge's reply to the plugin from Claude Code's hook output, already
/// narrowed so an ask is held: `{"deny": reason}`, `{"args": ...}` for a
/// rewrite, or `None` to let the call run unchanged.
#[must_use]
pub fn reply(call: &Value, output: Option<&Value>) -> Option<Value> {
    let specific = &output?["hookSpecificOutput"];
    match specific["permissionDecision"].as_str() {
        Some("deny") => Some(json!({
            "deny": specific["permissionDecisionReason"]
                .as_str()
                .unwrap_or("A Coder check refused this step."),
        })),
        _ => specific.get("updatedInput").map(|input| {
            json!({"args": args_from_claude(call["tool"].as_str().unwrap_or_default(), input)})
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opencode_calls_read_as_claude_code_tools() {
        let call = json!({
            "tool": "edit", "sessionID": "ses_1", "callID": "c1", "cwd": "/work",
            "args": {"filePath": "/work/a.txt", "oldString": "a", "newString": "b"},
        });
        let input = to_claude(&call);
        assert_eq!(input["tool_name"], "Edit");
        assert_eq!(input["hook_event_name"], "PreToolUse");
        assert_eq!(
            input["tool_input"],
            json!({"file_path": "/work/a.txt", "old_string": "a", "new_string": "b"})
        );
        assert_eq!(tool_name("bash"), "Bash");
        assert_eq!(tool_name("coder_agent"), "mcp__coder__agent");
        assert_eq!(tool_name("context7_search"), "context7_search");
        let patch = args_to_claude("apply_patch", &json!({"patchText": "*** Begin Patch"}));
        assert_eq!(patch, json!({"input": "*** Begin Patch"}));
        assert_eq!(
            args_from_claude("apply_patch", &patch),
            json!({"patchText": "*** Begin Patch"})
        );
        let multi = args_to_claude(
            "multiedit",
            &json!({"filePath": "x", "edits": [{"oldString": "a", "newString": "b"}]}),
        );
        assert_eq!(multi["edits"][0]["old_string"], "a");
    }

    #[test]
    fn a_denial_and_a_rewrite_answer_in_opencodes_terms() {
        let call = json!({"tool": "bash", "args": {"command": "cargo test"}});
        let deny = json!({"hookSpecificOutput": {"hookEventName": "PreToolUse",
            "permissionDecision": "deny", "permissionDecisionReason": "Held for the person."}});
        assert_eq!(
            reply(&call, Some(&deny)),
            Some(json!({"deny": "Held for the person."}))
        );
        let rewrite = json!({"hookSpecificOutput": {"hookEventName": "PreToolUse",
            "permissionDecision": "allow", "updatedInput": {"command": "squeezed cargo test"}}});
        assert_eq!(
            reply(&call, Some(&rewrite)),
            Some(json!({"args": {"command": "squeezed cargo test"}}))
        );
        assert_eq!(reply(&call, None), None);
        assert_eq!(reply(&call, Some(&json!({"systemMessage": "note"}))), None);
    }

    #[test]
    fn the_plugin_fails_closed_and_asks_the_bridge_in_opencodes_format() {
        assert!(PLUGIN_SOURCE.contains("\"coder-hook\", \"--format\", \"opencode\""));
        assert!(PLUGIN_SOURCE.contains(PROGRAM_VAR));
        assert!(PLUGIN_SOURCE.contains("if (text === null) throw"));
        assert!(PLUGIN_SOURCE.contains("tool.execute.before"));
    }
}
