//! Frozen requirement coverage and independent host checks for an exact candidate.
use super::*;
use crate::{
    capability::{self, Trust},
    verification,
};
use coder_boundary::Snapshot;
use serde_json::{Value, json};

pub const REQUIREMENTS_SCHEMA: &str = "openagents.coder.task-requirements.v1";
pub const CANDIDATE: &str = "{candidate_digest}";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub statement: String,
    pub checks: Vec<String>,
}

/// Host-prepared coverage, frozen in the execution grant before the candidate exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    pub schema: String,
    pub version: u64,
    pub requirements: Vec<Requirement>,
    pub plan: Value,
    /// File or directory scopes whose ancestor instructions must be captured.
    pub instruction_targets: Vec<PathBuf>,
    pub source_exclusions: Vec<String>,
}

impl Requirements {
    pub fn validate(&self) -> Result<verification::Plan, Error> {
        let plan: verification::Plan = serde_json::from_value(self.plan.clone())
            .map_err(|_| Error::InvalidCommand("invalid protected verification plan"))?;
        plan.validate()
            .map_err(|_| Error::InvalidCommand("invalid protected verification bounds"))?;
        if self.schema != REQUIREMENTS_SCHEMA
            || self.version == 0
            || self.requirements.is_empty()
            || self.requirements.len() > 64
            || self.instruction_targets.len() > 64
            || self.source_exclusions.len() > 256
            || plan.input_digest != CANDIDATE
            || plan.checks.iter().any(|check| {
                check.arguments != [CANDIDATE]
                    || !matches!(&check.acceptance,
                verification::Acceptance::Suite { input_digest, suite_digest } if input_digest == CANDIDATE && is_digest(suite_digest))
            })
        {
            return Err(Error::InvalidCommand(
                "requirements need nonempty coverage and independent typed suites",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut covered = BTreeSet::new();
        for requirement in &self.requirements {
            if !identifier(&requirement.id, false)
                || !ids.insert(&requirement.id)
                || !text(&requirement.statement, 8192, false)
                || requirement.checks.is_empty()
                || requirement.checks.len() > 64
            {
                return Err(Error::InvalidCommand("invalid requirement or coverage map"));
            }
            for check in &requirement.checks {
                if !plan.checks.iter().any(|entry| &entry.id == check) {
                    return Err(Error::InvalidCommand(
                        "requirement references an unknown check",
                    ));
                }
                covered.insert(check);
            }
        }
        if plan.checks.iter().any(|check| !covered.contains(&check.id)) {
            return Err(Error::InvalidCommand(
                "every check needs declared requirement coverage",
            ));
        }
        for target in &self.instruction_targets {
            if target.as_os_str().is_empty()
                || target
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
            {
                return Err(Error::UnsafePath);
            }
        }
        Ok(plan)
    }

    pub fn digest(&self) -> String {
        atif::digest(&json!(self))
    }

    fn protected_suites(&self, workspace: &Path) -> Result<Vec<ProtectedSuite>, Error> {
        let plan = self.validate()?;
        let workspace = workspace.canonicalize()?;
        let mut suites = Vec::new();
        for check in &plan.checks {
            let manifest = check.manifest.canonicalize()?;
            let entry = capability::Entry::load(&manifest, capability::Source::Operator)
                .map_err(|_| Error::InvalidCommand("the protected suite manifest is unreadable"))?;
            let declared = Path::new(&entry.manifest.detect.binary);
            if !declared.is_absolute() {
                return Err(Error::UnsafePath);
            }
            let program = declared.canonicalize()?;
            let verification::Acceptance::Suite { suite_digest, .. } = &check.acceptance else {
                return Err(Error::InvalidTransition);
            };
            let metadata = std::fs::metadata(&program)?;
            if !metadata.is_file() || metadata.len() > 128 * 1024 * 1024 {
                return Err(Error::LimitExceeded);
            }
            if manifest.starts_with(&workspace)
                || program.starts_with(&workspace)
                || program != declared
                || entry.digest != check.manifest_digest
                || entry.manifest.transport != capability::SUBPROCESS
                || format!(
                    "sha256:{}",
                    capability::digest_file(&program).map_err(|_| Error::UnsafePath)?
                ) != *suite_digest
            {
                return Err(Error::InvalidCommand(
                    "the suite must pin an unchanged executable and manifest outside the candidate workspace",
                ));
            }
            suites.push(ProtectedSuite {
                check: check.id.clone(),
                manifest,
                manifest_digest: entry.digest,
                program,
                program_digest: suite_digest.clone(),
            });
        }
        Ok(suites)
    }
}

fn is_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    })
}

/// The independent program and capability manifest frozen before the candidate exists.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedSuite {
    pub check: String,
    pub manifest: PathBuf,
    pub manifest_digest: String,
    pub program: PathBuf,
    pub program_digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContextFile {
    pub path: PathBuf,
    pub scope: PathBuf,
    pub digest: String,
    pub text: String,
}

