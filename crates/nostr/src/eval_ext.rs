//! NIP-EVAL's extension evaluation profile (`nips/openagents/NIP-EVAL.md`,
//! "Extension evaluation profile"; `docs/extensions/evaluation.md`).
//!
//! An extension evaluation runs a suite of cases against an agent with one
//! extension admitted (the `subject` arm) and with it absent (the
//! `baseline` arm). This module holds the profile's wire records and
//! nothing that runs them:
//!
//! - the **case manifest** (`openagents.eval-case.v1`): the suite's `cases`
//!   artifact, one entry per case with the exact bytes of its files;
//! - the **suite** (`openagents.eval-suite.v1` with `purpose: "operation"`)
//!   and the NIP-EXT package that publishes it as an `eval-suite`
//!   component;
//! - the **profile record** `meta.ext_eval` (`openagents.ext-eval.v1`) a
//!   report carries;
//! - the **result publication**: a `3189` with the `oa:ext-eval:v1`
//!   marker, its report inline in `meta.ext_eval_report`;
//! - **check linkage** ([`confirms`]): whether one publication confirms or
//!   disputes another, from the two signed events alone.
//!
//! No kind is allocated: suites ride NIP-EXT `3184`/`3185`, results NIP-EVAL
//! `3189`, hosted runs NIP-CJ execution jobs, and credit NIP-XP
//! (`xp::eval_check`, `xp::eval_adopt`).

use std::collections::BTreeSet;

use serde_json::{Value, json};

use crate::contracts::{
    ArtifactRef, ContractError, DefinitionRef, RefusalCode, check_artifact_bytes, digest_bytes,
    jcs, parse_artifact, parse_definition, parse_strict,
};
use crate::domain::Event;
use crate::ext;
use crate::kb::{
    self, Pointer, Unsigned, is_hex, malformed, mismatch, reject, require, requires_empty,
    t_values, tag, text, unsupported,
};

/// The case manifest's schema, and the schema every case file's
/// ArtifactRef names.
pub const CASE_SCHEMA: &str = "openagents.eval-case.v1";
/// NIP-EVAL's suite schema.
pub const SUITE_SCHEMA: &str = "openagents.eval-suite.v1";
/// The profile record in a report's `meta.ext_eval`.
pub const PROFILE_SCHEMA: &str = "openagents.ext-eval.v1";
/// NIP-EVAL's promotion decision, which adoption cites.
pub const ADMISSION_SCHEMA: &str = "openagents.eval-admission.v1";
/// The `t` marker a result publication carries beside `oa:eval:v1`.
pub const PROFILE_MARKER: &str = "oa:ext-eval:v1";
/// The NIP-EXT component kind a published suite has.
pub const COMPONENT_KIND: &str = "eval-suite";
/// The component name of the Gym gate that decides the verdict.
pub const GATE: &str = "ext-eval-v1";
/// The most bytes a report inline in a publication may have.
pub const MAX_REPORT_BYTES: usize = 64 * 1024;
/// The most cases a suite may hold.
pub const MAX_CASES: usize = 256;
/// Runs per arm, at most; the default is [`DEFAULT_RUNS`].
pub const MAX_RUNS: u64 = 10;
/// Runs per arm when a case names none.
pub const DEFAULT_RUNS: u64 = 3;
/// Grader files per case, at most.
pub const MAX_GRADERS: usize = 64;
/// Fixture files per case, at most.
pub const MAX_FIXTURES: usize = 256;
/// Bytes one case file may have, at most.
pub const MAX_CASE_FILE_BYTES: u64 = 1024 * 1024;
/// The hosted runner's bounds: cases, runs per arm, and arms.
pub const HOSTED_MAX_CASES: u64 = 8;
/// See [`HOSTED_MAX_CASES`].
pub const HOSTED_MAX_RUNS: u64 = 3;
/// See [`HOSTED_MAX_CASES`].
pub const HOSTED_ARMS: u64 = 2;
/// Measurements a report may carry, at most.
pub const MAX_MEASUREMENTS: usize = 1_024;

/// The `e` tag markers a result publication uses: the suite's release, the
/// subject's release, the publication a check checks, and a hosted run's
/// request.
pub const E_MARKERS: &[&str] = &["suite", "subject", "check", "request"];

/// Case directory names discovery skips, so a case can't be named one.
const RESERVED: &[&str] = &[".git", ".openagents", "node_modules", "results"];

/// Whether the extension ought to be used on a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseKind {
    /// The extension should help: "a test where the tool should help".
    ShouldFire,
    /// The extension should stay out of the way.
    ShouldNotFire,
}

impl CaseKind {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            CaseKind::ShouldFire => "should-fire",
            CaseKind::ShouldNotFire => "should-not-fire",
        }
    }

    /// The kind a word names; an unknown word is `None`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "should-fire" => Some(CaseKind::ShouldFire),
            "should-not-fire" => Some(CaseKind::ShouldNotFire),
            _ => None,
        }
    }
}

/// A report's verdict under the `ext-eval-v1` gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The gate keeps the extension: **Better**.
    Pass,
    /// The gate rejects it: **Worse**.
    Fail,
    /// **No clear change**.
    Inconclusive,
}

impl Verdict {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
            Verdict::Inconclusive => "inconclusive",
        }
    }

    /// The verdict a word names; an unknown word is `None`.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "pass" => Some(Verdict::Pass),
            "fail" => Some(Verdict::Fail),
            "inconclusive" => Some(Verdict::Inconclusive),
            _ => None,
        }
    }
}

/// One named file of a case: a grader or a fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseFile {
    /// A grader's name, or a fixture's path below `fixtures/`.
    pub name: String,
    pub artifact: ArtifactRef,
}

