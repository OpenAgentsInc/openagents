//! The append-only result store and its receipt chain.
//!
//! A run writes one row per scored item, and the rows land in a file that is
//! only ever appended to. Every row carries a receipt over each of its other
//! fields and names the receipt of the row before it, so the file reads back
//! as a chain that anyone can check without the run that wrote it. A worse
//! result cannot be quietly removed and a better one cannot be quietly
//! inserted: both break the chain, and they break it in different ways.
//!
//! The two ways are the point. An *edit* to a row that is still in place
//! shows up as a row whose contents no longer digest to the receipt it
//! carries. An *insert, removal, or reorder* shows up as a row whose
//! `previous_receipt` no longer names the row in front of it. Those are two
//! kinds of dishonesty, so [`verify_chain`] reports them as two faults with
//! two messages rather than one "the chain is broken".
//!
//! The design is carried from the coding-agent Gym in `~/work/coder` and
//! reimplemented here.
//!
//! # What a row has to be
//!
//! [`row`](crate::row) is being written alongside this module, so the store
//! does not name its type. It works on any row that can produce its canonical
//! field set, which [`ChainRow`] states as a single method. A blanket
//! implementation covers everything that implements `serde::Serialize` and
//! serializes to a JSON object, so a row type satisfies the trait by deriving
//! `Serialize` and never has to know that the store exists. Rows read back
//! from a file are plain `serde_json::Value` objects and take the same path,
//! which means verification runs on what is on disk rather than on what a
//! struct would have produced.
//!
//! For the same reason [`KNOWN_ROW_SCHEMAS`] lists schema strings rather than
//! types. A store holds what earlier versions of this program wrote, and a
//! reader has to be able to refuse a row it does not understand without
//! linking against the code that wrote it. The list names
//! [`crate::row::SCHEMA`] rather than restating it, because two spellings of
//! one schema is the same ambiguity the rest of this crate exists to remove.
//!
//! # What the rows may be read as
//!
//! The chain says the file was not edited. It does not say that two sets of
//! rows in it are a comparison, and they often are not: rows pin the digest
//! of what they were scored on, and a number taken across two suites is a
//! number about neither. [`admit_comparison`] is that rule, and it answers
//! with what the two sides are rather than only whether they pass — same
//! items and same question text is a door comparison, same items and
//! different text is a question-text comparison, and different items is not
//! a comparison at all. [`crate::questions`] explains why the second of
//! those had to become expressible.
//!
//! [`Comparison`] here is what two sets of rows are. [`crate::gate::Comparison`]
//! is the scores of one, ready to be judged. They meet in the `gym compare`
//! command, which asks this module what it is holding before it asks a gate
//! what to think of it.
//!
//! # One writer at a time
//!
//! An append reads the head and then writes, so two writers running that
//! sequence at once would produce two rows claiming the same predecessor and
//! a file that no longer verifies. The store is single-writer and enforces
//! it with an exclusive lock file beside the store. A second writer is
//! refused rather than queued; [`StoreError::is_locked`] tells a caller that
//! retrying is the right response.

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs;
use std::io::{ErrorKind, Write as _};
use std::path::{Path, PathBuf};

/// The schemas this store reads. A row carrying anything else is a hard
/// error, because a reader that skips what it does not understand reports a
/// denominator it cannot account for.
///
/// The entries are schema strings, not types. A store holds what earlier
/// versions of this program wrote, and this list is what lets a reader refuse
/// a row it does not understand without linking against the code that wrote
/// it. `crate::row::SCHEMA` is a `&str` for the same reason, and naming it
/// here keeps one declaration of the string while leaving the allowlist a
/// list of names.
pub const KNOWN_ROW_SCHEMAS: &[&str] = &[crate::row::SCHEMA];

/// The field holding a row's own receipt.
pub const RECEIPT_FIELD: &str = "receipt";

/// The field holding the receipt of the row before it.
pub const PREVIOUS_RECEIPT_FIELD: &str = "previous_receipt";

/// The fields that together identify one trial. Re-scoring a run does not
/// make it a second run, so a second row with the same values for all seven
/// is refused. This matters more here than it does in the reference
/// implementation: the doors the Gym scores are near-deterministic, so
/// without the rule, running the same command ten times yields ten identical
/// rows that read as ten trials.
///
/// `question_digest` is one of them because the question text is a
/// perturbation axis like the option order: the same items asked a reworded
/// question are a different trial, not a repeat of the last one. A row that
/// does not carry the field reads as null, so the rows written before
/// [`crate::questions`] existed keep the key they always had relative to
/// each other.
pub const PERTURBATION_KEY_FIELDS: [&str; 7] = [
    "suite_digest",
    "question_digest",
    "door_identity",
    "estimator",
    "seed_base",
    "permutation",
    "item_id",
];

/// A row the store can chain: anything that can produce its canonical field
/// set as a JSON object.
pub trait ChainRow {
    /// The row's top-level fields, in the order the row defines them. Key
    /// order does not affect a receipt, which sorts keys, but it does decide
    /// how the row reads on disk.
    fn fields(&self) -> Result<Map<String, Value>, StoreError>;
}

impl<T: Serialize> ChainRow for T {
    fn fields(&self) -> Result<Map<String, Value>, StoreError> {
        let value = serde_json::to_value(self).map_err(|e| StoreError::NotARow {
            detail: format!("it did not serialize: {e}"),
        })?;
        match value {
            Value::Object(fields) => Ok(fields),
            other => Err(StoreError::NotARow {
                detail: format!("it serialized to {}", type_name_of(&other)),
            }),
        }
    }
}

/// What went wrong with a chain. The two faults are different accusations,
/// so they are different values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainFault {
    /// A row's contents no longer digest to the receipt it carries, so the
    /// row was edited after it was written.
    Edited,
    /// A row's `previous_receipt` does not name the row in front of it, so
    /// rows were inserted, removed, or reordered.
    Resequenced,
}

impl ChainFault {
    /// A short name for the fault, for a log line or a table cell.
    pub fn as_str(self) -> &'static str {
        match self {
            ChainFault::Edited => "edited",
            ChainFault::Resequenced => "resequenced",
        }
    }
}

/// The result of reading a chain end to end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainVerdict {
    /// Every row verifies. `head` is the receipt the next row must name, and
    /// is `None` for an empty store.
    Ok { rows: usize, head: Option<String> },
    /// The chain breaks at `index`, for the reason in `fault`.
    Broken {
        index: usize,
        fault: ChainFault,
        detail: String,
    },
}

