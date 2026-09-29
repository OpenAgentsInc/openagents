//! The hosted runner's wire: the NIP-CJ execution request a phone sends
//! to run a test set on our computers, and the answers it gets back
//! (`nips/openagents/NIP-EVAL.md`, "Hosted runs";
//! `docs/extensions/evaluation.md`, "Where runs execute").
//!
//! A request is a `25920` signed by the trainer's key, with exactly one
//! `p` (the runner) and an `expiration` equal to its `deadline`, whose
//! NIP-44 body is [`request_body`]. Its `input` is one of two actions:
//!
//! - **run** ([`run_input`]): a published suite's release or the caller's
//!   draft, against a catalog tool's DefinitionRef or the draft's tool,
//!   with 1 to 3 runs per arm and the baseline arm on. `check` names the
//!   result a rerun checks; checks don't count against the quota.
//! - **publish** ([`publish_input`]): the report of a run this trainer
//!   asked for, which the runner then publishes: the suite's release
//!   (once) and the `3189` it signs, naming the trainer.
//!
//! The runner answers with the execution family's `27020` `accepted` and
//! `progress` (whose `meta.ext_eval` counts finished case runs:
//! [`progress_meta`]) and one `26920` result, whose `output` is
//! [`run_output`] or [`publish_output`]. A refusal names one of
//! [`NOT_ADMITTED`], [`OVER_QUOTA`], or [`TOO_LARGE`], or the execution
//! layer's own codes.
//!
//! Everything here is pure: builders and parsers. The runner itself is
//! `crates/eval-runner`.

use serde_json::{Map, Value, json};

use crate::cj_conversation::{self, SubjectSource, SuiteSource};
use crate::contracts::{
    ArtifactRef, ContractError, DefinitionRef, RefusalCode, digest_bytes, jcs, parse_artifact,
    parse_definition,
};
use crate::domain::Tag;
use crate::kb::{is_hex, malformed, mismatch, reject, require, tag, unsupported};

use super::{
    EventPointer, HOSTED_ARMS, HOSTED_MAX_CASES, HOSTED_MAX_RUNS, Headline, Verdict, event_pointer,
};

/// The schema of a hosted request's `input` and of the runner's `output`.
pub const SCHEMA: &str = "openagents.ext-eval-hosted.v1";
/// The deployed hosted runner's public key. Its secret stays on the
/// runner's host (`docs/deployment/eval-runner.md`).
pub const RUNNER: &str = "a7cff3ee1ff0209f971b9f24673db310ab858899c9d9a99b640e6cb29b1753f0";
/// The relay the hosted runner listens on.
pub const RELAY: &str = "wss://relay.openagents.com";
/// The program package and component a request's `target` names:
/// `<runner>:ext-eval/run`.
pub const PROGRAM: &str = "ext-eval/run";
/// How long after it's signed a request's run may take, in seconds: its
/// `deadline` is `created_at` plus this.
pub const DEADLINE_SECONDS: u64 = 3_600;
/// How long after its deadline the runner keeps a job's answers.
pub const RETAIN_SECONDS: u64 = 7 * 86_400;
/// The largest draft a request may carry, as JSON.
pub const MAX_DRAFT_BYTES: usize = cj_conversation::MAX_DRAFT_BYTES;
/// Refused: not a catalog tool or a chat-made tool, a draft that asks for
/// `exec` or `network`, a suite or target the runner doesn't run, or
/// admission is switched off.
pub const NOT_ADMITTED: &str = "not_admitted";
/// Refused: the trainer's runs for the UTC day, or the day's total.
pub const OVER_QUOTA: &str = "over_quota";
/// Refused: more tests, runs, or arms than the runner runs, a draft over
/// 64 KiB, or a result too large to publish.
pub const TOO_LARGE: &str = "too_large";

