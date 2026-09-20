//! Building a caller's suite: labelled records in, a pinned suite and
//! question set out.
//!
//! A caller hands over a JSONL file where each line is one labelled
//! decision — `family`, `kind`, `state`, `truth`, and the `question` the
//! door should read — and `build` emits the same pinned, three-partition
//! suite our own suites are: `label_source` names the caller, `label_rule`
//! records how the labels were produced, and provenance carries the
//! caller's licence statement and the input file's hash. `gym report` can
//! then print the agreement ceilings a caller stated beside each family's
//! scores.
//!
//! The record contract is strict where a pinned artifact should be:
//! unknown fields, a truth outside its own option set, a `kind` that
//! disagrees with `question.type`, an item id that collides with a family
//! name, and a float anywhere in `state` all fail the build. The float
//! refusal is one the file format keeps even now that this builder and the
//! loader share one canonicalizer: a float in state is a spelling the
//! digest cannot prove two readers agreed on.
//!
//! Question keying follows [`crate::questions::QuestionSet::ask`], which
//! reads an item id before a family: a family whose items share one
//! identical question is keyed by family name, a family whose items carry
//! different option sets is keyed per item id, and both coexist in one
//! set.
//!
//! Partitions draw 40/40/20 calibration/development/locked per family,
//! paraphrase groups held whole, under a fixed seed. The shuffle is a
//! reimplementation of CPython's `random.Random(9464)` — the builder this
//! module replaced was Python, and a caller suite it emitted is only
//! reproducible if the draw is identical. The committed caller fixture's
//! digest is the test that proves the match.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::suite::canonicalize;

/// The schema a written suite declares.
const SUITE_SCHEMA: &str = "openagents.gym.suite.v1";

/// The schema a written question set declares.
const QUESTION_SCHEMA: &str = "openagents.gym.question_set.v1";

/// The gate a caller suite names unless the caller names another.
const DEFAULT_GATE: &str = "probability-v2";

/// The seed every partition draw starts from, matching the builder this
/// module replaced.
const SEED: u32 = 9464;

/// The share of each family's items each partition takes, in fill order:
/// the smallest first, so the remainder lands in development rather than
/// leaving the locked partition empty. Locked is spent once through the
/// ledger; calibration fits maps; development is read freely.
const SPLIT: [(&str, f64); 3] = [("locked", 0.2), ("calibration", 0.4), ("development", 0.4)];

/// The fields a caller's record may carry. Anything else is refused rather
/// than silently digested, because a pinned suite is evidence.
const FIELDS: [&str; 8] = [
    "id",
    "family",
    "kind",
    "state",
    "truth",
    "question",
    "group",
    "label_rule",
];

/// Everything a build needs that is not in the records file.
pub struct Spec {
    /// The caller's JSONL file.
    pub input: String,
    /// The suite's name, e.g. `caller-acme-v1`; also the question set's id.
    pub name: String,
    /// Who the labels are, e.g. `acme`.
    pub label_source: String,
    /// How the labels were produced, in one sentence.
    pub label_rule: String,
    /// Where the data came from.
    pub source: String,
    /// The caller's licence statement for measurement use.
    pub licence: String,
    /// The suite's creation date; today when the caller does not say.
    pub created: String,
    /// A description to use instead of the generated one.
    pub description: Option<String>,
    /// The gate id the suite names.
    pub gate: String,
    /// `family=ceiling` pairs: the agreement ceiling a family's labels rest
    /// on, which `gym report` prints beside that family's scores.
    pub agreement: Vec<String>,
}

impl Spec {
    /// The required pieces, with the defaults the command offers.
    pub fn new(
        input: String,
        name: String,
        label_source: String,
        label_rule: String,
        source: String,
        licence: String,
    ) -> Self {
        Self {
            input,
            name,
            label_source,
            label_rule,
            source,
            licence,
            created: crate::eval::now_utc()[..10].to_string(),
            description: None,
            gate: DEFAULT_GATE.to_string(),
            agreement: Vec::new(),
        }
    }
}

/// What a build produced: both documents, the digest, and the shape of the
/// question set for the summary line.
#[derive(Debug)]
pub struct Built {
    /// The suite document, ready to write as JSON.
    pub suite: Value,
    /// The question-set document, ready to write as JSON.
    pub questions: Value,
    /// The suite's content digest.
    pub digest: String,
    /// How many items the suite holds.
    pub items: usize,
    /// How many families share one question per family.
    pub family_keyed: usize,
    /// How many families key a question per item id.
    pub item_keyed: usize,
}

