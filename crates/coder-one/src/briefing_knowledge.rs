//! Knowledge-base entries the host selected for a delegate's briefing
//! (issue #9746).
//!
//! The host searches Coder's knowledge base with the task instruction,
//! selects entries by a stated rule, and writes them to a JSON file. The
//! episode reads that file from `CODER_ONE_BRIEFING_KNOWLEDGE`, and
//! [`crate::delegate::Briefing::build_knowing`] adds each entry, whole, in
//! a section the delegate reads right after the task. Nothing here
//! searches: the selection is the host's, and it's on the record.
//!
//! The file is `{"note"?, "entries": [{"id", "version", "sha256",
//! "score"?, "text"}]}` with other fields kept for the record. `note`, when
//! present, replaces [`NOTE`] as the paragraph under the heading, so the
//! host's wording is on the record with its selection. `sha256` is the
//! knowledge base's entry digest: the sha256 of the entry file's bytes,
//! which `text` holds verbatim. An entry whose text doesn't match its
//! digest is refused.

use sha2::{Digest, Sha256};

/// The variable that names the selection file.
pub const ENV: &str = "CODER_ONE_BRIEFING_KNOWLEDGE";

/// The file the episode keeps a copy of the selection in, under its
/// artifacts.
pub const ARTIFACT: &str = "briefing-knowledge.json";

/// The section's heading.
pub const HEADING: &str = "## What Coder's knowledge base says";

/// The paragraph under the heading unless the selection gives its own.
pub const NOTE: &str = "Coder wrote these entries from its earlier runs. \
Each one is whole. Use what applies to this task, and check it against the \
task's own words and data.";

/// One entry, whole.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    pub id: String,
    pub version: u64,
    /// The entry digest, hex.
    pub sha256: String,
    /// The search score the host ranked it by, when it recorded one.
    #[serde(default)]
    pub score: Option<f64>,
    /// The entry file's text, verbatim.
    pub text: String,
    /// Jev's probability that the entry applies, when Jev chose it
    /// ([`crate::briefing_jev`]). Never read from the host's file.
    #[serde(skip)]
    pub jev: Option<f64>,
}

impl Entry {
    /// How the briefing record names the entry: its id, version, and
    /// digest.
    #[must_use]
    pub fn label(&self) -> String {
        format!(
            "knowledge {} v{} sha256={}",
            self.id, self.version, self.sha256
        )
    }

    /// The entry as the briefing carries it.
    #[must_use]
    pub fn section(&self) -> String {
        let jev = self
            .jev
            .map_or(String::new(), |p| format!(", Jev p={p:.2}"));
        format!(
            "### {} (version {}, sha256 {}{jev})\n\n{}\n",
            self.id,
            self.version,
            &self.sha256[..self.sha256.len().min(12)],
            self.text.trim_end()
        )
    }
}

/// A requirement Jev flagged as easy to miss, with its probability
/// ([`crate::briefing_jev`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Flagged {
    pub text: String,
    pub p: f64,
}

/// The host's selection: the entries in its order, and the paragraph
/// that introduces them when it isn't [`NOTE`]. When Jev chose the
/// entries ([`crate::briefing_jev`]), `entries` holds only the ones it
/// kept, `chosen_from` counts the candidates it judged, and `flagged`
/// holds the requirements it flagged; neither is read from the file.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Knowledge {
    #[serde(default)]
    pub note: Option<String>,
    pub entries: Vec<Entry>,
    #[serde(skip)]
    pub chosen_from: Option<usize>,
    #[serde(skip)]
    pub flagged: Vec<Flagged>,
}

impl Knowledge {
    /// No selection: the briefing has no knowledge section.
    pub const NONE: Self = Self {
        note: None,
        entries: Vec::new(),
        chosen_from: None,
        flagged: Vec::new(),
    };