/// The hosted program's description, whose digest the target pins: what
/// it runs and within which bounds.
fn program_document() -> Value {
    json!({
        "v": SCHEMA,
        "program": "ext-eval",
        "runs": "openagents ext eval run, in the run sandbox",
        "input": SCHEMA,
        "bounds": {
            "cases": HOSTED_MAX_CASES,
            "runs": HOSTED_MAX_RUNS,
            "arms": HOSTED_ARMS,
            "draft_bytes": MAX_DRAFT_BYTES,
        },
        "effects": ["read", "write"],
    })
}

fn document_ref(value: &Value, schema: &str) -> Value {
    let bytes = jcs(value).unwrap_or_default();
    json!({
        "digest": digest_bytes(&bytes),
        "size": bytes.len(),
        "media_type": "application/json",
        "schema": schema,
    })
}

/// The `target` a request to `runner` names: the `ext-eval` program's
/// DefinitionRef.
#[must_use]
pub fn target(runner: &str) -> Value {
    json!({
        "id": format!("{runner}:{PROGRAM}"),
        "artifact": document_ref(&program_document(), SCHEMA),
    })
}

/// The `lock`: the hosted program is its own whole closure.
#[must_use]
pub fn lock() -> Value {
    document_ref(
        &json!({"v": SCHEMA, "lock": "ext-eval", "program": program_document()}),
        SCHEMA,
    )
}

/// The `context`: none beyond the input.
#[must_use]
pub fn context() -> Value {
    document_ref(&json!({"v": SCHEMA, "context": []}), SCHEMA)
}

/// The `requirements`: reads, and writes inside the run sandbox; never
/// `exec` or `network`. The report is sealed to the requester until the
/// requester asks to publish it.
#[must_use]
pub fn requirements() -> Value {
    document_ref(
        &json!({
            "v": SCHEMA,
            "effects": ["read", "write"],
            "disclosure": "the report is sealed to the requester until the requester publishes it",
        }),
        SCHEMA,
    )
}

/// The execute body of a request to `runner` with `input`
/// ([`run_input`] or [`publish_input`]), signed at `now`. Its `deadline`
/// is `now` plus [`DEADLINE_SECONDS`]; the event's `expiration` must equal
/// it ([`request_tags`]).
///
/// # Errors
///
/// When `request` isn't 1 to 128 printable ASCII characters or `input`
/// doesn't parse ([`parse_input`]).
pub fn request_body(
    runner: &str,
    request: &str,
    input: &Value,
    now: u64,
) -> Result<Value, ContractError> {
    if request.is_empty() || request.len() > 128 || !request.bytes().all(|b| b.is_ascii_graphic()) {
        return Err(malformed("request"));
    }
    if !is_hex(runner) {
        return Err(malformed("runner"));
    }
    parse_input(input)?;
    let deadline = now + DEADLINE_SECONDS;
    Ok(json!({
        "v": crate::execution::SCHEMA,
        "requires": [],
        "type": "execute",
        "request": request,
        "attempt": 1,
        "run": request,
        "target": target(runner),
        "lock": lock(),
        "input": input,
        "context": context(),
        "requirements": requirements(),
        "bounds": {"wall_ms": DEADLINE_SECONDS * 1_000},
        "deadline": deadline,
        "retain_until": deadline + RETAIN_SECONDS,
    }))
}

/// The tags a request to `runner` carries: its one `p` and the
/// `expiration` equal to the body's `deadline`.
#[must_use]
pub fn request_tags(runner: &str, deadline: u64) -> Vec<Tag> {
    vec![
        tag(&["p", runner]),
        tag(&["expiration", &deadline.to_string()]),
    ]
}

/// A run request's input, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct RunInput {
    /// The suite: a published release or the draft's cases.
    pub suite: SuiteSource,
    /// The tool: a DefinitionRef or the draft's tool.
    pub subject: SubjectSource,
    /// The draft, when either side is the draft.
    pub draft: Option<cj_conversation::Draft>,
    /// Runs per arm, 1 to 3.
    pub runs: u64,
    /// The publication this run checks, when it's a check.
    pub check: Option<String>,
}

