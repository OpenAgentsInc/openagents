//! The tool a test set is written for, and the catalog chat picks from.
//!
//! A tool is either an existing one (a catalog tool, a published
//! extension, or an extension directory on the operator's computer), named
//! by its DefinitionRef, or one made in chat: a skill, which is
//! plain-language guidance Coder follows, that may also turn on catalog
//! tools. Each tool declares its operations, the names its trajectory
//! records when Coder reaches it; the floor checks test prompts against
//! them as a bounded field.

use nostr::cj_conversation::{DraftTool, MAX_USES};
use nostr::contracts::{DefinitionRef, parse_definition};
use serde_json::json;

/// Where a tool comes from.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// An existing tool, by its definition.
    Existing(DefinitionRef),
    /// A tool made in chat: its guidance and the catalog tools it turns on,
    /// as qualified IDs.
    Made { skill: String, uses: Vec<String> },
}

/// A tool as the interview knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    /// Its name, as the phone shows it.
    pub name: String,
    /// One plain sentence: what it does.
    pub summary: String,
    /// The tool's own words, read at step 1: its description, its README,
    /// its steps. Bounded by the reader.
    pub words: String,
    /// Where it comes from.
    pub source: Source,
    /// The operation names its trajectory records.
    pub operations: Vec<String>,
}

impl Tool {
    /// The draft's `tool` object.
    #[must_use]
    pub fn draft_tool(&self) -> DraftTool {
        match &self.source {
            Source::Existing(definition) => DraftTool {
                name: self.name.clone(),
                summary: self.summary.clone(),
                catalog: Some(definition.clone()),
                skill: None,
                uses: Vec::new(),
            },
            Source::Made { skill, uses } => DraftTool {
                name: self.name.clone(),
                summary: self.summary.clone(),
                catalog: None,
                skill: Some(skill.clone()),
                uses: uses.clone(),
            },
        }
    }

    /// Whether the tool is made in chat.
    #[must_use]
    pub fn is_made(&self) -> bool {
        matches!(self.source, Source::Made { .. })
    }

    /// The names a test prompt must not use: the tool's name, its
    /// operations, and for a made tool the names and operations of the
    /// catalog tools it turns on.
    #[must_use]
    pub fn forbidden_names(&self, catalog: &Catalog) -> Vec<String> {
        let mut names = vec![self.name.clone()];
        names.extend(self.operations.iter().cloned());
        if let Source::Made { uses, .. } = &self.source {
            for id in uses {
                if let Some(tool) = catalog.by_id(id) {
                    names.push(tool.name.clone());
                    names.extend(tool.operations.iter().cloned());
                }
            }
        }
        names.retain(|name| !name.trim().is_empty());
        names.sort();
        names.dedup();
        names
    }
}

/// The tools chat offers to test and to turn on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalog {
    /// The tools, in the order chat lists them.
    pub tools: Vec<Tool>,
}

/// One starter tool: its program step, its component, and its pinned wasm.
struct Guest {
    name: &'static str,
    summary: &'static str,
    words: &'static str,
    step: &'static str,
    component: &'static str,
    digest: &'static str,
    size: u64,
}

/// The package key the evidence-guest program names its guests under
/// (`programs/evidence-guests.json`).
pub const STARTER_KEY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const GUESTS: [Guest; 3] = [
    Guest {
        name: "Project map",
        summary: "Shows Coder how the project is laid out before it starts.",
        words: "Maps the workspace it is granted: how many files and bytes, languages by extension, the top-level entries and their file counts, the directories one level below, the largest files, build manifests, and test files. It reads sizes, not contents. It runs as a program step before Coder starts, so Coder spends less time looking around. It does not read or change code, and it does not answer questions by itself.",
        step: "repo_map",
        component: "repo-map",
        digest: "sha256:0f788d7adacbe189aed5c8be9aa96a13e2568074f3db213d708c5ac7254a1536",
        size: 134_838,
    },
    Guest {
        name: "Code finder",
        summary: "Finds the right lines of code.",
        words: "Searches the granted workspace for up to sixteen literal patterns (a star matches any run within a line) and returns matching lines grouped by file, files that match more patterns first, with per-pattern counts. It skips binary, lock, and minified files. It runs as a program step when the task yields search terms. It does not change code or decide what the lines mean.",
        step: "code_search",
        component: "code-search",
        digest: "sha256:ade4a8590f3364eca53279c181a4fd7df99d52c85813180395b05112d18b5bf7",
        size: 109_343,
    },
    Guest {
        name: "Test reader",
        summary: "Reads test failures for Coder.",
        words: "Parses test reports in the granted workspace: JUnit XML, cargo test output, or pytest output, chosen from the content. It returns each report's format and counts, and each failing test with its file, line, and message. It runs as a program step when a file shows a test report. It does not run tests or fix them.",
        step: "test_report",
        component: "test-report",
        digest: "sha256:37d024ef86db9c24ea750131dcf621322ed5a7153678f435692f34e6596ce887",
        size: 121_806,
    },
];

