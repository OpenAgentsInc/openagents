//! The question text, separated from the items it is asked about, and
//! carrying its own digest.
//!
//! An item is a state and a label. A *question set* is how that state gets
//! asked about: one question per family, in the shape the door reads. They
//! are two things and they vary independently, so they have two digests.
//!
//! # Why this is a third digest and not a fourth field
//!
//! [`crate::suite::Suite::compute_digest`] hashes the items, so before this
//! module existed, rewording a question changed the suite's digest. Every
//! row pins [`crate::row::Row::suite_digest`] and refuses to be compared
//! across it, which is the property that makes the record trustworthy: a
//! changed label is tampering, not drift.
//!
//! The consequence was that a reworded question was not a candidate against
//! a pinned suite — it was a different suite, and the store correctly refused
//! the comparison. That is right, and it also blocked the one experiment
//! `docs/text-optimization.md` records nine optimizer programs having been
//! built for and never run.
//!
//! [`crate::gate`] already solved the same shape of problem. A suite's digest
//! deliberately excludes its gate, so tightening a floor produces a new rule
//! rather than making historical runs read as drifted, and each row pins the
//! gate it was judged by. The question set follows that precedent exactly. A
//! run pins three digests:
//!
//! - the **suite**, which is what was asked about and what the answer is,
//! - the **question set**, which is how it was asked,
//! - the **gate**, which is what bar judged it.
//!
//! Two runs are a door comparison when the first two match. They are a
//! question-text comparison when the suite matches and the question set does
//! not. When the suite does not match, they are not a comparison at all, and
//! [`crate::store::admit_comparison`] says so.
//!
//! # What the digest covers
//!
//! [`QuestionSet::digest`] hashes the questions and nothing else. The id, the
//! suite the set was written for, and `$comment` are all outside it, for the
//! reason the suite's own name is outside its digest: the digest answers
//! "was the same text served?", and a renamed set served the same text.
//!
//! That buys one property worth having. A set derived from a suite that
//! carries its question text inline, through [`QuestionSet::authored`], has
//! the same digest as a committed file that spells out the same text. The
//! text has one digest however it reaches the door, so the committed
//! `support-v2-three-way-v1.json` and the text inside the committed suite are
//! checkably one thing rather than two spellings of one thing.
//!
//! # What a suite author writes
//!
//! An item carries `id`, `family`, `kind`, `state`, `truth`, and `partition`.
//! The question text goes in a file in `crates/gym/questions/`, once per
//! family, and the suite manifest names it in its `questions` field. The
//! older shape, where every item carries its own copy of its family's
//! question, still loads: `support-v2-three-way` is written that way, its
//! digest is `54fbf4137c…`, and nothing here moves it.
//!
//! What a suite may not do is mix the two. A per-item override of a
//! set-provided question is exactly the per-item question data this module
//! exists to remove, so [`crate::suite::Suite::load`] refuses it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::suite::{Item, Suite};

/// The schema every question-set file is tagged with.
pub const SCHEMA: &str = "openagents.gym.question_set.v1";

/// The environment variable that points at a directory of question sets.
pub const QUESTIONS_DIR_VAR: &str = "GYM_QUESTIONS_DIR";