/// One case entry of a case manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseEntry {
    /// The case directory's name.
    pub id: String,
    pub kind: CaseKind,
    /// Runs per arm.
    pub runs: u64,
    /// `prompt.md`.
    pub prompt: ArtifactRef,
    /// `case.toml`, when the case has one.
    pub config: Option<ArtifactRef>,
    /// `graders/<name>.md`, in name order.
    pub graders: Vec<CaseFile>,
    /// `fixtures/<path>`, in path order.
    pub fixtures: Vec<CaseFile>,
}

impl CaseEntry {
    /// Every file of the case, prompt first.
    pub fn files(&self) -> impl Iterator<Item = &ArtifactRef> {
        std::iter::once(&self.prompt)
            .chain(self.config.iter())
            .chain(self.graders.iter().map(|g| &g.artifact))
            .chain(self.fixtures.iter().map(|f| &f.artifact))
    }
}

/// A verified case manifest: a suite's `cases` artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseManifest {
    /// In lexicographic order of `id`.
    pub cases: Vec<CaseEntry>,
}

impl CaseManifest {
    /// The number of should-not-fire cases.
    #[must_use]
    pub fn should_not_fire(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.kind == CaseKind::ShouldNotFire)
            .count()
    }
}

/// Whether `id` may name a case: a plain directory name of ASCII letters,
/// digits, `-`, `_`, and `.`, not starting with `.`, at most 128 bytes, and
/// not one discovery skips.
#[must_use]
pub fn valid_case_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && !RESERVED.contains(&id)
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

fn valid_grader_name(name: &str) -> bool {
    valid_case_id(name) && name.len() <= 64
}

/// A relative path of plain components: no `.`, `..`, empty, or absolute
/// part, no backslash or control character, at most 512 bytes.
fn valid_relative_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && !path.starts_with('/')
        && !path.chars().any(|c| c == '\\' || c.is_control())
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn case_artifact(value: &Value, what: &str) -> Result<ArtifactRef, ContractError> {
    let artifact = parse_artifact(value)?;
    if artifact.schema.as_deref() != Some(CASE_SCHEMA) {
        return Err(mismatch(format!("{what}: schema isn't {CASE_SCHEMA}")));
    }
    if artifact.size > MAX_CASE_FILE_BYTES {
        return Err(ContractError::new(RefusalCode::LimitExceeded, what));
    }
    Ok(artifact)
}

fn named_files(
    value: &Value,
    key: &str,
    max: usize,
    check_name: fn(&str) -> bool,
    schema: bool,
) -> Result<Vec<CaseFile>, ContractError> {
    let items = value.as_array().ok_or_else(|| malformed(key))?;
    if items.len() > max {
        return Err(ContractError::new(RefusalCode::LimitExceeded, key));
    }
    let field = if key == "graders" { "name" } else { "path" };
    let mut out: Vec<CaseFile> = Vec::new();
    for item in items {
        let object = item.as_object().ok_or_else(|| malformed(key))?;
        reject(object, &[field, "artifact"])?;
        let name = text(object, field)?;
        if !check_name(&name) {
            return Err(malformed(format!("{key}.{field}")));
        }
        let artifact = if schema {
            case_artifact(require(object, "artifact")?, key)?
        } else {
            let artifact = parse_artifact(require(object, "artifact")?)?;
            if artifact.size > MAX_CASE_FILE_BYTES {
                return Err(ContractError::new(RefusalCode::LimitExceeded, key));
            }
            artifact
        };
        if out.last().is_some_and(|last| last.name >= name) {
            return Err(malformed(format!("{key}: names are unique and in order")));
        }
        out.push(CaseFile { name, artifact });
    }
    Ok(out)
}

fn case_entry(value: &Value) -> Result<CaseEntry, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("case"))?;
    reject(
        object,
        &[
            "id", "kind", "runs", "prompt", "config", "graders", "fixtures",
        ],
    )?;
    let id = text(object, "id")?;
    if !valid_case_id(&id) {
        return Err(malformed("case.id"));
    }
    let kind = CaseKind::parse(&text(object, "kind")?).ok_or_else(|| unsupported("case.kind"))?;
    let runs = require(object, "runs")?
        .as_u64()
        .filter(|r| (1..=MAX_RUNS).contains(r))
        .ok_or_else(|| malformed("case.runs"))?;
    let prompt = case_artifact(require(object, "prompt")?, "case.prompt")?;
    let config = match require(object, "config")? {
        Value::Null => None,
        value => Some(case_artifact(value, "case.config")?),
    };
    let graders = named_files(
        require(object, "graders")?,
        "graders",
        MAX_GRADERS,
        valid_grader_name,
        true,
    )?;
    let fixtures = named_files(
        require(object, "fixtures")?,
        "fixtures",
        MAX_FIXTURES,
        valid_relative_path,
        false,
    )?;
    if graders.is_empty() && config.is_none() {
        return Err(malformed(
            "case: a case needs at least one grader, as a file or in case.toml",
        ));
    }
    Ok(CaseEntry {
        id,
        kind,
        runs,
        prompt,
        config,
        graders,
        fixtures,
    })
}

/// The canonical bytes (RFC 8785) of a case manifest from its entries,
/// after checking them. The bytes' digest is the suite's `cases`
/// ArtifactRef.
///
/// # Errors
///
/// When an entry is invalid, two cases share an ID, or the cases aren't in
/// lexicographic order.
pub fn case_manifest(cases: &[Value]) -> Result<Vec<u8>, ContractError> {
    let value = json!({"v": CASE_SCHEMA, "requires": [], "cases": cases});
    let bytes = jcs(&value)?;
    parse_case_manifest(&bytes)?;
    Ok(bytes)
}

/// Checks a case manifest's bytes.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_case_manifest(bytes: &[u8]) -> Result<CaseManifest, ContractError> {
    let value = parse_strict(bytes)?;
    parse_case_manifest_value(&value)
}