impl Catalog {
    /// The starter catalog: Project map, Code finder, and Test reader, the
    /// three evidence guests, pinned to the modules
    /// `programs/evidence-guests.json` runs.
    ///
    /// # Panics
    ///
    /// Never: the pinned references are constants a test checks.
    #[must_use]
    pub fn starter() -> Self {
        let tools = GUESTS
            .iter()
            .map(|guest| {
                let definition = parse_definition(&json!({
                    "id": format!("{STARTER_KEY}:openagents/{}", guest.component),
                    "artifact": {
                        "digest": guest.digest,
                        "size": guest.size,
                        "media_type": "application/wasm",
                    },
                }))
                .expect("the starter references are valid");
                Tool {
                    name: guest.name.into(),
                    summary: guest.summary.into(),
                    words: guest.words.into(),
                    source: Source::Existing(definition),
                    operations: vec![guest.step.into()],
                }
            })
            .collect();
        Self { tools }
    }

    /// The tool whose definition has `id`.
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<&Tool> {
        self.tools
            .iter()
            .find(|tool| matches!(&tool.source, Source::Existing(d) if d.id == id))
    }

    /// The tool named exactly `name` or with definition `name`, ignoring
    /// case: an exact lookup of a bounded field the model wrote.
    #[must_use]
    pub fn by_name(&self, name: &str) -> Option<&Tool> {
        let name = name.trim();
        self.tools.iter().find(|tool| {
            tool.name.eq_ignore_ascii_case(name)
                || matches!(&tool.source, Source::Existing(d) if d.id == name)
        })
    }

    /// The qualified IDs of the catalog tools `names` name, at most
    /// [`MAX_USES`], in catalog order; names nothing here answers are
    /// dropped.
    #[must_use]
    pub fn uses(&self, names: &[String]) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for tool in &self.tools {
            let Source::Existing(definition) = &tool.source else {
                continue;
            };
            if names.iter().any(|name| {
                self.by_name(name)
                    .is_some_and(|named| named.name == tool.name)
            }) && ids.len() < MAX_USES
            {
                ids.push(definition.id.clone());
            }
        }
        ids
    }

    /// The tool a draft names. A catalog definition this catalog knows is
    /// its tool; an unknown definition is a tool with no operations known.
    #[must_use]
    pub fn tool_of(&self, draft: &DraftTool) -> Tool {
        if let Some(definition) = &draft.catalog {
            if let Some(tool) = self.by_id(&definition.id) {
                return tool.clone();
            }
            return Tool {
                name: draft.name.clone(),
                summary: draft.summary.clone(),
                words: draft.summary.clone(),
                source: Source::Existing(definition.clone()),
                operations: Vec::new(),
            };
        }
        let operations = draft
            .uses
            .iter()
            .filter_map(|id| self.by_id(id))
            .flat_map(|tool| tool.operations.iter().cloned())
            .collect();
        Tool {
            name: draft.name.clone(),
            summary: draft.summary.clone(),
            words: draft.skill.clone().unwrap_or_default(),
            source: Source::Made {
                skill: draft.skill.clone().unwrap_or_default(),
                uses: draft.uses.clone(),
            },
            operations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The starter pins are the modules the evidence-guest program runs.
    #[test]
    fn the_starter_pins_match_the_evidence_guest_program() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../programs/evidence-guests.json");
        let program: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let steps = program["definition"]["steps"].as_array().unwrap();
        let catalog = Catalog::starter();
        assert_eq!(catalog.tools.len(), steps.len());
        for (tool, step) in catalog.tools.iter().zip(steps) {
            let Source::Existing(definition) = &tool.source else {
                panic!("a starter tool is existing")
            };
            assert_eq!(tool.operations, [step["name"].as_str().unwrap()]);
            assert_eq!(definition.id, step["target"]["id"].as_str().unwrap());
            assert_eq!(
                definition.artifact.digest,
                step["target"]["artifact"]["digest"].as_str().unwrap()
            );
            assert_eq!(
                definition.artifact.size,
                step["target"]["artifact"]["size"].as_u64().unwrap()
            );
        }
    }

    #[test]
    fn uses_are_exact_names_or_ids_only() {
        let catalog = Catalog::starter();
        let ids = catalog.uses(&[
            "project MAP".into(),
            "Code finder".into(),
            "a map of the project".into(),
        ]);
        assert_eq!(ids.len(), 2);
        assert!(ids[0].ends_with("/repo-map"));
        assert!(ids[1].ends_with("/code-search"));
        let made = Tool {
            name: "Changelog helper".into(),
            summary: String::new(),
            words: String::new(),
            source: Source::Made {
                skill: "Write short entries.".into(),
                uses: ids.clone(),
            },
            operations: Vec::new(),
        };
        let names = made.forbidden_names(&catalog);
        assert!(names.contains(&"Project map".to_string()));
        assert!(names.contains(&"code_search".to_string()));
        let draft = made.draft_tool();
        assert_eq!(
            catalog.tool_of(&draft).operations,
            ["repo_map", "code_search"]
        );
    }
}
