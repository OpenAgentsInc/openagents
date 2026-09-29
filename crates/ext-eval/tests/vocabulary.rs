//! The one vocabulary (issue #9957): people add capabilities, shipped as
//! extension packages, as programs, plugins, skills, or knowledge entries.
//! The retired umbrellas must not come back into the README or the
//! glossary, which is where the words are defined.

use std::path::Path;

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// "Make a tool" and "plugin system" were the umbrellas the decision retired.
#[test]
fn the_readme_and_glossary_keep_the_one_vocabulary() {
    for rel in ["README.md", "docs/glossary.md"] {
        let text = read(rel).to_lowercase();
        for phrase in ["make a tool", "plugin system"] {
            assert!(
                !text.contains(phrase),
                "{rel} says {phrase:?}; people add capabilities (docs/glossary.md, One vocabulary)"
            );
        }
    }
}

/// The glossary defines the umbrella and its four kinds in one place.
#[test]
fn the_glossary_defines_the_vocabulary() {
    let glossary = read("docs/glossary.md");
    assert!(glossary.contains("## One vocabulary: what you can add"));
    for kind in [
        "| Program |",
        "| Plugin |",
        "| Skill |",
        "| Knowledge entry |",
    ] {
        assert!(
            glossary.contains(kind),
            "the what-you-can-add table lacks {kind}"
        );
    }
}
