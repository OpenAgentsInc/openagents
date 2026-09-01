//! Converter tests: Claude Code sessions and Codex rollouts to ATIF.
//!
//! Fixture-driven. The fixtures under `tests/gym-fixtures/` are hand-written
//! from the real record shapes — synthesized, never copied from a live
//! session — and every assertion here runs the same pipeline the corpus
//! importer runs: convert, summarize, qualify, redact, tripwire, digest,
//! ledger, verify.

use openagents_cli::gym::convert::{
    SERVER_TRACE_CAP_BYTES, convert_claude_session, convert_codex_rollout,
};
use openagents_cli::gym::corpus::{
    INVENTORY_BOUNDS, inventory, prepare_import, record_import, verify_ledger,
};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/gym-fixtures")
        .join(name)
}

/// A fixture home with the Claude and Codex fixtures planted in their stores.
fn planted_home(tmp: &Path) -> PathBuf {
    let home = tmp.join("home");
    let claude = home.join(".claude/projects/-Users-fixture-work-openagents");
    let codex = home.join(".codex/sessions/2026/08/21");
    fs::create_dir_all(&claude).unwrap();
    fs::create_dir_all(&codex).unwrap();
    fs::copy(
        fixture("claude-session.jsonl"),
        claude.join("fx-claude-0001.jsonl"),
    )
    .unwrap();
    fs::copy(
        fixture("codex-rollout.jsonl"),
        codex.join("rollout-2026-08-21T09-00-00-fx-codex-0001.jsonl"),
    )
    .unwrap();
    home
}