/// Checks a case manifest already parsed as JSON.
///
/// # Errors
///
/// As [`parse_case_manifest`].
pub fn parse_case_manifest_value(value: &Value) -> Result<CaseManifest, ContractError> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed("case manifest"))?;
    reject(object, &["v", "requires", "cases", "meta"])?;
    if object.get("v").and_then(Value::as_str) != Some(CASE_SCHEMA) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(object)?;
    let items = require(object, "cases")?
        .as_array()
        .ok_or_else(|| malformed("cases"))?;
    if items.is_empty() {
        return Err(malformed("cases: a suite has at least one case"));
    }
    if items.len() > MAX_CASES {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "cases"));
    }
    let mut cases: Vec<CaseEntry> = Vec::new();
    for item in items {
        let case = case_entry(item)?;
        if let Some(last) = cases.last() {
            if last.id == case.id {
                return Err(ContractError::new(
                    RefusalCode::Conflict,
                    format!("two cases are named {}", case.id),
                ));
            }
            if last.id > case.id {
                return Err(malformed("cases: in lexicographic order of id"));
            }
        }
        cases.push(case);
    }
    Ok(CaseManifest { cases })
}

/// A verified extension suite (`openagents.eval-suite.v1`,
/// `purpose: "operation"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suite {
    /// The suite's qualified component ID.
    pub id: String,
    pub workload: ArtifactRef,
    /// The case manifest.
    pub cases: ArtifactRef,
    pub partition: ArtifactRef,
    /// Names the suite author as the label source.
    pub labels: ArtifactRef,
    pub metrics: ArtifactRef,
    /// The Gym gate `ext-eval-v1`.
    pub acceptance: DefinitionRef,
    pub environment: ArtifactRef,
}

/// Checks a suite's bytes as the profile reads them.
///
/// # Errors
///
/// A typed refusal naming the first check that failed: another purpose, a
/// `cases` artifact of another schema, or an acceptance that isn't the
/// `ext-eval-v1` gate.
pub fn parse_suite(bytes: &[u8]) -> Result<Suite, ContractError> {
    let value = parse_strict(bytes)?;
    let object = value.as_object().ok_or_else(|| malformed("suite"))?;
    reject(
        object,
        &[
            "v",
            "requires",
            "id",
            "purpose",
            "workload",
            "cases",
            "partition",
            "labels",
            "metrics",
            "acceptance",
            "environment",
            "meta",
        ],
    )?;
    if object.get("v").and_then(Value::as_str) != Some(SUITE_SCHEMA) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(object)?;
    if object.get("purpose").and_then(Value::as_str) != Some("operation") {
        return Err(unsupported("purpose: an extension suite's is operation"));
    }
    let id = text(object, "id")?;
    let acceptance = parse_definition(require(object, "acceptance")?)?;
    if acceptance.id.rsplit('/').next() != Some(GATE) {
        return Err(mismatch(format!("acceptance: the gate is {GATE}")));
    }
    let cases = parse_artifact(require(object, "cases")?)?;
    if cases.schema.as_deref() != Some(CASE_SCHEMA) {
        return Err(mismatch("cases: schema"));
    }
    let artifact = |key: &str| parse_artifact(require(object, key)?);
    let suite = Suite {
        id,
        workload: artifact("workload")?,
        cases,
        partition: artifact("partition")?,
        labels: artifact("labels")?,
        metrics: artifact("metrics")?,
        acceptance,
        environment: artifact("environment")?,
    };
    if !valid_qualified(&suite.id) {
        return Err(malformed("id"));
    }
    Ok(suite)
}

/// A published suite's package, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuitePackage {
    /// The package manifest as NIP-EXT reads it.
    pub manifest: ext::Manifest,
    /// The `eval-suite` component's slug.
    pub slug: String,
    pub suite: Suite,
    pub cases: CaseManifest,
}

/// Checks a NIP-EXT package manifest that publishes a suite: exactly one
/// component of kind `eval-suite`, whose definition is `suite` (an
/// `openagents.eval-suite.v1`), whose case manifest is `cases`, and whose
/// every case file is a listed file of the package. It checks bytes and
/// listings only; the files' own bytes are NIP-EXT's closure check
/// (`ext::verify_closure`).
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn check_suite_package(
    manifest: &Value,
    suite: &[u8],
    cases: &[u8],
) -> Result<SuitePackage, ContractError> {
    let parsed = ext::parse_manifest(manifest)?;
    let components = manifest
        .get("components")
        .and_then(Value::as_array)
        .ok_or_else(|| malformed("components"))?;
    let suites: Vec<&Value> = components
        .iter()
        .filter(|c| c.get("kind").and_then(Value::as_str) == Some(COMPONENT_KIND))
        .collect();
    let [component] = suites.as_slice() else {
        return Err(malformed(format!(
            "a suite package has exactly one {COMPONENT_KIND} component"
        )));
    };
    let slug = component
        .get("slug")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("component.slug"))?
        .to_string();
    let definition = parse_artifact(
        component
            .get("definition")
            .ok_or_else(|| malformed("component.definition"))?,
    )?;
    if definition.schema.as_deref() != Some(SUITE_SCHEMA) {
        return Err(mismatch("the eval-suite definition's schema"));
    }
    check_artifact_bytes(&definition, suite)?;
    let parsed_suite = parse_suite(suite)?;
    check_artifact_bytes(&parsed_suite.cases, cases)?;
    let parsed_cases = parse_case_manifest(cases)?;
    let listed: BTreeSet<(&str, u64)> = parsed
        .files
        .iter()
        .map(|f| (f.digest.as_str(), f.size))
        .collect();
    let needed = std::iter::once(&parsed_suite.cases)
        .chain(parsed_cases.cases.iter().flat_map(CaseEntry::files));
    for artifact in needed {
        if !listed.contains(&(artifact.digest.as_str(), artifact.size)) {
            return Err(ContractError::new(
                RefusalCode::ContentUnavailable,
                format!("a case file isn't listed: {}", artifact.digest),
            ));
        }
    }
    Ok(SuitePackage {
        manifest: parsed,
        slug,
        suite: parsed_suite,
        cases: parsed_cases,
    })
}

