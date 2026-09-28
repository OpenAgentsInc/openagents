//! NIP-XP's `reproduce` rule and the run evidence it reads
//! (`nips/openagents/NIP-XP.md`, "`reproduce`").
//!
//! A **recipe** pins how an attempt ran: the benchmark, the task, the
//! task's image, the agent, the model, its effort, and the knowledge
//! setting. **Run evidence** is a NIP-EVAL `3189` publication, marked
//! `oa:xp:run:v1`, whose subject is a recipe and whose inline report
//! carries the recipe and a bounded extract of one graded run's record,
//! with the digest of the whole record file. A **claim** is run evidence of
//! a published attempt; a **reproduction** is run evidence, signed by a
//! different key, of a new run of the claim's recipe that cites the claim.
//!
//! A `reproduce` quest pins one claim and its recipe's digest. It is
//! completed by a reproduction that passes on the same recipe with a run
//! record of its own.

use serde_json::{Map, Value, json};

use super::{
    AWARD_KIND, Award, KB_TRANSFER, QUEST_KIND, Quest, REPRODUCE, REPRODUCE_ROLES, coordinate,
    in_season, parse_quest, uniqueness_key,
};
use crate::contracts::{
    ArtifactRef, ContractError, RefusalCode, check_artifact_bytes, digest_bytes, digest_value, jcs,
    parse_artifact, parse_definition, parse_strict,
};
use crate::domain::Event;
use crate::kb::{
    self, Pointer, Unsigned, is_hex, malformed, mismatch, one_tag, reject, require, requires_empty,
    t_values, tag, text,
};

/// The marker that makes a `3189` run evidence rather than NIP-KB evidence.
pub const RUN_MARKER: &str = "oa:xp:run:v1";
/// The recipe's schema.
pub const RECIPE_SCHEMA: &str = "openagents.xp-recipe.v1";
/// The most bytes an inline run report may have.
pub const MAX_RUN_REPORT_BYTES: usize = 32 * 1024;
/// The most characters a recipe or record text field may have.
pub const MAX_FIELD_CHARS: usize = 256;

const RECIPE_KEYS: &[&str] = &[
    "v",
    "benchmark",
    "benchmark_version",
    "task",
    "image",
    "agent",
    "model",
    "effort",
    "knowledge",
];

const RECORD_KEYS: &[&str] = &[
    "task",
    "image",
    "model",
    "effort",
    "knowledge",
    "reward",
    "ending",
    "steps",
    "seconds",
    "usd",
    "summary",
];

/// How an attempt ran. Two runs with the same recipe ran the same task on
/// the same image with the same agent, model, effort, and knowledge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    /// Such as `terminal-bench`.
    pub benchmark: String,
    /// Such as `2.1`.
    pub benchmark_version: String,
    pub task: String,
    /// The task's container image, as the run record names it.
    pub image: String,
    /// Such as `microcoder`.
    pub agent: String,
    pub model: String,
    pub effort: String,
    /// The knowledge setting, such as `off`.
    pub knowledge: String,
}

impl Recipe {
    /// The recipe as its JSON object, whose canonical form has the digest a
    /// quest pins.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({
            "v": RECIPE_SCHEMA,
            "benchmark": self.benchmark,
            "benchmark_version": self.benchmark_version,
            "task": self.task,
            "image": self.image,
            "agent": self.agent,
            "model": self.model,
            "effort": self.effort,
            "knowledge": self.knowledge,
        })
    }
}

/// The bounded extract of one graded run's record that run evidence
/// carries.
#[derive(Debug, Clone, PartialEq)]
pub struct RunRecord {
    pub task: String,
    pub image: String,
    pub model: String,
    pub effort: String,
    pub knowledge: String,
    /// The grader's reward: 1 is a pass.
    pub reward: f64,
    /// How the run ended, such as `finished`.
    pub ending: Option<String>,
    pub steps: Option<u64>,
    pub seconds: Option<f64>,
    pub usd: Option<f64>,
    /// The exact bytes of the whole record file (Microcoder's
    /// `summary.json`), which the referee checks before accepting.
    pub summary: ArtifactRef,
}