/// What went wrong loading a question set or asking it for a question.
#[derive(Debug, thiserror::Error)]
pub enum QuestionError {
    /// The file could not be read.
    #[error("question set file {path}: {source}")]
    Read {
        /// The file that could not be read.
        path: PathBuf,
        /// The underlying error.
        source: std::io::Error,
    },
    /// The file is not a question set.
    #[error("question set file {path} is not a question set: {source}")]
    Parse {
        /// The file that failed to parse.
        path: PathBuf,
        /// The underlying error.
        source: serde_json::Error,
    },
    /// The document declares a schema this build does not read.
    #[error("question set {id} carries schema {found}, and this build reads {SCHEMA}")]
    Schema {
        /// The set that declared it.
        id: String,
        /// The schema the file declared.
        found: String,
    },
    /// The document parsed and says something impossible.
    #[error("question set {id}: {problem}")]
    Invalid {
        /// The set that declared it.
        id: String,
        /// What is wrong, and what to do about it.
        problem: String,
    },
    /// No file in the directory declares that id.
    #[error(
        "no question set with id {id} in {dir}; set {QUESTIONS_DIR_VAR} to the directory \
         holding the question sets"
    )]
    NotFound {
        /// The id that was asked for.
        id: String,
        /// Where it was looked for.
        dir: PathBuf,
    },
    /// The set has no question for a family the suite holds.
    #[error(
        "question set {id} has no question for the {family} family, so item {item} cannot be \
         asked; a set that covers part of a suite scores a door on a denominator nobody named"
    )]
    NotCovered {
        /// The set that was asked.
        id: String,
        /// The family it does not cover.
        family: String,
        /// The item that wanted it.
        item: String,
    },
    /// The items carry two spellings of one family's question.
    #[error(
        "the {family} family carries more than one question text across its items, so there is \
         no one text to lift out of them; give the suite a question set instead"
    )]
    Ambiguous {
        /// The family with more than one spelling.
        family: String,
    },
    /// The items carry no question text, so there is none to lift out.
    #[error(
        "the items of {suite} carry no question text, so the suite's own text is whatever its \
         `questions` field names; load that set rather than deriving one"
    )]
    NotAuthored {
        /// The suite that was asked.
        suite: String,
    },
}

/// One question per family, with its own digest.
///
/// As committed to `crates/gym/questions/`, or as derived from a suite whose
/// items carry their question text inline.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionSet {
    /// Rationale prose carried in the file. Outside the digest: commentary
    /// on a question is not the question, and editing it must not orphan the
    /// rows recorded under it.
    #[serde(default, rename = "$comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<Value>,
    /// The document schema.
    pub schema: String,
    /// The set's id, which is also its file name. Versioned, because
    /// rewording a question produces a new set rather than new history.
    pub id: String,
    /// The suite this set was written for, by name.
    ///
    /// Provenance for a reader, and not a check: a name is not a digest, and
    /// [`crate::suite::Suite`]'s own tests rename a suite without moving
    /// anything. What is checked is coverage, item by item, in
    /// [`QuestionSet::ask`].
    pub suite: String,
    /// The question each family is asked, in the shape the door reads.
    pub questions: BTreeMap<String, Value>,
}

impl QuestionSet {
    /// The set a suite's items carry inline, lifted out of them.
    ///
    /// This is the older suite shape read as the newer one. It is derived
    /// rather than loaded, so it is not run through [`QuestionSet::validate`]:
    /// what the items say is what was served, and a check here would be a
    /// second opinion about a digest that is already recorded.
    ///
    /// # Errors
    ///
    /// Returns [`QuestionError::NotAuthored`] when the items carry no
    /// question text, and [`QuestionError::Ambiguous`] when one family's
    /// items carry more than one spelling of it.
    pub fn authored(suite: &Suite) -> Result<Self, QuestionError> {
        let mut questions: BTreeMap<String, Value> = BTreeMap::new();
        for item in &suite.items {
            let Some(question) = &item.question else {
                continue;
            };
            match questions.get(&item.family) {
                Some(held) if held == question => {}
                Some(_) => {
                    return Err(QuestionError::Ambiguous {
                        family: item.family.clone(),
                    });
                }
                None => {
                    questions.insert(item.family.clone(), question.clone());
                }
            }
        }
        if questions.is_empty() {
            return Err(QuestionError::NotAuthored {
                suite: suite.name.clone(),
            });
        }
        Ok(Self {
            comment: None,
            schema: SCHEMA.to_string(),
            id: format!("{}-authored", suite.name),
            suite: suite.name.clone(),
            questions,
        })
    }

