//! The skill library: what worked, kept so it can work again.
//!
//! A skill is the paper's banked success — a named [`interpret`]
//! program, a description of what it does, a version, and a content
//! digest so two runs talking about `gatherOakLogs v2` mean the same
//! bytes. Skills are runtime data: an episode writes them under
//! `~/.openagents/voyager/skills/`, and a skill that proves itself can
//! promote into the repository's `skills/` directory the way a
//! question set lives in `questions/` — files before events, digested
//! as a whole.
//!
//! Retrieval is a judgment, so it goes through the decision door:
//! [`SkillStore::retrieve`] asks one `choice` question over the
//! candidate descriptions — the same measured-by-the-gym path the
//! paper's ada-002 index filled — and a `none` option is always
//! present, because the honest answer to "which of these fits" is
//! often "none of them". Without a door the store lists rather than
//! guesses: callers get the candidate set, not a keyword match.
//!
//! The store layout, one directory, flat:
//!
//! ```text
//! skills/
//!   index.json                    — name → latest version
//!   gatherOakLogs.v1.json         — {name, version, description, source, digest}
//!   gatherOakLogs.v2.json
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::decide::Door;
use crate::error::{Error, Result};

/// The record schema a skill file declares.
pub const SKILL_SCHEMA: &str = "voyager.skill/v1";

/// One banked program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Skill {
    /// `voyager.skill/v1`.
    pub schema: String,
    /// The skill's name — also its file stem.
    pub name: String,
    /// Its version; a re-banked name increments.
    pub version: u32,
    /// What it does, in the words retrieval reads.
    pub description: String,
    /// The Lua program.
    pub source: String,
    /// `sha256:` of the record with this field blanked.
    #[serde(default)]
    pub digest: String,
}

impl Skill {
    /// The canonical bytes the digest covers — every field but the
    /// digest itself, in schema order.
    fn digest_bytes(&self) -> Result<Vec<u8>> {
        let mut body = self.clone();
        body.digest.clear();
        serde_json::to_vec(&body).map_err(|error| Error::episode(format!("skill: {error}")))
    }

    /// The digest this record earns.
    pub fn digest(&self) -> Result<String> {
        Ok(format!(
            "sha256:{:x}",
            Sha256::digest(&self.digest_bytes()?)
        ))
    }
}

/// The `index.json` shape: name → latest version.
#[derive(Default, Serialize, Deserialize)]
struct Index {
    skills: BTreeMap<String, u32>,
}

/// A directory of skills — the runtime store under `~/.openagents`,
/// or the curated `skills/` in the repository.
pub struct SkillStore {
    dir: PathBuf,
}