/// A hosted request's input.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    /// Run a suite in both arms.
    Run(Box<RunInput>),
    /// Publish the report of a run this trainer asked for.
    Publish {
        /// The report the run's result named.
        report: ArtifactRef,
    },
}

fn suite_value(suite: &SuiteSource) -> Value {
    match suite {
        SuiteSource::Published(release) => release.to_value(),
        SuiteSource::Draft => json!("draft"),
    }
}

fn subject_value(subject: &SubjectSource) -> Value {
    match subject {
        SubjectSource::Definition(definition) => {
            let mut value = json!({
                "id": definition.id,
                "artifact": artifact_json(&definition.artifact),
            });
            if let Some(event) = &definition.event {
                value["event"] =
                    json!({"id": event.id, "pubkey": event.pubkey, "kind": event.kind});
            }
            value
        }
        SubjectSource::Draft => json!("draft"),
    }
}

fn artifact_json(artifact: &ArtifactRef) -> Value {
    let mut value = json!({
        "digest": artifact.digest,
        "size": artifact.size,
        "media_type": artifact.media_type,
    });
    if let Some(schema) = &artifact.schema {
        value["schema"] = json!(schema);
    }
    value
}

/// A run request's input, as the `start_eval` offer names the suite and
/// the tool. `draft` is the draft object (`cj_conversation::draft_value`)
/// and is required exactly when either side is the draft; `check` names
/// the publication a rerun checks.
///
/// # Errors
///
/// When the result doesn't parse ([`parse_input`]).
pub fn run_input(
    suite: &SuiteSource,
    subject: &SubjectSource,
    draft: Option<&Value>,
    runs: u64,
    check: Option<&str>,
) -> Result<Value, ContractError> {
    let value = json!({
        "v": SCHEMA,
        "action": "run",
        "suite": suite_value(suite),
        "subject": subject_value(subject),
        "draft": draft.cloned().unwrap_or(Value::Null),
        "runs": runs,
        "baseline": true,
        "check": check,
    });
    parse_input(&value)?;
    Ok(value)
}

/// A publish request's input: the report a run's result named.
#[must_use]
pub fn publish_input(report: &ArtifactRef) -> Value {
    json!({"v": SCHEMA, "action": "publish", "report": artifact_json(report)})
}

/// Checks a hosted request's `input`: its version, the action's closed
/// fields, 1 to [`HOSTED_MAX_RUNS`] runs with the baseline on, a draft
/// exactly when either side is the draft, at most [`MAX_DRAFT_BYTES`] and
/// [`HOSTED_MAX_CASES`] tests, and a `check` that is an event ID.
///
/// # Errors
///
/// A typed refusal: [`RefusalCode::LimitExceeded`] past a bound, and
/// [`RefusalCode::Malformed`] or [`RefusalCode::Unsupported`] otherwise.
pub fn parse_input(value: &Value) -> Result<Input, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("input"))?;
    if object.get("v").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "input.v",
        ));
    }
    match object.get("action").and_then(Value::as_str) {
        Some("run") => parse_run(object).map(|run| Input::Run(Box::new(run))),
        Some("publish") => {
            reject(object, &["v", "action", "report"])?;
            let report = parse_artifact(require(object, "report")?)?;
            if report.schema.as_deref() != Some(crate::kb::REPORT_SCHEMA) {
                return Err(mismatch("input.report: schema"));
            }
            Ok(Input::Publish { report })
        }
        _ => Err(unsupported("input.action")),
    }
}

