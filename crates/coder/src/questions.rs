//! Question sets: the wording a `decide` step names but does not carry.
//!
//! [NIP-PRG](../../../nips/openagents/NIP-PRG.md) is explicit that a
//! `decide` step carries a **question identifier** and never the
//! question's wording, because rewording a question changes what was
//! asked. A program that inlined its text could not say which version
//! produced a result, and two runs of "the same" program would not be
//! comparable.
//!
//! So the text lives here, in one file per set, addressed by identifier
//! and digested as a whole. [`crate::program`] refuses a `decide` step
//! that carries wording; this module is where the wording it refused is
//! supposed to be.
//!
//! # What a set may fill in at run time
//!
//! Two things, and nothing else:
//!
//! - A Choice question declaring `"options": "supplied"` gets its options
//!   from the run, **beside the ones it declares itself**. The
//!   program-selection question's options are the programs this host would
//!   admit, which is how an operator without an executor gets a shorter
//!   option set rather than a broken one, plus the `none` the file
//!   declares, which is the answer almost every turn has. An option whose
//!   wording is the same on every host belongs in the set, where it is
//!   digested with the rest of the wording; only the slugs and summaries
//!   the host resolved come from the run.
//! - A set declaring `per_requirement` is a template: the host makes one
//!   question per requirement and writes the requirement's name into the
//!   instructions, because a set of identical questions asked under
//!   different identifiers gives a model nothing to tell them apart with.
//!
//! Both are bounded fields — a slug and an identifier — chosen after the
//! question set was, which is the line `AGENTS.md` draws for deterministic
//! parsing.

use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use jev::{Question, Questions};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The question-set body version this reads.
pub const SET_VERSION: u32 = 1;

/// The variable that moves the question directory.
pub const DIR_ENV: &str = "CODER_QUESTION_DIR";

/// The value an `options` field takes when the host fills the options in.
const SUPPLIED: &str = "supplied";

/// What a per-requirement template writes the requirement's name into.
const REQUIREMENT: &str = "{requirement}";

/// One question set: the wording behind one identifier.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Set {
    pub v: u32,
    /// The identifier a `decide` step names.
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub summary: String,
    /// The question whose answer a `refuse_below` bound reads. A set with
    /// no gate answers nothing that can be gated.
    #[serde(default)]
    pub gate: String,
    /// The questions, as they go on the wire.
    #[serde(default)]
    pub questions: IndexMap<String, Value>,
    /// The template a `per_requirement` step asks once per requirement.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_requirement: Option<Value>,
}

/// What a run supplies to a set before it goes out.
#[derive(Clone, Debug)]
pub enum Fill {
    /// The set as it stands.
    None,
    /// The options a supplied Choice question offers, in order.
    Options(Vec<(String, String)>),
    /// The requirements a per-requirement set asks about, in order.
    Requirements(Vec<String>),
}

