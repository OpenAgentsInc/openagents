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
    let arm = Options::parse(&args("7 --decider kind=haiku-arm,relevance=jev")).unwrap();
    assert_eq!(arm.kind_decider(), Decider::HaikuArm);
    assert_eq!(arm.relevance_decider(), Decider::Jev);
    let subq = Options::parse(&args("7 --decider relevance=subq")).unwrap();
    assert_eq!(subq.relevance_decider(), Decider::Subq);
    assert_eq!(Decider::parse(Decider::Subq.name()), Ok(Decider::Subq));
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
        required: false,
    }]
}

#[test]
fn required_checks_are_rust_compiles_and_required_checks_of_other_kinds() {
    let mut list = checks();
    list.push(Check {
        id: "check:coder-new".into(),
        argv: args("cargo check -p coder-new"),
        filter: false,
        what: "compile".into(),
        required: false,
    });
    list.push(Check {
        id: "tests:scripts".into(),
        argv: args("python3 -m pytest scripts/tests/test_x.py"),
        filter: false,
        what: "run the script's tests".into(),
        required: true,
    });
    let briefing = crate::issue_run::decide::Briefing {
        checks: list,
        ..Default::default()
    };
    assert_eq!(
        crate::issue_run::setup::required_checks(&briefing),
        vec!["check:coder-new".to_owned(), "tests:scripts".to_owned()]
    );
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
    assert_eq!(
        input["command"],
        "cargo test --tests -p coder-new -- issue_run"
    );
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

#[test]
fn a_package_name_is_read_from_its_cargo_toml() {
    use crate::issue_run::setup::package_name;
    assert_eq!(
        package_name(
            "[package]\nname = \"verse-net\"\nversion = \"1.0.0\"\n[dependencies]\nname = \"x\"\n"
        ),
        Some("verse-net".into())
    );
    assert_eq!(package_name("[workspace]\nmembers = []\n"), None);
}

#[test]
fn a_briefed_browser_build_gets_a_wasm_check() {
    use crate::issue_run::{decide::Briefing, setup::briefed_wasm_checks};
    let root = std::env::temp_dir().join(format!("coder-wasm-check-{}", std::process::id()));
    let web = root.join("crates/game-web");
    let native = root.join("crates/game-rules");
    std::fs::create_dir_all(web.join("src")).unwrap();
    std::fs::create_dir_all(native.join("src")).unwrap();
    std::fs::write(
        web.join("Cargo.toml"),
        "[package]\nname = \"game-web\"\n[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nweb-sys = \"0.3\"\n",
    )
    .unwrap();
    std::fs::write(
        native.join("Cargo.toml"),
        "[package]\nname = \"game-rules\"\n",
    )
    .unwrap();
    let briefing = Briefing {
        files: vec![
            "crates/game-web/src/app.rs".into(),
            "crates/game-web/src/lib.rs".into(),
            "crates/game-rules/src/lib.rs".into(),
        ],
        ..Briefing::default()
    };
    let checks = briefed_wasm_checks(&root, &briefing);
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0].id, "wasm:game-web");
    assert!(checks[0].required);
    assert!(checks[0].argv.iter().any(|a| a == "wasm32-unknown-unknown"));
}

#[test]
fn a_docs_change_is_held_to_a_link_check_not_rust_builds() {
    use crate::issue_run::setup::checks_for_paths;
    let root = std::env::temp_dir().join(format!("coder-docs-check-{}", std::process::id()));
    std::fs::create_dir_all(root.join("docs")).unwrap();
    std::fs::create_dir_all(root.join("scripts/coder")).unwrap();
    std::fs::create_dir_all(root.join("crates/a/src")).unwrap();
    std::fs::write(root.join("docs/zones.md"), "[x](zones.md)\n").unwrap();
    std::fs::write(root.join("scripts/coder/check_md_links.py"), "").unwrap();
    std::fs::write(
        root.join("crates/a/Cargo.toml"),
        "[package]\nname = \"a\"\n",
    )
    .unwrap();
    let docs = checks_for_paths(&root, &["docs/zones.md".to_owned()]);
    let both = checks_for_paths(
        &root,
        &["docs/zones.md".to_owned(), "crates/a/src/lib.rs".to_owned()],
    );
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "links:docs");
    assert!(docs[0].required);
    assert_eq!(
        docs[0].argv.last().map(String::as_str),
        Some("docs/zones.md")
    );
    let ids: Vec<&str> = both.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["check:a", "test:a", "links:docs"]);
}

