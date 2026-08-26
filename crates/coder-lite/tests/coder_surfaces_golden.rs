//! coder-lite's own voice, pinned byte for byte.
//!
//! The system prompt this crate composes is a staged artifact
//! (`surfaces/coder/system-prompt.v1.json`, OpenAgentsInc/openagents#122): a
//! change to it is meant to be a diff over that artifact with a measured
//! delta behind it, never a quiet edit in passing. This test composes the
//! prompt from fixed inputs and compares it with `coder-surfaces-golden.json`,
//! so moving the text out of `runtime.rs` and into the artifact had to be a
//! move and nothing else.
//!
//! Re-pin deliberately, with `UPDATE_CODER_SURFACES_GOLDEN=1 cargo test -p
//! coder-lite --test coder_surfaces_golden`, and only when the change to the
//! text is the change you meant to make.

use std::collections::BTreeMap;
use std::path::PathBuf;

use coder_lite::runtime::system_prompt;
use openagents_cli::tools::ToolDefinition;

fn declared(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("A declared {name}."),
        parameters: serde_json::json!({"type": "object"}),
    }
}

fn captured() -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    out.insert("system_prompt.no_tools".to_string(), system_prompt(&[]));
    out.insert(
        "system_prompt.one_tool".to_string(),
        system_prompt(&[declared("alpha")]),
    );
    out.insert(
        "system_prompt.two_tools".to_string(),
        system_prompt(&[declared("alpha"), declared("beta")]),
    );
    out
}

#[test]
fn the_composed_prompt_matches_the_pinned_golden() {
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
    assert_eq!(captured, expected);
}
