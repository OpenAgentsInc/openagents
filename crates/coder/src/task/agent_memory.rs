//! A workshop agent's typed memory (`docs/verse/workshop-agent.md`,
//! "Memory"): `agents/NAME/memory.jsonl` under the host root, mode `0600`.
//!
//! Each entry is one of five kinds, at most [`ENTRY_MAX`] bytes. A
//! preference the agent proposes is a candidate, a NIP-POL preference
//! candidate, and no briefing carries it until the owner accepts it. The
//! secret screen (`secret-screen`) refuses an entry that looks like a
//! credential or holds a credential this host keeps. Forgetting removes
//! the entry and journals that it was forgotten, not what it said. A
//! briefing carries at most [`BRIEFING_MAX`] bytes, chosen by relevance to
//! the request, and the journal records which entries it carried: the CTX
//! selection receipt.

use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::agent::{Entry, Kind, Store};

/// One memory entry's schema.
pub const SCHEMA: &str = "openagents.agent-memory-entry.v1";
/// The longest entry, bytes.
pub const ENTRY_MAX: usize = 2048;
/// The most entries an agent keeps; the oldest outcome goes first.
pub const MAX_ENTRIES: usize = 256;
/// The most memory one briefing carries, bytes.
pub const BRIEFING_MAX: usize = 12 * 1024;

/// What an entry is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    /// What a project needs, from a task's evidence.
    Project,
    /// How the owner wants work done; active only once accepted.
    Preference,
    /// What happened, written by the host from the journal.
    Outcome,
    /// Anything the owner told it to remember.
    Note,
    /// What a reflection inferred from the records its `sources` cite,
    /// after code checked each citation (`agent_reflect`).
    Insight,
}

impl MemoryKind {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Preference => "preference",
            Self::Outcome => "outcome",
            Self::Note => "note",
            Self::Insight => "insight",
        }
    }
}

/// Whether an entry shapes behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryState {
    Active,
    /// A preference waiting for the owner.
    Candidate,
    Rejected,
}

impl MemoryState {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Candidate => "candidate",
            Self::Rejected => "rejected",
        }
    }
}

/// Who wrote an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Author {
    Owner,
    Agent,
    Host,
}

/// One entry (`openagents.agent-memory-entry.v1`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryEntry {
    pub schema: String,
    pub v: u32,
    pub requires: Vec<String>,
    pub id: u64,
    pub kind: MemoryKind,
    pub state: MemoryState,
    pub author: Author,
    pub text: String,
    /// Unix seconds.
    pub at: u64,
    /// What it came from, such as a journal entry's position.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<String>,
}

/// An agent's memory.
#[derive(Clone, Debug)]
pub struct Memory {
    store: Store,
    screen: secret_screen::Screen,
}

impl Memory {
    /// `store`'s memory, screened by `screen`.
    #[must_use]
    pub fn new(store: Store, screen: secret_screen::Screen) -> Self {
        Self { store, screen }
    }

    fn path(&self) -> PathBuf {
        self.store.dir().join("memory.jsonl")
    }

    /// The agent's store.
    #[must_use]
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// The secret screen every write passes.
    #[must_use]
    pub fn screen(&self) -> &secret_screen::Screen {
        &self.screen
    }

    /// Replaces the working file with `entries`, without the engram
    /// write-through: `agent_engrams::reconcile` uses it to take engram
    /// heads that are newer than their rows.
    ///
    /// # Errors
    /// When the memory cannot be written.
    pub fn replace_entries(&self, entries: &[MemoryEntry]) -> Result<(), String> {
        self.write(entries)
    }

    /// Every entry, oldest first. A line that does not read is skipped.
    /// A missing file is rebuilt from the engram store first, when it
    /// holds entries (`agent_engrams::rebuild_from_engrams`).
    ///
    /// # Errors
    /// When the file exists and cannot be read.
    pub fn entries(&self) -> Result<Vec<MemoryEntry>, String> {
        if std::fs::symlink_metadata(self.path())
            .is_err_and(|e| e.kind() == std::io::ErrorKind::NotFound)
        {
            let _ = super::agent_engrams::rebuild_from_engrams(self);
        }
        let Some(text) = super::sales::privacy::read_agent_text(&self.store, &self.path())? else {
            return Ok(Vec::new());
        };
        Ok(text
            .lines()
            .filter_map(|line| serde_json::from_str::<MemoryEntry>(line).ok())
            .filter(|entry| entry.schema == SCHEMA && entry.v == 1 && entry.requires.is_empty())
            .collect())
    }

