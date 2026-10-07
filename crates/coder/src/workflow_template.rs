//! One reusable operations template over an explicitly approved input snapshot.
//!
//! Records describe local operator approval, not customer consent or attestation.
//! Each invocation captures one file; no prior task, home, or protected grader is
//! available to the guest. Publication, disclosure, and payment remain separate.

use std::collections::BTreeMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::program::Program;
use crate::program_authority::Grant;
use crate::runtime::{Budget, Inputs, Run, Runtime};

pub const MAX_SOURCE_BYTES: usize = 60 * 1024;
pub const MAX_RECORD_BYTES: usize = 256 * 1024;
pub const MAX_PROGRAM_BYTES: usize = 192 * 1024;
pub const PACKAGE: &str = include_str!("../../../plugins/meeting-followup/package.json");
pub const PROGRAM: &str =
    include_str!("../../../plugins/meeting-followup/programs/meeting-followup.json");

/// The independently selected customer task and exact local input/release.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: String,
    pub task: String,
    pub customer: String,
    pub human_owner: String,
    /// A local reviewer reference. This runner performs no delivery to it.
    pub recipient: String,
    /// The operator's current rights revision, rechecked at dispatch.
    pub permission_epoch: u64,
    pub source_sha256: String,
    pub package_digest: String,
    pub program_digest: String,
}

/// A deliberate host-side approval for this snapshot and current rights.
#[derive(Clone, Debug)]
pub struct Approval {
    pub snapshot_digest: String,
    pub recipient: String,
    pub permission_epoch: u64,
}

/// One bounded runtime attempt. A stopped or absent value remains visible.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub finished: bool,
    pub stopped: Option<String>,
    pub value: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub snapshot: Snapshot,
    pub snapshot_digest: String,
    pub with_template: Attempt,
    /// Same retained guest and profile, empty text and scope, no file handles.
    /// This measures file access/extraction, not model quality or customer ROI.
    pub without_file_access: Attempt,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Citation {
    pub from: String,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub owner: Option<String>,
    pub task: String,
    pub due: Option<String>,
    pub source: Citation,
    pub text: String,
}

/// Exact expectations selected by the independent checker, never the guest.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Protected {
    pub schema: String,
    pub source_sha256: String,
    pub items: Vec<Item>,
    pub unassigned: usize,
    pub done_left_out: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Check {
    pub schema: String,
    pub passed: bool,
    pub failures: Vec<String>,
    pub snapshot_digest: String,
    pub report_digest: String,
    pub protected_digest: String,
    pub items_with_template: usize,
    pub items_without_file_access: usize,
    pub customer_accepted: bool,
    pub independently_attested: bool,
}

pub fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Verify the selected package/program bytes against this supported release.
pub fn release(package: &[u8], program: &[u8]) -> Result<(), String> {
    if package != PACKAGE.as_bytes() || program != PROGRAM.as_bytes() {
        return Err("The selected template differs from the supported exact release.".into());
    }
    let _: crate::package::Package = serde_json::from_slice(package).map_err(|e| e.to_string())?;
    Program::parse(program)?;
    Ok(())
}

impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != "openagents.workflow-template.snapshot.v1"
            || self.permission_epoch == 0
            || [
                &self.task,
                &self.customer,
                &self.human_owner,
                &self.recipient,
            ]
            .iter()
            .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
            || self.source_sha256.len() != 64
            || !self
                .source_sha256
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            || self.package_digest != crate::package::digest(PACKAGE)
            || self.program_digest != crate::package::digest(PROGRAM)
        {
            return Err("Invalid meeting follow-up snapshot or unsupported release.".into());
        }
        Ok(())
    }

    pub fn digest(&self) -> String {
        atif::digest(&json!(self))
    }

    pub fn bind_source(&self, source: &[u8]) -> Result<(), String> {
        self.validate()?;
        source_text(source)?;
        if sha256(source) != self.source_sha256 {
            return Err("Meeting bytes changed; prepare and approve a new snapshot.".into());
        }
        Ok(())
    }
}

fn source_text(source: &[u8]) -> Result<&str, String> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err("Meeting notes exceed 60 KiB; select a smaller complete input.".into());
    }
    std::str::from_utf8(source).map_err(|_| "Meeting notes must be UTF-8.".into())
}