fn parse_run(object: &Map<String, Value>) -> Result<RunInput, ContractError> {
    reject(
        object,
        &[
            "v", "action", "suite", "subject", "draft", "runs", "baseline", "check",
        ],
    )?;
    let suite = match require(object, "suite")? {
        Value::String(word) if word == "draft" => SuiteSource::Draft,
        value => SuiteSource::Published(event_pointer(
            value,
            crate::kinds::EXT_RELEASE,
            "input.suite",
        )?),
    };
    let subject = match require(object, "subject")? {
        Value::String(word) if word == "draft" => SubjectSource::Draft,
        value => SubjectSource::Definition(Box::new(parse_definition(value)?)),
    };
    let runs = require(object, "runs")?
        .as_u64()
        .ok_or_else(|| malformed("input.runs"))?;
    if runs == 0 {
        return Err(malformed("input.runs: at least 1"));
    }
    if runs > HOSTED_MAX_RUNS {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            format!("input.runs: the hosted runner runs at most {HOSTED_MAX_RUNS} per arm"),
        ));
    }
    if require(object, "baseline")? != &Value::Bool(true) {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "input.baseline: the hosted runner always runs both arms",
        ));
    }
    let needs_draft = suite == SuiteSource::Draft || subject == SubjectSource::Draft;
    let draft = match (require(object, "draft")?, needs_draft) {
        (Value::Null, false) => None,
        (Value::Null, true) => return Err(malformed("input.draft: the draft side needs it")),
        (_, false) => return Err(malformed("input.draft: only a draft side sends one")),
        (value, true) => {
            let bytes = serde_json::to_vec(value).map_or(usize::MAX, |bytes| bytes.len());
            if bytes > MAX_DRAFT_BYTES {
                return Err(ContractError::new(
                    RefusalCode::LimitExceeded,
                    "input.draft: at most 64 KiB",
                ));
            }
            let draft = cj_conversation::parse_draft(value)?;
            if suite == SuiteSource::Draft {
                if draft.cases.is_empty() {
                    return Err(malformed(
                        "input.draft: a draft suite has at least one test",
                    ));
                }
                if draft.cases.len() as u64 > HOSTED_MAX_CASES {
                    return Err(ContractError::new(
                        RefusalCode::LimitExceeded,
                        format!(
                            "input.draft: the hosted runner runs at most {HOSTED_MAX_CASES} tests"
                        ),
                    ));
                }
            }
            Some(draft)
        }
    };
    let check = match require(object, "check")? {
        Value::Null => None,
        Value::String(id) if is_hex(id) => Some(id.clone()),
        _ => return Err(malformed("input.check")),
    };
    Ok(RunInput {
        suite,
        subject,
        draft,
        runs,
        check,
    })
}

/// A run's progress as the runner reports it: case runs finished across
/// both arms, of those planned (cases × runs × arms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub completed: u64,
    pub planned: u64,
}

/// The `meta` a `27020` progress payload carries.
#[must_use]
pub fn progress_meta(progress: Progress) -> Value {
    json!({"ext_eval": {"completed": progress.completed, "planned": progress.planned}})
}

/// Reads a progress payload's `meta.ext_eval`.
///
/// # Errors
///
/// When the payload isn't progress or the counts are missing or
/// inconsistent.
pub fn parse_progress(payload: &Value) -> Result<Progress, ContractError> {
    if payload.get("type").and_then(Value::as_str) != Some("progress") {
        return Err(mismatch("type: progress"));
    }
    let meta = payload
        .pointer("/meta/ext_eval")
        .and_then(Value::as_object)
        .ok_or_else(|| malformed("meta.ext_eval"))?;
    reject(meta, &["completed", "planned"])?;
    let count = |key: &str| {
        meta.get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("meta.ext_eval.{key}")))
    };
    let progress = Progress {
        completed: count("completed")?,
        planned: count("planned")?,
    };
    if progress.planned == 0 || progress.completed > progress.planned {
        return Err(malformed("meta.ext_eval: completed is at most planned"));
    }
    Ok(progress)
}

