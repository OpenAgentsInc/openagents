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

/// What a per-finding template writes the finding's name into.
const FINDING: &str = "{finding}";

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
    /// The template a `per_finding` step asks once per review finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_finding: Option<Value>,
    /// The workload policy the set binds: which models may answer it,
    /// how large a state it asks over, the policy revision a trace
    /// records, and the confidence under which it abstains. Absent is
    /// unbound — a set that declares no policy asks under whatever the
    /// host allows.
    #[serde(default, skip_serializing_if = "Policy::is_empty")]
    pub policy: Policy,
}

/// The workload policy a decision function binds.
///
/// A set's policy is a claim about itself the host enforces, the way a
/// program's bounds are a claim the host enforces: a state bigger than
/// `state_max_bytes`, a requested or answering model outside `models`,
/// and a gated confidence under `abstain_below` are each a refusal, not
/// a quieter answer. `v` is the policy revision — carried into every
/// recorded decision so two runs under different policies are never
/// read as the same measurement.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Policy {
    /// The policy revision. `0` declares no revision.
    #[serde(default)]
    pub v: u32,
    /// The model or artifact identities the function admits answers
    /// from, by the name the door requests and the answer reports.
    /// Empty admits any model.
    #[serde(default)]
    pub models: Vec<String>,
    /// The largest state, in bytes as serialized, the function asks
    /// over. A bigger state refuses rather than truncating.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_max_bytes: Option<u64>,
    /// The confidence under which a gated answer abstains: the gate's
    /// probability below the floor is the typed abstention the set
    /// declares, not a read a caller treats as an answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstain_below: Option<f64>,
    /// The evaluation and calibration evidence the function is bound
    /// to — the measurement references its claims rest on, recorded in
    /// the decision's provenance so a trace can say which evidence a
    /// run's answers were judged under. A set that names none is
    /// unmeasured, and the record says so by saying nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<String>,
}

impl Policy {
    /// Whether the policy declares nothing — serialized away so a set
    /// without one keeps the digest it has always had.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.v == 0
            && self.models.is_empty()
            && self.state_max_bytes.is_none()
            && self.abstain_below.is_none()
            && self.evidence.is_empty()
    }
}

/// Which template a set is, when it is one.
///
/// A `decide` step's `per_requirement` or `per_finding` bound has to match
/// the set's template: a fixed set cannot be asked per item, and a
/// templated set cannot be asked once. Both bounds on one step refuse at
/// admission rather than guessing which the step meant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    /// One question per requirement.
    Requirement,
    /// One question per review finding.
    Finding,
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
    /// The findings a per-finding set asks about, in order.
    Findings(Vec<String>),
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
        if self.questions.is_empty() && self.per_requirement.is_none() && self.per_finding.is_none()
        {
            return Err("a question set with no questions asks nothing".to_string());
        }
        if self.per_requirement.is_some() && self.per_finding.is_some() {
            return Err(
                "a set is asked once per requirement or once per finding, and this one claims both"
                    .to_string(),
            );
        }
        if let Some(floor) = self.policy.abstain_below
            && !(0.0..=1.0).contains(&floor)
        {
            return Err(format!("policy abstain_below is {floor}, outside 0 to 1"));
        }
        if self.policy.state_max_bytes == Some(0) {
            return Err("a state bound of zero asks over nothing".to_string());
        }
        if self.policy.models.iter().any(|model| model.is_empty()) {
            return Err("policy names an empty model identity".to_string());
        }
        if self
            .policy
            .evidence
            .iter()
            .any(|reference| reference.trim().is_empty())
        {
            return Err("policy names an empty evidence reference".to_string());
        }
        if !self.questions.is_empty() && self.template().is_some() {
            return Err("a set is a fixed set or a template, and this one is both".to_string());
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
            .chain(self.per_finding.iter().map(|template| (FINDING, template)))
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
        if let Some(template) = &self.per_finding
            && !instructions_of(template).contains(FINDING)
        {
            return Err(format!(
                "the per-finding template writes no {FINDING}, so every finding would be asked the same question under a different name"
            ));
        }
        Ok(())
    }

    /// Whether this set is asked once per requirement.
    #[must_use]
    pub fn templated(&self) -> bool {
        self.per_requirement.is_some()
    }

    /// Which template this set is, when it is one.
    #[must_use]
    pub fn template(&self) -> Option<Template> {
        match (&self.per_requirement, &self.per_finding) {
            (Some(_), _) => Some(Template::Requirement),
            (None, Some(_)) => Some(Template::Finding),
            (None, None) => None,
        }
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
        let questions = match (&self.per_requirement, &self.per_finding, fill) {
            (Some(template), _, Fill::Requirements(requirements)) => {
                if requirements.is_empty() {
                    return Err(format!("{} has nothing to ask about", self.id));
                }
                requirements
                    .iter()
                    .map(|requirement| {
                        (
                            requirement.clone(),
                            Question::Raw(written(template, REQUIREMENT, requirement)),
                        )
                    })
                    .collect()
            }
            (Some(_), _, _) => {
                return Err(format!(
                    "{} is asked once per requirement and this call named none",
                    self.id
                ));
            }
            (None, Some(template), Fill::Findings(findings)) => {
                if findings.is_empty() {
                    return Err(format!("{} has nothing to ask about", self.id));
                }
                findings
                    .iter()
                    .map(|finding| {
                        (
                            finding.clone(),
                            Question::Raw(written(template, FINDING, finding)),
                        )
                    })
                    .collect()
            }
            (None, Some(_), _) => {
                return Err(format!(
                    "{} is asked once per finding and this call named none",
                    self.id
                ));
            }
            (None, None, Fill::Requirements(_)) | (None, None, Fill::Findings(_)) => {
                return Err(format!("{} is not asked per item", self.id));
            }
            (None, None, fill) => self
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
        let body = match (&self.per_requirement, &self.per_finding) {
            (Some(template), _) => json!({ "per_requirement": template }),
            (None, Some(template)) => json!({ "per_finding": template }),
            (None, None) => json!({ "questions": self.questions }),
        };
        atif::digest(&body)
    }

    /// What a host records about the wording it asked from, beside the
    /// answer.
    #[must_use]
    pub fn provenance(&self) -> Value {
        record(&self.id, &self.gate, self.digest(), &self.policy)
    }
}