/// Everything the store refuses to do, and why.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("could not read {path}: {detail}")]
    Read { path: String, detail: String },

    #[error("could not write {path}: {detail}")]
    Write { path: String, detail: String },

    #[error("{path} line {line} is not JSON: {detail}")]
    NotJson {
        path: String,
        line: usize,
        detail: String,
    },

    #[error("{path} line {line} is {found}, and every row is a JSON object")]
    NotAnObject {
        path: String,
        line: usize,
        found: String,
    },

    #[error(
        "{path} line {line} has schema {schema}, and this store reads {known}. \
         Move the file to an archive and start a new store rather than reading \
         a row whose fields you cannot account for"
    )]
    UnknownSchema {
        path: String,
        line: usize,
        schema: String,
        known: String,
    },

    #[error("broken chain in {path}: {detail}")]
    BrokenChain {
        path: String,
        index: usize,
        fault: ChainFault,
        detail: String,
    },

    #[error(
        "the row follows {follows} but the store's head is {head}, so it does \
         not extend the chain"
    )]
    DoesNotExtend { follows: String, head: String },

    #[error(
        "the row carries {carried} but its contents digest to {computed}, so it \
         was not sealed against the fields it holds"
    )]
    ReceiptMismatch { carried: String, computed: String },

    #[error(
        "row {index} already records this perturbation ({detail}), and \
         re-scoring a run does not make it a second run"
    )]
    DuplicatePerturbation { index: usize, detail: String },

    #[error(
        "another writer holds {lock}{holder}. The store takes one writer at a \
         time: wait for that writer to finish, or remove the lock file if no \
         writer is running"
    )]
    Locked { lock: String, holder: String },

    #[error(
        "{path} does not end with a newline, so appending would join two rows. \
         An earlier write was interrupted; repair the last line before writing \
         again"
    )]
    Truncated { path: String },

    #[error("a row must serialize to a JSON object, but {detail}")]
    NotARow { detail: String },

    #[error("these rows are not a comparison: {detail}")]
    NotComparable { detail: String },
}

impl StoreError {
    /// Whether another writer holds the store. A caller that meets this can
    /// retry; every other error means the append was wrong, not early.
    pub fn is_locked(&self) -> bool {
        matches!(self, StoreError::Locked { .. })
    }
}

/// An append-only result file.
#[derive(Debug, Clone)]
pub struct Store {
    path: PathBuf,
}

impl Store {
    /// The store at `path`. Nothing is read or created until you ask for it.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Store { path: path.into() }
    }

    /// Where the rows live.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Every row in the store, in file order, with the schema allowlist
    /// applied. A store that does not exist yet reads as no rows.
    pub fn rows(&self) -> Result<Vec<Value>, StoreError> {
        read_rows(&self.path)
    }

    /// Every row, with the chain verified. A broken chain is an error rather
    /// than a verdict here, because a caller reading rows to compute a number
    /// has no business continuing.
    pub fn verified_rows(&self) -> Result<Vec<Value>, StoreError> {
        let rows = self.rows()?;
        match verify_chain(&rows) {
            ChainVerdict::Ok { .. } => Ok(rows),
            ChainVerdict::Broken {
                index,
                fault,
                detail,
            } => Err(StoreError::BrokenChain {
                path: self.path.display().to_string(),
                index,
                fault,
                detail,
            }),
        }
    }

    /// The receipt at the head of the store, with the chain verified: what
    /// the next row's `previous_receipt` must name.
    pub fn head(&self) -> Result<Option<String>, StoreError> {
        let rows = self.verified_rows()?;
        Ok(head_of(&rows))
    }

    /// Seal `row` against the store's head and append it.
    ///
    /// Sealing happens under the writer lock, so a row cannot be sealed
    /// against a head that another writer has already moved. Any
    /// `previous_receipt` or `receipt` the row arrives with is replaced;
    /// those two fields belong to the store. The sealed row comes back,
    /// because that, and not what you passed in, is what is on disk.
    pub fn append<R: ChainRow>(&self, row: &R) -> Result<Value, StoreError> {
        let fields = row.fields()?;
        let lock = WriteLock::acquire(&self.path)?;
        let rows = self.verified_rows()?;
        let sealed = seal_fields(fields, head_of(&rows).as_deref());
        refuse_duplicate(&rows, &sealed)?;
        self.write_line(&sealed)?;
        drop(lock);
        Ok(sealed)
    }

    /// Append a row that is already sealed.
    ///
    /// Use this when the row was sealed elsewhere, such as by a run that
    /// built its whole chain before writing any of it. The row is refused,
    /// before the file is touched, when its receipt does not match its
    /// contents, when it does not extend the store's head, or when it repeats
    /// a perturbation the store already records.
    pub fn append_sealed<R: ChainRow>(&self, row: &R) -> Result<(), StoreError> {
        let sealed = Value::Object(row.fields()?);
        let lock = WriteLock::acquire(&self.path)?;
        let rows = self.verified_rows()?;

        let carried = sealed
            .get(RECEIPT_FIELD)
            .and_then(Value::as_str)
            .unwrap_or("");
        let computed = receipt_of(&sealed);
        if carried != computed {
            return Err(StoreError::ReceiptMismatch {
                carried: if carried.is_empty() {
                    "no receipt".to_string()
                } else {
                    carried.to_string()
                },
                computed,
            });
        }

        let follows = previous_receipt_of(&sealed);
        let head = head_of(&rows);
        if follows != head {
            return Err(StoreError::DoesNotExtend {
                follows: name_or_null(follows.as_deref()),
                head: name_or_null(head.as_deref()),
            });
        }

        refuse_duplicate(&rows, &sealed)?;
        self.write_line(&sealed)?;
        drop(lock);
        Ok(())
    }

    /// Write one row, with the lock already held.
    fn write_line(&self, row: &Value) -> Result<(), StoreError> {
        self.refuse_truncated()?;
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|e| StoreError::Write {
                path: parent.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        let mut line = serde_json::to_string(row).map_err(|e| StoreError::Write {
            path: self.path.display().to_string(),
            detail: e.to_string(),
        })?;
        line.push('\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| StoreError::Write {
                path: self.path.display().to_string(),
                detail: e.to_string(),
            })?;
        file.write_all(line.as_bytes())
            .map_err(|e| StoreError::Write {
                path: self.path.display().to_string(),
                detail: e.to_string(),
            })
    }

    /// Refuse to append to a file whose last write was cut off, because the
    /// new row would land on the end of the old one.
    fn refuse_truncated(&self) -> Result<(), StoreError> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(StoreError::Read {
                    path: self.path.display().to_string(),
                    detail: e.to_string(),
                });
            }
        };
        if bytes.is_empty() || bytes.ends_with(b"\n") {
            Ok(())
        } else {
            Err(StoreError::Truncated {
                path: self.path.display().to_string(),
            })
        }
    }
}