/// An exact event a record names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPointer {
    pub id: String,
    pub pubkey: String,
    pub kind: u16,
}

impl EventPointer {
    /// As a `{id, pubkey, kind}` object.
    #[must_use]
    pub fn to_value(&self) -> Value {
        json!({"id": self.id, "pubkey": self.pubkey, "kind": self.kind})
    }

    /// As a [`Pointer`].
    #[must_use]
    pub fn pointer(&self) -> Pointer {
        Pointer {
            id: self.id.clone(),
            pubkey: self.pubkey.clone(),
        }
    }
}

/// Reads a closed `{id, pubkey, kind}` object whose kind is `kind`.
///
/// # Errors
///
/// When it isn't one.
pub fn event_pointer(value: &Value, kind: u16, what: &str) -> Result<EventPointer, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed(what))?;
    reject(object, &["id", "pubkey", "kind"])?;
    let pointer = EventPointer {
        id: text(object, "id")?,
        pubkey: text(object, "pubkey")?,
        kind: object
            .get("kind")
            .and_then(Value::as_u64)
            .and_then(|k| u16::try_from(k).ok())
            .ok_or_else(|| malformed(format!("{what}.kind")))?,
    };
    if !is_hex(&pointer.id) || !is_hex(&pointer.pubkey) {
        return Err(malformed(what));
    }
    if pointer.kind != kind {
        return Err(mismatch(format!("{what}.kind")));
    }
    Ok(pointer)
}

/// The headline counts: cases passed per arm, out of the cases scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Headline {
    pub subject_passed: u64,
    /// `None` when the report has no baseline.
    pub baseline_passed: Option<u64>,
    pub total: u64,
}

/// A verified `meta.ext_eval` (`openagents.ext-eval.v1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// The `ext-eval-v1` gate's digest.
    pub gate: String,
    /// Each case's ID and kind, in the case manifest's order.
    pub cases: Vec<(String, CaseKind)>,
    pub headline: Headline,
    /// The signed NIP-CJ execution request a hosted runner served.
    pub requester: Option<EventPointer>,
}

/// The `meta.ext_eval` object for `profile`.
#[must_use]
pub fn profile_value(profile: &Profile) -> Value {
    json!({
        "v": PROFILE_SCHEMA,
        "gate": profile.gate,
        "cases": profile.cases.iter().map(|(id, kind)| json!({"id": id, "kind": kind.word()})).collect::<Vec<_>>(),
        "headline": {
            "subject_passed": profile.headline.subject_passed,
            "baseline_passed": profile.headline.baseline_passed,
            "total": profile.headline.total,
        },
        "requester": profile.requester.as_ref().map(EventPointer::to_value),
    })
}

fn digest_text(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(is_hex)
}

/// Checks a `meta.ext_eval` object.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_profile(value: &Value) -> Result<Profile, ContractError> {
    let object = value
        .as_object()
        .ok_or_else(|| malformed("meta.ext_eval"))?;
    reject(object, &["v", "gate", "cases", "headline", "requester"])?;
    if object.get("v").and_then(Value::as_str) != Some(PROFILE_SCHEMA) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "meta.ext_eval.v",
        ));
    }
    let gate = text(object, "gate")?;
    if !digest_text(&gate) {
        return Err(malformed("meta.ext_eval.gate"));
    }
    let items = require(object, "cases")?
        .as_array()
        .ok_or_else(|| malformed("meta.ext_eval.cases"))?;
    if items.is_empty() || items.len() > MAX_CASES {
        return Err(malformed("meta.ext_eval.cases"));
    }
    let mut cases = Vec::new();
    let mut seen = BTreeSet::new();
    for item in items {
        let case = item.as_object().ok_or_else(|| malformed("case"))?;
        reject(case, &["id", "kind"])?;
        let id = text(case, "id")?;
        if !valid_case_id(&id) || !seen.insert(id.clone()) {
            return Err(malformed("meta.ext_eval.cases.id"));
        }
        let kind = CaseKind::parse(&text(case, "kind")?).ok_or_else(|| unsupported("case kind"))?;
        cases.push((id, kind));
    }
    let headline = require(object, "headline")?
        .as_object()
        .ok_or_else(|| malformed("headline"))?;
    reject(headline, &["subject_passed", "baseline_passed", "total"])?;
    let count = |key: &str| {
        headline
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("headline.{key}")))
    };
    let total = count("total")?;
    let subject_passed = count("subject_passed")?;
    let baseline_passed = match require(headline, "baseline_passed")? {
        Value::Null => None,
        value => Some(
            value
                .as_u64()
                .ok_or_else(|| malformed("headline.baseline_passed"))?,
        ),
    };
    if total != cases.len() as u64
        || subject_passed > total
        || baseline_passed.is_some_and(|b| b > total)
    {
        return Err(mismatch(
            "headline: total is the case count, and no arm passes more",
        ));
    }
    let requester = match require(object, "requester")? {
        Value::Null => None,
        value => Some(event_pointer(
            value,
            crate::kinds::CJ_EXECUTION_REQUEST,
            "requester",
        )?),
    };
    Ok(Profile {
        gate,
        cases,
        headline: Headline {
            subject_passed,
            baseline_passed,
            total,
        },
        requester,
    })
}

/// One arm of a report: what ran and the lock it held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arm {
    pub definition: DefinitionRef,
    /// The run lock the arm held.
    pub lock: ArtifactRef,
    pub configuration: ArtifactRef,
}

/// A verified extension evaluation report
/// (`openagents.eval-report.v1` under the profile).
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// The suite, with its release EventRef when published.
    pub suite: ArtifactRef,
    pub subject: Arm,
    /// `None` when the report ran the subject arm alone.
    pub baseline: Option<Arm>,
    pub evaluator: String,
    pub started_at: u64,
    pub ended_at: u64,
    pub verdict: Verdict,
    pub profile: Profile,
}