impl Set {
    /// Reads a question set from a local file.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the file is not a set this host asks:
    /// unreadable, unparseable, a `v` it does not know, an identifier
    /// outside the grammar, no questions at all, or a question naming no
    /// type.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let set: Self =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        set.validate()
            .map_err(|reason| format!("{}: {reason}", path.display()))?;
        Ok(set)
    }

    /// Whether this set is one this host asks.
    ///
    /// # Errors
    ///
    /// Returns the first reason it is not.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != SET_VERSION {
            return Err(format!(
                "body version is {}, this version reads {SET_VERSION}",
                self.v
            ));
        }
        if !is_question_id(&self.id) {
            return Err(format!("{:?} is not a question-set identifier", self.id));
        }
        if self.questions.is_empty() && self.per_requirement.is_none() {
            return Err("a question set with no questions asks nothing".to_string());
        }
        if !self.questions.is_empty() && self.per_requirement.is_some() {
            return Err(
                "a set is a fixed set or a per-requirement template, and this one is both"
                    .to_string(),
            );
        }
        let named: Vec<(&str, &Value)> = self
            .questions
            .iter()
            .map(|(id, question)| (id.as_str(), question))
            .chain(
                self.per_requirement
                    .iter()
                    .map(|template| (REQUIREMENT, template)),
            )
            .collect();
        for (id, question) in named {
            if question.get("type").and_then(Value::as_str).is_none() {
                return Err(format!("question {id:?} names no type"));
            }
            if let Some(options) = question.get("options").and_then(Value::as_str)
                && options != SUPPLIED
            {
                return Err(format!(
                    "question {id:?} asks for {options:?} options, and this host supplies only {SUPPLIED:?}"
                ));
            }
            if question
                .get("criteria")
                .is_some_and(|criteria| !criteria.is_object())
            {
                return Err(format!(
                    "question {id:?} declares criteria that are not an option set, and the run's options would replace them"
                ));
            }
        }
        if let Some(template) = &self.per_requirement
            && !instructions_of(template).contains(REQUIREMENT)
        {
            return Err(format!(
                "the per-requirement template writes no {REQUIREMENT}, so every requirement would be asked the same question under a different name"
            ));
        }
        Ok(())
    }

    /// Whether this set is asked once per requirement.
    #[must_use]
    pub fn templated(&self) -> bool {
        self.per_requirement.is_some()
    }

    /// Whether a question in this set takes its options from the run.
    #[must_use]
    pub fn supplies_options(&self) -> bool {
        self.questions
            .values()
            .any(|question| question.get("options").and_then(Value::as_str) == Some(SUPPLIED))
    }

    /// The questions one call asks.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the run and the set do not fit: a
    /// supplied Choice question with no options to offer, a template with
    /// no requirements, or either one given the other's fill.
    pub fn build(&self, fill: &Fill) -> Result<Questions, String> {
        let questions = match (&self.per_requirement, fill) {
            (Some(template), Fill::Requirements(requirements)) => {
                if requirements.is_empty() {
                    return Err(format!("{} has nothing to ask about", self.id));
                }
                requirements
                    .iter()
                    .map(|requirement| {
                        (
                            requirement.clone(),
                            Question::Raw(written(template, requirement)),
                        )
                    })
                    .collect()
            }
            (Some(_), _) => {
                return Err(format!(
                    "{} is asked once per requirement and this call named none",
                    self.id
                ));
            }
            (None, Fill::Requirements(_)) => {
                return Err(format!("{} is not asked per requirement", self.id));
            }
            (None, fill) => self
                .questions
                .iter()
                .map(|(id, question)| {
                    Ok((
                        id.clone(),
                        Question::Raw(filled(id, question, fill).map_err(|why| why.to_string())?),
                    ))
                })
                .collect::<Result<Questions, String>>()?,
        };
        questions
            .validate()
            .map_err(|error| format!("{}: {error}", self.id))?;
        Ok(questions)
    }

    /// The digest of the wording, as it stands on disk.
    ///
    /// The digest of what went on the wire is recorded by the decision
    /// call beside the answer. This one names the **set**, so two runs
    /// that asked from the same file say so even when the run filled
    /// different options in.
    #[must_use]
    pub fn digest(&self) -> String {
        let body = match &self.per_requirement {
            Some(template) => json!({ "per_requirement": template }),
            None => json!({ "questions": self.questions }),
        };
        atif::digest(&body)
    }

    /// What a host records about the wording it asked from, beside the
    /// answer.
    #[must_use]
    pub fn provenance(&self) -> Value {
        json!({
            "question_set": self.id,
            "set_digest": self.digest(),
            "gate": match self.gate.is_empty() {
                true => Value::Null,
                false => json!(self.gate),
            },
        })
    }
}

/// One file a host would not ask from, and why.
#[derive(Clone, Debug)]
pub struct Refused {
    pub source: String,
    pub reason: String,
}

/// The question sets a host has resolved, and the ones it refused.
#[derive(Clone, Debug, Default)]
pub struct Registry {
    sets: BTreeMap<String, Set>,
    refused: Vec<Refused>,
}