/// Reads the records, validates them, and emits the suite and question
/// set.
///
/// # Errors
///
/// Returns a description of the first contract a record breaks, in the
/// words a caller can act on.
pub fn build(spec: &Spec) -> Result<Built, String> {
    let records = read_records(&spec.input)?;
    let mut items: Vec<Map<String, Value>> = records
        .iter()
        .map(|record| to_item(record, spec))
        .collect::<Result<_, _>>()?;
    assign_ids(&mut items)?;
    partition(&mut items)?;
    let (questions, keying) = key_questions(&items);
    for item in &mut items {
        no_floats(item.get("state").unwrap_or(&Value::Null), item_id(item))?;
        item.remove("question");
    }

    let digested: Vec<Value> = items.iter().map(digested).collect();
    let mut hasher = Sha256::new();
    hasher.update(canonicalize(&Value::Array(digested)).as_bytes());
    let digest = format!("{:x}", hasher.finalize());

    let agreement = agreement_map(spec, &keying)?;
    let family_count = keying.len();
    let item_count = items.len();
    let suite = suite_document(spec, &digest, &agreement, items, family_count);
    let question_set = question_set_document(spec, questions);

    Ok(Built {
        suite,
        questions: question_set,
        digest,
        items: item_count,
        family_keyed: keying.values().filter(|key| **key == "family").count(),
        item_keyed: keying.values().filter(|key| **key == "item").count(),
    })
}

/// A Python-style repr for one JSON value, for errors a caller reads.
fn repr(value: &Value) -> String {
    match value {
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Null => "None".to_string(),
        Value::String(text) => format!("'{text}'"),
        other => other.to_string(),
    }
}

/// The label a knowledgeable answer lands on, as our item's `truth`.
fn truth_of(record: &Map<String, Value>) -> Result<String, String> {
    let kind = record["kind"].as_str().unwrap_or_default();
    let truth = &record["truth"];
    let where_ = record
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| record["family"].as_str().unwrap_or("?"));
    match kind {
        "noul" => match truth {
            Value::Bool(true) => Ok("yes".to_string()),
            Value::Bool(false) => Ok("no".to_string()),
            Value::String(text) => match text.as_str() {
                "true" | "yes" => Ok("yes".to_string()),
                "false" | "no" => Ok("no".to_string()),
                _ => Err(format!(
                    "{where_}: a noul's truth is yes or no, got {}",
                    repr(truth)
                )),
            },
            _ => Err(format!(
                "{where_}: a noul's truth is yes or no, got {}",
                repr(truth)
            )),
        },
        "choice" => {
            let criteria = record["question"]
                .get("criteria")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            match truth.as_str() {
                Some(option) if criteria.contains_key(option) => Ok(option.to_string()),
                _ => Err(format!(
                    "{where_}: truth {} is not in its own option set",
                    repr(truth)
                )),
            }
        }
        "score" => {
            if truth.is_boolean() {
                return Err(format!(
                    "{where_}: a score's truth is a level index, got {}",
                    repr(truth)
                ));
            }
            let index = match truth {
                Value::Number(number) if number.is_i64() => number.as_i64(),
                Value::Number(number) if number.is_u64() => {
                    number.as_u64().and_then(|n| i64::try_from(n).ok())
                }
                Value::String(text) => match text.parse::<i64>() {
                    // `"01"` parses but is not a level index's spelling.
                    Ok(parsed) if text.as_str() == parsed.to_string() => Some(parsed),
                    _ => None,
                },
                _ => None,
            };
            let Some(index) = index else {
                return Err(format!(
                    "{where_}: a score's truth is a level index, got {}",
                    repr(truth)
                ));
            };
            let levels = record["question"]
                .get("criteria")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            if !(0..levels as i64).contains(&index) {
                return Err(format!(
                    "{where_}: truth {} is outside {levels} levels",
                    repr(truth)
                ));
            }
            Ok(index.to_string())
        }
        _ => Err(format!("{where_}: unknown kind {}", repr(&record["kind"]))),
    }
}