fn arm(value: &Value, what: &str) -> Result<Arm, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed(what))?;
    reject(object, &["definition", "lock", "configuration"])?;
    Ok(Arm {
        definition: parse_definition(require(object, "definition")?)?,
        lock: parse_artifact(require(object, "lock")?)?,
        configuration: parse_artifact(require(object, "configuration")?)?,
    })
}

const COUNTS: &[&str] = &[
    "planned",
    "attempted",
    "completed",
    "refused",
    "failed",
    "cancelled",
    "unknown",
    "excluded",
];

fn coverage(value: &Value, what: &str) -> Result<(), ContractError> {
    let object = value.as_object().ok_or_else(|| malformed(what))?;
    reject(object, COUNTS)?;
    let mut counts = std::collections::BTreeMap::new();
    for key in COUNTS {
        let count = object
            .get(*key)
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("{what}.{key}")))?;
        counts.insert(*key, count);
    }
    let terminal: u64 = ["completed", "refused", "failed", "cancelled", "unknown"]
        .iter()
        .map(|k| counts[k])
        .sum();
    if terminal != counts["attempted"] {
        return Err(mismatch(format!(
            "{what}: the five outcome counts sum to attempted"
        )));
    }
    Ok(())
}

fn measurement(value: &Value, baseline: bool) -> Result<(), ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("measurement"))?;
    reject(
        object,
        &[
            "arm",
            "metric",
            "value",
            "denominator",
            "unknown_count",
            "uncertainty",
            "evidence",
        ],
    )?;
    match text(object, "arm")?.as_str() {
        "subject" => {}
        "baseline" | "comparison" if baseline => {}
        "baseline" | "comparison" => {
            return Err(mismatch("measurement: no baseline to measure"));
        }
        _ => return Err(unsupported("measurement.arm")),
    }
    let metric = text(object, "metric")?;
    if metric.is_empty() || metric.len() > 128 {
        return Err(malformed("measurement.metric"));
    }
    match require(object, "value")? {
        Value::Null => {}
        Value::Number(n) if n.as_f64().is_some_and(f64::is_finite) => {}
        _ => return Err(malformed("measurement.value")),
    }
    for key in ["denominator", "unknown_count"] {
        require(object, key)?
            .as_u64()
            .ok_or_else(|| malformed(format!("measurement.{key}")))?;
    }
    match require(object, "uncertainty")? {
        Value::Null => {}
        value => {
            parse_artifact(value)?;
        }
    }
    require(object, "evidence")?
        .as_array()
        .ok_or_else(|| malformed("measurement.evidence"))?;
    Ok(())
}

/// Checks a report's bytes under the profile: NIP-EVAL's report fields,
/// both arms' shapes, coverage that sums, measurements that name only the
/// arms that ran, `meta.ext_eval`, and a verdict a report without a
/// baseline can't claim beyond `inconclusive`.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_report(bytes: &[u8]) -> Result<Report, ContractError> {
    if bytes.len() > MAX_REPORT_BYTES {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "report"));
    }
    let value = parse_strict(bytes)?;
    let object = value.as_object().ok_or_else(|| malformed("report"))?;
    reject(
        object,
        &[
            "v",
            "requires",
            "suite",
            "partition",
            "subject",
            "baseline",
            "evaluator",
            "started_at",
            "ended_at",
            "runs",
            "coverage",
            "measurements",
            "verdict",
            "limitations",
            "meta",
        ],
    )?;
    if object.get("v").and_then(Value::as_str) != Some(kb::REPORT_SCHEMA) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "report.v",
        ));
    }
    requires_empty(object)?;
    let suite = parse_artifact(require(object, "suite")?)?;
    if suite.schema.as_deref() != Some(SUITE_SCHEMA) {
        return Err(mismatch("report.suite: schema"));
    }
    if let Some(event) = &suite.event
        && event.kind != crate::kinds::EXT_RELEASE
    {
        return Err(mismatch("report.suite.event: a NIP-EXT release"));
    }
    parse_artifact(require(object, "partition")?)?;
    parse_artifact(require(object, "runs")?)?;
    parse_artifact(require(object, "limitations")?)?;
    let subject = arm(require(object, "subject")?, "subject")?;
    if let Some(event) = &subject.definition.event
        && event.kind != crate::kinds::EXT_RELEASE
    {
        return Err(mismatch("subject.definition.event: a NIP-EXT release"));
    }
    let baseline = match require(object, "baseline")? {
        Value::Null => None,
        value => Some(arm(value, "baseline")?),
    };
    let evaluator = text(object, "evaluator")?;
    if !is_hex(&evaluator) {
        return Err(malformed("evaluator"));
    }
    let time = |key: &str| {
        object
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(key))
    };
    let (started_at, ended_at) = (time("started_at")?, time("ended_at")?);
    if started_at > ended_at {
        return Err(malformed("started_at is after ended_at"));
    }
    let cover = require(object, "coverage")?
        .as_object()
        .ok_or_else(|| malformed("coverage"))?;
    reject(cover, &["subject", "baseline"])?;
    coverage(require(cover, "subject")?, "coverage.subject")?;
    match (require(cover, "baseline")?, &baseline) {
        (Value::Null, None) => {}
        (value, Some(_)) if !value.is_null() => coverage(value, "coverage.baseline")?,
        _ => return Err(mismatch("coverage.baseline follows the baseline arm")),
    }
    let measurements = require(object, "measurements")?
        .as_array()
        .ok_or_else(|| malformed("measurements"))?;
    if measurements.len() > MAX_MEASUREMENTS {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "measurements",
        ));
    }
    for item in measurements {
        measurement(item, baseline.is_some())?;
    }
    let verdict =
        Verdict::parse(&text(object, "verdict")?).ok_or_else(|| unsupported("verdict"))?;
    if baseline.is_none() && verdict != Verdict::Inconclusive {
        return Err(mismatch(
            "a report without a baseline can't claim a change: its verdict is inconclusive",
        ));
    }
    let meta = require(object, "meta")?
        .as_object()
        .ok_or_else(|| malformed("meta"))?;
    let profile = parse_profile(
        meta.get("ext_eval")
            .ok_or_else(|| malformed("meta.ext_eval"))?,
    )?;
    if profile.headline.baseline_passed.is_some() != baseline.is_some() {
        return Err(mismatch(
            "headline.baseline_passed follows the baseline arm",
        ));
    }
    Ok(Report {
        suite,
        subject,
        baseline,
        evaluator,
        started_at,
        ended_at,
        verdict,
        profile,
    })
}