    /// The paragraph under the heading: the host's note or [`NOTE`], and
    /// [`crate::briefing_jev::KEPT_NOTE`] after it when Jev chose the
    /// entries.
    #[must_use]
    pub fn note(&self) -> String {
        let note = self
            .note
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty())
            .unwrap_or(NOTE);
        match self.chosen_from {
            Some(_) => format!("{note} {}", crate::briefing_jev::KEPT_NOTE),
            None => note.to_string(),
        }
    }
}

/// Reads a selection file's JSON, in the host's order. Refuses an entry
/// whose text doesn't hash to its digest, and a repeated id.
///
/// # Errors
///
/// A message naming the problem.
pub fn parse(json: &str) -> Result<Knowledge, String> {
    let selection: Knowledge = serde_json::from_str(json)
        .map_err(|error| format!("the briefing knowledge isn't a selection: {error}"))?;
    let mut seen = std::collections::BTreeSet::new();
    for entry in &selection.entries {
        let actual = hex(&Sha256::digest(entry.text.as_bytes()));
        if !actual.eq_ignore_ascii_case(&entry.sha256) {
            return Err(format!(
                "knowledge entry {} says sha256 {} but its text hashes to {actual}",
                entry.id, entry.sha256
            ));
        }
        if !seen.insert(entry.id.clone()) {
            return Err(format!("knowledge entry {} appears twice", entry.id));
        }
    }
    Ok(selection)
}