/// Read every row in a file, with the schema allowlist applied. A file that
/// does not exist reads as no rows; a row whose schema the store does not
/// know is an error naming what to do about it, never a silent skip.
pub fn read_rows(path: &Path) -> Result<Vec<Value>, StoreError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(StoreError::Read {
                path: path.display().to_string(),
                detail: e.to_string(),
            });
        }
    };
    let shown = path.display().to_string();
    let mut rows = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(trimmed).map_err(|e| StoreError::NotJson {
            path: shown.clone(),
            line,
            detail: e.to_string(),
        })?;
        if !value.is_object() {
            return Err(StoreError::NotAnObject {
                path: shown.clone(),
                line,
                found: type_name_of(&value).to_string(),
            });
        }
        let schema = value.get("schema").and_then(Value::as_str).unwrap_or("");
        if !known_row_schema(schema) {
            return Err(StoreError::UnknownSchema {
                path: shown.clone(),
                line,
                schema: if schema.is_empty() {
                    "no schema".to_string()
                } else {
                    schema.to_string()
                },
                known: KNOWN_ROW_SCHEMAS.join(", "),
            });
        }
        rows.push(value);
    }
    Ok(rows)
}

/// Whether the store reads rows carrying `schema`.
pub fn known_row_schema(schema: &str) -> bool {
    KNOWN_ROW_SCHEMAS.contains(&schema)
}

/// Check a chain end to end, reporting the first row that does not hold.
pub fn verify_chain(rows: &[Value]) -> ChainVerdict {
    let mut previous: Option<String> = None;
    for (index, row) in rows.iter().enumerate() {
        let carried = row.get(RECEIPT_FIELD).and_then(Value::as_str);
        let computed = receipt_of(row);
        if carried != Some(computed.as_str()) {
            return ChainVerdict::Broken {
                index,
                fault: ChainFault::Edited,
                detail: format!(
                    "row {index} ({}) carries {} but its contents digest to {computed}, \
                     so the row was edited after it was written",
                    describe_row(row),
                    carried.unwrap_or("no receipt"),
                ),
            };
        }
        let follows = previous_receipt_of(row);
        if follows != previous {
            return ChainVerdict::Broken {
                index,
                fault: ChainFault::Resequenced,
                detail: format!(
                    "row {index} ({}) follows {} but the row before it is {}, so rows \
                     were inserted, removed, or reordered",
                    describe_row(row),
                    name_or_null(follows.as_deref()),
                    name_or_null(previous.as_deref()),
                ),
            };
        }
        previous = Some(computed);
    }
    ChainVerdict::Ok {
        rows: rows.len(),
        head: previous,
    }
}

/// Seal a row against `previous`: put `previous_receipt` on it, then take the
/// receipt over everything else.
pub fn seal<R: ChainRow>(row: &R, previous: Option<&str>) -> Result<Value, StoreError> {
    Ok(seal_fields(row.fields()?, previous))
}

/// The receipt over a row: every top-level field except `receipt`, keys
/// sorted in ASCII order, serialized as a JSON array of `[key, value]` pairs,
/// then SHA-256. `previous_receipt` is an ordinary field, which is to say it
/// sits *under* the receipt, and that is what chains the rows together.
pub fn receipt_of(row: &Value) -> String {
    let Some(fields) = row.as_object() else {
        return "receipt:".to_string();
    };
    let mut keys: Vec<&String> = fields.keys().filter(|key| *key != RECEIPT_FIELD).collect();
    keys.sort();
    let pairs: Vec<Value> = keys
        .into_iter()
        .map(|key| {
            Value::Array(vec![
                Value::String(key.clone()),
                fields.get(key).cloned().unwrap_or(Value::Null),
            ])
        })
        .collect();
    let source = serde_json::to_string(&Value::Array(pairs)).unwrap_or_default();
    format!("receipt:{}", hex_digest(source.as_bytes()))
}

/// The key that identifies one trial, over [`PERTURBATION_KEY_FIELDS`]. A
/// field the row does not carry reads as null, so a row that names none of
/// them is a trial of nothing in particular, and only one of those fits in a
/// store.
pub fn perturbation_key(row: &Value) -> String {
    let values: Vec<Value> = PERTURBATION_KEY_FIELDS
        .iter()
        .map(|field| canonical(row.get(*field).unwrap_or(&Value::Null)))
        .collect();
    let source = serde_json::to_string(&Value::Array(values)).unwrap_or_default();
    format!("perturbation:{}", hex_digest(source.as_bytes()))
}

/// What two sets of rows may be read as, when they may be read as anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Comparison {
    /// Same items, same question text. Whatever differs between the two
    /// sides — the door, the adapter, the estimator — it is not what was
    /// asked.
    Doors,
    /// Same items, different question text. A reworded question against the
    /// items it left alone, which is a candidate rather than a second suite.
    QuestionText,
}

impl Comparison {
    /// A short name for the comparison, for a report line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Doors => "doors",
            Self::QuestionText => "question text",
        }
    }
}