/// A verified result publication: a `3189` with the `oa:ext-eval:v1`
/// marker.
#[derive(Debug, Clone, PartialEq)]
pub struct Publication {
    /// The event ID.
    pub id: String,
    /// The signer, the report's evaluator.
    pub evaluator: String,
    pub created_at: u64,
    /// The report's ArtifactRef.
    pub report_ref: ArtifactRef,
    pub report: Report,
    /// The suite's NIP-EXT release, from the report's suite ArtifactRef.
    pub suite_release: EventPointer,
    /// The subject's NIP-EXT release, when the subject is published.
    pub subject_release: Option<EventPointer>,
    /// The publication this one checks, when it's a check.
    pub checks: Option<String>,
}

impl Publication {
    /// The trainer the result is credited to: the requester of a hosted
    /// run, otherwise the evaluator.
    #[must_use]
    pub fn trainer(&self) -> &str {
        self.report
            .profile
            .requester
            .as_ref()
            .map_or(self.evaluator.as_str(), |r| r.pubkey.as_str())
    }

    /// The suite author: the root key of the suite's release.
    #[must_use]
    pub fn suite_author(&self) -> &str {
        &self.suite_release.pubkey
    }

    /// The verdict.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        self.report.verdict
    }
}

fn pointer_from(event: &crate::contracts::EventRef) -> EventPointer {
    EventPointer {
        id: event.id.clone(),
        pubkey: event.pubkey.clone(),
        kind: event.kind,
    }
}

/// The parts of a result `3189` for the exact `report` bytes (at most
/// 64 KiB). The report names everything the tags carry: the suite's
/// release (its suite ArtifactRef's `event`, required to publish), the
/// subject's release (when published), and a hosted run's requester.
/// `checks` names the publication this one checks, if any. The signer must
/// be the report's evaluator.
///
/// # Errors
///
/// When the report isn't valid under the profile, its suite has no
/// release, or `checks` isn't an event ID.
pub fn publication(report: &str, checks: Option<&str>) -> Result<Unsigned, ContractError> {
    let parsed = parse_report(report.as_bytes())?;
    let suite = parsed.suite.event.as_ref().ok_or_else(|| {
        malformed("report.suite.event: publish the suite's release before its results")
    })?;
    if checks.is_some_and(|id| !is_hex(id)) {
        return Err(malformed("checks"));
    }
    let value = parse_strict(report.as_bytes())?;
    let subject = value["subject"]["definition"].clone();
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
        "subject": subject,
        "supersedes": [],
        "meta": {"ext_eval_report": report},
    });
    let mut tags = vec![
        tag(&["t", kb::EVAL_MARKER]),
        tag(&["t", PROFILE_MARKER]),
        tag(&["x", digest.trim_start_matches("sha256:")]),
        tag(&["e", &suite.id, "", "suite"]),
    ];
    if let Some(release) = &parsed.subject.definition.event {
        tags.push(tag(&["e", &release.id, "", "subject"]));
    }
    if let Some(id) = checks {
        tags.push(tag(&["e", id, "", "check"]));
    }
    if let Some(requester) = &parsed.profile.requester {
        tags.push(tag(&["e", &requester.id, "", "request"]));
        tags.push(tag(&["p", &requester.pubkey]));
    }
    Ok(Unsigned {
        kind: kb::EVIDENCE_KIND,
        tags,
        content: content.to_string(),
    })
}