/// Reads the selection the environment names, when it names one.
///
/// # Errors
///
/// A message when the file can't be read or parsed.
pub fn from_env() -> Result<Option<(std::path::PathBuf, String, Knowledge)>, String> {
    let Some(path) = std::env::var(ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let path = std::path::PathBuf::from(path);
    let json = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {ENV} {}: {error}", path.display()))?;
    let knowledge = parse(&json)?;
    Ok(Some((path, json, knowledge)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::delegate::{BRIEFING_CAP, Briefing, BriefingInputs};

    pub(crate) fn entry(id: &str, body: &str) -> Entry {
        let text = format!("---\nid: {id}\nversion: 2\n---\n\n{body}\n");
        Entry {
            id: id.to_string(),
            version: 2,
            sha256: hex(&Sha256::digest(text.as_bytes())),
            score: Some(0.9),
            text,
            jev: None,
        }
    }

    fn knowing(entries: &[Entry]) -> Knowledge {
        Knowledge {
            note: None,
            entries: entries.to_vec(),
            ..Knowledge::NONE
        }
    }

    fn inputs() -> BriefingInputs {
        BriefingInputs {
            instruction: "Compute the exposure for two netting sets.".to_string(),
            requirements: vec![("Write results.csv".to_string(), Some(0.1))],
            files: vec![(
                "inputs/a.csv".to_string(),
                Some(0.8),
                "a,b\n1,2".to_string(),
            )],
            spans: Vec::new(),
            commands: vec![("ls inputs".to_string(), Some(0))],
            last_output: Some("a.csv".to_string()),
            conclusion: "The inputs are CSV files.".to_string(),
            directions: "Work in /app.".to_string(),
        }
    }

    #[test]
    fn no_knowledge_builds_the_same_briefing_as_before() {
        let plain = Briefing::build(&inputs(), BRIEFING_CAP);
        let knowing = Briefing::build_knowing(&inputs(), &Knowledge::NONE, BRIEFING_CAP);
        assert_eq!(plain, knowing);
        assert!(!plain.text.contains(HEADING));
    }

    #[test]
    fn the_section_follows_the_task_and_keeps_the_hosts_order() {
        let entries = [
            entry("finance.first", "EAD is alpha times RC plus PFE."),
            entry("slip.second", "Check the sign of a sold put."),
        ];
        let briefing = Briefing::build_knowing(&inputs(), &knowing(&entries), BRIEFING_CAP);
        let text = &briefing.text;
        let task = text.find("## The task").unwrap();
        let section = text.find(HEADING).unwrap();
        let requirements = text.find("## Requirements").unwrap();
        assert!(task < section && section < requirements);
        assert!(text.contains(NOTE));
        let first = text.find("### finance.first (version 2, sha256 ").unwrap();
        let second = text.find("### slip.second (version 2, sha256 ").unwrap();
        assert!(first < second);
        // Each entry goes in verbatim.
        assert!(text.contains(entries[0].text.trim_end()));
        assert!(briefing.included.contains(&entries[0].label()));
        assert!(briefing.included.contains(&entries[1].label()));
        assert!(briefing.omitted.is_empty());
    }

    #[test]
    fn an_entry_that_doesnt_fit_is_left_out_whole_and_named() {
        let entries = [
            entry("small.one", "Short."),
            entry("large.two", &"x".repeat(3_000)),
            entry("small.three", "Also short."),
        ];
        let cap = 2_000;
        let briefing = Briefing::build_knowing(&inputs(), &knowing(&entries), cap);
        assert!(briefing.chars() <= cap, "{} characters", briefing.chars());
        assert!(!briefing.text.contains(&"x".repeat(100)));
        assert!(briefing.text.contains("Also short."));
        let left_out = briefing
            .omitted
            .iter()
            .find(|item| item.starts_with(&entries[1].label()))
            .expect("the large entry is named as left out");
        assert!(left_out.ends_with("characters)"));
        // The delegate is told which entry it didn't get.
        assert!(briefing.text.contains("Left out for length: large.two v2"));
        assert!(briefing.included.contains(&entries[2].label()));
        let recorded = briefing.record();
        assert!(
            recorded["omitted"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item.as_str().unwrap().contains(&entries[1].sha256))
        );
    }

    #[test]
    fn knowledge_comes_before_the_explorers_evidence_under_the_cap() {
        let entries = [entry("finance.big", &"y".repeat(1_200))];
        let mut inputs = inputs();
        inputs.files = (0..10)
            .map(|i| (format!("inputs/f{i}.csv"), Some(0.5), "z".repeat(300)))
            .collect();
        let briefing = Briefing::build_knowing(&inputs, &knowing(&entries), 2_500);
        assert!(briefing.chars() <= 2_500);
        assert!(briefing.included.contains(&entries[0].label()));
        assert!(
            briefing
                .omitted
                .iter()
                .any(|item| item.starts_with("file inputs/f9.csv"))
        );
    }

    #[test]
    fn the_hosts_note_replaces_the_default_paragraph() {
        let entries = vec![entry("finance.first", "EAD is alpha times RC plus PFE.")];
        let json =
            serde_json::json!({ "note": "Act on these entries.", "entries": entries }).to_string();
        let knowledge = parse(&json).unwrap();
        let briefing = Briefing::build_knowing(&inputs(), &knowledge, BRIEFING_CAP);
        assert!(
            briefing
                .text
                .contains(&format!("{HEADING}\n\nAct on these entries.\n"))
        );
        assert!(!briefing.text.contains(NOTE));
    }

    #[test]
    fn parse_keeps_the_order_and_refuses_a_wrong_digest() {
        let entries = vec![entry("b.two", "Two."), entry("a.one", "One.")];
        let json = serde_json::json!({ "command": "kb search", "entries": entries }).to_string();
        let parsed = parse(&json).unwrap();
        assert_eq!(parsed.entries, entries);
        assert_eq!(parsed.note(), NOTE);

        let mut wrong = entries.clone();
        wrong[0].text.push('!');
        let json = serde_json::json!({ "entries": wrong }).to_string();
        assert!(parse(&json).unwrap_err().contains("hashes to"));

        let twice = vec![entries[0].clone(), entries[0].clone()];
        let json = serde_json::json!({ "entries": twice }).to_string();
        assert!(parse(&json).unwrap_err().contains("appears twice"));
    }
}