impl RunRecord {
    /// Whether the grader accepted the run.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.reward >= 1.0
    }

    /// Whether the run followed `recipe`: the same task, image, model,
    /// effort, and knowledge setting.
    #[must_use]
    pub fn follows(&self, recipe: &Recipe) -> bool {
        self.task == recipe.task
            && self.image == recipe.image
            && self.model == recipe.model
            && self.effort == recipe.effort
            && self.knowledge == recipe.knowledge
    }
}

/// A verified run evidence `3189`.
#[derive(Debug, Clone, PartialEq)]
pub struct RunEvidence {
    pub recipe: Recipe,
    /// Lowercase hex SHA-256 of the recipe's canonical JSON.
    pub recipe_digest: String,
    /// The subject's qualified ID, `<owner>:recipe/<task>`.
    pub subject_id: String,
    /// The key that owns the recipe: the claimant.
    pub owner: String,
    pub record: RunRecord,
    /// `pass` or `fail`, matching the record's reward.
    pub verdict: String,
    /// The `e` tags: the claim a reproduction cites.
    pub cites: Vec<String>,
}

fn field(object: &Map<String, Value>, key: &str) -> Result<String, ContractError> {
    let value = text(object, key)?;
    if value.is_empty() || value.chars().count() > MAX_FIELD_CHARS {
        return Err(malformed(key));
    }
    Ok(value)
}

fn parse_recipe(value: &Value) -> Result<Recipe, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("recipe"))?;
    reject(object, RECIPE_KEYS)?;
    if object.get("v").and_then(Value::as_str) != Some(RECIPE_SCHEMA) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "recipe.v",
        ));
    }
    let recipe = Recipe {
        benchmark: field(object, "benchmark")?,
        benchmark_version: field(object, "benchmark_version")?,
        task: field(object, "task")?,
        image: field(object, "image")?,
        agent: field(object, "agent")?,
        model: field(object, "model")?,
        effort: field(object, "effort")?,
        knowledge: field(object, "knowledge")?,
    };
    if recipe.task.chars().any(char::is_whitespace) {
        return Err(malformed("recipe.task"));
    }
    Ok(recipe)
}

/// The lowercase hex SHA-256 of `recipe`'s canonical JSON (RFC 8785),
/// which a `reproduce` quest pins.
///
/// # Errors
///
/// When `recipe` isn't a valid recipe.
pub fn recipe_digest(recipe: &Value) -> Result<String, ContractError> {
    parse_recipe(recipe)?;
    Ok(digest_value(recipe)?
        .trim_start_matches("sha256:")
        .to_string())
}

fn recipe_artifact(recipe: &Value) -> Result<Value, ContractError> {
    let bytes = jcs(recipe)?;
    Ok(json!({
        "digest": digest_bytes(&bytes),
        "size": bytes.len(),
        "media_type": "application/json",
        "schema": RECIPE_SCHEMA,
    }))
}

fn subject_id(owner: &str, task: &str) -> String {
    let slug: String = task
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    format!("{owner}:recipe/{slug}")
}

fn optional<T>(
    object: &Map<String, Value>,
    key: &str,
    read: impl Fn(&Value) -> Option<T>,
) -> Result<Option<T>, ContractError> {
    match require(object, key)? {
        Value::Null => Ok(None),
        value => read(value).map(Some).ok_or_else(|| malformed(key)),
    }
}

fn parse_record(value: &Value) -> Result<RunRecord, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("record"))?;
    reject(object, RECORD_KEYS)?;
    let reward = require(object, "reward")?
        .as_f64()
        .filter(|r| r.is_finite() && *r >= 0.0)
        .ok_or_else(|| malformed("record.reward"))?;
    let summary = parse_artifact(require(object, "summary")?)?;
    Ok(RunRecord {
        task: field(object, "task")?,
        image: field(object, "image")?,
        model: field(object, "model")?,
        effort: field(object, "effort")?,
        knowledge: field(object, "knowledge")?,
        reward,
        ending: optional(object, "ending", |v| {
            v.as_str()
                .filter(|s| !s.is_empty() && s.chars().count() <= MAX_FIELD_CHARS)
                .map(str::to_string)
        })?,
        steps: optional(object, "steps", Value::as_u64)?,
        seconds: optional(object, "seconds", |v| {
            v.as_f64().filter(|s| s.is_finite() && *s >= 0.0)
        })?,
        usd: optional(object, "usd", |v| {
            v.as_f64().filter(|s| s.is_finite() && *s >= 0.0)
        })?,
        summary,
    })
}