/// Exact initial instruction inputs. Files are data and cannot widen the grant.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema: String,
    pub task_revision: u64,
    pub prompt: String,
    pub instructions: Vec<ContextFile>,
    pub suites: Vec<ProtectedSuite>,
    pub digest: String,
}

impl Context {
    fn expected_digest(&self) -> String {
        atif::digest(
            &json!({"schema":self.schema,"task_revision":self.task_revision,"prompt":self.prompt,"instructions":self.instructions,"suites":self.suites}),
        )
    }

    pub(super) fn valid(&self, task: &Task) -> bool {
        self.schema == "openagents.coder.task-context.v1"
            && self.task_revision == task.revision
            && self.prompt == task.effective_prompt()
            && self.digest == self.expected_digest()
            && self
                .instructions
                .iter()
                .all(|input| input.digest == digest_bytes(input.text.as_bytes()))
    }

    pub(super) fn capture(
        task: &Task,
        workspace: &Path,
        requirements: Option<&Requirements>,
    ) -> Result<Self, Error> {
        let suites = requirements
            .map(|requirements| requirements.protected_suites(workspace))
            .transpose()?
            .unwrap_or_default();
        let mut paths = BTreeSet::from([PathBuf::from("AGENTS.md")]);
        if let Some(requirements) = requirements {
            requirements.validate()?;
            for target in &requirements.instruction_targets {
                if workspace.join(target).is_dir() {
                    paths.insert(target.join("AGENTS.md"));
                }
                for parent in target.ancestors().skip(1) {
                    if !parent.as_os_str().is_empty() {
                        paths.insert(parent.join("AGENTS.md"));
                    }
                }
            }
        }
        let mut instructions = Vec::new();
        let mut total = 0;
        for path in paths {
            if !workspace.join(&path).try_exists()? {
                continue;
            }
            let mut bytes = Vec::new();
            artifact::confined_file(workspace, &path)?
                .take(MAX_COMMAND_BYTES as u64 + 1)
                .read_to_end(&mut bytes)?;
            total += bytes.len();
            if bytes.len() > MAX_COMMAND_BYTES || total > 256 * 1024 {
                return Err(Error::LimitExceeded);
            }
            let text = String::from_utf8(bytes.clone())
                .map_err(|_| Error::InvalidCommand("instructions must be UTF-8"))?;
            instructions.push(ContextFile {
                scope: path.parent().unwrap_or(Path::new("")).to_path_buf(),
                path,
                digest: digest_bytes(&bytes),
                text,
            });
        }
        instructions.sort_by(|a, b| {
            a.scope
                .components()
                .count()
                .cmp(&b.scope.components().count())
                .then(a.path.cmp(&b.path))
        });
        let mut context = Self {
            schema: "openagents.coder.task-context.v1".into(),
            task_revision: task.revision,
            prompt: task.effective_prompt().into(),
            instructions,
            suites,
            digest: String::new(),
        };
        context.digest = context.expected_digest();
        Ok(context)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub schema: String,
    pub requirements_digest: String,
    pub context_digest: String,
    pub candidate_snapshot: Option<String>,
    pub verdict: Checks,
    pub evidence: Option<Value>,
    pub reason: Option<String>,
}

pub(super) async fn execute(task: &Task, trust: &Trust) -> Result<Report, Error> {
    let run = task.run.as_ref().ok_or(Error::InvalidTransition)?;
    let requirements = run
        .admission
        .grant
        .requirements
        .as_ref()
        .ok_or(Error::InvalidTransition)?;
    let result = run.result.as_ref().ok_or(Error::InvalidTransition)?;
    let mut report = Report {
        schema: "openagents.coder.task-checks.v1".into(),
        requirements_digest: requirements.digest(),
        context_digest: run.admission.context.digest.clone(),
        candidate_snapshot: result.candidate_snapshot.clone(),
        verdict: Checks::Unavailable,
        evidence: None,
        reason: None,
    };
    let Some(candidate) = result.candidate_snapshot.as_ref() else {
        report.reason = Some("the candidate snapshot is incomplete or unavailable".into());
        return Ok(report);
    };
    let before = Snapshot::observe(&run.admission.workspace);
    if task.execution != Execution::Finished
        || !before.is_complete()
        || before.digest() != *candidate
    {
        report.reason = Some("the completed candidate is unavailable or changed".into());
        return Ok(report);
    }
    let current_suites = match requirements.protected_suites(&run.admission.workspace) {
        Ok(suites) if suites == run.admission.context.suites => suites,
        _ => {
            report.reason =
                Some("the protected suite program or manifest changed or is unavailable".into());
            return Ok(report);
        }
    };
    if current_suites.is_empty() {
        report.reason = Some("the frozen context contains no independent suites".into());
        return Ok(report);
    }
    let plan = prepared(requirements, candidate)?;
    match verification::run(&run.admission.workspace, &plan, trust).await {
        Ok(evidence) => {
            report.verdict = if evidence.before_snapshot != *candidate
                || evidence.after_snapshot != *candidate
            {
                Checks::Unavailable
            } else {
                match evidence.verdict {
                    verification::Verdict::Passed if evidence.checks.len() == plan.checks.len() => {
                        Checks::Passed
                    }
                    verification::Verdict::Failed => Checks::Failed,
                    _ => Checks::Unavailable,
                }
            };
            report.evidence = Some(json!(evidence));
        }
        Err(reason) => report.reason = Some(reason),
    }
    Ok(report)
}

fn prepared(requirements: &Requirements, candidate: &str) -> Result<verification::Plan, Error> {
    let mut plan = requirements.validate()?;
    plan.input_digest = candidate.to_string();
    for check in &mut plan.checks {
        if let verification::Acceptance::Suite { input_digest, .. } = &mut check.acceptance {
            *input_digest = candidate.to_string();
        }
        check.arguments = vec![candidate.to_string()];
    }
    Ok(plan)
}

// Deserialize only the existing verification engine's retained report contract.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    schema: String,
    plan_digest: String,
    input_digest: String,
    before_snapshot: String,
    after_snapshot: String,
    verdict: verification::Verdict,
    checks: Vec<Checked>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Checked {
    suite_evidence: Option<verification::SuiteEvidence>,
    id: String,
    verdict: verification::Verdict,
    reason: String,
    elapsed_ms: u64,
    stdout_digest: String,
    stderr_digest: String,
    output_truncated: bool,
}
impl Report {
    /// Validate retained coverage and suite evidence on both admission and replay.
    pub(super) fn validate(
        &self,
        requirements: &Requirements,
        context: &Context,
        candidate: Option<&str>,
    ) -> Result<(), Error> {
        if self.schema != "openagents.coder.task-checks.v1"
            || self.requirements_digest != requirements.digest()
            || self.context_digest != context.digest
            || self.candidate_snapshot.as_deref() != candidate
        {
            return Err(Error::InvalidTransition);
        }
        let Some(raw) = &self.evidence else {
            return if matches!(self.verdict, Checks::Unavailable | Checks::Disputed)
                && self
                    .reason
                    .as_ref()
                    .is_some_and(|reason| !reason.is_empty())
            {
                Ok(())
            } else {
                Err(Error::InvalidTransition)
            };
        };
        let candidate = candidate.ok_or(Error::InvalidTransition)?;
        let plan = prepared(requirements, candidate)?;
        let evidence: Evidence =
            serde_json::from_value(raw.clone()).map_err(|_| Error::InvalidTransition)?;
        if evidence.schema != verification::SCHEMA
            || evidence.plan_digest != plan.digest()
            || evidence.input_digest != candidate
            || evidence.checks.len() > plan.checks.len()
            || context.suites.len() != plan.checks.len()
        {
            return Err(Error::InvalidTransition);
        }
        for ((actual, expected), suite) in evidence
            .checks
            .iter()
            .zip(&plan.checks)
            .zip(&context.suites)
        {
            let verification::Acceptance::Suite { suite_digest, .. } = &expected.acceptance else {
                return Err(Error::InvalidTransition);
            };
            if actual.id != expected.id
                || suite.check != expected.id
                || suite.program_digest != *suite_digest
                || suite.manifest_digest != expected.manifest_digest
                || actual.reason.is_empty()
                || actual.elapsed_ms > plan.seconds.saturating_mul(1000).saturating_add(60000)
                || actual.stdout_digest.is_empty()
                || actual.stderr_digest.is_empty()
            {
                return Err(Error::InvalidTransition);
            }
            if let Some(proof) = &actual.suite_evidence
                && (proof.schema != verification::SCHEMA
                    || proof.input_digest != candidate
                    || proof.suite_digest != *suite_digest)
            {
                return Err(Error::InvalidTransition);
            }
            if actual.verdict == verification::Verdict::Passed
                && (actual.output_truncated
                    || actual
                        .suite_evidence
                        .as_ref()
                        .is_none_or(|proof| proof.verdict != verification::Verdict::Passed))
            {
                return Err(Error::InvalidTransition);
            }
        }
        let snapshots_match =
            evidence.before_snapshot == candidate && evidence.after_snapshot == candidate;
        let expected = if !snapshots_match {
            Checks::Unavailable
        } else {
            match evidence.verdict {
                verification::Verdict::Passed
                    if evidence.checks.len() == plan.checks.len()
                        && evidence
                            .checks
                            .iter()
                            .all(|c| c.verdict == verification::Verdict::Passed) =>
                {
                    Checks::Passed
                }
                verification::Verdict::Failed
                    if evidence
                        .checks
                        .iter()
                        .any(|c| c.verdict == verification::Verdict::Failed) =>
                {
                    Checks::Failed
                }
                verification::Verdict::Unverifiable => Checks::Unavailable,
                _ => return Err(Error::InvalidTransition),
            }
        };
        if self.verdict != expected {
            return Err(Error::InvalidTransition);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