/// A finished run's `output`.
#[derive(Debug, Clone, PartialEq)]
pub struct RunOutput {
    /// The report, which a publish names.
    pub report: ArtifactRef,
    /// The `3188` private artifact holding the report, sealed to the
    /// requester.
    pub sealed: EventPointer,
    /// Tests passed per arm.
    pub headline: Headline,
    /// The gate's verdict.
    pub verdict: Verdict,
    /// Changes in time and cost, in plain words, apart from the verdict.
    pub notes: Vec<String>,
}

/// A finished run's `output` value.
#[must_use]
pub fn run_output(output: &RunOutput) -> Value {
    json!({
        "v": SCHEMA,
        "action": "run",
        "report": artifact_json(&output.report),
        "sealed": output.sealed.to_value(),
        "headline": {
            "subject_passed": output.headline.subject_passed,
            "baseline_passed": output.headline.baseline_passed,
            "total": output.headline.total,
        },
        "verdict": output.verdict.word(),
        "notes": output.notes,
    })
}

/// Reads a finished run's `output`.
///
/// # Errors
///
/// A typed refusal naming the first field that doesn't check.
pub fn parse_run_output(value: &Value) -> Result<RunOutput, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("output"))?;
    reject(
        object,
        &[
            "v", "action", "report", "sealed", "headline", "verdict", "notes",
        ],
    )?;
    if object.get("v").and_then(Value::as_str) != Some(SCHEMA)
        || object.get("action").and_then(Value::as_str) != Some("run")
    {
        return Err(mismatch("output: a run's output"));
    }
    let report = parse_artifact(require(object, "report")?)?;
    if report.schema.as_deref() != Some(crate::kb::REPORT_SCHEMA) {
        return Err(mismatch("output.report: schema"));
    }
    let sealed = event_pointer(
        require(object, "sealed")?,
        crate::contracts::ARTIFACT_ENVELOPE_KIND,
        "output.sealed",
    )?;
    let headline = require(object, "headline")?
        .as_object()
        .ok_or_else(|| malformed("output.headline"))?;
    reject(headline, &["subject_passed", "baseline_passed", "total"])?;
    let count = |key: &str| {
        headline
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| malformed(format!("output.headline.{key}")))
    };
    let headline = Headline {
        subject_passed: count("subject_passed")?,
        baseline_passed: match headline.get("baseline_passed") {
            Some(Value::Null) | None => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .ok_or_else(|| malformed("output.headline.baseline_passed"))?,
            ),
        },
        total: count("total")?,
    };
    let verdict = object
        .get("verdict")
        .and_then(Value::as_str)
        .and_then(Verdict::parse)
        .ok_or_else(|| unsupported("output.verdict"))?;
    let notes = require(object, "notes")?
        .as_array()
        .ok_or_else(|| malformed("output.notes"))?
        .iter()
        .map(|note| {
            note.as_str()
                .filter(|note| note.chars().count() <= 200)
                .map(str::to_string)
                .ok_or_else(|| malformed("output.notes"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if notes.len() > 4 {
        return Err(malformed("output.notes: at most 4"));
    }
    Ok(RunOutput {
        report,
        sealed,
        headline,
        verdict,
        notes,
    })
}

/// A finished publish's `output`: the suite's release and the result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOutput {
    pub suite_release: EventPointer,
    pub result: EventPointer,
}

/// A finished publish's `output` value.
#[must_use]
pub fn publish_output(output: &PublishOutput) -> Value {
    json!({
        "v": SCHEMA,
        "action": "publish",
        "suite_release": output.suite_release.to_value(),
        "result": output.result.to_value(),
    })
}

/// Reads a finished publish's `output`.
///
/// # Errors
///
/// A typed refusal naming the first field that doesn't check.
pub fn parse_publish_output(value: &Value) -> Result<PublishOutput, ContractError> {
    let object = value.as_object().ok_or_else(|| malformed("output"))?;
    reject(object, &["v", "action", "suite_release", "result"])?;
    if object.get("v").and_then(Value::as_str) != Some(SCHEMA)
        || object.get("action").and_then(Value::as_str) != Some("publish")
    {
        return Err(mismatch("output: a publish's output"));
    }
    Ok(PublishOutput {
        suite_release: event_pointer(
            require(object, "suite_release")?,
            crate::kinds::EXT_RELEASE,
            "output.suite_release",
        )?,
        result: event_pointer(
            require(object, "result")?,
            crate::kb::EVIDENCE_KIND,
            "output.result",
        )?,
    })
}

/// Checks that an execute's `target` is the `ext-eval` program of
/// `runner`, byte for byte.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] for any other target.
pub fn check_target(target: &DefinitionRef, runner: &str) -> Result<(), ContractError> {
    let expected = parse_definition(&self::target(runner))?;
    if target.id != expected.id || target.artifact.digest != expected.artifact.digest {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "target: this runner runs only its ext-eval program",
        ));
    }
    Ok(())
}