/// Whether two sets of rows are a comparison, and which one.
///
/// This is the rule the whole record is built to hold. Rows pin the digest of
/// what they were scored on, and a comparison across a changed suite is not a
/// worse result — it is a number about two different things, reported as one.
/// So it is refused rather than reported as drift, exactly as
/// [`crate::suite::Suite::load`] refuses a tampered suite.
///
/// The question set is what makes that refusal survivable. Before
/// [`crate::questions`], rewording a question changed the suite digest, so
/// the one experiment `docs/text-optimization.md` asks for landed on the
/// wrong side of this rule. A reword now changes `question_digest` and
/// nothing else, and the two sides come back as [`Comparison::QuestionText`].
///
/// A side whose rows disagree with each other is refused before the two sides
/// are compared at all: a side that scored two suites is not one side.
///
/// # Errors
///
/// Returns [`StoreError::NotComparable`] when a side is empty, when a side
/// disagrees with itself, when the two sides pin different suite digests,
/// when either side names no suite, or when one side records which question
/// set it served and the other does not. The last is the `unknown is never
/// zero` rule applied to text: an unrecorded question set is not the authored
/// one.
pub fn admit_comparison(left: &[Value], right: &[Value]) -> Result<Comparison, StoreError> {
    let left_suite = agreed(left, "suite_digest", "first")?;
    let right_suite = agreed(right, "suite_digest", "second")?;
    let (Some(left_suite), Some(right_suite)) = (left_suite, right_suite) else {
        return Err(StoreError::NotComparable {
            detail: "a side names no suite, so it names no items either".to_string(),
        });
    };
    if left_suite != right_suite {
        return Err(StoreError::NotComparable {
            detail: format!(
                "the first side scored suite {} and the second scored {}, so the two sides did \
                 not score the same items. A suite digest covers every label and every \
                 partition assignment; two digests are two experiments, and one number over \
                 both of them is a number about neither",
                shorten_digest(left_suite),
                shorten_digest(right_suite),
            ),
        });
    }
    let left_questions = agreed(left, "question_digest", "first")?;
    let right_questions = agreed(right, "question_digest", "second")?;
    match (left_questions, right_questions) {
        (Some(left), Some(right)) if left != right => Ok(Comparison::QuestionText),
        (Some(_), Some(_)) | (None, None) => Ok(Comparison::Doors),
        _ => Err(StoreError::NotComparable {
            detail: "one side records which question set it served and the other does not, so \
                     there is no saying whether the two were asked the same thing; an \
                     unrecorded question set is not the authored one"
                .to_string(),
        }),
    }
}

/// One side's value for a field, when the side agrees with itself.
fn agreed<'a>(rows: &'a [Value], field: &str, side: &str) -> Result<Option<&'a str>, StoreError> {
    let mut seen: Option<Option<&str>> = None;
    for row in rows {
        let value = row.get(field).and_then(Value::as_str);
        match seen {
            None => seen = Some(value),
            Some(held) if held == value => {}
            Some(held) => {
                return Err(StoreError::NotComparable {
                    detail: format!(
                        "the {side} side holds rows with {field} {} and rows with {}, so it is \
                         not one side",
                        name_or_unrecorded(held),
                        name_or_unrecorded(value),
                    ),
                });
            }
        }
    }
    seen.ok_or_else(|| StoreError::NotComparable {
        detail: format!("the {side} side holds no rows"),
    })
}

fn name_or_unrecorded(value: Option<&str>) -> String {
    value.map_or_else(|| "unrecorded".to_string(), shorten_digest)
}

/// Enough of a digest to tell two of them apart in a message.
fn shorten_digest(digest: &str) -> String {
    let head: String = digest.chars().take(16).collect();
    if head.len() < digest.len() {
        format!("{head}\u{2026}")
    } else {
        head
    }
}

/// The receipt the next row must name, for rows that have already verified.
fn head_of(rows: &[Value]) -> Option<String> {
    rows.last()
        .and_then(|row| row.get(RECEIPT_FIELD))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn previous_receipt_of(row: &Value) -> Option<String> {
    row.get(PREVIOUS_RECEIPT_FIELD)
        .filter(|value| !value.is_null())
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn seal_fields(mut fields: Map<String, Value>, previous: Option<&str>) -> Value {
    fields.remove(RECEIPT_FIELD);
    fields.insert(
        PREVIOUS_RECEIPT_FIELD.to_string(),
        match previous {
            Some(receipt) => Value::String(receipt.to_string()),
            None => Value::Null,
        },
    );
    let mut fields = as_read_back(fields);
    let unsealed = Value::Object(fields.clone());
    fields.insert(
        RECEIPT_FIELD.to_string(),
        Value::String(receipt_of(&unsealed)),
    );
    Value::Object(fields)
}

/// A row's fields as a reader will parse them back.
///
/// A receipt is a promise about what is in the file, so it is taken over what
/// comes back out of the file rather than over what the writer was holding.
/// The two are not always the same number. `serde_json` writes an `f64`
/// exactly and parses one approximately, so a latency measured as
/// 1474.8615419999999 milliseconds is written with all of those digits and
/// read back as 1474.861542 — a different double, a different digest, and a
/// chain that breaks on the first verification with nobody having touched the
/// file.
///
/// That is a real failure and it happened on the first live run to record a
/// measured float. Normalizing here costs one round trip per append and makes
/// the chain a property of the file rather than of the process that wrote it.
/// A row is sealed over the value it will read back as; where that differs
/// from the value in memory, the file and the receipt agree with each other
/// and the last digit of a microsecond is the price.
fn as_read_back(fields: Map<String, Value>) -> Map<String, Value> {
    let Ok(rendered) = serde_json::to_string(&Value::Object(fields.clone())) else {
        return fields;
    };
    match serde_json::from_str::<Value>(&rendered) {
        Ok(Value::Object(parsed)) => parsed,
        _ => fields,
    }
}

fn refuse_duplicate(rows: &[Value], candidate: &Value) -> Result<(), StoreError> {
    let key = perturbation_key(candidate);
    for (index, row) in rows.iter().enumerate() {
        if perturbation_key(row) == key {
            return Err(StoreError::DuplicatePerturbation {
                index,
                detail: describe_perturbation(candidate),
            });
        }
    }
    Ok(())
}

/// The perturbation fields of a row, for an error message.
fn describe_perturbation(row: &Value) -> String {
    let mut out = String::new();
    for field in PERTURBATION_KEY_FIELDS {
        if !out.is_empty() {
            out.push_str(", ");
        }
        let value = row.get(field).unwrap_or(&Value::Null);
        let _ = write!(out, "{field}={}", shorten(&value.to_string()));
    }
    out
}

/// Enough of a row to find it, for an error message.
fn describe_row(row: &Value) -> String {
    let field = |name: &str| row.get(name).and_then(Value::as_str).unwrap_or("unknown");
    format!(
        "item {}, door {}, recorded {}",
        field("item_id"),
        field("door"),
        field("recorded_at"),
    )
}

fn shorten(text: &str) -> String {
    const LIMIT: usize = 48;
    if text.chars().count() <= LIMIT {
        return text.to_string();
    }
    let head: String = text.chars().take(LIMIT).collect();
    format!("{head}...")
}

fn name_or_null(receipt: Option<&str>) -> String {
    receipt.unwrap_or("null").to_string()
}

fn type_name_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// A value with every object's keys sorted, at every depth. A receipt follows
/// the reference implementation and sorts only the top level, because it
/// digests a row exactly as the row was written. A perturbation key has no
/// such history to match, so it sorts throughout and does not depend on the
/// order a nested field such as `door_identity` happens to serialize in.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(fields) => {
            let mut keys: Vec<&String> = fields.keys().collect();
            keys.sort();
            let mut sorted = Map::new();
            for key in keys {
                sorted.insert(
                    key.clone(),
                    canonical(fields.get(key).unwrap_or(&Value::Null)),
                );
            }
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest.iter() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// The exclusive lock one writer holds while it reads the head and appends.
///
/// The lock is a file created with `create_new`, which the operating system
/// refuses to do twice, so it holds across processes on one machine and not
/// only across threads. Dropping the guard removes the file, including while
/// a panic unwinds; a writer killed outright leaves the file behind, and the
/// error names it so a person can remove it.
#[derive(Debug)]
struct WriteLock {
    path: PathBuf,
}

impl WriteLock {
    fn acquire(store: &Path) -> Result<Self, StoreError> {
        let path = lock_path(store);
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|e| StoreError::Write {
                path: parent.display().to_string(),
                detail: e.to_string(),
            })?;
        }
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                let _ = write!(file, "{}", std::process::id());
                Ok(WriteLock { path })
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(StoreError::Locked {
                lock: path.display().to_string(),
                holder: match fs::read_to_string(&path) {
                    Ok(pid) if !pid.trim().is_empty() => format!(", held by pid {}", pid.trim()),
                    _ => String::new(),
                },
            }),
            Err(e) => Err(StoreError::Write {
                path: path.display().to_string(),
                detail: e.to_string(),
            }),
        }
    }
}

