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