#[test]
fn claude_session_converts_to_atif_the_server_would_accept() {
    let document = convert_claude_session(&fixture("claude-session.jsonl")).unwrap();

    // The server accepts schema_version starting with "ATIF/1." or "ATIF-v1.".
    assert_eq!(document["schema_version"], "ATIF-v1.7");
    assert_eq!(document["session_id"], "fx-claude-0001");
    assert_eq!(document["agent"]["name"], "claude-code");
    assert_eq!(document["agent"]["model_name"], "claude-fable-5");
    assert_eq!(document["agent"]["version"], "2.0.11");

    let steps = document["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 12, "user turns plus one step per message id");

    // The first step is the user directive.
    assert_eq!(steps[0]["source"], "user");
    assert!(
        steps[0]["message"]
            .as_str()
            .unwrap()
            .contains("github.com/OpenAgentsInc/openagents")
    );

    // A streamed assistant message (thinking, text, tool_use records sharing
    // one message id) is one agent step.
    let merged = &steps[1];
    assert_eq!(merged["source"], "agent");
    assert!(
        merged["reasoning_content"]
            .as_str()
            .unwrap()
            .contains("timing assumption")
    );
    assert!(
        merged["message"]
            .as_str()
            .unwrap()
            .contains("run the suite")
    );
    let calls = merged["tool_calls"].as_array().unwrap();
    assert_eq!(calls[0]["function_name"], "Bash");
    assert_eq!(calls[0]["tool_call_id"], "toolu_0001");

    // The following user record's tool_result attached to the calling step.
    let results = merged["observation"]["results"].as_array().unwrap();
    assert_eq!(results[0]["source_call_id"], "toolu_0001");
    assert_eq!(results[0]["status"], "failed");
    assert!(results[0]["content"].as_str().unwrap().contains("FAILED"));

    // No sidechain record became a step.
    assert!(
        !steps.iter().any(|s| s["message"]
            .as_str()
            .unwrap_or("")
            .contains("subagent traffic")),
        "sidechain records must be skipped"
    );

    // cwd and gitBranch travel in extra; token totals are per unique message.
    assert_eq!(document["extra"]["cwd"], "/Users/fixture/work/openagents");
    assert_eq!(document["extra"]["git_branch"], "main");
    assert_eq!(document["final_metrics"]["total_steps"], 12);
    assert_eq!(document["final_metrics"]["total_prompt_tokens"], 1471);
    assert_eq!(document["final_metrics"]["total_completion_tokens"], 192);
}

#[test]
fn codex_rollout_converts_to_atif_the_server_would_accept() {
    let document = convert_codex_rollout(&fixture("codex-rollout.jsonl")).unwrap();

    assert_eq!(document["schema_version"], "ATIF-v1.7");
    assert_eq!(document["session_id"], "fx-codex-0001");
    assert_eq!(document["agent"]["name"], "codex");
    assert_eq!(document["agent"]["model_name"], "gpt-5.4");
    assert_eq!(document["agent"]["version"], "0.117.0");

    let steps = document["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 13);
    assert_eq!(steps[0]["source"], "user");

    // Developer-role harness scaffolding is skipped, not a user step.
    assert!(!steps.iter().any(|s| {
        s["message"]
            .as_str()
            .unwrap_or("")
            .contains("permissions instructions")
    }));
    assert_eq!(
        document["extra"]["skipped_records"]["message_role_developer"],
        1
    );

    // function_call/function_call_output pair to a tool call plus observation,
    // with the preceding reasoning summary attached.
    let call_step = &steps[1];
    assert_eq!(call_step["tool_calls"][0]["tool_call_id"], "call_fx_0001");
    assert_eq!(call_step["tool_calls"][0]["function_name"], "exec_command");
    assert_eq!(
        call_step["tool_calls"][0]["arguments"]["cmd"], "cat README.md",
        "argument strings are parsed into structured JSON"
    );
    assert!(
        call_step["reasoning_content"]
            .as_str()
            .unwrap()
            .contains("patch the badge")
    );
    assert!(
        call_step["observation"]["results"][0]["content"]
            .as_str()
            .unwrap()
            .contains("fixture readme")
    );

    // A custom tool call carries its input and the patch_apply_end marker.
    let patch_step = steps
        .iter()
        .find(|s| s["tool_calls"][0]["tool_call_id"] == "call_fx_0002")
        .unwrap();
    assert_eq!(patch_step["tool_calls"][0]["function_name"], "apply_patch");
    assert!(
        patch_step["tool_calls"][0]["arguments"]["input"]
            .as_str()
            .unwrap()
            .contains("Begin Patch")
    );
    assert_eq!(patch_step["extra"]["patch_apply"]["success"], true);

    // The last cumulative token_count wins; the compaction marker is noted.
    assert_eq!(document["final_metrics"]["total_prompt_tokens"], 5400);
    assert_eq!(document["final_metrics"]["total_completion_tokens"], 410);
    assert_eq!(
        document["final_metrics"]["extra"]["cached_input_tokens"],
        2100
    );
    assert_eq!(document["extra"]["compactions"], 1);
    assert_eq!(document["extra"]["cwd"], "/Users/fixture/work/openagents");
}

#[test]
fn converted_fixtures_pass_the_qualification_filters() {
    let tmp = tempfile::tempdir().unwrap();
    let home = planted_home(tmp.path());
    let out = tmp.path().join("inventory.json");
    let doc = inventory(&home, &[], &out, INVENTORY_BOUNDS).unwrap();

    for source in ["claude_session", "codex_session"] {
        let row = doc.rows.iter().find(|r| r.source == source).unwrap();
        assert!(
            row.qualifies,
            "{source} fixture must qualify; excluded: {:?}",
            row.excluded_because
        );
        assert!(row.digest.as_ref().unwrap().starts_with("sha256:"));
        assert!(row.steps.unwrap() >= 10);
        assert_eq!(row.repo_hint.as_deref(), Some("OpenAgentsInc/openagents"));
        assert!(row.model.is_some());
    }
}

#[test]
fn a_planted_secret_in_a_converted_session_trips_the_tripwire() {
    let tmp = tempfile::tempdir().unwrap();
    let home = planted_home(tmp.path());
    // Plant an env secret in the Codex fixture copy. Redaction rewrites the
    // value but the `NAME=` marker remains, which is exactly what the
    // tripwire refuses to upload.
    let rollout =
        home.join(".codex/sessions/2026/08/21/rollout-2026-08-21T09-00-00-fx-codex-0001.jsonl");
    let mut text = fs::read_to_string(&rollout).unwrap();
    text.push_str(
        "\n{\"timestamp\":\"2026-08-21T09:00:21.000Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"my env has OPENAGENTS_TOKEN=fx_planted_value_1234 in it\"}]}}\n",
    );
    fs::write(&rollout, text).unwrap();

    let out = tmp.path().join("inventory.json");
    inventory(&home, &[], &out, INVENTORY_BOUNDS).unwrap();
    let ledger = tmp.path().join("corpus.jsonl");
    let error = prepare_import(&out, "ledger", &ledger, "/Users/fixture").unwrap_err();
    assert!(error.to_string().contains("tripwire"), "{error}");
}

#[test]
fn an_oversized_source_is_skipped_before_any_read() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let codex = home.join(".codex/sessions/2026/08/21");
    fs::create_dir_all(&codex).unwrap();
    // A sparse file: stat reports 201 MB without writing them.
    let path = codex.join("rollout-huge.jsonl");
    let file = fs::File::create(&path).unwrap();
    file.set_len(201 * 1024 * 1024).unwrap();
    drop(file);

    let out = tmp.path().join("inventory.json");
    let doc = inventory(&home, &[], &out, INVENTORY_BOUNDS).unwrap();
    let row = doc.rows.iter().find(|r| r.path == path).unwrap();
    assert!(!row.qualifies);
    assert!(
        row.excluded_because
            .as_ref()
            .unwrap()
            .contains(&"oversized_source".to_string()),
        "{:?}",
        row.excluded_because
    );
    assert!(row.digest.is_none(), "an oversized source is never read");
}

#[test]
fn converted_import_is_idempotent_and_verify_detects_an_edited_source() {
    let tmp = tempfile::tempdir().unwrap();
    let home = planted_home(tmp.path());
    let out = tmp.path().join("inventory.json");
    inventory(&home, &[], &out, INVENTORY_BOUNDS).unwrap();
    let ledger = tmp.path().join("corpus.jsonl");

    let (prepared, skipped, _) = prepare_import(&out, "ledger", &ledger, "/Users/fixture").unwrap();
    assert_eq!(prepared.len(), 2, "both converted fixtures import");
    assert_eq!(skipped, 0);
    for (index, row) in prepared.iter().enumerate() {
        record_import(
            &ledger,
            "ledger",
            row,
            format!("00000000-0000-0000-0000-00000000000{index}"),
            row.digest.clone(),
        )
        .unwrap();
    }

    // Re-import skips every already-recorded digest: conversion is
    // deterministic, so the digests come out the same.
    let (again, skipped_again, _) =
        prepare_import(&out, "ledger", &ledger, "/Users/fixture").unwrap();
    assert!(again.is_empty(), "{}", again.len());
    assert_eq!(skipped_again, 2);

    // Untouched sources verify clean.
    let clean = verify_ledger(&ledger, &[]).unwrap();
    assert!(clean.drifts.is_empty(), "{:?}", clean.drifts);
    assert_eq!(clean.verified, 2);

    // An edited source is drift, named per row.
    let rollout =
        home.join(".codex/sessions/2026/08/21/rollout-2026-08-21T09-00-00-fx-codex-0001.jsonl");
    let mut text = fs::read_to_string(&rollout).unwrap();
    text.push_str(
        "\n{\"timestamp\":\"2026-08-21T09:59:00.000Z\",\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"a record added after import\"}]}}\n",
    );
    fs::write(&rollout, text).unwrap();

    let drifted = verify_ledger(&ledger, &[]).unwrap();
    assert_eq!(
        drifted.verified, 1,
        "the untouched Claude row still verifies"
    );
    assert_eq!(drifted.drifts.len(), 1, "{:?}", drifted.drifts);
    assert!(
        drifted.drifts[0].contains("drifted"),
        "{:?}",
        drifted.drifts
    );
}

#[test]
fn a_session_over_the_server_cap_is_truncated_from_the_middle() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("big-session.jsonl");
    let mut lines = String::new();
    let filler = "x".repeat(60_000);
    for index in 0..300 {
        lines.push_str(&format!(
            "{{\"type\":\"user\",\"sessionId\":\"fx-big\",\"timestamp\":\"2026-08-20T10:{:02}:00.000Z\",\"isSidechain\":false,\"message\":{{\"role\":\"user\",\"content\":\"turn {index} {filler}\"}}}}\n",
            index % 60
        ));
    }
    fs::write(&path, lines).unwrap();

    let document = convert_claude_session(&path).unwrap();
    let serialized = serde_json::to_string(&document).unwrap();
    assert!(
        serialized.len() <= SERVER_TRACE_CAP_BYTES,
        "converted document must fit the server cap; got {} bytes",
        serialized.len()
    );

    let truncation = &document["extra"]["truncation"];
    assert!(!truncation.is_null(), "the elision must be marked in extra");
    let removed = truncation["removed_steps"].as_u64().unwrap();
    let kept = truncation["kept_steps"].as_u64().unwrap();
    assert_eq!(removed + kept, 300);

    // Head and tail survive; the cut is in the middle.
    let steps = document["steps"].as_array().unwrap();
    let text_of = |step: &Value| step["message"].as_str().unwrap().to_string();
    assert!(text_of(&steps[0]).starts_with("turn 0 "));
    assert!(text_of(steps.last().unwrap()).starts_with("turn 299 "));
}