impl Drop for WriteLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_path(store: &Path) -> PathBuf {
    let mut name = store.as_os_str().to_os_string();
    name.push(".lock");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    /// One trial's fields, before the store seals them. Every perturbation
    /// field is here except `question_digest`, so two calls with the same
    /// `item` and `seed` describe the same trial; `asked` adds the text.
    fn trial(item: &str, seed: u64) -> Map<String, Value> {
        let mut fields = Map::new();
        fields.insert("schema".into(), json!(crate::row::SCHEMA));
        fields.insert("recorded_at".into(), json!("2026-09-19T12:00:00Z"));
        fields.insert("suite".into(), json!("routing"));
        fields.insert("suite_digest".into(), json!("suite:6f1c"));
        fields.insert("split".into(), json!("calibration"));
        fields.insert("item_id".into(), json!(item));
        fields.insert("door".into(), json!("kev"));
        fields.insert(
            "door_identity".into(),
            json!({
                "model": "kev-1",
                "base_signature": "base:aa01",
                "adapter": null,
                "verified": true,
            }),
        );
        fields.insert("estimator".into(), json!("l2"));
        fields.insert("seed_base".into(), json!(seed));
        fields.insert("permutation".into(), json!(0));
        fields.insert("correct".into(), json!(true));
        fields.insert("latency_ms".into(), Value::Null);
        fields
    }

    fn store_in(dir: &tempfile::TempDir) -> Store {
        Store::at(dir.path().join("results.jsonl"))
    }

