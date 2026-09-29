//! The two arms: what the subject admits and the baseline doesn't.
//!
//! The subject arm admits the extension's components: its programs
//! through `CODER_PROGRAMS` under the runtime's ceilings (Wasm guests ride
//! inside them as `module` steps), and its skills as digested guidance
//! appended to the child's instructions. The baseline arm admits none of
//! them. `run.append_instructions` is appended in both arms.
//!
//! A [`Subject`] is resolved by the caller, which owns the package
//! resolver: `openagents ext eval` resolves a directory's package record
//! with `coder::package` and hands over the exact bytes it verified.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde_json::{Value, json};

use crate::artifact::{ArtifactRef, JSON, json_bytes};
use crate::case::{Case, Grant};
use crate::record::Arm;

/// The run lock document's schema.
pub const LOCK_SCHEMA: &str = "openagents.ext-eval-lock.v1";
/// The schema a subject's package record is referenced under.
pub const PACKAGE_SCHEMA: &str = "openagents.coder-package.v1";
/// The schema the baseline's agent identity is referenced under.
pub const AGENT_SCHEMA: &str = "openagents.ext-eval-agent.v1";

/// One program the subject admits, as the exact bytes of its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// The slug the program resolves under.
    pub slug: String,
    /// The file's exact bytes.
    pub bytes: Vec<u8>,
}

/// One skill the subject admits: plain-language guidance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skill {
    /// The skill's name, its file name without `.md`.
    pub name: String,
    /// The file's exact bytes.
    pub bytes: Vec<u8>,
}

/// The extension under test, resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct Subject {
    /// The package slug.
    pub slug: String,
    /// The subject's DefinitionRef: `{id, artifact, event?}`.
    pub definition: Value,
    /// The package lock the resolver produced, as JSON.
    pub package_lock: Value,
    /// The programs it admits.
    pub programs: Vec<Program>,
    /// The skills it admits.
    pub skills: Vec<Skill>,
}

/// The agent binary both arms run, pinned by its bytes' digest, with the
/// question sets it asks its decision door (Coder's own wording, not the
/// extension's), which both arms get.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentPin {
    /// The binary's absolute path.
    pub path: PathBuf,
    /// Its bytes' `sha256:` digest.
    pub digest: String,
    /// Its size.
    pub size: u64,
    /// Question set files, by file name, written to the child's
    /// `~/.openagents/questions/` in both arms.
    pub questions: Vec<(String, Vec<u8>)>,
}

impl AgentPin {
    /// Pins the binary at `path`.
    ///
    /// # Errors
    ///
    /// Returns why the binary can't be read.
    pub fn of(path: PathBuf) -> Result<Self, String> {
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        Ok(Self {
            digest: nostr::contracts::digest_bytes(&bytes),
            size: bytes.len() as u64,
            path,
            questions: Vec::new(),
        })
    }

    /// The same agent with every `*.json` question set in `dir`.
    ///
    /// # Errors
    ///
    /// Returns why the directory or a file can't be read.
    pub fn with_questions(mut self, dir: &std::path::Path) -> Result<Self, String> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|error| format!("{}: {error}", dir.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json") && path.is_file())
            .collect();
        paths.sort();
        self.questions.clear();
        for path in paths {
            let bytes =
                std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.questions.push((name, bytes));
        }
        Ok(self)
    }

    fn value(&self) -> Value {
        let questions: Vec<Value> = self
            .questions
            .iter()
            .map(|(name, bytes)| {
                json!({ "name": name, "digest": nostr::contracts::digest_bytes(bytes) })
            })
            .collect();
        json!({ "digest": self.digest, "size": self.size, "questions": questions })
    }
}

impl Subject {
    /// The operations the extension supplies: each program's slug, each
    /// step's name, and each Wasm guest's operation. An `operation_used`
    /// grader on one of them is subject-only unless it says otherwise.
    #[must_use]
    pub fn operations(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for program in &self.programs {
            out.insert(program.slug.clone());
            let Ok(value) = serde_json::from_slice::<Value>(&program.bytes) else {
                continue;
            };
            let steps = value
                .pointer("/definition/steps")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for step in &steps {
                if let Some(name) = step.get("name").and_then(Value::as_str) {
                    out.insert(name.to_string());
                    if let Some(operation) = value
                        .pointer(&format!("/binding/steps/{name}/module/operation"))
                        .and_then(Value::as_str)
                    {
                        out.insert(operation.to_string());
                    }
                }
            }
        }
        out
    }

    /// The comma-separated program slugs `CODER_PROGRAMS` names.
    #[must_use]
    pub fn program_grant(&self) -> String {
        self.programs
            .iter()
            .map(|program| program.slug.as_str())
            .collect::<Vec<_>>()
            .join(",")
    }