/// The record extract run evidence carries, from the bytes of a
/// Microcoder run's `summary.json`: its task, image, model, effort,
/// knowledge setting, reward, how it ended, steps, seconds, and cost, and
/// the digest and size of the whole file.
///
/// # Errors
///
/// When the bytes aren't a graded run record: no task, image, model,
/// effort, knowledge setting, or reward.
pub fn record_from_summary(bytes: &[u8]) -> Result<Value, ContractError> {
    let summary = parse_strict(bytes)?;
    let get = |key: &str| summary.get(key).and_then(Value::as_str);
    let outcome = summary.get("outcome");
    let required = |value: Option<&str>, key: &str| {
        value
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| malformed(format!("summary.{key}")))
    };
    let reward = summary
        .get("reward")
        .and_then(Value::as_f64)
        .ok_or_else(|| {
            ContractError::new(
                RefusalCode::NotAdmitted,
                "the run has no reward: it wasn't graded",
            )
        })?;
    let record = json!({
        "task": required(get("task"), "task")?,
        "image": required(get("image"), "image")?,
        "model": required(get("model"), "model")?,
        "effort": required(get("effort"), "effort")?,
        "knowledge": required(get("kb"), "kb")?,
        "reward": reward,
        "ending": outcome.and_then(|o| o.pointer("/ending/reason")).and_then(Value::as_str),
        "steps": outcome.and_then(|o| o.get("steps")).and_then(Value::as_u64),
        "seconds": outcome.and_then(|o| o.get("seconds")).and_then(Value::as_f64),
        "usd": outcome.and_then(|o| o.get("known_usd").or_else(|| o.get("usd"))).and_then(Value::as_f64),
        "summary": {
            "digest": digest_bytes(bytes),
            "size": bytes.len(),
            "media_type": "application/json",
        },
    });
    parse_record(&record)?;
    Ok(record)
}

/// The referee's check of a run record: `bytes` are the exact file
/// `record` names, and the extract `record` carries is the one those bytes
/// give.
///
/// # Errors
///
/// [`RefusalCode::IdentityMismatch`] when the bytes or the extract differ.
pub fn check_run_record(record: &RunRecord, bytes: &[u8]) -> Result<(), ContractError> {
    check_artifact_bytes(&record.summary, bytes)?;
    if parse_record(&record_from_summary(bytes)?)? != *record {
        return Err(mismatch("the extract doesn't match the run record"));
    }
    Ok(())
}

/// The recipe a Microcoder run record ran, for `benchmark` at `version`.
///
/// # Errors
///
/// When the bytes aren't a graded run record.
pub fn recipe_from_summary(
    bytes: &[u8],
    benchmark: &str,
    version: &str,
) -> Result<Value, ContractError> {
    let record = parse_record(&record_from_summary(bytes)?)?;
    let recipe = json!({
        "v": RECIPE_SCHEMA,
        "benchmark": benchmark,
        "benchmark_version": version,
        "task": record.task,
        "image": record.image,
        "agent": "microcoder",
        "model": record.model,
        "effort": record.effort,
        "knowledge": record.knowledge,
    });
    parse_recipe(&recipe)?;
    Ok(recipe)
}