impl SkillStore {
    /// Opens a store at `dir`, creating it when missing.
    ///
    /// # Errors
    ///
    /// The directory must create; existing files must parse as they
    /// are read.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        std::fs::create_dir_all(dir.as_ref())?;
        Ok(SkillStore {
            dir: dir.as_ref().to_path_buf(),
        })
    }

    /// The store's directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where skills live by default: `VOYAGER_SKILL_DIR`, else
    /// `~/.openagents/voyager/skills`.
    pub fn default_dir() -> PathBuf {
        if let Some(dir) = std::env::var_os("VOYAGER_SKILL_DIR") {
            return PathBuf::from(dir);
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        home.join(".openagents").join("voyager").join("skills")
    }

    /// The index, or an empty one when no skill has banked yet.
    fn index(&self) -> Result<Index> {
        let path = self.dir.join("index.json");
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| Error::episode(format!("{}: {error}", path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Index::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// One skill file by name and version — `None` reads the index's
    /// latest.
    fn skill_path(&self, name: &str, version: u32) -> PathBuf {
        self.dir.join(format!("{name}.v{version}.json"))
    }

    /// Banks a program: a new name writes version 1, a re-banked name
    /// writes one version up. The digest seals the record before it
    /// lands, and the index moves to the new version.
    ///
    /// # Errors
    ///
    /// The record must serialize and the file must write.
    pub fn add(&self, name: &str, description: &str, source: &str) -> Result<Skill> {
        if !valid_name(name) {
            return Err(Error::episode(format!(
                "skill name {name:?} is not a usable file stem"
            )));
        }
        let index = self.index()?;
        let version = index.skills.get(name).copied().unwrap_or(0) + 1;
        let mut skill = Skill {
            schema: SKILL_SCHEMA.to_string(),
            name: name.to_string(),
            version,
            description: description.to_string(),
            source: source.to_string(),
            digest: String::new(),
        };
        skill.digest = skill.digest()?;
        let path = self.skill_path(name, version);
        std::fs::write(&path, serde_json::to_vec_pretty(&skill)?)
            .map_err(|error| Error::episode(format!("{}: {error}", path.display())))?;
        let mut index = index;
        index.skills.insert(name.to_string(), version);
        std::fs::write(
            self.dir.join("index.json"),
            serde_json::to_vec_pretty(&index)?,
        )?;
        Ok(skill)
    }

    /// Reads one skill: an exact `name`, or `name@N` for a version
    /// other than the index's latest.
    ///
    /// # Errors
    ///
    /// The named skill must exist, parse, and digest-match — a record
    /// whose bytes moved under its name is tampering, not a skill.
    pub fn get(&self, name: &str) -> Result<Skill> {
        let (name, version) = match name.split_once('@') {
            Some((name, at)) => (
                name,
                at.parse::<u32>()
                    .map_err(|_| Error::episode(format!("skill version in {name:?}@{at:?}")))?,
            ),
            None => {
                let index = self.index()?;
                (
                    name,
                    index.skills.get(name).copied().ok_or_else(|| {
                        Error::episode(format!("no skill named {name:?} in {}", self.dir.display()))
                    })?,
                )
            }
        };
        let path = self.skill_path(name, version);
        let bytes = std::fs::read(&path)
            .map_err(|error| Error::episode(format!("{}: {error}", path.display())))?;
        let skill: Skill = serde_json::from_slice(&bytes)
            .map_err(|error| Error::episode(format!("{}: {error}", path.display())))?;
        if skill.schema != SKILL_SCHEMA {
            return Err(Error::episode(format!(
                "{}: schema {:?} is not {SKILL_SCHEMA:?}",
                path.display(),
                skill.schema
            )));
        }
        if skill.digest != skill.digest()? {
            return Err(Error::episode(format!(
                "{}: digest does not match the record",
                path.display()
            )));
        }
        Ok(skill)
    }

    /// Every skill the index names, latest versions, in name order —
    /// the candidate set a retrieval asks over.
    ///
    /// # Errors
    ///
    /// Every indexed skill must load and digest-match.
    pub fn list(&self) -> Result<Vec<Skill>> {
        let index = self.index()?;
        let mut out = Vec::new();
        for (name, version) in &index.skills {
            out.push(self.get(&format!("{name}@{version}"))?);
        }
        Ok(out)
    }

    /// Which of the banked skills fits `goal`, asked of the decision
    /// door: one `choice` question whose options are the candidates'
    /// names plus `none`. The exchange is recorded like every other
    /// decision the episode makes.
    ///
    /// # Errors
    ///
    /// The door must answer; a refusal is an error, and so is a pick
    /// outside the candidate set — the model chose nothing real.
    pub fn retrieve(&self, door: &mut Door, goal: &str, purpose: &str) -> Result<Option<Skill>> {
        let candidates = self.list()?;
        if candidates.is_empty() {
            return Ok(None);
        }
        let mut options: Vec<(String, String)> = candidates
            .iter()
            .map(|skill| {
                (
                    format!("{}@{}", skill.name, skill.version),
                    skill.description.clone(),
                )
            })
            .collect();
        options.push(("none".to_string(), "no banked skill fits".to_string()));
        let state = json!({
            "goal": goal,
            "candidates": candidates.iter().map(|skill| json!({
                "name": format!("{}@{}", skill.name, skill.version),
                "description": skill.description,
            })).collect::<Vec<_>>(),
        });
        let picked = door.choose(
            state,
            "Which banked skill best fits this goal? Answer none when nothing fits.",
            &options,
            purpose,
        )?;
        if picked.choice == "none" {
            return Ok(None);
        }
        self.get(&picked.choice).map(Some)
    }
}

/// A name a skill file can carry: word characters and dashes only —
/// the digest is the identity, but the file stem is how a person
/// finds it.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// The directories a skill lookup searches, in order:
/// `VOYAGER_SKILL_DIR`, the repository's `skills/`, then
/// `~/.openagents/voyager/skills`. The first hit answers.
pub fn lookup_dirs(repo: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = std::env::var_os("VOYAGER_SKILL_DIR") {
        dirs.push(PathBuf::from(dir));
    }
    dirs.push(repo.join("skills"));
    dirs.push(SkillStore::default_dir());
    dirs
}

/// Reads a skill by name across the lookup dirs — the same precedence
/// a world manifest uses for its own registry.
///
/// # Errors
///
/// A named skill must exist in one of the dirs; a malformed store is
/// an error, not a miss.
pub fn lookup(repo: &Path, name: &str) -> Result<Skill> {
    let mut misses = Vec::new();
    for dir in lookup_dirs(repo) {
        if !dir.is_dir() {
            continue;
        }
        match SkillStore::open(&dir).and_then(|store| store.get(name)) {
            Ok(skill) => return Ok(skill),
            Err(error) => misses.push(format!("{}: {error}", dir.display())),
        }
    }
    Err(Error::episode(format!(
        "no skill {name:?}; looked in {}",
        lookup_dirs(repo)
            .iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_banked_skill_reads_back_with_its_digest() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::open(dir.path()).unwrap();
        let skill = store
            .add(
                "gatherOakLogs",
                "dig nearby log blocks",
                "mine([\"oak_log\"], 3);",
            )
            .unwrap();
        assert_eq!(skill.version, 1);
        assert!(skill.digest.starts_with("sha256:"));

        let read = store.get("gatherOakLogs").unwrap();
        assert_eq!(read.source, "mine([\"oak_log\"], 3);");
        assert_eq!(read.digest, skill.digest);
    }

    #[test]
    fn rebanking_a_name_versions_up_and_keeps_both() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::open(dir.path()).unwrap();
        store
            .add("walkHome", "walk to camp", "walk(0, 0);")
            .unwrap();
        let v2 = store
            .add(
                "walkHome",
                "walk to camp then report",
                "walk(0, 0); say(\"home\");",
            )
            .unwrap();
        assert_eq!(v2.version, 2);
        assert_eq!(store.get("walkHome").unwrap().version, 2);
        assert_eq!(store.get("walkHome@1").unwrap().source, "walk(0, 0);");
        assert_eq!(store.list().unwrap().len(), 1);
    }

    #[test]
    fn a_moved_record_fails_its_digest() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::open(dir.path()).unwrap();
        store.add("probe", "look around", "state();").unwrap();
        let path = dir.path().join("probe.v1.json");
        let mut record: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        record["description"] = json!("say something else entirely");
        std::fs::write(&path, serde_json::to_vec_pretty(&record).unwrap()).unwrap();
        assert!(store.get("probe").is_err());
    }

    #[test]
    fn an_empty_store_retrieves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::open(dir.path()).unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn names_that_cannot_be_file_stems_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = SkillStore::open(dir.path()).unwrap();
        assert!(store.add("../escape", "x", "y").is_err());
        assert!(store.add("", "x", "y").is_err());
    }
}