impl Registry {
    /// Reads every question set in one directory.
    ///
    /// # Errors
    ///
    /// Returns the underlying error when the directory cannot be read.
    pub fn read(dir: &Path) -> Result<Self, String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        let mut registry = Registry::default();
        for path in paths {
            match Set::load(&path) {
                Ok(set) => {
                    registry.sets.insert(set.id.clone(), set);
                }
                Err(reason) => registry.refused.push(Refused {
                    source: path.display().to_string(),
                    reason,
                }),
            }
        }
        Ok(registry)
    }

    /// Reads each directory in turn. The first definition of an identifier
    /// wins, so an operator's own directory overrides the repository's.
    #[must_use]
    pub fn open(dirs: &[PathBuf]) -> Self {
        let mut merged = Registry::default();
        for dir in dirs {
            let Ok(registry) = Registry::read(dir) else {
                continue;
            };
            for (id, set) in registry.sets {
                merged.sets.entry(id).or_insert(set);
            }
            merged.refused.extend(registry.refused);
        }
        merged
    }

    /// One set by identifier.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Set> {
        self.sets.get(id)
    }

    /// The identifiers, in order.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        self.sets.keys().cloned().collect()
    }

    /// The files this host would not ask from, each with its reason.
    #[must_use]
    pub fn refused(&self) -> &[Refused] {
        &self.refused
    }
}

/// Where a host looks for question sets, in order.
#[must_use]
pub fn search(repository: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(dir) = env::var_os(DIR_ENV).filter(|dir| !dir.is_empty()) {
        dirs.push(PathBuf::from(dir));
    }
    if let Some(root) = repository {
        dirs.push(root.join("questions"));
    }
    if let Some(home) = env::var_os("HOME").filter(|home| !home.is_empty()) {
        dirs.push(PathBuf::from(home).join(".openagents").join("questions"));
    }
    dirs
}

/// Whether a string is a question-set identifier.
///
/// The same grammar NIP-CAP gives a slug, with dots, because an identifier
/// is versioned in its name: `openagents.independence.v1`.
#[must_use]
pub fn is_question_id(id: &str) -> bool {
    let mut characters = id.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first.is_ascii_digit())
        && id.len() <= 64
        && characters.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
}

/// One question with whatever the run supplies written in.
///
/// The criteria the question declares stay, and the run's options join
/// them. A declared option is one whose wording is the same on every host
/// — `none` on the program-selection question — so it belongs in the set
/// and inside the set's digest rather than in whichever caller happened to
/// build the option list.
fn filled(id: &str, question: &Value, fill: &Fill) -> Result<Value, String> {
    let Some(SUPPLIED) = question.get("options").and_then(Value::as_str) else {
        return Ok(question.clone());
    };
    let Fill::Options(options) = fill else {
        return Err(format!(
            "question {id:?} takes its options from the run, and this call supplied none"
        ));
    };
    if options.is_empty() {
        return Err(format!("question {id:?} has no options to offer"));
    }
    let mut body = question.as_object().cloned().unwrap_or_default();
    body.remove("options");
    let mut criteria: Map<String, Value> = body
        .get("criteria")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (name, description) in options {
        criteria.insert(name.clone(), json!(description));
    }
    body.insert("criteria".to_string(), Value::Object(criteria));
    Ok(Value::Object(body))
}

/// One question of a per-requirement set, named for its requirement.
fn written(template: &Value, requirement: &str) -> Value {
    let mut body = template.as_object().cloned().unwrap_or_default();
    body.insert(
        "instructions".to_string(),
        json!(instructions_of(template).replace(REQUIREMENT, requirement)),
    );
    Value::Object(body)
}