/// Checks that an execute's `requirements` ask for reads and sandbox
/// writes only: the hosted requirements document, exactly.
///
/// # Errors
///
/// [`RefusalCode::NotAdmitted`] for any other requirements.
pub fn check_requirements(requirements: &ArtifactRef) -> Result<(), ContractError> {
    let expected = parse_artifact(&self::requirements())?;
    if requirements.digest != expected.digest {
        return Err(ContractError::new(
            RefusalCode::NotAdmitted,
            "requirements: the hosted runner grants read and sandbox write only",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cj_conversation::parse_draft;

    fn release() -> EventPointer {
        EventPointer {
            id: "11".repeat(32),
            pubkey: "22".repeat(32),
            kind: crate::kinds::EXT_RELEASE,
        }
    }

    fn definition() -> DefinitionRef {
        parse_definition(&json!({
            "id": format!("{}:project-map/project-map", "33".repeat(32)),
            "artifact": {"digest": format!("sha256:{}", "44".repeat(32)), "size": 10, "media_type": "application/json"},
        }))
        .unwrap()
    }

    fn draft(cases: usize) -> Value {
        let cases: Vec<Value> = (0..cases)
            .map(|n| {
                json!({
                    "id": format!("case-{n}"),
                    "kind": "should-fire",
                    "prompt": "+++\nv = \"openagents.eval-case.v1\"\n+++\n\nList the files.\n",
                    "graders": [{"name": "files", "text": "+++\ntype = \"regex\"\npattern = \"a\"\n+++\n"}],
                })
            })
            .collect();
        json!({
            "v": crate::cj_conversation::DRAFT_SCHEMA,
            "tool": {"name": "Brief", "summary": "A brief.", "catalog": null,
                     "skill": "Say what you found.", "uses": []},
            "cases": cases,
        })
    }

    #[test]
    fn a_published_run_round_trips_through_the_request_body() {
        let input = run_input(
            &SuiteSource::Published(release()),
            &SubjectSource::Definition(Box::new(definition())),
            None,
            3,
            None,
        )
        .unwrap();
        let body = request_body(RUNNER, "req-1", &input, 1_000).unwrap();
        assert_eq!(body["deadline"], 1_000 + DEADLINE_SECONDS);
        assert_eq!(
            body["retain_until"],
            1_000 + DEADLINE_SECONDS + RETAIN_SECONDS
        );
        let target = parse_definition(&body["target"]).unwrap();
        check_target(&target, RUNNER).unwrap();
        assert!(check_target(&target, &"55".repeat(32)).is_err());
        check_requirements(&parse_artifact(&body["requirements"]).unwrap()).unwrap();
        let Input::Run(run) = parse_input(&body["input"]).unwrap() else {
            panic!("a run")
        };
        assert_eq!(run.suite, SuiteSource::Published(release()));
        assert_eq!(run.runs, 3);
        assert!(run.draft.is_none() && run.check.is_none());
        let tags = request_tags(RUNNER, 1_000 + DEADLINE_SECONDS);
        assert_eq!(tags[0].value(), Some(RUNNER));
    }

    #[test]
    fn a_draft_run_carries_its_draft_and_nothing_else_does() {
        let value = draft(2);
        let input = run_input(
            &SuiteSource::Draft,
            &SubjectSource::Draft,
            Some(&value),
            1,
            None,
        )
        .unwrap();
        let Input::Run(run) = parse_input(&input).unwrap() else {
            panic!("a run")
        };
        assert_eq!(run.draft.unwrap(), parse_draft(&value).unwrap());
        // A draft side with no draft, and a draft with no draft side.
        assert!(run_input(&SuiteSource::Draft, &SubjectSource::Draft, None, 1, None).is_err());
        assert!(
            run_input(
                &SuiteSource::Published(release()),
                &SubjectSource::Definition(Box::new(definition())),
                Some(&value),
                1,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn the_bounds_refuse_as_too_large() {
        let refuse = |input: Result<Value, ContractError>| input.unwrap_err().code;
        assert_eq!(
            refuse(run_input(
                &SuiteSource::Draft,
                &SubjectSource::Draft,
                Some(&draft(9)),
                1,
                None
            )),
            RefusalCode::LimitExceeded
        );
        assert_eq!(
            refuse(run_input(
                &SuiteSource::Published(release()),
                &SubjectSource::Definition(Box::new(definition())),
                None,
                4,
                None
            )),
            RefusalCode::LimitExceeded
        );
        let mut big = draft(1);
        big["tool"]["skill"] = json!("x".repeat(16 * 1024));
        for n in 0..8 {
            big["cases"][0]["graders"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name": format!("g{n}"), "text": "y".repeat(7 * 1024)}));
        }
        assert_eq!(
            refuse(run_input(
                &SuiteSource::Draft,
                &SubjectSource::Draft,
                Some(&big),
                1,
                None
            )),
            RefusalCode::LimitExceeded
        );
        let mut one_arm = run_input(
            &SuiteSource::Published(release()),
            &SubjectSource::Definition(Box::new(definition())),
            None,
            1,
            None,
        )
        .unwrap();
        one_arm["baseline"] = json!(false);
        assert_eq!(
            parse_input(&one_arm).unwrap_err().code,
            RefusalCode::LimitExceeded
        );
    }

    #[test]
    fn a_publish_names_a_report_and_the_answers_round_trip() {
        let report = parse_artifact(&json!({
            "digest": format!("sha256:{}", "66".repeat(32)), "size": 9,
            "media_type": "application/json", "schema": crate::kb::REPORT_SCHEMA,
        }))
        .unwrap();
        assert_eq!(
            parse_input(&publish_input(&report)).unwrap(),
            Input::Publish {
                report: report.clone()
            }
        );
        let output = RunOutput {
            report: report.clone(),
            sealed: EventPointer {
                id: "77".repeat(32),
                pubkey: RUNNER.into(),
                kind: crate::contracts::ARTIFACT_ENVELOPE_KIND,
            },
            headline: Headline {
                subject_passed: 7,
                baseline_passed: Some(5),
                total: 8,
            },
            verdict: Verdict::Pass,
            notes: vec!["Faster: 12.0 s against 30.0 s per run.".into()],
        };
        assert_eq!(parse_run_output(&run_output(&output)).unwrap(), output);
        let published = PublishOutput {
            suite_release: release(),
            result: EventPointer {
                id: "88".repeat(32),
                pubkey: RUNNER.into(),
                kind: crate::kb::EVIDENCE_KIND,
            },
        };
        assert_eq!(
            parse_publish_output(&publish_output(&published)).unwrap(),
            published
        );
        let progress = Progress {
            completed: 3,
            planned: 48,
        };
        let payload = json!({"type": "progress", "meta": progress_meta(progress)});
        assert_eq!(parse_progress(&payload).unwrap(), progress);
        let over = json!({"type": "progress", "meta": progress_meta(Progress { completed: 49, planned: 48 })});
        assert!(parse_progress(&over).is_err());
    }
}
