//! The one vocabulary (issue #10087, replacing #9957): a plugin is anything
//! people add, and it contains skills, workflows, knowledge, Wasm, and
//! tests. The specifications keep capability (a measured effect), extension
//! package (the container), component, and program beneath it. The retired
//! umbrellas must not come back into the README or the glossary, which is
//! where the words are defined.

use std::path::Path;

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// "Make a tool" and "add a capability" were umbrellas the decisions
/// retired: the thing people add is a plugin.
#[test]
fn the_readme_and_glossary_keep_the_one_vocabulary() {
    for rel in ["README.md", "docs/glossary.md", "docs/plugins/README.md"] {
        let text = read(rel).to_lowercase();
        for phrase in ["make a tool", "add a capability", "make a capability"] {
            assert!(
                !text.contains(phrase),
                "{rel} says {phrase:?}; people add plugins (docs/glossary.md, One vocabulary)"
            );
        }
    }
}

/// The glossary defines the word and its five parts in one place, with the
/// internal terms beneath it.
#[test]
fn the_glossary_defines_the_vocabulary() {
    let glossary = read("docs/glossary.md");
    assert!(glossary.contains("## One vocabulary: what you can add"));
    assert!(glossary.contains("**A plugin is anything you add to OpenAgents.**"));
    for part in [
        "| Workflow (program) |",
        "| Wasm |",
        "| Skill |",
        "| Knowledge |",
        "| Tests |",
    ] {
        assert!(
            glossary.contains(part),
            "the what-you-can-add table lacks {part}"
        );
    }
    for internal in [
        "**extension package**",
        "**component**",
        "**capability claim**",
    ] {
        assert!(glossary.contains(internal), "the glossary lacks {internal}");
    }
}