/// The record a trace carries about the wording a decision asked from:
/// the function's identity, the wording's digest, the gate that read
/// it, and the policy that bound it — its revision and the evidence
/// the function's claims rest on. One shape for a set the host read
/// from a file and a function it builds in code.
fn record(id: &str, gate: &str, digest: String, policy: &Policy) -> Value {
    let mut provenance = json!({
        "question_set": id,
        "set_digest": digest,
        "gate": match gate.is_empty() {
            true => Value::Null,
            false => json!(gate),
        },
        "policy_version": match policy.v {
            0 => Value::Null,
            v => json!(v),
        },
    });
    if !policy.evidence.is_empty() {
        provenance["evidence"] = json!(policy.evidence);
    }
    provenance
}

/// The same record for a function the host builds in code rather than
/// reads from a file — [`classify`](crate::classify)'s turn and round
/// questions. A code-built function carries no policy yet, so the
/// record names no revision; when one binds, it binds through `Set`'s
/// policy field and this record.
#[must_use]
pub fn function(id: &str, gate: &str, digest: String) -> Value {
    record(id, gate, digest, &Policy::default())
}

/// The digest of the wording a host asks from, for a function it
/// builds in code rather than reads from a file. The same
/// `atif::digest` path [`Set::digest`] takes, over the questions as
/// they stand — so a trace can tell two runs of the same wording from
/// a wording that changed between them.
#[must_use]
pub fn wording_digest(questions: &Questions) -> String {
    atif::digest(&json!({ "questions": questions }))
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

/// One question of a templated set, named for the item it asks about.
fn written(template: &Value, marker: &str, item: &str) -> Value {
    let mut body = template.as_object().cloned().unwrap_or_default();
    body.insert(
        "instructions".to_string(),
        json!(instructions_of(template).replace(marker, item)),
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
                "openagents.evidence-relevance.v1",
                "openagents.independence.v1",
                "openagents.independence.v2",
                "openagents.program.v1",
                "openagents.review-finding.v1"
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
    fn a_finding_template_asks_once_per_finding_and_names_each_one() {
        let set = repository_questions()
            .get("openagents.review-finding.v1")
            .cloned()
            .unwrap();
        assert_eq!(set.template(), Some(Template::Finding));
        assert!(!set.templated());
        let questions = set
            .build(&Fill::Findings(vec!["f1".to_string(), "f2".to_string()]))
            .unwrap();
        assert_eq!(questions.len(), 2);
        let Some(Question::Raw(body)) = questions.get("f2") else {
            panic!("one question per finding, named for it");
        };
        assert!(
            body["instructions"].as_str().unwrap().contains("f2"),
            "a question that did not name its finding would be its siblings' twin"
        );
        assert!(set.build(&Fill::Findings(Vec::new())).is_err());
        assert!(set.build(&Fill::None).is_err());
        assert!(set.build(&Fill::Requirements(vec!["t1".into()])).is_err());
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
    fn a_bound_set_names_its_policy_revision_in_what_a_trace_records() {
        let set: Set = serde_json::from_str(
            r#"{"v":1,"id":"openagents.bound.v1","gate":"q",
                "questions":{"q":{"type":"noul"}},
                "policy":{"v":3,"models":["kev-0.5b"],"abstain_below":0.6,
                    "evidence":["docs/coder/measurements/eval.md"]}}"#,
        )
        .unwrap();
        set.validate().unwrap();
        let provenance = set.provenance();
        assert_eq!(provenance["policy_version"], 3);
        assert_eq!(provenance["question_set"], "openagents.bound.v1");
        assert!(provenance["set_digest"].is_string());
        assert_eq!(
            provenance["evidence"],
            json!(["docs/coder/measurements/eval.md"]),
            "the evidence a function's claims rest on travels with its record"
        );

        // An unbound set records no revision — a trace cannot invent a
        // policy the wording never declared.
        let unbound: Set = serde_json::from_str(
            r#"{"v":1,"id":"openagents.unbound.v1","questions":{"q":{"type":"noul"}}}"#,
        )
        .unwrap();
        assert!(unbound.provenance()["policy_version"].is_null());
        assert!(
            unbound.provenance().get("evidence").is_none(),
            "an unmeasured function names no evidence"
        );
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
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{"q":{"type":"noul"}},"policy":{"abstain_below":1.5}}"#,
                "outside 0 to 1",
            ),
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{"q":{"type":"noul"}},"policy":{"state_max_bytes":0}}"#,
                "zero",
            ),
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{"q":{"type":"noul"}},"policy":{"models":[""]}}"#,
                "empty model identity",
            ),
            (
                r#"{"v":1,"id":"openagents.a.v1","questions":{"q":{"type":"noul"}},"policy":{"evidence":["  "]}}"#,
                "empty evidence reference",
            ),
        ] {
            let set: Set = serde_json::from_str(body).unwrap();
            let reason = set.validate().expect_err(body);
            assert!(reason.contains(expected), "{body} said {reason:?}");
        }
    }
}