/// The parts of a run evidence `3189` signed by `evaluator`: the `recipe`
/// owned by `owner` (the claimant, which is `evaluator` for a claim), the
/// run `record` (see [`record_from_summary`]), and the claim event IDs it
/// `cites` (one for a reproduction, none for a claim). The verdict is
/// `pass` when the record's reward is 1 or more, else `fail`.
///
/// # Errors
///
/// When the recipe or record isn't valid, the record didn't follow the
/// recipe, or the report is too large.
pub fn run_evidence(
    evaluator: &str,
    owner: &str,
    recipe: &Value,
    record: &Value,
    cites: &[String],
) -> Result<Unsigned, ContractError> {
    let parsed_recipe = parse_recipe(recipe)?;
    let parsed_record = parse_record(record)?;
    if !parsed_record.follows(&parsed_recipe) {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "the run didn't follow the recipe: its task, image, model, effort, or knowledge differs",
        ));
    }
    if !is_hex(evaluator) || !is_hex(owner) || cites.iter().any(|id| !is_hex(id)) {
        return Err(malformed("key or cited event ID"));
    }
    let definition = json!({
        "id": subject_id(owner, &parsed_recipe.task),
        "artifact": recipe_artifact(recipe)?,
    });
    parse_definition(&definition)?;
    let verdict = if parsed_record.passed() {
        "pass"
    } else {
        "fail"
    };
    let report = json!({
        "v": kb::REPORT_SCHEMA,
        "requires": [],
        "evaluator": evaluator,
        "subject": {"definition": definition},
        "verdict": verdict,
        "meta": {"run": {"recipe": recipe, "record": record}},
    })
    .to_string();
    if report.len() > MAX_RUN_REPORT_BYTES {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "run report"));
    }
    let digest = digest_bytes(report.as_bytes());
    let content = json!({
        "v": kb::PUBLICATION_VERSION,
        "requires": [],
        "report": {
            "digest": digest,
            "size": report.len(),
            "media_type": "application/json",
            "schema": kb::REPORT_SCHEMA,
        },
        "subject": definition,
        "supersedes": [],
        "meta": {"run_report": report},
    });
    let mut tags = vec![
        tag(&["t", kb::EVAL_MARKER]),
        tag(&["t", RUN_MARKER]),
        tag(&["x", digest.trim_start_matches("sha256:")]),
    ];
    for id in cites {
        tags.push(tag(&["e", id]));
    }
    Ok(Unsigned {
        kind: kb::EVIDENCE_KIND,
        tags,
        content: content.to_string(),
    })
}

/// Checks a signed run evidence `3189`: the NIP-EVAL envelope, the inline
/// report against its digest, the signer as the report's evaluator, a
/// subject that is exactly the recipe the report carries, a record that
/// followed that recipe, and a verdict that matches the record's reward.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_run_evidence(event: &Event) -> Result<RunEvidence, ContractError> {
    if event.kind != kb::EVIDENCE_KIND {
        return Err(mismatch("kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| mismatch("event signature"))?;
    let mut markers: Vec<&str> = t_values(event).filter(|t| t.starts_with("oa:")).collect();
    markers.sort_unstable();
    if markers != [kb::EVAL_MARKER, RUN_MARKER] {
        return Err(mismatch("run evidence tags"));
    }
    let value = parse_strict(event.content.as_bytes())?;
    let object = value.as_object().ok_or_else(|| malformed("publication"))?;
    reject(
        object,
        &["v", "requires", "report", "subject", "supersedes", "meta"],
    )?;
    if object.get("v").and_then(Value::as_str) != Some(kb::PUBLICATION_VERSION) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(object)?;
    let report_ref = parse_artifact(require(object, "report")?)?;
    if one_tag(event, "x")? != report_ref.digest.trim_start_matches("sha256:") {
        return Err(mismatch("x tag"));
    }
    let subject_value = require(object, "subject")?;
    let subject = parse_definition(subject_value)?;
    if subject.event.is_some() {
        return Err(malformed("subject.event"));
    }
    if require(object, "supersedes")?
        .as_array()
        .is_none_or(|a| !a.is_empty())
    {
        return Err(malformed("supersedes"));
    }
    let meta = require(object, "meta")?
        .as_object()
        .ok_or_else(|| malformed("meta"))?;
    reject(meta, &["run_report"])?;
    let report_text = meta
        .get("run_report")
        .and_then(Value::as_str)
        .ok_or_else(|| ContractError::new(RefusalCode::ContentUnavailable, "meta.run_report"))?;
    if report_text.len() > MAX_RUN_REPORT_BYTES {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "run report"));
    }
    check_artifact_bytes(&report_ref, report_text.as_bytes())?;
    let report = parse_strict(report_text.as_bytes())?;
    let report = report.as_object().ok_or_else(|| malformed("report"))?;
    reject(
        report,
        &["v", "requires", "evaluator", "subject", "verdict", "meta"],
    )?;
    if report.get("v").and_then(Value::as_str) != Some(kb::REPORT_SCHEMA) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "report.v",
        ));
    }
    requires_empty(report)?;
    if report.get("evaluator").and_then(Value::as_str) != Some(event.pubkey.as_str()) {
        return Err(mismatch("evaluator"));
    }
    if report.get("subject").and_then(|s| s.get("definition")) != Some(subject_value) {
        return Err(mismatch("report subject"));
    }
    let run = report
        .get("meta")
        .and_then(|m| m.get("run"))
        .and_then(Value::as_object)
        .ok_or_else(|| malformed("report.meta.run"))?;
    reject(run, &["recipe", "record"])?;
    let recipe_value = require(run, "recipe")?;
    let recipe = parse_recipe(recipe_value)?;
    check_artifact_bytes(&subject.artifact, &jcs(recipe_value)?)?;
    debug_assert_eq!(&recipe.to_value(), recipe_value);
    if subject.artifact.schema.as_deref() != Some(RECIPE_SCHEMA) {
        return Err(mismatch("subject schema"));
    }
    let owner = subject
        .id
        .split_once(':')
        .map(|(owner, _)| owner.to_string())
        .unwrap_or_default();
    if subject.id != subject_id(&owner, &recipe.task) {
        return Err(mismatch("subject ID"));
    }
    let record = parse_record(require(run, "record")?)?;
    if !record.follows(&recipe) {
        return Err(mismatch("the record didn't follow the recipe"));
    }
    let verdict = text(report, "verdict")?;
    let expected = if record.passed() { "pass" } else { "fail" };
    if verdict != expected {
        return Err(mismatch("the verdict disagrees with the record's reward"));
    }
    Ok(RunEvidence {
        recipe_digest: subject
            .artifact
            .digest
            .trim_start_matches("sha256:")
            .to_string(),
        recipe,
        subject_id: subject.id,
        owner,
        record,
        verdict,
        cites: event.tag_values("e").map(str::to_string).collect(),
    })
}

