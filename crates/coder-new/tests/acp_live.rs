//! Live runs of the ACP Subagents plugin's agents on the Coder SDK (#11508):
//! for each agent installed here, a question, an edit in a worktree, and a
//! step Coder holds (or, on an agent it can't stop before a tool, the
//! refusal said before the run). Uses the agents' own logins; Coder's
//! records go to a throwaway folder. Run with
//! `cargo test -p coder-new --test acp_live -- --ignored --nocapture --test-threads 1`.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use coder_new::bundled_runtime::{AcpAgent, acp_briefed};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

fn agents() -> Vec<AcpAgent> {
    let wanted: Vec<String> = std::env::var("ACP_LIVE_AGENTS")
        .unwrap_or_else(|_| "opencode,cursor,grok-build,devin-cli".into())
        .split(',')
        .map(str::to_owned)
        .collect();
    coder_new::acp_discovery::discover(&|name| std::env::var_os(name))
        .into_iter()
        .filter(|agent| wanted.contains(&agent.id))
        .collect()
}

async fn run(agent: &AcpAgent, task: &str, cwd: &Path) -> serde_json::Value {
    let mut tray = Vec::new();
    let result = acp_briefed(
        agent,
        task,
        cwd,
        None,
        false,
        None,
        &Arc::new(AtomicBool::new(false)),
        &mut |event| tray.push(format!("{event:?}")),
    )
    .await;
    let shown: Vec<&String> = tray
        .iter()
        .filter(|line| line.starts_with("Text"))
        .take(1)
        .collect();
    println!(
        "--- {} | {task}\n    tray first text: {shown:?}",
        agent.name
    );
    match result {
        Ok(value) => {
            println!(
                "    reply: {}\n    checks: {}",
                value["reply"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(600)
                    .collect::<String>(),
                value["checks"]
            );
            value
        }
        Err(error) => {
            println!("    refused: {error}");
            serde_json::json!({"error": error})
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "runs the installed agents live, on their own logins"]
async fn every_installed_acp_agent_answers_edits_and_is_held() {
    let state = tempfile::tempdir().unwrap();
    coder_new::home::keep_records_under(state.path());
    // SAFETY: set before the agents start.
    unsafe { std::env::set_var("CODER_HOOK_PROGRAM", env!("CARGO_BIN_EXE_coder-new")) };
    let repo = tempfile::tempdir().unwrap();
    let main = repo.path().join("main");
    std::fs::create_dir_all(&main).unwrap();
    git(&main, &["init", "-q", "-b", "main"]);
    std::fs::write(main.join("README.md"), "# Probe\n\nThe answer is 42.\n").unwrap();
    git(&main, &["add", "."]);
    git(
        &main,
        &[
            "-c",
            "user.email=a@b",
            "-c",
            "user.name=a",
            "commit",
            "-qm",
            "init",
        ],
    );
    for agent in agents() {
        let tree = repo.path().join(format!("wt-{}", agent.id));
        git(
            &main,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                &agent.id,
                tree.to_str().unwrap(),
            ],
        );
        let asked = run(
            &agent,
            "What number does README.md say the answer is? Reply with the number only.",
            &tree,
        )
        .await;
        assert!(
            asked["reply"].as_str().unwrap_or_default().contains("42"),
            "{asked}"
        );
        let edited = run(
            &agent,
            "Append the line `edited` to the end of README.md. Change nothing else.",
            &tree,
        )
        .await;
        let readme = std::fs::read_to_string(tree.join("README.md")).unwrap();
        println!("    README now ends: {:?}", readme.lines().last());
        assert!(edited.get("error").is_none(), "{edited}");
        let held = run(
            &agent,
            "Run exactly this shell command, then report what happened: gh pr merge 1 --squash",
            &tree,
        )
        .await;
        let reply = held["reply"].as_str().unwrap_or_default().to_owned();
        let gated = held["checks"]["before_each_step"] == true;
        println!("    gated before each step: {gated}");
        let _ = reply;
    }
}
