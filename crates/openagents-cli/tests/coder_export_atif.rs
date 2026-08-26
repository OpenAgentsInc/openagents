//! `/export` writes a real ATIF document for a transcript built through the
//! `Entry` constructors the markdown port introduced (commit 2185306d80).
//!
//! This lives in its own test binary because it swaps `HOME` to keep the
//! export out of the developer's `~/.openagents/exports`. `HOME` is
//! process-global, and cargo runs the tests in one binary on threads, so a
//! neighbour reading the environment mid-swap would be a real race. One test
//! per process removes the race rather than papering over it.

use openagents_cli::coder::export::export_trajectory;
use openagents_cli::coder::tui::{Entry, Role, ToolCall, now_ms};

fn delegate_call() -> ToolCall {
    ToolCall {
        call_id: "call-7".to_string(),
        function_name: "delegate".to_string(),
        arguments: serde_json::json!({ "agent": "devin", "task": "Read src/main.rs" }),
        output: None,
        error: None,
    }
}

#[test]
fn export_writes_an_atif_document_for_a_constructor_built_transcript() {
    // Runs the real exporter, but into a scratch HOME so it cannot write to
    // the developer's `~/.openagents/exports`. The clipboard copy is left to
    // fail or succeed on its own; only the file is asserted.
    let scratch = std::env::temp_dir().join(format!("Coder-export-{}", now_ms()));
    std::fs::create_dir_all(&scratch).unwrap();

    let mut entries = vec![
        Entry::new(Role::Notice, "found ACP agents: devin"),
        Entry::new(Role::You, "explain rust"),
    ];
    let mut tool = Entry::tool_call("delegate devin: Read src/main.rs");
    tool.tool = Some(delegate_call());
    tool.output = Some("Reading file...\nDone".to_string());
    entries.push(tool);
    entries.push(Entry::new(
        Role::Assistant,
        "Rust is a systems language.",
    ));
    // `/export` itself is an interface command and must not become a step.
    entries.push(Entry::new(Role::You, "/export"));

    let previous = std::env::var("HOME").ok();
    // SAFETY: single-threaded within this test; restored before returning.
    unsafe { std::env::set_var("HOME", &scratch) };
    let result = export_trajectory(&entries, "coder-auto", "openagents", "main");
    unsafe {
        match previous {
            Some(home) => std::env::set_var("HOME", home),
            None => std::env::remove_var("HOME"),
        }
    }

    let body = std::fs::read_to_string(&result.path)
        .unwrap_or_else(|e| panic!("export wrote nothing to {}: {e}", result.path));
    let document: serde_json::Value = serde_json::from_str(&body).unwrap();

    assert_eq!(document["schema_version"], "ATIF-v1.7");
    let steps = document["steps"].as_array().unwrap();
    assert_eq!(
        steps.len(),
        3,
        "expected user + assistant + tool steps, got {steps:#?}"
    );
    assert_eq!(result.steps, 3);
    assert_eq!(steps[0]["source"], "user");
    assert!(steps[1].get("tool_calls").is_some());
    assert_eq!(steps[2]["message"], "Rust is a systems language.");

    let tool_step = steps
        .iter()
        .find(|s| s.get("tool_calls").is_some())
        .expect("the delegate call did not survive into the export");
    assert_eq!(tool_step["tool_calls"][0]["function_name"], "delegate");
    assert_eq!(tool_step["tool_calls"][0]["tool_call_id"], "call-7");
    assert_eq!(
        tool_step["observation"]["results"][0]["content"],
        "Reading file...\nDone"
    );

    // Timestamps come from `Entry::at`; an unstamped entry would serialize as
    // the epoch and make every exported trajectory look like 1970.
    let stamp = steps[0]["timestamp"].as_str().unwrap();
    assert!(
        !stamp.starts_with("1970-"),
        "export timestamps fell back to the epoch: {stamp}"
    );

    // The notice is kept out of `steps` and recorded alongside them.
    let notices = document["extra"]["notices"].as_array().unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0]["text"], "found ACP agents: devin");

    // The trajectory records the version this binary was *published* as, which
    // is what `openagents_cli::VERSION` carries: `ops/release-cli.sh` threads
    // the published name in through `OPENAGENTS_CLI_RELEASE_VERSION`, and the
    // crate manifest is only the fallback for a build that is not a release.
    //
    // This read `env!("CARGO_PKG_VERSION")` until 2026-08-26, so every trace
    // exported from 0.0.1, 0.0.2 and 0.0.3 claimed to come from `0.1.0` -- the
    // one version that was withdrawn and must never be attributed to anything.
    //
    // Note what this assertion can and cannot do. In a normal `cargo test` the
    // two expressions are the same string, so it passes against the bug as
    // happily as against the fix; it holds the intent and catches someone
    // replacing this with a literal. What actually separates them is a release
    // build, where they differ by construction:
    //
    //     OPENAGENTS_CLI_RELEASE_VERSION=9.9.9 cargo test -p Coder \
    //       --test export_atif
    //
    // That run fails against the old code and passes against this one.
    assert_eq!(
        document["agent"]["version"], openagents_cli::VERSION,
        "the exported trajectory must name the published version, not the crate manifest"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}