    /// Reads a question set from JSON.
    ///
    /// # Errors
    ///
    /// Returns [`QuestionError::Parse`] when the document is not a question
    /// set, and [`QuestionError::Schema`] or [`QuestionError::Invalid`] when
    /// it parses and says something this build cannot serve.
    pub fn from_json(source: &str, path: &Path) -> Result<Self, QuestionError> {
        let set: Self = serde_json::from_str(source).map_err(|source| QuestionError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
        set.validate()?;
        Ok(set)
    }

    /// Reads a question set from a file.
    ///
    /// The file name is the set's id, so a reword cannot be renamed without
    /// being renamed everywhere.
    ///
    /// # Errors
    ///
    /// Returns [`QuestionError::Read`] when the file cannot be read, and the
    /// errors [`QuestionSet::from_json`] returns otherwise.
    pub fn load(path: &Path) -> Result<Self, QuestionError> {
        let source = std::fs::read_to_string(path).map_err(|source| QuestionError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let set = Self::from_json(&source, path)?;
        let stem = path.file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
        if stem != set.id {
            return Err(QuestionError::Invalid {
                id: set.id.clone(),
                problem: format!("lives in {stem}.json; the file name is the question set id"),
            });
        }
        Ok(set)
    }

    /// Reads every question set in a directory, ordered by id.
    ///
    /// # Errors
    ///
    /// Returns [`QuestionError::Read`] when the directory cannot be listed,
    /// and the errors [`QuestionSet::load`] returns for any file in it. A set
    /// that does not load is an error rather than a skip, for the reason
    /// [`crate::gate::Gate::load_dir`] gives: text that quietly disappears is
    /// how a door gets asked a question nobody named.
    pub fn load_dir(dir: &Path) -> Result<Vec<Self>, QuestionError> {
        let entries = std::fs::read_dir(dir).map_err(|source| QuestionError::Read {
            path: dir.to_path_buf(),
            source,
        })?;
        let mut paths = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| QuestionError::Read {
                path: dir.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                paths.push(path);
            }
        }
        paths.sort();
        paths.iter().map(|path| Self::load(path)).collect()
    }

    /// Rejects a document that parsed and says something impossible.
    ///
    /// # Errors
    ///
    /// Returns [`QuestionError::Schema`] or [`QuestionError::Invalid`].
    pub fn validate(&self) -> Result<(), QuestionError> {
        if self.schema != SCHEMA {
            return Err(QuestionError::Schema {
                id: self.id.clone(),
                found: self.schema.clone(),
            });
        }
        for (field, value) in [("id", &self.id), ("suite", &self.suite)] {
            if value.trim().is_empty() {
                return Err(QuestionError::Invalid {
                    id: self.id.clone(),
                    problem: format!("has no {field}; a row that pins this text has to name it"),
                });
            }
        }
        if self.questions.is_empty() {
            return Err(QuestionError::Invalid {
                id: self.id.clone(),
                problem: "covers no family; an empty set asks nothing".into(),
            });
        }
        for (family, question) in &self.questions {
            let text = |key: &str| question.get(key).and_then(Value::as_str).unwrap_or_default();
            if !question.is_object() {
                return Err(QuestionError::Invalid {
                    id: self.id.clone(),
                    problem: format!("the {family} question is not an object"),
                });
            }
            for key in ["type", "instructions"] {
                if text(key).trim().is_empty() {
                    return Err(QuestionError::Invalid {
                        id: self.id.clone(),
                        problem: format!(
                            "the {family} question has no {key}; the text is the whole point of \
                             a question set, and an empty one asks nothing"
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    /// The digest over the questions, and nothing else.
    ///
    /// Object keys are sorted, so the digest does not move when a file
    /// reorders its fields. The id, the suite, and `$comment` are outside it:
    /// renaming a set leaves its digest alone, exactly as renaming a suite
    /// leaves the suite's alone, because both digests answer what was served
    /// rather than what it was called.
    #[must_use]
    pub fn digest(&self) -> String {
        let value = serde_json::to_value(&self.questions).unwrap_or(Value::Null);
        let mut hasher = Sha256::new();
        hasher.update(crate::suite::canonicalize(&value).as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// The families this set covers, in sorted order.
    #[must_use]
    pub fn families(&self) -> Vec<&str> {
        self.questions.keys().map(String::as_str).collect()
    }

    /// The question one item is asked.
    ///
    /// The set wins over any text the item carries inline. A suite is either
    /// written with its questions inline or written against a set, never
    /// both, and this is what makes a reword of the set a reword of the run.
    ///
    /// # Errors
    ///
    /// Returns [`QuestionError::NotCovered`] when the set holds no question
    /// for the item's family.
    pub fn ask(&self, item: &Item) -> Result<&Value, QuestionError> {
        self.questions
            .get(&item.family)
            .ok_or_else(|| QuestionError::NotCovered {
                id: self.id.clone(),
                family: item.family.clone(),
                item: item.id.clone(),
            })
    }

    /// Whether this set covers every family a suite holds.
    #[must_use]
    pub fn covers(&self, suite: &Suite) -> bool {
        suite
            .families()
            .iter()
            .all(|family| self.questions.contains_key(family))
    }
}

/// The directory the question sets live in.
///
/// Reads `GYM_QUESTIONS_DIR` when it is set, and falls back to the committed
/// `questions/` directory beside this crate. The path is a default, not the
/// rule: the text itself is loaded from those files rather than compiled in.
#[must_use]
pub fn questions_dir() -> PathBuf {
    std::env::var_os(QUESTIONS_DIR_VAR)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("questions"))
}

/// Loads one question set by id from [`questions_dir`].
///
/// # Errors
///
/// Returns [`QuestionError::NotFound`] when no file declares that id, and the
/// errors [`QuestionSet::load`] returns otherwise.
pub fn load(id: &str) -> Result<QuestionSet, QuestionError> {
    let dir = questions_dir();
    let path = dir.join(format!("{id}.json"));
    if !path.exists() {
        return Err(QuestionError::NotFound {
            id: id.to_string(),
            dir,
        });
    }
    QuestionSet::load(&path)
}

/// Loads every committed question set, ordered by id.
///
/// # Errors
///
/// Returns the errors [`QuestionSet::load_dir`] returns.
pub fn load_all() -> Result<Vec<QuestionSet>, QuestionError> {
    QuestionSet::load_dir(&questions_dir())
}

/// The set a run of this suite serves.
///
/// `named` overrides the suite's `questions` field, which overrides the text
/// the items carry inline. A suite that names no set and carries no inline
/// text has no question to ask, and that is an error rather than a default.
///
/// # Errors
///
/// Returns the errors [`load`] and [`QuestionSet::authored`] return, and
/// [`QuestionError::NotCovered`] when the resolved set misses a family the
/// suite holds.
pub fn resolve(suite: &Suite, named: Option<&str>) -> Result<QuestionSet, QuestionError> {
    let id = named.or(suite.questions.as_deref());
    let set = match id {
        Some(id) => load(id)?,
        None => QuestionSet::authored(suite)?,
    };
    for item in &suite.items {
        set.ask(item)?;
    }
    Ok(set)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::suite::{SUPPORT_V2_THREE_WAY, support_v2_three_way};
    use serde_json::json;

    fn suite() -> Suite {
        support_v2_three_way().expect("the committed suite loads")
    }

    fn written(dir: &Path, name: &str, set: &Value) -> PathBuf {
        let path = dir.join(format!("{name}.json"));
        std::fs::write(&path, serde_json::to_string_pretty(set).expect("a document"))
            .expect("the file is written");
        path
    }

    fn a_set() -> Value {
        json!({
            "schema": SCHEMA,
            "id": "routing-v2",
            "suite": "support-v2-three-way",
            "questions": {
                "routing": {
                    "type": "choice",
                    "instructions": "Which team should handle this message?",
                    "criteria": { "billing": "money", "technical": "bugs", "sales": "plans" },
                },
            },
        })
    }

    #[test]
    fn the_committed_set_is_the_text_the_suite_carries_inline() {
        // The one check that keeps a file and a suite from becoming two
        // spellings of one thing. It holds because the digest covers the
        // questions and not the name they are filed under.
        let suite = suite();
        let authored = QuestionSet::authored(&suite).expect("the items carry their text");
        let committed = load("support-v2-three-way-v1").expect("the committed set loads");
        assert_eq!(committed.questions, authored.questions);
        assert_eq!(committed.digest(), authored.digest());
        assert!(committed.covers(&suite));
        // And in the same order. The digest sorts keys, so it would hold
        // through a reordering; the door would not. Serving the options in
        // another order is a permutation, which `gym permute` measures as
        // its own axis and which must not arrive here by accident.
        assert_eq!(
            serde_json::to_string(&committed.questions).expect("the committed set renders"),
            serde_json::to_string(&authored.questions).expect("the authored set renders")
        );
    }

    #[test]
    fn the_committed_suite_names_the_committed_set() {
        let suite = suite();
        assert_eq!(suite.questions.as_deref(), Some("support-v2-three-way-v1"));
        let resolved = resolve(&suite, None).expect("the named set loads and covers the suite");
        assert_eq!(resolved.id, "support-v2-three-way-v1");
        assert_eq!(
            resolved.digest(),
            QuestionSet::authored(&suite).expect("authored").digest()
        );
    }

    #[test]
    fn the_authored_digest_is_recorded_literally() {
        // Recorded rather than recomputed, for the reason the suite's own
        // digest is: a test that derives the number it is checking from the
        // thing it is checking cannot catch the thing changing.
        assert_eq!(
            QuestionSet::authored(&suite()).expect("authored").digest(),
            "9745b1d9a0f3828888762a33ab36dc1b5093708b79febdd1ec7af2d7571ce0ec"
        );
    }

    /// The ids of the hand-written candidates the routing experiment scores.
    const CANDIDATES: [&str; 3] = [
        "support-v2-three-way-v2",
        "support-v2-three-way-v3",
        "support-v2-three-way-v4",
    ];

    #[test]
    fn the_routing_candidates_reword_one_family_and_freeze_the_answer_space() {
        // The experiment `docs/text-optimization.md` asks for. What makes a
        // reword a candidate rather than a second suite is that it changes
        // the text and nothing else, so this pins the nothing else: the two
        // untouched families word for word, the question type, and the
        // option names. An option name is answer-space identity, and a
        // candidate that hands back other keys is answering another
        // question rather than answering this one better.
        let suite = suite();
        let baseline = load("support-v2-three-way-v1").expect("the committed set loads");
        let options = |question: &Value| -> Vec<String> {
            question["criteria"]
                .as_object()
                .expect("a choice question names its options")
                .keys()
                .cloned()
                .collect()
        };
        for id in CANDIDATES {
            let candidate = load(id).expect("the candidate loads");
            assert!(candidate.covers(&suite), "{id} covers every family");
            assert_ne!(candidate.digest(), baseline.digest(), "{id} is a reword");
            for family in ["severity", "urgency"] {
                assert_eq!(
                    candidate.questions[family], baseline.questions[family],
                    "{id} leaves {family} word for word"
                );
            }
            let before = &baseline.questions["routing"];
            let after = &candidate.questions["routing"];
            assert_ne!(after, before, "{id} rewords routing");
            assert_eq!(after["type"], before["type"], "{id} keeps the question type");
            assert_eq!(options(after), options(before), "{id} freezes the option names");
        }
    }

    #[test]
    fn each_routing_candidate_is_its_own_text() {
        // Three candidates over two fields only say which field carries an
        // effect while the three are three. Two that collapsed to one text
        // would land in the store as one side and read as a repeat.
        let mut digests: Vec<String> = Vec::new();
        for id in CANDIDATES {
            let digest = load(id).expect("the candidate loads").digest();
            assert!(!digests.contains(&digest), "{id} repeats another candidate's text");
            digests.push(digest);
        }
    }

    #[test]
    fn rewording_a_question_moves_the_question_digest_and_nothing_else() {
        let suite = suite();
        let before = QuestionSet::authored(&suite).expect("authored");
        let mut after = before.clone();
        let routing = after.questions.get_mut("routing").expect("the routing question");
        routing["instructions"] = json!("Which team handles this?");

        assert_ne!(after.digest(), before.digest(), "the reword is a new set");
        assert_eq!(suite.digest, suite.compute_digest().expect("a digest"));
        assert_eq!(
            suite.digest,
            support_v2_three_way().expect("the suite").digest,
            "the items did not move"
        );
        assert_eq!(
            after.questions["urgency"], before.questions["urgency"],
            "one family's reword leaves the others alone"
        );
    }

    #[test]
    fn renaming_a_set_leaves_its_digest_alone() {
        let set = QuestionSet::authored(&suite()).expect("authored");
        let mut renamed = set.clone();
        renamed.id = "support-v2-three-way-v9".to_string();
        renamed.suite = "another-suite".to_string();
        renamed.comment = Some(json!("a note that is not the text"));
        assert_eq!(renamed.digest(), set.digest());
    }

    #[test]
    fn a_set_that_misses_a_family_is_refused_rather_than_asked() {
        let suite = suite();
        let mut partial = QuestionSet::authored(&suite).expect("authored");
        partial.questions.remove("severity");
        let severity = suite
            .items
            .iter()
            .find(|item| item.family == "severity")
            .expect("the suite holds severity items");
        assert!(matches!(
            partial.ask(severity),
            Err(QuestionError::NotCovered { .. })
        ));
        assert!(!partial.covers(&suite));
    }

    #[test]
    fn two_spellings_of_one_family_cannot_be_lifted_out_of_the_items() {
        let mut suite = suite();
        suite.items[0].question = Some(json!({
            "type": "choice",
            "instructions": "Which team handles this?",
            "criteria": { "billing": "money" },
        }));
        assert!(matches!(
            QuestionSet::authored(&suite),
            Err(QuestionError::Ambiguous { family }) if family == "routing"
        ));
    }

    #[test]
    fn a_suite_with_no_inline_text_has_no_set_to_derive() {
        let mut suite = suite();
        for item in &mut suite.items {
            item.question = None;
        }
        assert!(matches!(
            QuestionSet::authored(&suite),
            Err(QuestionError::NotAuthored { .. })
        ));
    }

    #[test]
    fn a_set_loads_from_a_file_named_after_it() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let path = written(dir.path(), "routing-v2", &a_set());
        let set = QuestionSet::load(&path).expect("the set loads");
        assert_eq!(set.id, "routing-v2");
        assert_eq!(set.families(), vec!["routing"]);

        let elsewhere = written(dir.path(), "routing-v3", &a_set());
        assert!(matches!(
            QuestionSet::load(&elsewhere),
            Err(QuestionError::Invalid { .. })
        ));
    }

    #[test]
    fn a_set_with_no_text_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut blank = a_set();
        blank["questions"]["routing"]["instructions"] = json!("   ");
        let path = written(dir.path(), "routing-v2", &blank);
        assert!(matches!(
            QuestionSet::load(&path),
            Err(QuestionError::Invalid { .. })
        ));
    }

    #[test]
    fn a_set_tagged_with_another_schema_is_refused() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let mut mistagged = a_set();
        mistagged["schema"] = json!("openagents.gym.question_set.v2");
        let path = written(dir.path(), "routing-v2", &mistagged);
        assert!(matches!(
            QuestionSet::load(&path),
            Err(QuestionError::Schema { .. })
        ));
    }

    #[test]
    fn a_variant_set_replaces_the_text_the_items_carry() {
        // The reword reaches the door. Before this module, it reached the
        // suite digest instead.
        let suite = suite();
        let mut variant = QuestionSet::authored(&suite).expect("authored");
        variant.id = "routing-v2".to_string();
        variant.questions.get_mut("routing").expect("routing")["instructions"] =
            json!("Which team handles this?");
        let routing = suite
            .items
            .iter()
            .find(|item| item.family == "routing")
            .expect("the suite holds routing items");
        assert_eq!(
            variant.ask(routing).expect("the variant covers routing")["instructions"],
            json!("Which team handles this?")
        );
        assert_eq!(
            routing.question.as_ref().expect("the item carries its text")["instructions"],
            json!("Which team should handle this message?"),
            "the item is unchanged, which is why the suite digest is"
        );
    }

    #[test]
    fn a_reworded_question_is_a_candidate_against_the_same_items() {
        // The whole issue, end to end, without a door. Two runs over the
        // same items and two question sets: the rows land in one store, the
        // chain verifies, and the store reads them as a question-text
        // comparison rather than refusing them as two suites.
        use crate::eval::{Disposition, Run};
        use crate::store::{Comparison, Store, admit_comparison};

        let suite = suite();
        let baseline = resolve(&suite, None).expect("the suite's own set");
        let mut variant = baseline.clone();
        variant.id = "support-v2-three-way-v2".to_string();
        variant.questions.get_mut("routing").expect("routing")["instructions"] =
            json!("Which team handles this?");

        let run = |set: &QuestionSet| Run {
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: Some(set.id.clone()),
            question_digest: Some(set.digest()),
            door: "lev-base".to_string(),
            door_identity: crate::row::DoorIdentity::published("lev-base", "sig:base-1", ""),
            estimator: "l2".to_string(),
            samples: Some(8),
            seed_base: Some(0),
            recorded_at: "2026-09-19T12:00:00Z".to_string(),
            gate_id: Some("probability-v1".to_string()),
            gate_digest: Some("gate:abc".to_string()),
        };

        let items: Vec<&Item> = suite
            .items
            .iter()
            .filter(|item| item.family == "routing")
            .take(4)
            .collect();
        let answered = |item: &Item| Disposition::Answered {
            chosen: item.truth.clone(),
            distribution: [(item.truth.clone(), 1.0)].into_iter().collect(),
        };

        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = Store::at(directory.path().join("rows.jsonl"));
        let scored = |set: &QuestionSet| -> Vec<serde_json::Value> {
            let run = run(set);
            items
                .iter()
                .map(|item| {
                    let row = run
                        .row(item, None, &answered(item), None)
                        .expect("an answered item produces a row");
                    store.append(&row).expect("the store takes the row")
                })
                .collect()
        };
        let before = scored(&baseline);
        let after = scored(&variant);

        assert_eq!(store.rows().expect("the rows read back").len(), 8);
        assert!(
            store.verified_rows().is_ok(),
            "the chain verifies across both runs"
        );
        assert_eq!(
            admit_comparison(&before, &after).expect("same items, two texts"),
            Comparison::QuestionText
        );

        // And the failure the record exists to prevent is still a failure.
        let mut elsewhere = suite.clone();
        elsewhere.items.truncate(190);
        elsewhere.digest = elsewhere.compute_digest().expect("a digest");
        let other_items = run(&baseline);
        let other = Run {
            suite_digest: elsewhere.digest.clone(),
            ..other_items
        };
        let rows: Vec<serde_json::Value> = items
            .iter()
            .map(|item| {
                serde_json::to_value(
                    other
                        .row(item, None, &answered(item), None)
                        .expect("a row"),
                )
                .expect("a row serializes")
            })
            .collect();
        assert!(admit_comparison(&before, &rows).is_err(), "different items");
    }

    #[test]
    fn the_committed_suite_file_still_carries_its_question_text() {
        // `support-v2-three-way` predates this module. Its text stays inline
        // and inside its digest; the committed set is the same text under a
        // name a row can pin.
        assert!(SUPPORT_V2_THREE_WAY.contains("Which team should handle this message?"));
    }
}