/// Checks a signed result publication: the kind and signature, exactly
/// the markers `oa:eval:v1` and `oa:ext-eval:v1`, the `x` tag, the closed
/// body, `meta.ext_eval_report` (at most 64 KiB) against the report
/// ArtifactRef's digest, the report under the profile, the signer as its
/// evaluator, the subject as the report's, and each `e` and `p` tag
/// against what the report names.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_publication(event: &Event) -> Result<Publication, ContractError> {
    if event.kind != kb::EVIDENCE_KIND {
        return Err(mismatch("kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| mismatch("event signature"))?;
    let mut markers: Vec<&str> = t_values(event).filter(|t| t.starts_with("oa:")).collect();
    markers.sort_unstable();
    if markers != [kb::EVAL_MARKER, PROFILE_MARKER] {
        return Err(mismatch(format!(
            "markers: a result carries exactly {} and {PROFILE_MARKER}",
            kb::EVAL_MARKER
        )));
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
    if report_ref.schema.as_deref() != Some(kb::REPORT_SCHEMA) {
        return Err(mismatch("report: schema"));
    }
    let xs: Vec<&str> = event.tag_values("x").collect();
    if xs != [report_ref.digest.trim_start_matches("sha256:")] {
        return Err(mismatch("x tag"));
    }
    let subject_value = require(object, "subject")?;
    parse_definition(subject_value)?;
    let supersedes = require(object, "supersedes")?
        .as_array()
        .ok_or_else(|| malformed("supersedes"))?;
    for item in supersedes {
        event_pointer(item, kb::EVIDENCE_KIND, "supersedes")?;
    }
    let meta = require(object, "meta")?
        .as_object()
        .ok_or_else(|| malformed("meta"))?;
    reject(meta, &["ext_eval_report"])?;
    let report_text = meta
        .get("ext_eval_report")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            ContractError::new(RefusalCode::ContentUnavailable, "meta.ext_eval_report")
        })?;
    if report_text.len() > MAX_REPORT_BYTES {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "meta.ext_eval_report is over 64 KiB",
        ));
    }
    check_artifact_bytes(&report_ref, report_text.as_bytes())?;
    let report = parse_report(report_text.as_bytes())?;
    if report.evaluator != event.pubkey {
        return Err(mismatch("the signer isn't the report's evaluator"));
    }
    let report_value = parse_strict(report_text.as_bytes())?;
    if report_value.pointer("/subject/definition") != Some(subject_value) {
        return Err(mismatch("subject: not the report's"));
    }
    let suite_release = report
        .suite
        .event
        .as_ref()
        .map(pointer_from)
        .ok_or_else(|| {
            malformed("report.suite.event: a published result names the suite's release")
        })?;
    let subject_release = report.subject.definition.event.as_ref().map(pointer_from);
    let mut tagged: std::collections::BTreeMap<&str, Vec<&str>> = std::collections::BTreeMap::new();
    for t in event.tags.iter().filter(|t| t.name() == Some("e")) {
        let id = t.value().unwrap_or_default();
        let marker = t.0.get(3).map_or("", String::as_str);
        if !E_MARKERS.contains(&marker) {
            return Err(unsupported(format!("e tag marker {marker:?}")));
        }
        if !is_hex(id) {
            return Err(malformed("e tag"));
        }
        tagged.entry(marker).or_default().push(id);
    }
    let one = |marker: &str| -> Result<Option<&str>, ContractError> {
        match tagged.get(marker).map(Vec::as_slice) {
            None => Ok(None),
            Some([id]) => Ok(Some(id)),
            Some(_) => Err(malformed(format!("more than one {marker} e tag"))),
        }
    };
    match one("suite")? {
        Some(id) if id == suite_release.id => {}
        Some(_) => return Err(mismatch("the suite e tag isn't the report's suite release")),
        None => return Err(malformed("a result has an e tag for the suite's release")),
    }
    if one("subject")? != subject_release.as_ref().map(|r| r.id.as_str()) {
        return Err(mismatch("the subject e tag follows the subject's release"));
    }
    let checks = one("check")?.map(str::to_string);
    if checks.as_deref() == Some(event.id.as_str()) {
        return Err(malformed("a publication can't check itself"));
    }
    let request = one("request")?;
    let people: Vec<&str> = event.tag_values("p").collect();
    match &report.profile.requester {
        Some(requester) => {
            if request != Some(requester.id.as_str()) || people != [requester.pubkey.as_str()] {
                return Err(mismatch(
                    "a hosted result carries the requester's p tag and the request's e tag",
                ));
            }
        }
        None => {
            if request.is_some() || !people.is_empty() {
                return Err(mismatch("only a hosted result names a requester"));
            }
        }
    }
    Ok(Publication {
        id: event.id.clone(),
        evaluator: event.pubkey.clone(),
        created_at: event.created_at,
        report_ref,
        report,
        suite_release,
        subject_release,
        checks,
    })
}

/// How a publication relates to another it may check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Linkage {
    /// A check whose verdict equals the original's.
    Confirm,
    /// A check whose verdict differs.
    Dispute,
    /// Not a check of this original: it doesn't cite it with the `check`
    /// marker, it ran another suite, subject, or subject lock, or the same
    /// trainer published both. A reader shows it as a separate result.
    NotACheck,
}

/// Whether `check` confirms `original`, from the two signed events alone:
/// a check cites the original with the `check` marker, has the same suite
/// ArtifactRef, the same subject DefinitionRef, and the same subject-arm
/// lock, and a different trainer (the requester of a hosted run, otherwise
/// the evaluator). It confirms when the verdicts are equal and disputes
/// otherwise. Whether a check earns credit is NIP-XP's `eval-check` rule.
///
/// # Errors
///
/// When either event isn't a valid result publication.
pub fn confirms(original: &Event, check: &Event) -> Result<Linkage, ContractError> {
    let original = parse_publication(original)?;
    let check = parse_publication(check)?;
    Ok(linkage(&original, &check))
}

/// [`confirms`] over publications already parsed.
#[must_use]
pub fn linkage(original: &Publication, check: &Publication) -> Linkage {
    let same_subject = |a: &DefinitionRef, b: &DefinitionRef| {
        a.id == b.id && a.artifact.digest == b.artifact.digest && a.artifact.size == b.artifact.size
    };
    if check.checks.as_deref() != Some(original.id.as_str())
        || check.report.suite.digest != original.report.suite.digest
        || !same_subject(
            &check.report.subject.definition,
            &original.report.subject.definition,
        )
        || check.report.subject.lock.digest != original.report.subject.lock.digest
        || check.trainer() == original.trainer()
    {
        return Linkage::NotACheck;
    }
    if check.verdict() == original.verdict() {
        Linkage::Confirm
    } else {
        Linkage::Dispute
    }
}

/// Checks the signed NIP-CJ execution request a hosted result names: the
/// exact event, its signature, and its worker (`p`) as the result's
/// signer, the hosted runner. The request's encrypted body is the
/// runner's to read.
///
/// # Errors
///
/// [`RefusalCode::IdentityMismatch`] when it isn't that request; a result
/// that names no requester has none to check.
pub fn check_request(result: &Publication, request: &Event) -> Result<(), ContractError> {
    let Some(requester) = &result.report.profile.requester else {
        return Err(mismatch("the result isn't hosted: it names no request"));
    };
    if request.id != requester.id
        || request.pubkey != requester.pubkey
        || request.kind != crate::kinds::CJ_EXECUTION_REQUEST
    {
        return Err(mismatch("not the request the result names"));
    }
    request
        .validate_crypto()
        .map_err(|_| mismatch("request signature"))?;
    let workers: Vec<&str> = request.tag_values("p").collect();
    if workers != [result.evaluator.as_str()] {
        return Err(mismatch("the request wasn't sent to the result's runner"));
    }
    Ok(())
}