/// The contract a caller's record must keep, checked before it is
/// digested.
fn check_record(record: &Map<String, Value>, line: usize) -> Result<(), String> {
    let where_ = record
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("line {line}"));
    let unknown: Vec<&str> = record
        .keys()
        .filter(|key| !FIELDS.contains(&key.as_str()))
        .map(String::as_str)
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "{where_}: unknown fields {unknown:?}; fix the export rather than guess"
        ));
    }
    for field in ["family", "kind", "state", "truth", "question"] {
        if !record.contains_key(field) {
            return Err(format!("{where_}: missing {field}"));
        }
    }
    match record["family"].as_str() {
        Some(family) if !family.is_empty() => {}
        _ => return Err(format!("{where_}: family must be a nonempty string")),
    }
    let Some(question) = record["question"].as_object() else {
        return Err(format!("{where_}: question must be an object"));
    };
    if question.get("type") != Some(&record["kind"]) {
        return Err(format!(
            "{where_}: kind {} does not match question.type {}",
            repr(&record["kind"]),
            repr(question.get("type").unwrap_or(&Value::Null))
        ));
    }
    if !question.get("instructions").is_some_and(Value::is_string) {
        return Err(format!("{where_}: question.instructions must be a string"));
    }
    let criteria = question.get("criteria");
    if record["kind"].as_str() == Some("score") {
        match criteria {
            Some(Value::Array(levels)) if levels.len() >= 2 => {}
            _ => {
                return Err(format!(
                    "{where_}: a score's criteria is an ordered list of at least two levels"
                ));
            }
        }
    } else {
        match criteria {
            Some(Value::Object(options)) if !options.is_empty() => {}
            _ => return Err(format!("{where_}: criteria must be a nonempty object")),
        }
    }
    Ok(())
}

fn read_records(path: &str) -> Result<Vec<Map<String, Value>>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let mut records = Vec::new();
    for (line, text) in text.lines().enumerate() {
        let line = line + 1;
        if text.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(text).map_err(|error| format!("{path} line {line}: {error}"))?;
        let Value::Object(record) = value else {
            return Err(format!("line {line}: a record is a JSON object"));
        };
        check_record(&record, line)?;
        records.push(record);
    }
    if records.is_empty() {
        return Err(format!("{path} holds no records"));
    }
    Ok(records)
}

/// One caller record as a suite item, with the question still attached
/// until the set is keyed — the suite carries items, the set carries text.
fn to_item(record: &Map<String, Value>, spec: &Spec) -> Result<Map<String, Value>, String> {
    let truth = truth_of(record)?;
    let mut item = Map::new();
    item.insert(
        "id".to_string(),
        record.get("id").cloned().unwrap_or(Value::Null),
    );
    item.insert("family".to_string(), record["family"].clone());
    item.insert("kind".to_string(), record["kind"].clone());
    item.insert("state".to_string(), record["state"].clone());
    item.insert("truth".to_string(), Value::String(truth));
    item.insert(
        "label_source".to_string(),
        Value::String(spec.label_source.clone()),
    );
    item.insert(
        "label_rule".to_string(),
        record
            .get("label_rule")
            .and_then(Value::as_str)
            .unwrap_or(&spec.label_rule)
            .to_string()
            .into(),
    );
    item.insert("question".to_string(), record["question"].clone());
    // Filled in by `partition`. Written before it so the digested dict
    // lists every field the reader hashes.
    item.insert("partition".to_string(), Value::Null);
    // `group` records that two items are paraphrases of one scenario. Kept
    // undigested, the way `external-v1` keeps `source_row`: it groups the
    // item's evidence rather than describing the item.
    if let Some(group) = record.get("group").filter(|group| truthy(group)) {
        item.insert("group".to_string(), group.clone());
    }
    Ok(item)
}

/// Python truthiness for a JSON value: null, false, zero, and empty are
/// the same "absent" here.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64() != Some(0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

fn item_id(item: &Map<String, Value>) -> &str {
    item.get("id").and_then(Value::as_str).unwrap_or("?")
}