/// Execute the exact program using immutable approved bytes and a fresh grant.
pub async fn run(
    snapshot: &Snapshot,
    approval: &Approval,
    source: &[u8],
) -> Result<Report, String> {
    snapshot.bind_source(source)?;
    if approval.snapshot_digest != snapshot.digest()
        || approval.recipient != snapshot.recipient
        || approval.permission_epoch != snapshot.permission_epoch
    {
        return Err(
            "Approval does not match this snapshot and current recipient/rights revision.".into(),
        );
    }
    let program = Program::parse(PROGRAM.as_bytes())?;
    let files = BTreeMap::from([("meeting.md".into(), source.to_vec())]);
    let runtime = bounded(Runtime::captured(files));
    let grant = Grant::selected(Some(&program.slug), Some("reads"));
    let inputs = Inputs::read("", "");
    runtime.admit(&program).map_err(|e| e.to_string())?;
    runtime
        .authorize(&program, &inputs, &grant)
        .map_err(|e| e.to_string())?;
    let with_template = attempt(runtime.run(&program, &inputs, &grant, None).await);
    let mut baseline = program.clone();
    baseline.steps[0].module.as_mut().unwrap()["read"] = json!([]);
    let without_file_access = attempt(
        bounded(Runtime::captured(BTreeMap::new()))
            .run(
                &baseline,
                &inputs,
                &Grant::selected(Some(&baseline.slug), Some("reads")),
                None,
            )
            .await,
    );
    Ok(Report {
        schema: "openagents.workflow-template.report.v1".into(),
        snapshot: snapshot.clone(),
        snapshot_digest: snapshot.digest(),
        with_template,
        without_file_access,
    })
}

fn bounded(runtime: Runtime) -> Runtime {
    runtime.with_budget(Budget {
        deadline: Some(Duration::from_secs(10)),
        max_steps: Some(1),
        ..Budget::default()
    })
}

fn attempt(run: Run) -> Attempt {
    let value = run
        .steps
        .last()
        .and_then(|s| serde_json::from_str::<Value>(&s.output).ok())
        .filter(|v| v["status"] == "ok")
        .and_then(|v| v.get("value").cloned());
    Attempt {
        finished: run.finished(),
        stopped: run.stopped.map(|e| e.to_string()),
        value,
    }
}

/// Check the exact report, source, and protected expectations independently.
/// A passing local check is not customer acceptance or remote attestation.
pub fn check(
    snapshot: &Snapshot,
    report: &Report,
    source: &[u8],
    expected: &Protected,
) -> Result<Check, String> {
    snapshot.bind_source(source)?;
    if report.schema != "openagents.workflow-template.report.v1"
        || report.snapshot != *snapshot
        || report.snapshot_digest != snapshot.digest()
        || expected.schema != "openagents.workflow-template.protected.v1"
        || expected.source_sha256 != snapshot.source_sha256
        || expected.items.len() > 200
    {
        return Err(
            "Report or protected checks do not bind this exact task/input snapshot.".into(),
        );
    }
    let mut failures = Vec::new();
    let lines: Vec<_> = source_text(source)?.lines().collect();
    for item in &expected.items {
        if item.source.from != "meeting.md"
            || item.source.line == 0
            || lines
                .get(item.source.line - 1)
                .map(|line| line.trim().chars().take(300).collect::<String>())
                .as_deref()
                != Some(item.text.as_str())
        {
            return Err("Protected expectations must cite exact lines of this input.".into());
        }
    }
    let mut counts = [0, 0];
    for (index, attempt) in [&report.with_template, &report.without_file_access]
        .iter()
        .enumerate()
    {
        let label = if index == 0 {
            "template"
        } else {
            "no-file-access baseline"
        };
        if !attempt.finished || attempt.stopped.is_some() {
            failures.push(format!("The {label} attempt stopped or failed."));
        }
        let Some(value) = &attempt.value else {
            failures.push(format!("The {label} output is missing."));
            continue;
        };
        let items = value
            .get("items")
            .cloned()
            .and_then(|v| serde_json::from_value::<Vec<Item>>(v).ok());
        counts[index] = items.as_ref().map_or(0, Vec::len);
        let wanted = if index == 0 {
            expected.items.as_slice()
        } else {
            &[]
        };
        let keys = [
            "kind",
            "found",
            "items",
            "unassigned",
            "done_left_out",
            "read",
            "unread",
            "truncated",
            "markdown",
        ];
        if !value.as_object().is_some_and(|object| {
            object.len() == keys.len() && keys.iter().all(|key| object.contains_key(*key))
        }) || !value["markdown"].is_string()
            || items.as_deref() != Some(wanted)
            || value["kind"] != "action-items"
            || value["found"] != !wanted.is_empty()
            || value["truncated"] != false
            || value["unread"] != json!([])
            || value["read"]
                != if index == 0 {
                    json!(["meeting.md"])
                } else {
                    json!([])
                }
            || value["unassigned"]
                != if index == 0 {
                    json!(expected.unassigned)
                } else {
                    json!(0)
                }
            || value["done_left_out"]
                != if index == 0 {
                    json!(expected.done_left_out)
                } else {
                    json!(0)
                }
        {
            failures.push(format!(
                "The {label} output fails the protected complete-result checks."
            ));
        }
    }
    Ok(Check {
        schema: "openagents.workflow-template.check.v1".into(),
        passed: failures.is_empty(),
        failures,
        snapshot_digest: snapshot.digest(),
        report_digest: atif::digest(&json!(report)),
        protected_digest: atif::digest(&json!(expected)),
        items_with_template: counts[0],
        items_without_file_access: counts[1],
        customer_accepted: false,
        independently_attested: false,
    })
}

#[cfg(test)]
#[path = "workflow_template/tests.rs"]
mod tests;