    fn lines_of(store: &Store) -> Vec<String> {
        fs::read_to_string(store.path())
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn rewrite(store: &Store, lines: &[String]) {
        let mut text = lines.join("\n");
        text.push('\n');
        fs::write(store.path(), text).unwrap();
    }

    fn broken(store: &Store) -> (usize, ChainFault, String) {
        match verify_chain(&store.rows().unwrap()) {
            ChainVerdict::Broken {
                index,
                fault,
                detail,
            } => (index, fault, detail),
            ChainVerdict::Ok { rows, .. } => panic!("{rows} rows verified, and they should not"),
        }
    }

    #[test]
    fn a_chain_round_trips_and_reports_its_head() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        assert_eq!(store.head().unwrap(), None, "an empty store has no head");

        let first = store.append(&trial("q1", 0)).unwrap();
        let second = store.append(&trial("q2", 0)).unwrap();
        let third = store.append(&trial("q3", 0)).unwrap();

        let rows = store.verified_rows().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], first);
        assert_eq!(rows[2], third);

        assert_eq!(rows[0]["previous_receipt"], Value::Null);
        assert_eq!(rows[1]["previous_receipt"], first["receipt"]);
        assert_eq!(rows[2]["previous_receipt"], second["receipt"]);

        match verify_chain(&rows) {
            ChainVerdict::Ok { rows: count, head } => {
                assert_eq!(count, 3);
                assert_eq!(head.as_deref(), third["receipt"].as_str());
            }
            ChainVerdict::Broken { detail, .. } => panic!("{detail}"),
        }
        assert_eq!(
            store.head().unwrap().as_deref(),
            third["receipt"].as_str(),
            "the head is the last row's receipt"
        );
    }

    #[test]
    fn an_edited_row_names_the_row_and_the_digest() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        for item in ["q1", "q2", "q3"] {
            store.append(&trial(item, 0)).unwrap();
        }

        let mut lines = lines_of(&store);
        let edited = lines[1].replace("\"correct\":true", "\"correct\":false");
        assert_ne!(edited, lines[1], "the middle row should have changed");
        lines[1] = edited;
        rewrite(&store, &lines);

        let (index, fault, detail) = broken(&store);
        assert_eq!(index, 1);
        assert_eq!(fault, ChainFault::Edited);
        assert!(detail.contains("row 1"), "{detail}");
        assert!(detail.contains("item q2"), "{detail}");
        assert!(detail.contains("its contents digest to"), "{detail}");
        assert!(detail.contains("edited after it was written"), "{detail}");

        let error = store.verified_rows().unwrap_err();
        assert!(matches!(
            error,
            StoreError::BrokenChain {
                fault: ChainFault::Edited,
                index: 1,
                ..
            }
        ));
        assert!(error.to_string().contains("broken chain"), "{error}");
    }

    #[test]
    fn a_removed_row_reads_as_a_resequence_and_not_as_an_edit() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        for item in ["q1", "q2", "q3"] {
            store.append(&trial(item, 0)).unwrap();
        }

        let mut lines = lines_of(&store);
        lines.remove(1);
        rewrite(&store, &lines);

        let (index, fault, detail) = broken(&store);
        assert_eq!(index, 1);
        assert_eq!(fault, ChainFault::Resequenced);
        assert!(
            detail.contains("inserted, removed, or reordered"),
            "{detail}"
        );
        assert!(
            !detail.contains("edited after it was written"),
            "a removal must not be reported as an edit: {detail}"
        );
    }

    #[test]
    fn an_inserted_row_reads_as_a_resequence() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        for item in ["q1", "q2"] {
            store.append(&trial(item, 0)).unwrap();
        }

        let mut lines = lines_of(&store);
        let copy = lines[0].clone();
        lines.insert(1, copy);
        rewrite(&store, &lines);

        let (index, fault, detail) = broken(&store);
        assert_eq!(index, 1);
        assert_eq!(fault, ChainFault::Resequenced);
        assert!(
            detail.contains("inserted, removed, or reordered"),
            "{detail}"
        );
    }

    #[test]
    fn a_reordered_pair_reads_as_a_resequence() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        for item in ["q1", "q2", "q3"] {
            store.append(&trial(item, 0)).unwrap();
        }

        let mut lines = lines_of(&store);
        lines.swap(1, 2);
        rewrite(&store, &lines);

        let (index, fault, _) = broken(&store);
        assert_eq!(index, 1);
        assert_eq!(fault, ChainFault::Resequenced);
    }

    #[test]
    fn an_edit_and_a_resequence_are_two_different_messages() {
        let dir = tempfile::tempdir().unwrap();
        let edit = store_in(&dir);
        let resequence = Store::at(dir.path().join("other.jsonl"));
        for store in [&edit, &resequence] {
            for item in ["q1", "q2", "q3"] {
                store.append(&trial(item, 0)).unwrap();
            }
        }

        let mut lines = lines_of(&edit);
        lines[1] = lines[1].replace("\"correct\":true", "\"correct\":false");
        rewrite(&edit, &lines);

        let mut lines = lines_of(&resequence);
        lines.remove(1);
        rewrite(&resequence, &lines);

        let (_, edit_fault, edit_detail) = broken(&edit);
        let (_, resequence_fault, resequence_detail) = broken(&resequence);
        assert_ne!(edit_fault, resequence_fault);
        assert_ne!(edit_detail, resequence_detail);
        assert_eq!(edit_fault.as_str(), "edited");
        assert_eq!(resequence_fault.as_str(), "resequenced");
    }

    #[test]
    fn a_row_that_does_not_extend_the_head_leaves_the_file_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.append(&trial("q1", 0)).unwrap();
        let head = store.append(&trial("q2", 0)).unwrap();

        let before = fs::read(store.path()).unwrap();
        let stray = seal(&trial("q3", 0), None).unwrap();
        let error = store.append_sealed(&stray).unwrap_err();

        assert!(matches!(error, StoreError::DoesNotExtend { .. }), "{error}");
        let message = error.to_string();
        assert!(message.contains("does not extend the chain"), "{message}");
        assert!(message.contains("follows null"), "{message}");
        assert!(
            message.contains(head["receipt"].as_str().unwrap()),
            "the message names the head: {message}"
        );

        let after = fs::read(store.path()).unwrap();
        assert_eq!(before, after, "the file must be untouched, byte for byte");
        assert!(
            !lock_path(store.path()).exists(),
            "a refused append leaves no lock behind"
        );
    }

    #[test]
    fn a_row_sealed_against_the_wrong_head_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.append(&trial("q1", 0)).unwrap();

        let before = fs::read(store.path()).unwrap();
        let stray = seal(&trial("q2", 0), Some("receipt:0000")).unwrap();
        let error = store.append_sealed(&stray).unwrap_err();

        assert!(matches!(error, StoreError::DoesNotExtend { .. }), "{error}");
        assert_eq!(before, fs::read(store.path()).unwrap());
    }

    #[test]
    fn a_receipt_that_does_not_match_its_contents_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let mut tampered = seal(&trial("q1", 0), None).unwrap();
        tampered["correct"] = json!(false);

        let error = store.append_sealed(&tampered).unwrap_err();
        assert!(
            matches!(error, StoreError::ReceiptMismatch { .. }),
            "{error}"
        );
        assert!(
            !store.path().exists(),
            "the store is not created for a row it refuses"
        );
    }

    #[test]
    fn a_duplicate_perturbation_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.append(&trial("q1", 0)).unwrap();
        store.append(&trial("q2", 0)).unwrap();

        let before = fs::read(store.path()).unwrap();
        let error = store.append(&trial("q1", 0)).unwrap_err();

        assert!(
            matches!(error, StoreError::DuplicatePerturbation { index: 0, .. }),
            "{error}"
        );
        let message = error.to_string();
        assert!(message.contains("item_id=\"q1\""), "{message}");
        assert!(
            message.contains("re-scoring a run does not make it a second run"),
            "{message}"
        );
        assert_eq!(before, fs::read(store.path()).unwrap());
    }

    #[test]
    fn re_scoring_the_same_trial_is_refused_even_when_the_score_changed() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.append(&trial("q1", 0)).unwrap();

        let mut rescored = trial("q1", 0);
        rescored.insert("correct".into(), json!(false));
        rescored.insert("recorded_at".into(), json!("2026-09-20T09:30:00Z"));

        let error = store.append(&rescored).unwrap_err();
        assert!(
            matches!(error, StoreError::DuplicatePerturbation { .. }),
            "a second score for one trial is not a second trial: {error}"
        );
        assert_eq!(store.rows().unwrap().len(), 1);
    }

    #[test]
    fn a_new_perturbation_of_the_same_item_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.append(&trial("q1", 0)).unwrap();

        store
            .append(&trial("q1", 1))
            .expect("a different seed base is a different trial");

        let mut permuted = trial("q1", 0);
        permuted.insert("permutation".into(), json!(1));
        store
            .append(&permuted)
            .expect("a different permutation is a different trial");

        assert_eq!(store.verified_rows().unwrap().len(), 3);
    }

    #[test]
    fn an_unknown_schema_is_a_hard_error_that_says_what_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.append(&trial("q1", 0)).unwrap();

        let mut lines = lines_of(&store);
        lines.push(json!({"schema": "openagents.gym.eval_row.v0", "item_id": "q2"}).to_string());
        rewrite(&store, &lines);

        let error = store.rows().unwrap_err();
        assert!(
            matches!(error, StoreError::UnknownSchema { line: 2, .. }),
            "{error}"
        );
        let message = error.to_string();
        assert!(message.contains("openagents.gym.eval_row.v0"), "{message}");
        assert!(message.contains(crate::row::SCHEMA), "{message}");
        assert!(message.contains("archive"), "{message}");
    }

    #[test]
    fn a_measured_float_that_does_not_round_trip_still_verifies() {
        // The value is from the first live run to record one: a single
        // call's latency, in milliseconds. `serde_json` writes it as
        // 1474.8615419999999 and parses that back as 1474.861542, which is a
        // different double. A receipt taken over the written value failed on
        // the next append, reporting a row nobody had touched as edited.
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let mut fields = trial("routing/010", 0);
        fields.insert("latency_ms".into(), json!(1474.8615419999999_f64));
        store.append(&fields).unwrap();
        store.append(&trial("routing/011", 0)).unwrap();

        match verify_chain(&store.rows().unwrap()) {
            ChainVerdict::Ok { rows, .. } => assert_eq!(rows, 2),
            broken => panic!("the chain broke on a float nobody edited: {broken:?}"),
        }

        // The number on disk is the one the receipt covers, and it is within
        // a microsecond of the measurement.
        let recorded = store.rows().unwrap()[0]["latency_ms"].as_f64().unwrap();
        assert!((recorded - 1474.861542).abs() < 1e-6, "{recorded}");
    }

    #[test]
    fn the_allowlist_names_schemas_rather_than_types() {
        // One declaration of the string, and it is still a string: a reader
        // checks a name it read off disk, and never has to deserialize a row
        // into the type that wrote it to find out whether it may.
        assert_eq!(KNOWN_ROW_SCHEMAS, &[crate::row::SCHEMA]);
        assert!(known_row_schema("openagents.gym.eval_row.v1"));
        assert!(!known_row_schema("openagents.gym.eval_row.v2"));

        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        // A row this store never wrote, carrying fields it does not know,
        // reads back because its schema is on the list.
        let foreign = json!({
            "schema": crate::row::SCHEMA,
            "item_id": "q1",
            "a_field_from_a_later_version": 7,
        });
        store.append(&foreign).unwrap();
        assert_eq!(store.rows().unwrap().len(), 1);
    }

    #[test]
    fn a_row_with_no_schema_is_refused_rather_than_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        fs::write(store.path(), "{\"item_id\":\"q1\"}\n").unwrap();

        let error = store.rows().unwrap_err();
        assert!(matches!(error, StoreError::UnknownSchema { .. }), "{error}");
        assert!(error.to_string().contains("no schema"), "{error}");
    }

    #[test]
    fn a_line_that_is_not_json_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        fs::write(store.path(), "not json\n").unwrap();
        assert!(matches!(
            store.rows().unwrap_err(),
            StoreError::NotJson { line: 1, .. }
        ));

        fs::write(store.path(), "[1,2,3]\n").unwrap();
        assert!(matches!(
            store.rows().unwrap_err(),
            StoreError::NotAnObject { line: 1, .. }
        ));
    }

    #[test]
    fn a_value_that_is_not_an_object_is_not_a_row() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let error = store.append(&"a result").unwrap_err();
        assert!(matches!(error, StoreError::NotARow { .. }), "{error}");
        assert!(error.to_string().contains("a string"), "{error}");
    }

    #[test]
    fn a_truncated_file_is_repaired_before_it_is_appended_to() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        store.append(&trial("q1", 0)).unwrap();

        let text = fs::read_to_string(store.path()).unwrap();
        fs::write(store.path(), text.trim_end()).unwrap();
        let before = fs::read(store.path()).unwrap();

        let error = store.append(&trial("q2", 0)).unwrap_err();
        assert!(matches!(error, StoreError::Truncated { .. }), "{error}");
        assert!(error.to_string().contains("join two rows"), "{error}");
        assert_eq!(before, fs::read(store.path()).unwrap());
    }

    #[test]
    fn a_second_writer_is_refused_while_the_lock_is_held() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);

        let lock = WriteLock::acquire(store.path()).unwrap();
        let error = store.append(&trial("q1", 0)).unwrap_err();
        assert!(error.is_locked(), "{error}");
        let message = error.to_string();
        assert!(message.contains("one writer at a time"), "{message}");
        assert!(
            message.contains(&std::process::id().to_string()),
            "the message names the holder: {message}"
        );

        drop(lock);
        store
            .append(&trial("q1", 0))
            .expect("the lock is released when the writer drops it");
    }

    #[test]
    fn concurrent_appends_do_not_interleave_into_a_broken_chain() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(&dir);
        let writers = 4;
        let rows_each = 3;

        std::thread::scope(|scope| {
            for writer in 0..writers {
                let store = store.clone();
                scope.spawn(move || {
                    for row in 0..rows_each {
                        let fields = trial(&format!("q{writer}-{row}"), writer as u64);
                        loop {
                            match store.append(&fields) {
                                Ok(_) => break,
                                Err(e) if e.is_locked() => {
                                    std::thread::sleep(Duration::from_millis(1));
                                }
                                Err(e) => panic!("{e}"),
                            }
                        }
                    }
                });
            }
        });

        let rows = store.rows().unwrap();
        assert_eq!(rows.len(), writers * rows_each);
        match verify_chain(&rows) {
            ChainVerdict::Ok { rows: count, head } => {
                assert_eq!(count, writers * rows_each);
                assert!(head.is_some());
            }
            ChainVerdict::Broken { detail, .. } => panic!("{detail}"),
        }
        assert!(
            !lock_path(store.path()).exists(),
            "every writer released the lock"
        );
    }

    #[test]
    fn a_receipt_does_not_depend_on_the_order_the_fields_were_written_in() {
        let mut forward = Map::new();
        forward.insert("a".into(), json!(1));
        forward.insert("b".into(), json!(2));
        let mut backward = Map::new();
        backward.insert("b".into(), json!(2));
        backward.insert("a".into(), json!(1));

        assert_eq!(
            receipt_of(&Value::Object(forward)),
            receipt_of(&Value::Object(backward))
        );
    }

    #[test]
    fn a_receipt_covers_every_field_including_the_one_before_it() {
        let scored = seal(&trial("q1", 0), None).unwrap();

        let mut changed = trial("q1", 0);
        changed.insert("correct".into(), json!(false));
        let changed = seal(&changed, None).unwrap();
        assert_ne!(scored["receipt"], changed["receipt"], "a field is covered");

        let chained = seal(&trial("q1", 0), Some("receipt:0000")).unwrap();
        assert_ne!(
            scored["receipt"], chained["receipt"],
            "previous_receipt sits under the receipt, which is what chains the rows"
        );
    }

    /// One row of a run, with the question set it served.
    fn asked(item: &str, questions: Option<&str>) -> Map<String, Value> {
        let mut fields = trial(item, 0);
        if let Some(digest) = questions {
            fields.insert("question_digest".into(), json!(digest));
        }
        fields
    }

    #[test]
    fn two_runs_over_the_same_items_and_the_same_text_are_a_door_comparison() {
        let control = [Value::Object(asked("q1", Some("questions:9745")))];
        let candidate = [Value::Object(asked("q1", Some("questions:9745")))];
        assert_eq!(
            admit_comparison(&control, &candidate).expect("one suite, one text"),
            Comparison::Doors
        );
    }

    #[test]
    fn two_runs_over_the_same_items_and_different_text_are_a_question_text_comparison() {
        // The comparison openagents#9386 exists to make expressible. Before
        // the question set, a reworded question changed the suite digest, so
        // these two sides read as two suites and were refused.
        let baseline = [Value::Object(asked("q1", Some("questions:9745")))];
        let variant = [Value::Object(asked("q1", Some("questions:0f3c")))];
        assert_eq!(
            admit_comparison(&baseline, &variant).expect("one suite, two texts"),
            Comparison::QuestionText
        );
        assert_eq!(Comparison::QuestionText.as_str(), "question text");
    }

    #[test]
    fn two_runs_whose_items_differ_are_still_refused() {
        // The failure this whole record is built to prevent, and the one the
        // question set must not have bought its way past. Separating the text
        // out did not make a changed label comparable; it made a changed
        // question stop looking like one.
        let ours = [Value::Object(asked("q1", Some("questions:9745")))];
        let mut theirs = asked("q1", Some("questions:9745"));
        theirs.insert("suite_digest".into(), json!("suite:0000"));
        let theirs = [Value::Object(theirs)];
        let refused = admit_comparison(&ours, &theirs).expect_err("two suites are two things");
        assert!(matches!(refused, StoreError::NotComparable { .. }));
        assert!(
            refused.to_string().contains("did not score the same items"),
            "{refused}"
        );
    }

    #[test]
    fn a_side_that_scored_two_suites_is_not_one_side() {
        let mut second = asked("q2", Some("questions:9745"));
        second.insert("suite_digest".into(), json!("suite:0000"));
        let mixed = [
            Value::Object(asked("q1", Some("questions:9745"))),
            Value::Object(second),
        ];
        let candidate = [Value::Object(asked("q1", Some("questions:9745")))];
        assert!(matches!(
            admit_comparison(&mixed, &candidate),
            Err(StoreError::NotComparable { .. })
        ));
    }

    #[test]
    fn a_run_that_does_not_say_what_it_asked_is_not_a_text_comparison() {
        // Unknown is never zero, and an unrecorded question set is never the
        // authored one. Two runs that both predate the field are still a door
        // comparison, because there the text is inside the suite digest.
        let recorded = [Value::Object(asked("q1", Some("questions:9745")))];
        let silent = [Value::Object(asked("q1", None))];
        assert!(matches!(
            admit_comparison(&recorded, &silent),
            Err(StoreError::NotComparable { .. })
        ));
        assert_eq!(
            admit_comparison(&silent, &[Value::Object(asked("q1", None))])
                .expect("two runs of the older shape"),
            Comparison::Doors
        );
    }

    #[test]
    fn an_empty_side_is_not_a_comparison() {
        let rows = [Value::Object(asked("q1", Some("questions:9745")))];
        assert!(matches!(
            admit_comparison(&rows, &[]),
            Err(StoreError::NotComparable { .. })
        ));
    }

    #[test]
    fn a_reworded_question_is_a_second_trial_and_not_a_repeat() {
        // The store keeps one row per perturbation, and the question text is
        // a perturbation axis like the option order: the same door answering
        // the same item under reworded text is a new measurement, not a
        // re-scoring of the last one.
        let directory = tempfile::tempdir().expect("a temporary directory");
        let store = store_in(&directory);
        store.append(&asked("q1", Some("questions:9745"))).expect("the baseline");
        store.append(&asked("q1", Some("questions:0f3c"))).expect("the variant");
        assert_eq!(store.rows().unwrap().len(), 2);

        let repeat = store.append(&asked("q1", Some("questions:0f3c")));
        assert!(matches!(
            repeat,
            Err(StoreError::DuplicatePerturbation { .. })
        ));
    }

    #[test]
    fn a_perturbation_key_ignores_the_order_of_a_nested_field() {
        let one = json!({
            "suite_digest": "suite:6f1c",
            "door_identity": {"model": "kev-1", "verified": true},
            "estimator": "l2",
            "seed_base": 0,
            "permutation": 0,
            "item_id": "q1",
        });
        let other = json!({
            "item_id": "q1",
            "permutation": 0,
            "seed_base": 0,
            "estimator": "l2",
            "door_identity": {"verified": true, "model": "kev-1"},
            "suite_digest": "suite:6f1c",
        });
        assert_eq!(perturbation_key(&one), perturbation_key(&other));

        let elsewhere = json!({
            "suite_digest": "suite:6f1c",
            "door_identity": {"model": "lev-1", "verified": false},
            "estimator": "l2",
            "seed_base": 0,
            "permutation": 0,
            "item_id": "q1",
        });
        assert_ne!(
            perturbation_key(&one),
            perturbation_key(&elsewhere),
            "a different door is a different trial"
        );

        let mut reworded = one.clone();
        reworded["question_digest"] = json!("questions:0f3c");
        assert_ne!(
            perturbation_key(&one),
            perturbation_key(&reworded),
            "a different question is a different trial"
        );
    }
}