/// The trainer of `result`, after checking a hosted result's request is
/// among `requests`: the requester when it verifies, the evaluator when
/// the result isn't hosted.
///
/// # Errors
///
/// [`RefusalCode::ContentUnavailable`] when a hosted result's request isn't
/// supplied, and [`check_request`]'s refusals.
pub fn verified_trainer<'a>(
    result: &'a Publication,
    requests: &[Event],
) -> Result<&'a str, ContractError> {
    if let Some(requester) = &result.report.profile.requester {
        let request = requests
            .iter()
            .find(|r| r.id == requester.id)
            .ok_or_else(|| {
                ContractError::new(
                    RefusalCode::ContentUnavailable,
                    "the hosted result's signed request",
                )
            })?;
        check_request(result, request)?;
    }
    Ok(result.trainer())
}

/// A verified adoption decision (`openagents.eval-admission.v1`) as the
/// profile reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Admission {
    /// The admitted extension, with its release EventRef.
    pub subject: DefinitionRef,
    /// The cited reports' digests.
    pub reports: Vec<String>,
    /// `admit`, `reject`, or `inconclusive`.
    pub decision: String,
    pub issuer: String,
    pub expires_at: u64,
}

/// Checks an admission document's bytes.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_admission(bytes: &[u8]) -> Result<Admission, ContractError> {
    let value = parse_strict(bytes)?;
    let object = value.as_object().ok_or_else(|| malformed("admission"))?;
    reject(
        object,
        &[
            "v",
            "requires",
            "subject",
            "reports",
            "policy",
            "scope",
            "decision",
            "issuer",
            "expires_at",
            "meta",
        ],
    )?;
    if object.get("v").and_then(Value::as_str) != Some(ADMISSION_SCHEMA) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    requires_empty(object)?;
    let subject = parse_definition(require(object, "subject")?)?;
    let items = require(object, "reports")?
        .as_array()
        .ok_or_else(|| malformed("reports"))?;
    let mut reports = Vec::new();
    for item in items {
        reports.push(parse_artifact(item)?.digest);
    }
    parse_definition(require(object, "policy")?)?;
    parse_artifact(require(object, "scope")?)?;
    let decision = text(object, "decision")?;
    if !matches!(decision.as_str(), "admit" | "reject" | "inconclusive") {
        return Err(unsupported("decision"));
    }
    let issuer = text(object, "issuer")?;
    if !is_hex(&issuer) {
        return Err(malformed("issuer"));
    }
    let expires_at = require(object, "expires_at")?
        .as_u64()
        .ok_or_else(|| malformed("expires_at"))?;
    Ok(Admission {
        subject,
        reports,
        decision,
        issuer,
        expires_at,
    })
}

/// A verified NIP-EXT release body as adoption reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// `<root pubkey>:<slug>`.
    pub package: String,
    pub manifest: ext::Manifest,
    /// The `openagents.eval-admission.v1` ArtifactRefs its manifest's
    /// `provenance.receipts` cite.
    pub admissions: Vec<ArtifactRef>,
}

/// Checks a signed NIP-EXT `3184` release and its manifest's exact bytes,
/// and reads the admissions its provenance cites.
///
/// # Errors
///
/// A typed refusal naming the first check that failed.
pub fn parse_release(event: &Event, manifest: &[u8]) -> Result<Release, ContractError> {
    if event.kind != crate::kinds::EXT_RELEASE {
        return Err(mismatch("kind"));
    }
    let body = ext::parse_record(event)?;
    if body.get("type").and_then(Value::as_str) != Some("release") {
        return Err(mismatch("type"));
    }
    let manifest_ref = parse_artifact(
        body.get("manifest")
            .ok_or_else(|| malformed("release.manifest"))?,
    )?;
    check_artifact_bytes(&manifest_ref, manifest)?;
    let value = parse_strict(manifest)?;
    let parsed = ext::parse_manifest(&value)?;
    let package = body
        .get("package")
        .and_then(Value::as_str)
        .ok_or_else(|| malformed("release.package"))?
        .to_string();
    if parsed.package != package {
        return Err(mismatch("the manifest is another package's"));
    }
    let mut admissions = Vec::new();
    if let Some(receipts) = value
        .pointer("/provenance/receipts")
        .and_then(Value::as_array)
    {
        for receipt in receipts {
            if let Ok(artifact) = parse_artifact(receipt)
                && artifact.schema.as_deref() == Some(ADMISSION_SCHEMA)
            {
                admissions.push(artifact);
            }
        }
    }
    Ok(Release {
        package,
        manifest: parsed,
        admissions,
    })
}

/// Bounds a hosted run request's size: at most [`HOSTED_MAX_CASES`]
/// cases, [`HOSTED_MAX_RUNS`] runs, and [`HOSTED_ARMS`] arms.
///
/// # Errors
///
/// [`RefusalCode::LimitExceeded`] past a bound, and
/// [`RefusalCode::Malformed`] for a zero.
pub fn check_hosted_size(cases: u64, runs: u64, arms: u64) -> Result<(), ContractError> {
    if cases == 0 || runs == 0 || arms == 0 {
        return Err(malformed("size: every count is at least 1"));
    }
    if cases > HOSTED_MAX_CASES || runs > HOSTED_MAX_RUNS || arms > HOSTED_ARMS {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            format!(
                "the hosted runner runs at most {HOSTED_MAX_CASES} tests, {HOSTED_MAX_RUNS} runs, and {HOSTED_ARMS} arms"
            ),
        ));
    }
    Ok(())
}

/// `<pubkey>:<package>/<component>`, the qualified component ID every
/// DefinitionRef uses.
pub(crate) fn valid_qualified(id: &str) -> bool {
    let slug = |s: &str| {
        !s.is_empty()
            && s.len() <= 64
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    id.split_once(':').is_some_and(|(key, rest)| {
        is_hex(key)
            && rest
                .split_once('/')
                .is_some_and(|(package, component)| slug(package) && slug(component))
    })
}

#[cfg(test)]
pub(crate) mod tests;