/// Caller ids win; the rest are `<family>/<NNN>` in input order.
fn assign_ids(items: &mut [Map<String, Value>]) -> Result<(), String> {
    let families: Vec<String> = {
        let mut seen = Vec::new();
        for item in items.iter() {
            let family = item["family"].as_str().unwrap_or_default().to_string();
            if !seen.contains(&family) {
                seen.push(family);
            }
        }
        seen
    };
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for item in items.iter_mut() {
        if item["id"].is_null() {
            let family = item["family"].as_str().unwrap_or_default().to_string();
            let count = counts.entry(family.clone()).or_insert(0);
            *count += 1;
            item.insert(
                "id".to_string(),
                Value::String(format!("{family}/{count:03}")),
            );
        } else {
            let id = item["id"].as_str().unwrap_or_default().to_string();
            if !item["id"].is_string() {
                return Err(format!(
                    "{}: an item id must be a string",
                    repr(&item["id"])
                ));
            }
            if families.contains(&id) {
                return Err(format!(
                    "{id}: an item id must not be a family name — a question keyed by \
                     that name would answer it"
                ));
            }
        }
    }
    let mut ids: Vec<&str> = items.iter().map(item_id).collect();
    ids.sort_unstable();
    let mut deduped = ids.clone();
    deduped.dedup();
    if ids.len() != deduped.len() {
        return Err("duplicate item ids".to_string());
    }
    Ok(())
}

/// Assign partitions per family, keeping a paraphrase group together.
fn partition(items: &mut [Map<String, Value>]) -> Result<(), String> {
    let mut families: Vec<String> = Vec::new();
    for item in items.iter() {
        let family = item["family"].as_str().unwrap_or_default().to_string();
        if !families.contains(&family) {
            families.push(family);
        }
    }
    for family in families {
        let members: Vec<usize> = items
            .iter()
            .enumerate()
            .filter(|(_, item)| item["family"].as_str() == Some(family.as_str()))
            .map(|(index, _)| index)
            .collect();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut order_of: BTreeMap<String, usize> = BTreeMap::new();
        for index in members.iter().copied() {
            let item = &items[index];
            let key = match item.get("group") {
                Some(group) if truthy(group) => canonicalize(group),
                _ => item_id(item).to_string(),
            };
            match order_of.get(&key) {
                Some(&group) => groups[group].push(index),
                None => {
                    order_of.insert(key, groups.len());
                    groups.push(vec![index]);
                }
            }
        }
        PyRandom::seeded(SEED).shuffle(&mut groups);

        let total = members.len();
        let mut targets: BTreeMap<&str, usize> = BTreeMap::new();
        let mut claimed = 0usize;
        for (name, share) in &SPLIT[..SPLIT.len() - 1] {
            let target = ((total as f64 * share).round_ties_even() as usize).max(1);
            targets.insert(name, target);
            claimed += target;
        }
        targets.insert("development", total - claimed);
        let mut counts: BTreeMap<&str, usize> = SPLIT.iter().map(|(name, _)| (*name, 0)).collect();
        for group in &groups {
            let mut placed = false;
            for (name, _) in &SPLIT {
                if counts[*name] < targets[*name] {
                    *counts.get_mut(*name).unwrap() += group.len();
                    for index in group {
                        items[*index]
                            .insert("partition".to_string(), Value::String((*name).to_string()));
                    }
                    placed = true;
                    break;
                }
            }
            if !placed {
                return Err(format!(
                    "{family}: no partition had room for a group; the split \
                     cannot place it"
                ));
            }
        }
        for (name, _) in &SPLIT {
            if counts[*name] == 0 {
                return Err(format!(
                    "{family}: the {name} partition came out empty; a family \
                     needs enough distinct items — or groups — to fill all three"
                ));
            }
        }
    }
    Ok(())
}

/// Key each family's question by family name when the items share it,
/// else per item id — the order `QuestionSet::ask` reads.
///
/// Returns the keyed questions and the family's keying per family name.
fn key_questions(items: &[Map<String, Value>]) -> (Map<String, Value>, BTreeMap<String, String>) {
    let mut questions = Map::new();
    let mut keying: BTreeMap<String, String> = BTreeMap::new();
    let mut families: Vec<String> = Vec::new();
    for item in items {
        let family = item["family"].as_str().unwrap_or_default().to_string();
        if !families.contains(&family) {
            families.push(family);
        }
    }
    for family in families {
        let members: Vec<&Map<String, Value>> = items
            .iter()
            .filter(|item| item["family"].as_str() == Some(family.as_str()))
            .collect();
        let first = &members[0]["question"];
        if members.iter().all(|item| &item["question"] == first) {
            questions.insert(family.clone(), first.clone());
            keying.insert(family, "family".to_string());
        } else {
            for item in members {
                questions.insert(item_id(item).to_string(), item["question"].clone());
            }
            keying.insert(family, "item".to_string());
        }
    }
    (questions, keying)
}