    fn write(&self, entries: &[MemoryEntry]) -> Result<(), String> {
        for entry in entries {
            super::sales::privacy::check_agent_copy(
                &self.store,
                &serde_json::to_string(entry).map_err(|_| "agent memory serialization failed")?,
            )?;
        }
        let mut body = Vec::new();
        for entry in entries {
            body.extend(serde_json::to_vec(entry).map_err(|e| e.to_string())?);
            body.push(b'\n');
        }
        let temp = self.store.dir().join(".memory.jsonl.tmp");
        let mut options = std::fs::OpenOptions::new();
        options.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temp)
            .map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
        file.write_all(&body)
            .and_then(|()| file.sync_all())
            .map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
        std::fs::rename(&temp, self.path())
            .map_err(|e| format!("cannot write {}: {e}", self.path().display()))
    }

    /// Adds an entry and journals it. A preference from anyone but the
    /// owner is a candidate. An entry the same as an active or candidate
    /// one of its kind is not added again; its ID comes back.
    ///
    /// # Errors
    /// The text is blank, too long, or refused by the secret screen, or
    /// the memory cannot be written.
    pub fn add(
        &self,
        kind: MemoryKind,
        author: Author,
        text: &str,
        sources: Vec<String>,
        now: u64,
    ) -> Result<u64, String> {
        let text = super::agent::ascii(text.trim());
        super::sales::privacy::check_agent_copy(&self.store, &text)?;
        if text.trim().is_empty() || text.len() > ENTRY_MAX {
            return Err(format!("a memory entry is 1 to {ENTRY_MAX} bytes"));
        }
        if let Err(refusal) = self.screen.check(&text) {
            let _ = self.store.append(&Entry::new(
                now,
                Kind::Memory,
                &format!("refused a {} entry: {refusal}", kind.word()),
            ));
            return Err(format!("memory refuses this: {refusal}"));
        }
        let mut entries = self.entries()?;
        if let Some(same) = entries
            .iter()
            .find(|e| e.kind == kind && e.text == text && e.state != MemoryState::Rejected)
        {
            return Ok(same.id);
        }
        let before = entries.clone();
        let id = entries.iter().map(|e| e.id).max().unwrap_or(0) + 1;
        let state = if kind == MemoryKind::Preference && author != Author::Owner {
            MemoryState::Candidate
        } else {
            MemoryState::Active
        };
        entries.push(MemoryEntry {
            schema: SCHEMA.into(),
            v: 1,
            requires: Vec::new(),
            id,
            kind,
            state,
            author,
            text: text.clone(),
            at: now,
            sources,
        });
        while entries.len() > MAX_ENTRIES {
            let oldest = entries
                .iter()
                .position(|e| e.kind == MemoryKind::Outcome)
                .unwrap_or(0);
            entries.remove(oldest);
        }
        self.write(&entries)?;
        super::agent_engrams::write_through(self, &before, &entries, now);
        self.store.append(&Entry::new(
            now,
            Kind::Memory,
            &format!(
                "{} {} entry {id}: {}",
                if state == MemoryState::Candidate {
                    "proposed"
                } else {
                    "wrote"
                },
                kind.word(),
                text
            ),
        ))?;
        Ok(id)
    }

    /// Accepts or rejects candidate `id`.
    ///
    /// # Errors
    /// No such candidate, or the memory cannot be written.
    pub fn decide(&self, id: u64, accept: bool, now: u64) -> Result<(), String> {
        let mut entries = self.entries()?;
        let before = entries.clone();
        let entry = entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("no memory entry {id}"))?;
        if entry.state != MemoryState::Candidate {
            return Err(format!("entry {id} is not a proposed preference"));
        }
        entry.state = if accept {
            MemoryState::Active
        } else {
            MemoryState::Rejected
        };
        self.write(&entries)?;
        super::agent_engrams::write_through(self, &before, &entries, now);
        self.store.append(&Entry::new(
            now,
            Kind::Memory,
            &format!(
                "the owner {} preference {id}",
                if accept { "accepted" } else { "rejected" }
            ),
        ))
    }

    /// Forgets entry `id`. The journal records that, not what it said.
    ///
    /// # Errors
    /// No such entry, or the memory cannot be written.
    pub fn forget(&self, id: u64, now: u64) -> Result<(), String> {
        let mut entries = self.entries()?;
        let before = entries.clone();
        entries.retain(|e| e.id != id);
        if entries.len() == before.len() {
            return Err(format!("no memory entry {id}"));
        }
        self.write(&entries)?;
        super::agent_engrams::write_through(self, &before, &entries, now);
        self.store.append(&Entry::new(
            now,
            Kind::Memory,
            &format!("forgot entry {id}"),
        ))
    }

    /// The word-overlap briefing for `request` in `workspace`: active
    /// entries only, most relevant first, within [`BRIEFING_MAX`], as plain
    /// lines, and the IDs it carried. Relevance is the words an entry
    /// shares with the request and the workspace; a note and an accepted
    /// preference always go first. The host briefs with the scored stream
    /// (`Memory::recall` in `agent_recall`); this stays as the baseline the
    /// interview compares it with and a host can fall back to.
    ///
    /// # Errors
    /// When the memory cannot be read.
    pub fn briefing(&self, request: &str, workspace: &str) -> Result<(String, Vec<u64>), String> {
        let words: std::collections::BTreeSet<String> = format!("{request} {workspace}")
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| w.len() >= 3)
            .map(str::to_ascii_lowercase)
            .collect();
        let mut scored: Vec<(i64, MemoryEntry)> = self
            .entries()?
            .into_iter()
            .filter(|e| e.state == MemoryState::Active)
            .map(|e| {
                let shared = e
                    .text
                    .split(|c: char| !c.is_ascii_alphanumeric())
                    .filter(|w| words.contains(&w.to_ascii_lowercase()))
                    .count() as i64;
                let standing = match e.kind {
                    MemoryKind::Preference | MemoryKind::Note => 1000,
                    MemoryKind::Project | MemoryKind::Insight => 10,
                    MemoryKind::Outcome => 0,
                };
                (standing + shared * 5, e)
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.at.cmp(&a.1.at)));
        let mut text = String::new();
        let mut carried = Vec::new();
        for (score, entry) in scored {
            if score == 0 && entry.kind == MemoryKind::Outcome && carried.len() >= 4 {
                continue;
            }
            let line = format!(
                "- ({}) {}\n",
                entry.kind.word(),
                entry.text.replace('\n', " ")
            );
            if text.len() + line.len() > BRIEFING_MAX {
                continue;
            }
            text.push_str(&line);
            carried.push(entry.id);
        }
        Ok((text, carried))
    }

    /// Entries as a device reads them, after ID `after`.
    ///
    /// # Errors
    /// When the memory cannot be read.
    pub fn rows(
        &self,
        after: Option<u64>,
    ) -> Result<Vec<coder_host::access::agent::MemoryRow>, String> {
        Ok(self
            .entries()?
            .into_iter()
            .filter(|e| after.is_none_or(|after| e.id > after))
            .take(128)
            .map(|e| coder_host::access::agent::MemoryRow {
                id: e.id,
                kind: e.kind.word().into(),
                state: e.state.word().into(),
                text: e.text,
                at: e.at,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(dir: &tempfile::TempDir) -> Memory {
        let store = Store::new(&dir.path().join("host"), "alice").unwrap();
        store.open(dir.path(), 1).unwrap();
        Memory::new(store, secret_screen::Screen::shapes())
    }

    #[test]
    fn preferences_wait_for_the_owner_and_briefings_skip_them() {
        let dir = tempfile::tempdir().unwrap();
        let memory = memory(&dir);
        let pref = memory
            .add(
                MemoryKind::Preference,
                Author::Agent,
                "Owner wants small commits.",
                vec!["journal:4".into()],
                10,
            )
            .unwrap();
        let note = memory
            .add(
                MemoryKind::Note,
                Author::Owner,
                "The atif crate is ours.",
                vec![],
                11,
            )
            .unwrap();
        memory
            .add(
                MemoryKind::Project,
                Author::Agent,
                "openagents: run cargo test -p for touched crates.",
                vec![],
                12,
            )
            .unwrap();
        let (text, carried) = memory.briefing("run the atif tests", "/w").unwrap();
        assert!(!text.contains("small commits"), "{text}");
        assert!(carried.contains(&note) && !carried.contains(&pref));
        memory.decide(pref, true, 20).unwrap();
        let (text, carried) = memory.briefing("anything", "/w").unwrap();
        assert!(text.contains("small commits") && carried.contains(&pref));
        assert!(memory.decide(pref, true, 21).is_err(), "decided once");
        // Adding the same note again keeps one entry.
        assert_eq!(
            memory
                .add(
                    MemoryKind::Note,
                    Author::Owner,
                    "The atif crate is ours.",
                    vec![],
                    22
                )
                .unwrap(),
            note
        );
    }

    #[test]
    fn the_secret_screen_refuses_and_forgetting_leaves_no_content() {
        let dir = tempfile::tempdir().unwrap();
        let memory = memory(&dir);
        let token = format!("ghp_{}", "Ab1".repeat(12));
        let refused = memory.add(
            MemoryKind::Note,
            Author::Owner,
            &format!("my token is {token}"),
            vec![],
            5,
        );
        assert!(refused.unwrap_err().contains("credential"));
        assert!(memory.entries().unwrap().is_empty());
        let id = memory
            .add(
                MemoryKind::Note,
                Author::Owner,
                "secret project codename falcon",
                vec![],
                6,
            )
            .unwrap();
        memory.forget(id, 7).unwrap();
        assert!(memory.entries().unwrap().is_empty());
        let journal =
            std::fs::read_to_string(dir.path().join("host/agents/alice/journal.jsonl")).unwrap();
        assert!(!journal.contains(&token));
        assert!(journal.contains("forgot entry"));
        // The forgotten text survives only in the line that wrote it,
        // which the journal never rewrites; the forget line holds none.
        let last = journal.lines().last().unwrap();
        assert!(!last.contains("falcon"));
    }
}
