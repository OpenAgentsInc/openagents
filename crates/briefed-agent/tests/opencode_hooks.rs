//! The briefed agent's OpenCode engine on the Coder SDK (#11530): started
//! as the bench starts it (`briefed-agent CONFIG.json`) on a stand-in
//! `opencode` that loads Coder's plugin from the folder it was given and
//! runs its `tool.execute.before` on four calls, as OpenCode does before
//! each tool. The plugin runs this program's `coder-hook`, which asks the
//! run's host: the floors hold a merge, the shared-checkout fence refuses
//! `git stash`, the worktree guard refuses a read outside the worktree,
//! and an edit inside runs.

use std::path::Path;

use serde_json::{Value, json};

const STAND_IN: &str = r#"#!/bin/sh
cat > /dev/null
exec node --input-type=module -e '
import { writeFileSync } from "node:fs";
const { CoderGuard } = await import(process.env.OPENCODE_CONFIG_DIR + "/plugin/coder-guard.js");
const hooks = await CoderGuard({ directory: process.cwd() });
const calls = [
  ["bash", { command: "gh pr merge 5 --squash" }],
  ["bash", { command: "git stash" }],
  ["read", { filePath: "/etc/hosts" }],
  ["edit", { filePath: process.cwd() + "/src/lib.rs", oldString: "a", newString: "b" }],
];
const results = [];
for (const [tool, args] of calls) {
  try {
    await hooks["tool.execute.before"]({ tool, sessionID: "ses_1", callID: "c1" }, { args });
    results.push("ran");
  } catch (error) {
    results.push(error.message);
  }
}
writeFileSync(process.env.REC, JSON.stringify(results));
'
"#;

fn node() -> bool {
    std::process::Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|out| out.status.success())
}

#[cfg(unix)]
#[test]
fn an_opencode_session_meets_the_floors_the_fence_and_the_worktree_guard() {
    if !node() {
        eprintln!("node isn't installed; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let worktree = root.join("tree");
    std::fs::create_dir_all(worktree.join("src")).unwrap();
    let program = root.join("opencode");
    std::fs::write(&program, STAND_IN).unwrap();
    let mut permissions = std::fs::metadata(&program).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    std::fs::set_permissions(&program, permissions).unwrap();
    std::fs::write(root.join("prompt.md"), "Fix it.").unwrap();
    std::fs::write(root.join("system.md"), "The briefing.").unwrap();
    let at = |name: &str| root.join(name).display().to_string();
    let config = json!({
        "engine": "opencode", "opencode_bin": at("opencode"), "worktree": worktree,
        "model": "opencode/big-pickle", "tools": ["Read", "Edit", "Bash"],
        "prompt_path": at("prompt.md"), "system_prompt_path": at("system.md"),
        "events_path": at("events.jsonl"), "summary_path": at("summary.json"),
        "timeout_secs": 60,
    });
    std::fs::write(root.join("agent.json"), config.to_string()).unwrap();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_briefed-agent"))
        .arg(root.join("agent.json"))
        .env("REC", root.join("results.json"))
        .status()
        .unwrap();
    assert!(status.success());
    let results: Vec<String> =
        serde_json::from_str(&std::fs::read_to_string(root.join("results.json")).unwrap())
            .unwrap();
    assert!(results[0].starts_with("Held for the person"), "{results:?}");
    assert!(results[1].contains("git stash"), "{results:?}");
    assert!(results[2].contains("worktree"), "{results:?}");
    assert_eq!(results[3], "ran", "{results:?}");
    let summary: Value =
        serde_json::from_str(&std::fs::read_to_string(Path::new(&at("summary.json"))).unwrap())
            .unwrap();
    assert_eq!(summary["error"], Value::Null, "{summary}");
}