    /// The subject arm's run lock: the package lock, every admitted
    /// program's and skill's digest, and the agent binary.
    #[must_use]
    pub fn lock_document(&self, agent: &AgentPin) -> Vec<u8> {
        let programs: Vec<Value> = self
            .programs
            .iter()
            .map(|program| {
                json!({
                    "slug": program.slug,
                    "digest": nostr::contracts::digest_bytes(&program.bytes),
                    "size": program.bytes.len(),
                })
            })
            .collect();
        let skills: Vec<Value> = self
            .skills
            .iter()
            .map(|skill| {
                json!({
                    "name": skill.name,
                    "digest": nostr::contracts::digest_bytes(&skill.bytes),
                    "size": skill.bytes.len(),
                })
            })
            .collect();
        json_bytes(&json!({
            "v": LOCK_SCHEMA,
            "requires": [],
            "arm": "subject",
            "definition": self.definition,
            "package": self.package_lock,
            "programs": programs,
            "skills": skills,
            "agent": agent.value(),
        }))
    }

    /// The skills as the guidance block the subject arm appends.
    #[must_use]
    pub fn skills_guidance(&self) -> Option<String> {
        if self.skills.is_empty() {
            return None;
        }
        let mut text = String::from("## Extension skills\n\n");
        for skill in &self.skills {
            text.push_str(&format!(
                "### {} ({})\n\n{}\n\n",
                skill.name,
                nostr::contracts::digest_bytes(&skill.bytes),
                String::from_utf8_lossy(&skill.bytes).trim_end()
            ));
        }
        Some(text)
    }
}

/// The baseline arm's DefinitionRef: the agent alone.
#[must_use]
pub fn baseline_definition(author: &str, agent: &AgentPin) -> Value {
    let bytes = baseline_identity(agent);
    json!({
        "id": format!("{author}:coder/coder"),
        "artifact": ArtifactRef::of(&bytes, JSON, Some(AGENT_SCHEMA)).value(),
    })
}

fn baseline_identity(agent: &AgentPin) -> Vec<u8> {
    json_bytes(&json!({
        "v": AGENT_SCHEMA,
        "requires": [],
        "agent": "coder",
        "binary": agent.value(),
    }))
}

/// The baseline arm's run lock: the agent binary and nothing admitted.
#[must_use]
pub fn baseline_lock(agent: &AgentPin) -> Vec<u8> {
    json_bytes(&json!({
        "v": LOCK_SCHEMA,
        "requires": [],
        "arm": "baseline",
        "programs": [],
        "skills": [],
        "agent": agent.value(),
    }))
}

/// The text appended to the child's instructions for `case` in `arm`:
/// the subject's skills in the subject arm, then the case's
/// `append_instructions` in both. `None` when there is nothing to append.
#[must_use]
pub fn guidance(arm: Arm, subject: &Subject, case: &Case) -> Option<String> {
    let mut parts = Vec::new();
    if arm == Arm::Subject
        && let Some(skills) = subject.skills_guidance()
    {
        parts.push(skills);
    }
    if let Some(extra) = &case.run.append_instructions
        && !extra.trim().is_empty()
    {
        parts.push(format!("## Case instructions\n\n{}\n", extra.trim_end()));
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

/// The `CODER_PROGRAM_EFFECTS` ceiling for the operator's grants: reads
/// always, and writes, subprocesses, and network with spend only when
/// granted.
#[must_use]
pub fn effects_ceiling(grants: &BTreeSet<Grant>) -> String {
    let mut effects = vec!["reads"];
    if grants.contains(&Grant::Write) {
        effects.push("writes");
    }
    if grants.contains(&Grant::Exec) {
        effects.push("subprocesses");
    }
    if grants.contains(&Grant::Network) {
        effects.push("network");
        effects.push("spend");
    }
    effects.join(",")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn subject() -> Subject {
        let program = json!({
            "definition": {"id": "a:b/map-it", "steps": [{"name": "repo_map", "kind": "module"}]},
            "binding": {"steps": {"repo_map": {"module": {"operation": "map"}}}},
        });
        Subject {
            slug: "map-it".into(),
            definition: json!({}),
            package_lock: json!({}),
            programs: vec![Program {
                slug: "map-it".into(),
                bytes: serde_json::to_vec(&program).unwrap(),
            }],
            skills: vec![Skill {
                name: "brief".into(),
                bytes: b"Say what you mapped.\n".to_vec(),
            }],
        }
    }

    #[test]
    fn operations_name_the_program_its_steps_and_its_guest_operations() {
        let operations = subject().operations();
        assert_eq!(
            operations,
            BTreeSet::from(["map".to_string(), "map-it".into(), "repo_map".into()])
        );
    }

    #[test]
    fn skills_are_digested_into_the_guidance() {
        let text = subject().skills_guidance().unwrap();
        assert!(text.contains("### brief (sha256:"));
        assert!(text.contains("Say what you mapped."));
    }

    #[test]
    fn the_ceiling_follows_the_grants() {
        assert_eq!(effects_ceiling(&BTreeSet::from([Grant::Read])), "reads");
        assert_eq!(
            effects_ceiling(&BTreeSet::from([Grant::Read, Grant::Write, Grant::Network])),
            "reads,writes,network,spend"
        );
    }
}