/// A question's instructions as text, or the empty string when it carries
/// none in a shape this host reads.
fn instructions_of(question: &Value) -> String {
    question
        .get("instructions")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repository_questions() -> Registry {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../questions");
        Registry::read(&dir).expect("the repository carries a questions dir")
    }

    #[test]
    fn the_registry_holds_the_sets_the_first_program_names() {
        let registry = repository_questions();
        assert!(registry.refused().is_empty(), "{:?}", registry.refused());
        assert_eq!(
            registry.ids(),
            [
                "openagents.completion.v1",
                "openagents.independence.v1",
                "openagents.program.v1"
            ]
        );
    }

    #[test]
    fn a_fixed_set_asks_what_the_file_says() {
        let set = repository_questions()
            .get("openagents.independence.v1")
            .cloned()
            .unwrap();
        let questions = set.build(&Fill::None).unwrap();
        assert_eq!(questions.len(), 3);
        assert_eq!(set.gate, "independent");
        assert!(questions.get("independent").is_some());
        assert!(!set.digest().is_empty());
    }

    #[test]
    fn a_supplied_choice_takes_its_options_from_the_run() {
        let set = repository_questions()
            .get("openagents.program.v1")
            .cloned()
            .unwrap();
        assert!(set.supplies_options());
        let questions = set
            .build(&Fill::Options(vec![
                (
                    "delegate-fan-out".to_string(),
                    "Runs one per task.".to_string(),
                ),
                ("run-suite".to_string(), "Scores doors.".to_string()),
            ]))
            .unwrap();
        let Some(Question::Raw(body)) = questions.get("program") else {
            panic!("the program question is the one that was filled");
        };
        assert_eq!(
            body["criteria"]["delegate-fan-out"],
            json!("Runs one per task.")
        );
        assert!(
            body.get("options").is_none(),
            "the marker does not go on the wire"
        );
        assert!(
            body["criteria"]["none"].is_string(),
            "the option the set declares survives the run's fill: {body}"
        );

        // A run with nothing to offer asks nothing rather than asking an
        // empty choice, which a door cannot answer.
        assert!(set.build(&Fill::Options(Vec::new())).is_err());
        assert!(set.build(&Fill::None).is_err());
    }

    #[test]
    fn a_template_asks_once_per_requirement_and_names_each_one() {
        let set = repository_questions()
            .get("openagents.completion.v1")
            .cloned()
            .unwrap();
        assert!(set.templated());
        let questions = set
            .build(&Fill::Requirements(vec![
                "t1".to_string(),
                "t2".to_string(),
            ]))
            .unwrap();
        assert_eq!(questions.len(), 2);
        let Some(Question::Raw(body)) = questions.get("t2") else {
            panic!("one question per requirement, named for it");
        };
        assert!(
            body["instructions"].as_str().unwrap().contains("t2"),
            "a question that did not name its requirement would be its siblings' twin"
        );
        assert!(set.build(&Fill::Requirements(Vec::new())).is_err());
        assert!(set.build(&Fill::None).is_err());
    }

    #[test]
    fn a_template_that_names_no_requirement_is_refused() {
        let set: Set = serde_json::from_str(
            r#"{"v":1,"id":"openagents.same.v1","per_requirement":
                {"type":"noul","instructions":"It landed."}}"#,
        )
        .unwrap();
        let reason = set
            .validate()
            .expect_err("every question would be the same");
        assert!(reason.contains(REQUIREMENT), "{reason}");
    }

    #[test]
    fn a_set_this_host_does_not_read_is_refused() {
        for (body, expected) in [
            (
                r#"{"v":7,"id":"openagents.a.v1","questions":{"q":{"type":"noul"}}}"#,
                "version",
            ),
            (
                r#"{"v":1,"id":"Openagents.A","questions":{"q":{"type":"noul"}}}"#,
                "identifier",
            ),
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{}}"#,
                "asks nothing",
            ),
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{"q":{}}}"#,
                "names no type",
            ),
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{"q":{"type":"choice","options":"fetched"}}}"#,
                "supplies only",
            ),
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{"q":{"type":"choice","options":"supplied","criteria":["none"]}}}"#,
                "not an option set",
            ),
        ] {
            let set: Set = serde_json::from_str(body).unwrap();
            let reason = set.validate().expect_err(body);
            assert!(reason.contains(expected), "{body} said {reason:?}");
        }
    }
}