/// The fields `gym::suite::Item` hashes, and no others.
fn digested(item: &Map<String, Value>) -> Value {
    let keys = [
        "id",
        "family",
        "kind",
        "state",
        "truth",
        "partition",
        "label_source",
        "label_rule",
    ];
    let mut out = Map::new();
    for key in keys {
        out.insert(
            key.to_string(),
            item.get(key).cloned().unwrap_or(Value::Null),
        );
    }
    Value::Object(out)
}

/// A float in `state` is a spelling the digest cannot prove two readers
/// agreed on; strings, integers, booleans, nulls, and containers of them
/// are fine.
fn no_floats(value: &Value, where_: &str) -> Result<(), String> {
    match value {
        Value::Number(number) if number.is_f64() => Err(format!(
            "{where_}: a float would make the digest's spelling ambiguous; \
             write it as a string"
        )),
        Value::Array(entries) => {
            for entry in entries {
                no_floats(entry, where_)?;
            }
            Ok(())
        }
        Value::Object(fields) => {
            for entry in fields.values() {
                no_floats(entry, where_)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The `family=ceiling` pairs as a provenance map, validated against the
/// families the suite actually holds.
fn agreement_map(
    spec: &Spec,
    keying: &BTreeMap<String, String>,
) -> Result<Map<String, Value>, String> {
    let mut agreement = Map::new();
    for pair in &spec.agreement {
        let Some((family, ceiling)) = pair.split_once('=') else {
            return Err(format!("--agreement takes FAMILY=CEILING, got '{pair}'"));
        };
        if !keying.contains_key(family) {
            return Err(format!(
                "--agreement names '{family}', which is not a family"
            ));
        }
        agreement.insert(family.to_string(), Value::String(ceiling.to_string()));
    }
    Ok(agreement)
}

fn suite_document(
    spec: &Spec,
    digest: &str,
    agreement: &Map<String, Value>,
    items: Vec<Map<String, Value>>,
    family_count: usize,
) -> Value {
    let input = Path::new(&spec.input);
    let bytes = std::fs::read(input).unwrap_or_default();
    let input_hash = format!("{:x}", Sha256::digest(&bytes));
    let file_name = input
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| spec.input.clone());

    let description = spec.description.clone().unwrap_or_else(|| {
        format!(
            "A caller suite: {} decisions across {family_count} families, labelled by {} \
             under the rule each item's `label_rule` records, from {}. Partitions are drawn \
             per family at 40/40/20 under a fixed seed, and a paraphrase group stays in one \
             partition because two spellings of a scenario are one item of evidence. The \
             question text lives in the question set of the same name — keyed by family \
             where a family shares one wording, by item id where items carry their own \
             option sets. Built by `gym build`.",
            items.len(),
            spec.label_source,
            spec.source,
        )
    });

    let mut provenance = Map::new();
    provenance.insert(
        "caller".to_string(),
        Value::String(spec.label_source.clone()),
    );
    provenance.insert("source".to_string(), Value::String(spec.source.clone()));
    provenance.insert("licence".to_string(), Value::String(spec.licence.clone()));
    let mut input_meta = Map::new();
    input_meta.insert("file".to_string(), Value::String(file_name));
    input_meta.insert("sha256".to_string(), Value::String(input_hash));
    provenance.insert("input".to_string(), Value::Object(input_meta));
    provenance.insert(
        "partition_rule".to_string(),
        Value::String(format!(
            "per family, paraphrase groups whole, seeded shuffle (seed {SEED}), \
             40% calibration / 40% development / 20% locked"
        )),
    );
    // The agreement ceiling a family's labels rest on, when the caller
    // states one. `gym report` prints it beside the family's scores so a
    // number is never read without the bar above it.
    provenance.insert("agreement".to_string(), Value::Object(agreement.clone()));

    let mut suite = Map::new();
    suite.insert(
        "schema".to_string(),
        Value::String(SUITE_SCHEMA.to_string()),
    );
    suite.insert("name".to_string(), Value::String(spec.name.clone()));
    suite.insert("created".to_string(), Value::String(spec.created.clone()));
    suite.insert("description".to_string(), Value::String(description));
    suite.insert("tier".to_string(), Value::String("scored".to_string()));
    suite.insert("gate".to_string(), Value::String(spec.gate.clone()));
    suite.insert("questions".to_string(), Value::String(spec.name.clone()));
    suite.insert("provenance".to_string(), Value::Object(provenance));
    suite.insert("digest".to_string(), Value::String(digest.to_string()));
    suite.insert(
        "items".to_string(),
        Value::Array(items.into_iter().map(Value::Object).collect()),
    );
    Value::Object(suite)
}

fn question_set_document(spec: &Spec, questions: Map<String, Value>) -> Value {
    let mut set = Map::new();
    set.insert(
        "$comment".to_string(),
        Value::String(format!(
            "The question text of `{}`, supplied by the caller with the items. Keys are \
             family names where a family shares one question, item ids where items carry \
             their own option sets; `QuestionSet::ask` reads an item id before it reads a \
             family.",
            spec.name
        )),
    );
    set.insert(
        "schema".to_string(),
        Value::String(QUESTION_SCHEMA.to_string()),
    );
    set.insert("id".to_string(), Value::String(spec.name.clone()));
    set.insert("suite".to_string(), Value::String(spec.name.clone()));
    set.insert("questions".to_string(), Value::Object(questions));
    Value::Object(set)
}

/// CPython's `random.Random`, reimplemented for the partition draw.
///
/// The builder this module replaced drew partitions with
/// `random.Random(9464).shuffle`, and a suite is pinned by its digest: a
/// different draw is a different suite. This is CPython's MT19937 —
/// `init_genrand`/`init_by_array` seeding for a small integer seed,
/// `genrand_uint32` tempering, `getrandbits` as the word-little-endian
/// assembly `_randommodule.c` writes, and `shuffle` over
/// `_randbelow`'s rejection sampling — so a suite built here digests to
/// the same value a suite built there did. The committed caller fixture
/// is the proof.
struct PyRandom {
    state: [u32; 624],
    index: usize,
}

impl PyRandom {
    /// `random.Random(seed)` for a non-negative integer seed.
    fn seeded(seed: u32) -> Self {
        // init_genrand(19650218): init_by_array always starts there, not
        // from the caller's seed.
        let mut state = [0u32; 624];
        state[0] = 19650218;
        for i in 1..624 {
            state[i] = 1812433253u32
                .wrapping_mul(state[i - 1] ^ (state[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        // init_by_array over the one-word key the small integer becomes.
        let key = [seed];
        let mut i = 1usize;
        let mut j = 0usize;
        for _ in 0..624usize.max(key.len()) {
            state[i] = (state[i] ^ (state[i - 1] ^ (state[i - 1] >> 30)).wrapping_mul(1664525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                state[0] = state[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..623 {
            state[i] = (state[i] ^ (state[i - 1] ^ (state[i - 1] >> 30)).wrapping_mul(1566083941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                state[0] = state[623];
                i = 1;
            }
        }
        state[0] = 0x80000000;
        Self { state, index: 624 }
    }

    /// `genrand_uint32`: one tempered word of MT19937.
    fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            self.twist();
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^= y >> 18;
        y
    }

    fn twist(&mut self) {
        for i in 0..624 {
            let y = (self.state[i] & 0x80000000) | (self.state[(i + 1) % 624] & 0x7fffffff);
            self.state[i] =
                self.state[(i + 397) % 624] ^ (y >> 1) ^ if y & 1 != 0 { 0x9908b0df } else { 0 };
        }
        self.index = 0;
    }

    /// `getrandbits(k)`: k bits as CPython assembles them, low words first,
    /// the top partial word keeping its high bits.
    fn getrandbits(&mut self, k: u32) -> u128 {
        if k == 0 {
            return 0;
        }
        if k <= 32 {
            return u128::from(self.next_u32() >> (32 - k));
        }
        let words = k.div_ceil(32);
        let mut out: u128 = 0;
        let mut remaining = k;
        for i in 0..words {
            let mut word = self.next_u32();
            if remaining < 32 {
                word >>= 32 - remaining;
            }
            out |= u128::from(word) << (32 * i);
            remaining = remaining.saturating_sub(32);
        }
        out
    }

    /// `_randbelow(n)`: uniform in `0..n` by rejection over `getrandbits`,
    /// with `k = n.bit_length()` exactly as CPython computes it.
    fn randbelow(&mut self, n: usize) -> usize {
        let k = usize::BITS - n.leading_zeros();
        loop {
            let drawn = self.getrandbits(k);
            if drawn < n as u128 {
                return drawn as usize;
            }
        }
    }

    /// `shuffle`: Fisher-Yates exactly as CPython runs it.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.randbelow(i + 1);
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(kind: &str, options: &[&str]) -> Value {
        if kind == "score" {
            return serde_json::json!({
                "type": "score",
                "instructions": "How bad?",
                "criteria": options,
            });
        }
        let criteria: Map<String, Value> = options
            .iter()
            .map(|name| (name.to_string(), Value::String(format!("the {name} case"))))
            .collect();
        serde_json::json!({
            "type": kind,
            "instructions": "Which one?",
            "criteria": criteria,
        })
    }

    fn record(family: &str, kind: &str, truth: Value, state: &str) -> Map<String, Value> {
        let options: Vec<&str> = match kind {
            "choice" => vec!["billing", "technical", "sales"],
            "score" => vec!["cosmetic", "impaired", "blocking"],
            _ => vec!["true", "false"],
        };
        serde_json::json!({
            "family": family,
            "kind": kind,
            "state": state,
            "truth": truth,
            "question": question(kind, &options),
        })
        .as_object()
        .unwrap()
        .clone()
    }

    fn sample(dir: &Path) -> String {
        let mut records = Vec::new();
        for index in 0..10 {
            let truth = ["billing", "technical", "sales"][index % 3];
            records.push(record(
                "routing",
                "choice",
                Value::String(truth.to_string()),
                &format!("message {index}"),
            ));
        }
        for index in 0..6 {
            records.push(record(
                "severity",
                "score",
                Value::Number((index % 3).into()),
                &format!("incident {index}"),
            ));
        }
        for index in 0..6 {
            let mut entry = record(
                "facts",
                "noul",
                Value::Bool(index % 2 == 0),
                &format!("claim {index}"),
            );
            entry.insert(
                "question".to_string(),
                serde_json::json!({
                    "type": "noul",
                    "instructions": format!("Is claim {index} supported?"),
                    "criteria": {"true": "supported", "false": "not"},
                }),
            );
            records.push(entry);
        }
        let path = dir.join("records.jsonl");
        let text = records
            .iter()
            .map(|record| serde_json::to_string(&Value::Object(record.clone())).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, text + "\n").unwrap();
        path.to_string_lossy().to_string()
    }

    fn spec(input: String) -> Spec {
        Spec::new(
            input,
            "caller-test-v1".to_string(),
            "acme".to_string(),
            "labelled by the caller".to_string(),
            "the caller's exports".to_string(),
            "the caller retains the labels".to_string(),
        )
    }

    /// The shuffle is CPython's: `random.Random(9464)` on six elements
    /// produces this exact permutation, and only the matching MT19937
    /// pipeline gets there.
    #[test]
    fn the_partition_draw_matches_cpythons() {
        let mut order: Vec<usize> = (0..6).collect();
        PyRandom::seeded(SEED).shuffle(&mut order);
        assert_eq!(order, vec![2, 4, 1, 3, 5, 0]);
    }

    /// The committed fixture's digest is the proof that the whole pipeline
    /// — validation, keying, the draw — matches what the Python builder
    /// emitted.
    #[test]
    fn the_caller_fixture_rebuilds_to_its_pinned_digest() {
        // `label_source` and `label_rule` are digested fields, so the spec
        // names what the fixture was built with.
        let mut fixture = spec("tests/fixtures/caller-v1/records.jsonl".to_string());
        fixture.name = "caller-v1".to_string();
        fixture.label_source = "fixture-caller".to_string();
        fixture.label_rule = "written for the intake test".to_string();
        let built = build(&fixture).expect("the fixture records build");
        assert_eq!(
            built.digest,
            "ea00961656b2dac6ff9fb38297330f9946c38425250c33a65eb05097bf757572"
        );
        assert_eq!(built.items, 22);
        assert_eq!(built.family_keyed, 2);
        assert_eq!(built.item_keyed, 1);
    }

    #[test]
    fn a_built_suite_loads_and_asks() {
        let dir = std::env::temp_dir().join(format!("gym-build-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let built = build(&spec(sample(&dir))).expect("the sample builds");
        let suite = crate::suite::Suite::load(&serde_json::to_string(&built.suite).unwrap())
            .expect("the suite loads");
        assert_eq!(suite.items.len(), 22);
        let set_path = dir.join("caller-test-v1.json");
        std::fs::write(&set_path, serde_json::to_string(&built.questions).unwrap()).unwrap();
        let questions =
            crate::questions::QuestionSet::load(&set_path).expect("the question set loads");
        for item in &suite.items {
            questions.ask(item).expect("every item asks");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validation_refuses_the_same_records() {
        let dir = std::env::temp_dir().join(format!("gym-build-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut good: Vec<Map<String, Value>> = (0..10)
            .map(|index| {
                record(
                    "routing",
                    "choice",
                    Value::String("billing".to_string()),
                    &format!("message {index}"),
                )
            })
            .collect();
        // Score items fill the other families so the suite can partition.
        for index in 0..6 {
            good.push(record(
                "severity",
                "score",
                Value::Number((index % 3).into()),
                &format!("incident {index}"),
            ));
        }
        for index in 0..6 {
            good.push(record(
                "facts",
                "noul",
                Value::Bool(index % 2 == 0),
                &format!("claim {index}"),
            ));
        }
        let write = |records: &[Map<String, Value>]| -> String {
            let path = dir.join("bad.jsonl");
            let text = records
                .iter()
                .map(|r| serde_json::to_string(&Value::Object(r.clone())).unwrap())
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(&path, text + "\n").unwrap();
            path.to_string_lossy().to_string()
        };
        let cases: Vec<(Map<String, Value>, &str)> = vec![
            (
                {
                    let mut r = good[0].clone();
                    r.insert("weight".to_string(), Value::from(2));
                    r
                },
                "unknown fields",
            ),
            (
                {
                    let mut r = good[0].clone();
                    r.remove("truth");
                    r
                },
                "missing truth",
            ),
            (
                {
                    let mut r = good[0].clone();
                    r.insert("kind".to_string(), Value::String("noul".to_string()));
                    r
                },
                "does not match",
            ),
            (
                {
                    let mut r = good[0].clone();
                    r.insert("truth".to_string(), Value::String("unknown".to_string()));
                    r
                },
                "not in its own option set",
            ),
            (
                {
                    let mut r = good[10].clone();
                    r.insert("truth".to_string(), Value::from(7));
                    r
                },
                "outside 3 levels",
            ),
            (
                {
                    let mut r = good[0].clone();
                    r.insert("state".to_string(), serde_json::json!({"score": 0.5}));
                    r
                },
                "float",
            ),
            (
                {
                    let mut r = good[16].clone();
                    r.insert("id".to_string(), Value::String("routing".to_string()));
                    r
                },
                "must not be a family name",
            ),
        ];
        for (bad, pattern) in cases {
            // The bad record stands first, as the Python test wrote it.
            let mut records = vec![bad];
            records.extend_from_slice(&good[1..]);
            let path = write(&records);
            let trouble = build(&spec(path)).unwrap_err();
            assert!(trouble.contains(pattern), "{pattern}: {trouble}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn agreement_lands_in_provenance_and_validates() {
        let dir = std::env::temp_dir().join(format!("gym-build-agree-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let input = sample(&dir);
        let mut with = spec(input.clone());
        with.agreement = vec!["routing=0.91".to_string(), "severity=0.80".to_string()];
        let built = build(&with).unwrap();
        assert_eq!(
            built.suite["provenance"]["agreement"],
            serde_json::json!({"routing": "0.91", "severity": "0.80"})
        );
        // A ceiling is provenance, not an item: the digest does not move.
        let bare = build(&spec(input.clone())).unwrap();
        assert_eq!(bare.digest, built.digest);
        let mut bad = spec(input.clone());
        bad.agreement = vec!["billing=0.99".to_string()];
        assert!(build(&bad).unwrap_err().contains("not a family"));
        let mut malformed = spec(input);
        malformed.agreement = vec!["noequals".to_string()];
        assert!(build(&malformed).unwrap_err().contains("FAMILY=CEILING"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_family_too_small_to_fill_fails() {
        let dir = std::env::temp_dir().join(format!("gym-build-small-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let records = [
            record(
                "routing",
                "choice",
                Value::String("billing".to_string()),
                "one",
            ),
            record(
                "routing",
                "choice",
                Value::String("technical".to_string()),
                "two",
            ),
        ];
        let path = dir.join("small.jsonl");
        let text = records
            .iter()
            .map(|r| serde_json::to_string(&Value::Object(r.clone())).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, text + "\n").unwrap();
        assert!(
            build(&spec(path.to_string_lossy().to_string()))
                .unwrap_err()
                .contains("came out empty")
        );
        std::fs::remove_dir_all(&dir).ok();
    }
}