#[test]
fn the_issue_text_carries_its_newest_comments() {
    use crate::issue_run::setup::with_comments;
    let comments: Vec<serde_json::Value> = (0..8)
        .map(|n| serde_json::json!({"author": {"login": "agent"}, "createdAt": format!("2026-10-11T0{n}:00:00Z"), "body": format!("note {n}")}))
        .collect();
    let text = with_comments("The bug.", &comments);
    assert!(text.starts_with("The bug."));
    assert!(text.contains("note 7") && text.contains("note 2"));
    assert!(!text.contains("note 1"), "only the newest six");
    assert_eq!(with_comments("The bug.", &[]), "The bug.");
}

#[test]
fn a_crate_outside_the_root_workspace_is_checked_through_its_manifest() {
    use crate::issue_run::setup::checks_for_paths;
    let root = std::env::temp_dir().join(format!("coder-nested-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["crates/a/src", "crates/phone/src"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/a\"]\nexclude = [\"crates/phone\"]\nresolver = \"2\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("crates/a/Cargo.toml"),
        "[package]\nname = \"a\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(root.join("crates/a/src/lib.rs"), "").unwrap();
    std::fs::write(
        root.join("crates/phone/Cargo.toml"),
        "[package]\nname = \"phone\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[workspace]\n",
    )
    .unwrap();
    std::fs::write(root.join("crates/phone/src/lib.rs"), "").unwrap();
    let checks = checks_for_paths(
        &root,
        &[
            "crates/a/src/lib.rs".to_owned(),
            "crates/phone/src/lib.rs".to_owned(),
        ],
    );
    let _ = std::fs::remove_dir_all(&root);
    let argv = |id: &str| {
        checks
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.argv.join(" "))
            .unwrap_or_default()
    };
    assert!(argv("check:a").contains("-p a"), "{}", argv("check:a"));
    assert!(
        argv("check:phone").contains("--manifest-path crates/phone/Cargo.toml"),
        "{}",
        argv("check:phone")
    );
    assert!(argv("test:phone").starts_with("cargo test --manifest-path crates/phone/Cargo.toml"));
}

#[test]
fn acp_tool_calls_draw_with_coder_widget_names() {
    let root = std::path::Path::new("/work/tree");
    assert_eq!(
        agent::acp_call(
            "read",
            "Read lib.rs",
            &json!({"path": "/work/tree/src/lib.rs"}),
            root
        ),
        ("Read".into(), json!({"path": "src/lib.rs"}))
    );
    assert_eq!(
        agent::acp_call(
            "edit",
            "Edit lib.rs",
            &json!({"file_path": "src/lib.rs"}),
            root
        ),
        ("Edit".into(), json!({"path": "src/lib.rs"}))
    );
    assert_eq!(
        agent::acp_call("create", "Write notes", &json!({"path": "notes.md"}), root),
        ("Write".into(), json!({"path": "notes.md"}))
    );
    let (name, input) = agent::acp_call(
        "search",
        "find tests",
        &json!({"query": "Solver::drive"}),
        root,
    );
    assert_eq!(name, "Grep");
    assert_eq!(input["pattern"], "Solver::drive");
    assert_eq!(
        agent::acp_call(
            "execute",
            "Ran cargo test",
            &json!({"command": "cargo test -p x"}),
            root
        ),
        ("Run".into(), json!({"command": "cargo test -p x"}))
    );
    // A kind without a file or command shape draws a Run of its title.
    assert_eq!(
        agent::acp_call("think", "Planning the change", &json!({}), root),
        ("Run".into(), json!({"command": "Planning the change"}))
    );
    // A path outside the worktree is shown as named, not remapped.
    assert_eq!(
        agent::acp_call("read", "Read x", &json!({"path": "/etc/passwd"}), root),
        ("Read".into(), json!({"path": "/etc/passwd"}))
    );
}

#[test]
fn acp_tool_results_feed_the_widgets() {
    assert_eq!(
        agent::acp_output("Run", &json!({"command": "ls"}), "a\nb", "completed"),
        json!({"output": "a\nb"})
    );
    assert_eq!(
        agent::acp_output("Read", &json!({"path": "x.rs"}), "l1\nl2\nl3", "completed")["total_lines"],
        json!(3)
    );
    assert_eq!(
        agent::acp_output("Grep", &json!({}), "a.rs:1:hit\nb.rs:2:hit", "completed")["count"],
        json!(2)
    );
    assert_eq!(
        agent::acp_output("Run", &json!({}), "", "failed"),
        json!({"error": "The tool failed."})
    );
    assert_eq!(
        agent::acp_output("Run", &json!({}), "boom", "failed"),
        json!({"error": "boom"})
    );
    assert_eq!(
        agent::acp_output("Run", &json!({}), "half", "cancelled"),
        json!({"error": "The agent stopped before this tool returned."})
    );
}

#[test]
fn the_boundary_answerer_holds_writes_to_the_worktree() {
    use acp_client::{PermissionAnswer, PermissionRequest};
    let root = std::path::Path::new("/work/tree");
    let request = |tool: serde_json::Value| {
        serde_json::from_value::<PermissionRequest>(json!({
            "toolCall": tool,
            "options": [
                {"optionId": "yes", "kind": "allow_once"},
                {"optionId": "no", "kind": "reject_once"}
            ]
        }))
        .unwrap()
    };
    // A write inside the worktree runs once.
    let (answer, why) = agent::boundary_answer(
        &request(json!({"kind": "edit", "rawInput": {"file_path": "/work/tree/src/x.rs"}})),
        root,
        true,
    );
    assert_eq!(answer, PermissionAnswer::Selected("yes".into()));
    assert_eq!(why, None);
    // A relative write inside resolves against the worktree.
    let (answer, _) = agent::boundary_answer(
        &request(json!({"kind": "edit", "rawInput": {"path": "src/x.rs"}})),
        root,
        true,
    );
    assert_eq!(answer, PermissionAnswer::Selected("yes".into()));
    // A write outside is refused, absolute or relative.
    for tool in [
        json!({"kind": "edit", "rawInput": {"file_path": "/etc/passwd"}}),
        json!({"kind": "edit", "rawInput": {"path": "../x.rs"}}),
        json!({"kind": "delete", "locations": [{"path": "/other/x.rs"}]}),
    ] {
        let (answer, why) = agent::boundary_answer(&request(tool), root, true);
        assert_eq!(answer, PermissionAnswer::Selected("no".into()));
        assert!(why.is_some());
    }
    // A hand that may not write refuses even inside the worktree.
    let (answer, why) = agent::boundary_answer(
        &request(json!({"kind": "edit", "rawInput": {"file_path": "/work/tree/x.rs"}})),
        root,
        false,
    );
    assert_eq!(answer, PermissionAnswer::Selected("no".into()));
    assert!(why.is_some());
    // A command carries no written paths; it is allowed once.
    let (answer, why) = agent::boundary_answer(
        &request(json!({"kind": "execute", "rawInput": {"command": "cargo test"}})),
        root,
        false,
    );
    assert_eq!(answer, PermissionAnswer::Selected("yes".into()));
    assert_eq!(why, None);
    // With no options to pick, the ask is cancelled rather than allowed.
    let bare = serde_json::from_value::<PermissionRequest>(json!({
        "toolCall": {"kind": "read"},
        "options": []
    }))
    .unwrap();
    let (answer, _) = agent::boundary_answer(&bare, root, false);
    assert_eq!(answer, PermissionAnswer::Cancelled);
}