/// The `reproduce` rule. A reproduction completes the quest when all hold:
///
/// 1. `claim` is the exact event the quest pins, and valid run evidence
///    whose recipe has the quest's digest and task, owned by its signer,
///    that passed.
/// 2. `reproduction` is valid run evidence of the same recipe that cites
///    the claim.
/// 3. The reproducer (its signer) isn't the claimant.
/// 4. The reproduction passed, and its run record isn't the claim's.
/// 5. The reproduction was published inside the season, and not before
///    the claim.
///
/// Returns the claim and the reproduction as parsed.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] when the completion fails the rule; other
/// codes when an event is invalid or isn't the one the quest names.
pub fn check_reproduce(
    quest: &Quest,
    claim: &Event,
    reproduction: &Event,
) -> Result<(RunEvidence, RunEvidence), ContractError> {
    let acceptance = &quest.acceptance;
    let (Some(pinned), Some(digest)) = (&acceptance.claim, &acceptance.recipe) else {
        return Err(mismatch("the quest's rule isn't reproduce"));
    };
    if pinned.id != claim.id || pinned.pubkey != claim.pubkey {
        return Err(mismatch("the claim isn't the one the quest pins"));
    }
    let claimed = parse_run_evidence(claim).map_err(|e| context("the claim", e))?;
    if &claimed.recipe_digest != digest {
        return Err(mismatch("the claim's recipe isn't the quest's"));
    }
    if claimed.recipe.task != acceptance.task {
        return Err(mismatch("the claim's task isn't the quest's"));
    }
    if claimed.owner != claim.pubkey {
        return Err(mismatch("the claim's recipe belongs to another key"));
    }
    let refuse = |why: String| Err(ContractError::new(RefusalCode::NotAdmitted, why));
    if !claimed.record.passed() {
        return refuse("the claimed attempt didn't pass".into());
    }
    let reproduced =
        parse_run_evidence(reproduction).map_err(|e| context("the reproduction", e))?;
    if reproduced.subject_id != claimed.subject_id || reproduced.recipe_digest != *digest {
        return Err(mismatch("the reproduction ran another recipe"));
    }
    if !reproduced.cites.iter().any(|id| id == &claim.id) {
        return Err(mismatch("the reproduction doesn't cite the claim"));
    }
    if reproduction.pubkey == claim.pubkey {
        return refuse(
            "the reproducer is the claimant: reproducing your own attempt earns nothing".into(),
        );
    }
    if !reproduced.record.passed() {
        return refuse(format!(
            "the reproduction's run didn't pass: its reward is {}",
            reproduced.record.reward
        ));
    }
    if reproduced.record.summary.digest == claimed.record.summary.digest {
        return refuse("the reproduction's run record is the claim's own".into());
    }
    let season = &quest.season;
    if reproduction.created_at < season.opens_at || reproduction.created_at > season.closes_at {
        return refuse(format!(
            "the reproduction isn't inside season {}",
            season.id
        ));
    }
    if reproduction.created_at < claim.created_at {
        return refuse("the reproduction is older than the claim it reproduces".into());
    }
    Ok((claimed, reproduced))
}

