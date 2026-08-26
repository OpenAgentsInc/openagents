//! The optimizable text this crate declares, pinned byte for byte.
//!
//! The system prompt, the lane notices, and the built-in tool descriptions are
//! staged artifacts (`surfaces/coder/`, OpenAgentsInc/openagents#122): a
//! change to any of them is meant to be a diff over an artifact with a
//! measured delta behind it, never a quiet edit in passing. This test is the
//! second half of that: it composes each surface from fixed inputs and
//! compares the result with `coder-surfaces-golden.json`, so moving the text
//! out of these files and into the artifact had to be a move and nothing else.
//!
//! Re-pin deliberately, with `UPDATE_CODER_SURFACES_GOLDEN=1 cargo test -p
//! openagents-cli --test coder_surfaces_golden`, and only when the change to
//! the text is the change you meant to make.

use std::collections::BTreeMap;
use std::path::PathBuf;

use openagents_cli::delegate::ChildOptions;
use openagents_cli::runtime::{CoderRuntimeSession, Lane};
use openagents_cli::tools::{DelegationGate, HarnessToolRegistry, ToolDefinition};

/// A directory that does not exist, so no plugin catalog is discovered and
/// the composed text depends on nothing outside this file.
const FIXED_CWD: &str = "/fixed/cwd";

/// Skills are discovered from `$HOME/.agents/skills` as well as the workspace,
/// so the machine running this test would otherwise decide what the `skill`
/// declaration says. Point `HOME` at an empty directory first: the surface
/// under test is the sentence around the catalog, not the catalog.
fn without_local_skills() -> tempfile::TempDir {
    let empty = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("HOME", empty.path()) };
    empty
}

fn registry() -> HarnessToolRegistry {
    HarnessToolRegistry::with_delegation(
        Some(PathBuf::from(FIXED_CWD)),
        DelegationGate {
            lane: "the parent lane".to_string(),
            user_token: None,
            max_count: 8,
            child: ChildOptions::default(),
        },
    )
}

fn session(lane: Lane) -> CoderRuntimeSession {
    CoderRuntimeSession::new(
        lane,
        Some("https://example.invalid/api/v1".to_string()),
        None,
        HarnessToolRegistry::new(Some(PathBuf::from(FIXED_CWD))),
    )
}

fn declared(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("A declared {name}."),
        parameters: serde_json::json!({"type": "object"}),
    }
}

fn captured() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();

    let two = [declared("alpha"), declared("beta")];
    out.insert(
        "system_prompt.local.no_tools".to_string(),
        session(Lane::Local(String::new())).build_system_prompt(&[]),
    );
    out.insert(
        "system_prompt.thread.no_tools".to_string(),
        session(Lane::default()).build_system_prompt(&[]),
    );
    out.insert(
        "system_prompt.thread.two_tools".to_string(),
        session(Lane::default()).build_system_prompt(&two),
    );

    for tool in registry().list_tools() {
        out.insert(format!("tool.{}", tool.name), tool.description);
    }
    out
}

#[test]
fn the_declared_text_matches_the_pinned_golden() {
    let _empty_home = without_local_skills();
    let golden_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("coder-surfaces-golden.json");
    let captured = captured();

    if std::env::var("UPDATE_CODER_SURFACES_GOLDEN").is_ok() {
        std::fs::write(
            &golden_path,
            format!("{}\n", serde_json::to_string_pretty(&captured).unwrap()),
        )
        .unwrap();
        return;
    }

    let raw = std::fs::read_to_string(&golden_path).expect("the golden file is checked in");
    let expected: BTreeMap<String, String> = serde_json::from_str(&raw).unwrap();

    for (key, want) in &expected {
        let got = captured
            .get(key)
            .unwrap_or_else(|| panic!("`{key}` is no longer declared"));
        assert_eq!(got, want, "`{key}` changed");
    }
    assert_eq!(
        captured.keys().collect::<Vec<_>>(),
        expected.keys().collect::<Vec<_>>(),
        "the set of declared surfaces changed"
    );
}
