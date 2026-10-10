use serde_json::json;

use super::*;
use crate::issue_run::{agent, cards, decide::Check};

fn args(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

#[test]
fn options_read_the_issue_and_flags() {
    let options =
        Options::parse(&args("11175 --decider clef --files 4 --plain --open-pr")).unwrap();
    assert_eq!(options.issue, 11175);
    assert_eq!(options.decider, Decider::Clef);
    assert_eq!(options.files, 4);
    assert!(options.plain && options.open_pr);
    assert_eq!(Options::parse(&args("#42")).unwrap().issue, 42);
    assert!(Options::parse(&args("--plain")).is_err());
    assert!(Options::parse(&args("--replay run.jsonl")).is_ok());
    assert!(Options::parse(&args("7 --decider maybe")).is_err());
}

#[test]
fn events_survive_a_recording() {
    let events = [
        Event::Set {
            index: 0,
            entry: Entry::User("Play issue #1".into()),
        },
        Event::Set {
            index: 1,
            entry: Entry::Tool {
                name: DECISION.into(),
                input: json!({"kind": "stage", "title": "Stage sym"}),
                output: json!({"ok": true}),
                running: false,
            },
        },
        Event::Set {
            index: 2,
            entry: Entry::Assistant {
                text: "Done.".into(),
                model: Some("claude-opus-5-5".into()),
                elapsed_ms: Some(12),
            },
        },
        Event::Busy(false),
        Event::Notice("Finished".into()),
    ];
    for event in events {
        assert_eq!(event_from_json(&event_to_json(&event)), Some(event));
    }
}

#[test]
fn file_tools_stay_inside_the_worktree() {
    let root = std::path::Path::new("/work/tree");
    assert_eq!(
        agent::inside(root, &json!({"file_path": "/work/tree/src/lib.rs"})),
        Some(Some("src/lib.rs".into()))
    );
    assert_eq!(
        agent::inside(root, &json!({"path": "crates/a/../b.rs"})),
        Some(Some("crates/b.rs".into()))
    );
    assert_eq!(
        agent::inside(root, &json!({"file_path": "/etc/passwd"})),
        None
    );
    assert_eq!(
        agent::inside(root, &json!({"file_path": "../outside.rs"})),
        None
    );
    assert_eq!(agent::inside(root, &json!({"pattern": "x"})), Some(None));
}

fn checks() -> Vec<Check> {
    vec![Check {
        id: "test:coder-new".into(),
        argv: args("cargo test -p coder-new"),
        filter: true,
        what: "run the tests".into(),
    }]
}

#[test]
fn sdk_tool_calls_become_coder_tool_items() {
    let root = std::path::Path::new("/work/tree");
    let (name, input) = agent::coder_call(
        "Edit",
        &json!({"file_path": "/work/tree/src/a.rs", "old_string": "a", "new_string": "b"}),
        root,
        &checks(),
    );
    assert_eq!(
        (name.as_str(), &input),
        ("Edit", &json!({"path": "src/a.rs"}))
    );
    let (name, input) = agent::coder_call(
        "mcp__checks__run_check",
        &json!({"check": "test:coder-new", "filter": "issue_run $(touch) x;y"}),
        root,
        &checks(),
    );
    assert_eq!(name, "Run");
    // Filter words that are not test names are dropped.
    assert_eq!(input["command"], "cargo test -p coder-new -- issue_run");
}

#[test]
fn an_edit_result_carries_the_cli_patch_as_a_diff() {
    let structured = json!({"structuredPatch": [{
        "oldStart": 10, "oldLines": 2, "newStart": 10, "newLines": 2,
        "lines": [" keep", "-old", "+new"]
    }]});
    let output = agent::coder_output(
        "Edit",
        &json!({}),
        &json!({"path": "src/a.rs"}),
        "The file was updated.",
        Some(&structured),
        false,
    );
    assert_eq!(output["diff"], "@@ -10,2 +10,2 @@\n keep\n-old\n+new\n");
    assert_eq!(
        (output["added"].clone(), output["removed"].clone()),
        (json!(1), json!(1))
    );
    // Without the CLI's patch, the edit's own strings make the diff.
    let fallback = agent::coder_output(
        "Edit",
        &json!({"old_string": "a\nb", "new_string": "a\nc"}),
        &json!({"path": "src/a.rs"}),
        "",
        None,
        false,
    );
    assert_eq!(fallback["added"], 1);
    let failed = agent::coder_output(
        "Read",
        &json!({}),
        &json!({"path": "x"}),
        "<tool_use_error>File does not exist.</tool_use_error>",
        None,
        true,
    );
    assert_eq!(failed["error"], "File does not exist.");
}

#[test]
fn grep_glob_and_checks_read_like_coder_tools() {
    let grep = agent::coder_output(
        "Grep",
        &json!({}),
        &json!({}),
        "",
        Some(&json!({"mode": "files_with_matches", "numFiles": 2, "filenames": ["a.rs", "b.rs"]})),
        false,
    );
    assert_eq!(
        (grep["count"].clone(), grep["files"].clone()),
        (json!(2), json!(2))
    );
    let content = agent::coder_output(
        "Grep",
        &json!({}),
        &json!({}),
        "",
        Some(
            &json!({"mode": "content", "numFiles": 0, "filenames": [], "content": "src/a.rs:1:x\nsrc/a.rs:2:y\nsrc/b.rs:3:z", "numLines": 3}),
        ),
        false,
    );
    assert_eq!(
        (content["count"].clone(), content["files"].clone()),
        (json!(3), json!(2))
    );
    let none = agent::coder_output(
        "Grep",
        &json!({}),
        &json!({}),
        "No files found",
        Some(&json!({"mode": "files_with_matches", "numFiles": 0, "filenames": []})),
        false,
    );
    assert_eq!(none["count"], 0);
    let glob = agent::coder_output("Glob", &json!({}), &json!({}), "a.rs\nb.rs", None, false);
    assert_eq!(glob["count"], 2);
    let passed = agent::coder_output(
        "Run",
        &json!({}),
        &json!({}),
        "cargo check: passed\nok",
        None,
        false,
    );
    assert!(passed.get("error").is_none());
    let failed = agent::coder_output(
        "Run",
        &json!({}),
        &json!({}),
        "cargo check: FAILED\nerror[E0425]: x",
        None,
        true,
    );
    assert_eq!(failed["output"], "error[E0425]: x");
    assert!(failed.get("error").is_some());
}

#[test]
fn check_output_keeps_errors_and_totals() {
    let text =
        "Compiling a\nerror[E0425]: cannot find value `x`\n --> src/lib.rs:1:1\n\nwarning: other\n";
    assert!(agent::trimmed(text, false).starts_with("error[E0425]"));
    assert!(!agent::trimmed(text, false).contains("Compiling"));
    let ok = "running 3 tests\ntest result: ok. 3 passed; 0 failed\n";
    assert_eq!(
        agent::trimmed(ok, true),
        "ok\ntest result: ok. 3 passed; 0 failed"
    );
}

fn text(lines: Vec<ratatui::text::Line<'static>>) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_model_card_shows_question_options_answer_door_latency_and_cost() {
    let card = json!({
        "kind": "model",
        "title": "Issue kind",
        "question": "What kind of change does this issue ask for?",
        "door": "Jev, TypeSafe API",
        "model": "jev-latest",
        "ms": 412,
        "cost_usd": 0.0003,
        "options": [
            {"name": "bug", "p": 0.7, "chosen": true},
            {"name": "feature", "p": 0.3, "chosen": false}
        ],
        "chosen": "bug",
    });
    let shown = text(cards::decision_lines(
        &card,
        &json!({"ok": true}),
        false,
        120,
        0,
    ));
    for part in [
        "◇ Model Issue kind",
        "Jev, TypeSafe API",
        "jev-latest",
        "412 ms",
        "$0.0003",
        "? What kind of change",
        "███████░░░ 0.70  ← answer",
        "→ bug",
    ] {
        assert!(shown.contains(part), "missing {part:?} in\n{shown}");
    }
}

#[test]
fn a_stage_card_lists_candidates_with_reasons() {
    let card = json!({
        "kind": "stage",
        "title": "Stage sym",
        "detail": "Paths and names the issue names",
        "count": 9,
        "more": 3,
        "ms": 38,
        "rows": [{"label": "crates/coder-new/src/lib.rs", "text": "crate `coder-new` named in the issue"}],
    });
    let shown = text(cards::decision_lines(
        &card,
        &json!({"ok": true}),
        false,
        120,
        0,
    ));
    assert!(
        shown.contains("◇ Stage Stage sym · 9 files · 38 ms"),
        "{shown}"
    );
    assert!(
        shown.contains("crates/coder-new/src/lib.rs  crate `coder-new` named"),
        "{shown}"
    );
    assert!(shown.contains("… 3 more"));
}

#[test]
fn the_summary_card_shows_cost_misses_checks_and_the_diff() {
    let summary = json!({
        "issue": 7, "title": "Fix the thing", "wall_ms": 125_000, "agent_ms": 90_000, "turns": 12,
        "input_tokens": 1000, "output_tokens": 2000, "cache_read_tokens": 50_000, "cache_write_tokens": 4000,
        "agent_usd": 1.25, "decision_usd": 0.002,
        "opened_outside_briefing": ["src/other.rs"],
        "checks": [{"id": "check:a", "ok": true}],
        "agent_checks": [{"id": "test:a", "ok": false}],
        "changed": ["src/a.rs"], "added": 1, "removed": 1,
        "fix": {"commit": "abc", "files": ["src/a.rs", "src/b.rs"], "both": ["src/a.rs"], "missed": ["src/b.rs"], "briefed": ["src/a.rs"]},
        "folder": "/tmp/run",
    });
    let diff = "diff --git a/src/a.rs b/src/a.rs\nindex 1..2 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,1 +1,1 @@\n-old\n+new\n";
    let shown = text(cards::summary_lines(&summary, &json!({"diff": diff}), 120));
    for part in [
        "■ Summary #7 Fix the thing",
        "2m 05s in all",
        "$1.25",
        "1 not in the briefing: src/other.rs",
        "check:a passed",
        "test:a failed",
        "changed 2 files; the agent changed 1 of them",
        "The real fix also changed src/b.rs",
        "src/a.rs",
        "new",
    ] {
        assert!(shown.contains(part), "missing {part:?} in\n{shown}");
    }
}

#[test]
fn diffs_split_per_file() {
    let diff = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/y b/y\nBinary files differ\n";
    assert_eq!(
        cards::split_diff(diff),
        vec![("x".to_owned(), "@@ -1 +1 @@\n-a\n+b\n".to_owned())]
    );
}

// ---- #11230: delivery gate, own worktree per run, unknown cost ----

use crate::issue_run::{
    setup,
    verdict::{self, Outcome, Status},
};

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

fn verdicts(list: &[(&str, bool)]) -> Vec<(String, bool)> {
    list.iter()
        .map(|(id, ok)| ((*id).to_owned(), *ok))
        .collect()
}

#[test]
fn only_a_clean_run_with_every_required_check_passing_delivers() {
    let required = ids(&["check:a", "check:b"]);
    let pass = verdict::judge(
        false,
        None,
        &required,
        &verdicts(&[("check:a", true), ("check:b", true)]),
    );
    assert_eq!(pass.status, Status::Passed);
    assert!(pass.delivers());

    let failed_check = verdict::judge(
        false,
        None,
        &required,
        &verdicts(&[("check:a", true), ("check:b", false)]),
    );
    assert_eq!(failed_check.status, Status::Failed);
    assert!(!failed_check.delivers());
    assert!(failed_check.reason.unwrap().contains("check:b failed"));

    let missing = verdict::judge(false, None, &required, &verdicts(&[("check:a", true)]));
    assert_eq!(missing.status, Status::Failed);
    assert!(missing.reason.unwrap().contains("check:b has no result"));

    let agent_error = verdict::judge(
        false,
        Some("The agent ran out of its time."),
        &required,
        &verdicts(&[("check:a", true), ("check:b", true)]),
    );
    assert_eq!(agent_error.status, Status::Failed);
    assert!(!agent_error.delivers());

    let unchecked = verdict::judge(false, None, &[], &[]);
    assert_eq!(unchecked.status, Status::Unchecked);
    assert!(!unchecked.delivers());

    let cancelled = verdict::judge(
        true,
        None,
        &required,
        &verdicts(&[("check:a", true), ("check:b", true)]),
    );
    assert_eq!(cancelled.status, Status::Cancelled);
    assert!(!cancelled.delivers());
}

#[test]
fn optional_checks_never_decide_the_outcome() {
    // A failing agent-only test check beside passing required checks.
    let outcome = verdict::judge(
        false,
        None,
        &ids(&["check:a"]),
        &verdicts(&[("check:a", true), ("test:a", false)]),
    );
    assert!(outcome.delivers());
}

fn test_transcript() -> (Transcript, mpsc::Receiver<Event>) {
    let (sender, receiver) = mpsc::channel();
    (
        Transcript {
            entries: Vec::new(),
            sender,
            log: None,
            started: Instant::now(),
            cancel: Arc::new(AtomicBool::new(false)),
        },
        receiver,
    )
}

#[test]
fn open_pr_refuses_a_run_that_did_not_pass() {
    let options = Options::parse(&args("7 --open-pr")).unwrap();
    let base = setup::Base {
        issue: 7,
        title: "t".into(),
        body: String::new(),
        closed: false,
        fix: None,
        base: "HEAD".into(),
        repo: "/nonexistent".into(),
        worktree: "/nonexistent/worktree".into(),
        fetch_error: None,
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for outcome in [
        Outcome::stopped(Status::Failed, "The required check check:a failed."),
        Outcome::stopped(Status::Failed, "The agent ended on an error: boom"),
        Outcome::stopped(Status::Unchecked, "no required check"),
        Outcome::stopped(Status::Cancelled, "stopped"),
    ] {
        let (mut transcript, events) = test_transcript();
        runtime.block_on(setup::open_pr(&options, &base, &outcome, &mut transcript));
        let events: Vec<Event> = events.try_iter().collect();
        assert!(
            events
                .iter()
                .all(|event| !matches!(event, Event::Set { .. })),
            "no git or gh step may run: {events:?}"
        );
        assert!(events.iter().any(
            |event| matches!(event, Event::Notice(text) if text.starts_with("No pull request"))
        ));
    }
}

#[test]
fn cost_keeps_unknown_components_and_has_no_total_without_them() {
    let partial = verdict::cost(
        verdict::component(None, "Claude Code reported no cost"),
        verdict::component(Some(0.002), ""),
    );
    assert!(partial["total_usd"].is_null());
    assert_eq!(partial["complete"], json!(false));
    assert_eq!(partial["known_subtotal_usd"], json!(0.002));
    assert!(partial["components"]["agent"]["usd"].is_null());
    assert_eq!(
        partial["components"]["agent"]["unknown_reason"],
        json!("Claude Code reported no cost")
    );
    let full = verdict::cost(
        verdict::component(Some(1.0), ""),
        verdict::component(Some(0.5), ""),
    );
    assert_eq!(full["total_usd"], json!(1.5));
    assert_eq!(full["complete"], json!(true));
}

#[test]
fn the_summary_card_never_shows_an_unknown_cost_as_zero() {
    let summary = json!({"agent_usd": null, "decision_usd": 0.002});
    let shown = cards::cost_text(&summary);
    assert!(shown.starts_with("unknown"), "{shown}");
    assert!(shown.contains("the agent unknown"), "{shown}");
    assert!(!shown.contains("$0 "), "{shown}");
    let summary = json!({
        "issue": 7, "title": "t",
        "outcome": {"status": "failed", "reason": "The required check check:a failed.", "delivers": false},
        "cost": verdict::cost(verdict::component(None, "no cost"), verdict::component(Some(0.0), "")),
        "optional_checks": [{"id": "test:a", "ok": null}],
    });
    let shown = text(cards::summary_lines(&summary, &json!({"diff": ""}), 160));
    for part in [
        "failed · not delivered: The required check check:a failed.",
        "unknown (known part $0)",
        "tokens    unknown",
        "test:a not run",
    ] {
        assert!(shown.contains(part), "missing {part:?} in\n{shown}");
    }
}

#[test]
fn run_ids_are_unique_and_runs_get_their_own_folders() {
    let a = run_id(7, 100);
    let b = run_id(7, 100);
    assert_ne!(a, b);
    assert!(a.starts_with("7-100-"));
    let state = tempfile::tempdir().unwrap();
    let mut options = Options::parse(&args("7")).unwrap();
    options.state = state.path().to_owned();
    let first = new_run(&options).unwrap();
    let second = new_run(&options).unwrap();
    assert_ne!(first.folder, second.folder);
    assert_eq!(first.attempt["index"], json!(1));
    assert_eq!(second.attempt["index"], json!(2));
    assert_eq!(second.attempt["prior_runs"], json!([first.id]));
    assert!(second.folder.join("run.json").exists());
}

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

/// A finished run folder with a worktree, its patch, and (when `captured`)
/// the trace marker.
fn finished_run(repo: &std::path::Path, folder: &std::path::Path, captured: bool) {
    std::fs::create_dir_all(folder).unwrap();
    let worktree = folder.join("worktree");
    git(
        repo,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            worktree.to_str().unwrap(),
            "HEAD",
        ],
    );
    std::fs::write(worktree.join("work.txt"), "uncommitted\n").unwrap();
    let patch = b"diff --git a/work.txt b/work.txt\n";
    std::fs::write(folder.join("change.patch"), patch).unwrap();
    let digest = setup::digest(patch);
    std::fs::write(
        folder.join("summary.json"),
        json!({"finished": true, "diff_sha256": digest, "repo": repo.display().to_string()})
            .to_string(),
    )
    .unwrap();
    if captured {
        std::fs::write(
            folder.join(setup::CAPTURED),
            json!({"diff_digest": digest}).to_string(),
        )
        .unwrap();
    }
}

#[test]
fn a_new_run_cleans_only_finished_and_captured_worktrees() {
    let root = tempfile::tempdir().unwrap();
    let repo = root.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["config", "user.email", "t@example.com"]);
    git(&repo, &["config", "user.name", "t"]);
    std::fs::write(repo.join("a.txt"), "a\n").unwrap();
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    let state = root.path().join("state");
    let done = state.join("7-1-done");
    let uncaptured = state.join("7-2-uncaptured");
    let active = state.join("7-3-active");
    finished_run(&repo, &done, true);
    finished_run(&repo, &uncaptured, false);
    // An active run: a worktree and run.json, no summary yet.
    std::fs::create_dir_all(&active).unwrap();
    git(
        &repo,
        &[
            "worktree",
            "add",
            "-q",
            "--detach",
            active.join("worktree").to_str().unwrap(),
            "HEAD",
        ],
    );
    std::fs::write(active.join("worktree/editing.txt"), "in progress\n").unwrap();
    let current = state.join("7-4-current");
    std::fs::create_dir_all(&current).unwrap();

    assert!(setup::cleanable(&done).is_ok());
    assert!(setup::cleanable(&uncaptured).is_err());
    assert!(setup::cleanable(&active).is_err());
    // A patch that is not the summarized diff keeps the worktree.
    let tampered = state.join("7-5-tampered");
    finished_run(&repo, &tampered, true);
    std::fs::write(tampered.join("change.patch"), b"other").unwrap();
    assert!(setup::cleanable(&tampered).is_err());

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let removed = runtime.block_on(setup::sweep(&state, &repo, &current));
    assert_eq!(removed, 1);
    assert!(!done.join("worktree").exists());
    assert!(done.join("summary.json").exists(), "the run's record stays");
    assert!(uncaptured.join("worktree/work.txt").exists());
    assert!(active.join("worktree/editing.txt").exists());
    assert!(tampered.join("worktree/work.txt").exists());
}