fn context(what: &str, error: ContractError) -> ContractError {
    ContractError::new(error.code, format!("{what}: {}", error.detail))
}

/// The parts of a `3193` accepting `reproduction` of the signed `quest`'s
/// claim. The award is only built when [`check_reproduce`] passes.
///
/// # Errors
///
/// When an event isn't valid, the completion fails the rule, or
/// `accepted_at` is outside the season or before the reproduction.
pub fn reproduce_award(
    quest: &Event,
    claim: &Event,
    reproduction: &Event,
    accepted_at: u64,
) -> Result<Unsigned, ContractError> {
    let parsed = parse_quest(quest)?;
    if parsed.acceptance.rule != REPRODUCE {
        return Err(mismatch("the quest's rule isn't reproduce"));
    }
    in_season(&parsed, accepted_at)?;
    check_reproduce(&parsed, claim, reproduction)?;
    if reproduction.created_at > accepted_at {
        return Err(mismatch("the reproduction is newer than its acceptance"));
    }
    let coordinate = coordinate(&quest.pubkey, &parsed.address);
    let key = uniqueness_key(&quest.pubkey, &parsed, &reproduction.pubkey);
    let keys = [&claim.pubkey, &reproduction.pubkey];
    let awardees: Vec<Value> = REPRODUCE_ROLES
        .iter()
        .zip(keys)
        .map(|(role, pubkey)| json!({"role": role, "pubkey": pubkey, "xp": parsed.award[*role]}))
        .collect();
    let content = json!({
        "v": 1, "requires": [], "type": "award",
        "quest": {"id": quest.id, "pubkey": quest.pubkey, "kind": QUEST_KIND, "coordinate": coordinate},
        "key": key,
        "accepted_at": accepted_at,
        "evidence": [
            {"id": claim.id, "pubkey": claim.pubkey, "kind": kb::EVIDENCE_KIND},
            {"id": reproduction.id, "pubkey": reproduction.pubkey, "kind": kb::EVIDENCE_KIND},
        ],
        "awardees": awardees,
    });
    Ok(Unsigned {
        kind: AWARD_KIND,
        tags: vec![
            tag(&["t", "oa:xp:award:v1"]),
            tag(&["a", &coordinate]),
            tag(&["e", &quest.id]),
            tag(&["e", &claim.id]),
            tag(&["e", &reproduction.id]),
            tag(&["p", &claim.pubkey]),
            tag(&["p", &reproduction.pubkey]),
        ],
        content: content.to_string(),
    })
}

/// Checks a parsed `reproduce` award against the signed claim and
/// reproduction it names, then the rule over them.
///
/// # Errors
///
/// As [`check_reproduce`], and [`RefusalCode::IdentityMismatch`] when an
/// event isn't the one the award names.
pub fn bind_reproduction(
    award: &Award,
    quest: &Quest,
    claim: &Event,
    reproduction: &Event,
) -> Result<(), ContractError> {
    if award.rule == KB_TRANSFER || award.evidence.len() != 2 {
        return Err(mismatch("the award isn't a reproduce award"));
    }
    let named =
        |pointer: &Pointer, event: &Event| pointer.id == event.id && pointer.pubkey == event.pubkey;
    if !named(&award.evidence[0], claim) {
        return Err(mismatch("claim"));
    }
    if !named(&award.evidence[1], reproduction) {
        return Err(mismatch("reproduction"));
    }
    check_reproduce(quest, claim, reproduction)?;
    if reproduction.created_at > award.accepted_at {
        return Err(mismatch("the reproduction is newer than its acceptance"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
