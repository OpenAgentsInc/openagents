//! Host-prepared, bounded checks over an immutable candidate workspace.
//!
//! These checks produce mechanical evidence. They do not approve integration,
//! prove a model's judgment, or replace review of the check's coverage.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coder_boundary::{Boundary, Snapshot};
use serde::{Deserialize, Serialize};
use serde_json::json;
use supervise::{Job, Limits};

use crate::capability::{self, Entry, Trust, Verified};

pub const SCHEMA: &str = "openagents.verification.v1";

/// The operator chooses both the command and what constitutes evidence.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Acceptance {
    /// Suitable for an explicitly reviewed build or test command only.
    ExitSuccess,
    /// A suite must bind its identity and input and report a typed verdict.
    Suite {
        suite_digest: String,
        input_digest: String,
    },
}

impl Acceptance {
    /// The evidence kind's name, as a program step's `acceptance` bound
    /// spells it.
    ///
    /// A gated check names the evidence it requires, and the host refuses
    /// one the installed plan cannot answer with: exit status cannot
    /// satisfy a requested typed suite, and suite evidence cannot satisfy
    /// a step that asked for a bare exit status.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Acceptance::ExitSuccess => "exit-success",
            Acceptance::Suite { .. } => "suite",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Check {
    pub id: String,
    pub manifest: PathBuf,
    pub manifest_digest: String,
    pub arguments: Vec<String>,
    pub seconds: u64,
    pub output_bytes: usize,
    pub acceptance: Acceptance,
}

/// This plan must come from protected host configuration, never the artifact.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub input_digest: String,
    pub seconds: u64,
    pub allow_unrestricted_reads: bool,
    pub allow_network: bool,
    pub checks: Vec<Check>,
}

impl Plan {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SCHEMA
            || self.input_digest.is_empty()
            || !(1..=3600).contains(&self.seconds)
            || self.checks.is_empty()
            || self.checks.len() > 64
        {
            return Err(
                "verification requires a known schema, pinned input, and bounded nonempty checks"
                    .into(),
            );
        }
        if !self.allow_unrestricted_reads || !self.allow_network {
            return Err(
                "this verification boundary cannot enforce read or network restrictions".into(),
            );
        }
        let mut ids = BTreeSet::new();
        for check in &self.checks {
            if check.id.is_empty()
                || !ids.insert(&check.id)
                || !check.manifest.is_absolute()
                || check.manifest_digest.is_empty()
                || check.seconds == 0
                || check.seconds > self.seconds
                || !(1..=1024 * 1024).contains(&check.output_bytes)
                || check.arguments.len() > 128
                || check
                    .arguments
                    .iter()
                    .any(|arg| arg.len() > 65536 || arg.contains('\0'))
            {
                return Err(
                    "verification check identity, adapter, arguments, or bounds are invalid".into(),
                );
            }
            if let Acceptance::Suite {
                suite_digest,
                input_digest,
            } = &check.acceptance
                && (suite_digest.is_empty() || input_digest != &self.input_digest)
            {
                return Err("suite evidence must bind the prepared verification input".into());
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&json!(self))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    Passed,
    Failed,
    Unverifiable,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SuiteEvidence {
    pub schema: String,
    pub suite_digest: String,
    pub input_digest: String,
    pub verdict: Verdict,
}

#[derive(Clone, Debug, Serialize)]
pub struct Checked {
    pub id: String,
    pub verdict: Verdict,
    pub reason: String,
    pub elapsed_ms: u64,
    pub stdout_digest: String,
    pub stderr_digest: String,
    pub output_truncated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub schema: String,
    pub plan_digest: String,
    pub input_digest: String,
    pub before_snapshot: String,
    pub after_snapshot: String,
    pub verdict: Verdict,
    pub checks: Vec<Checked>,
}

fn judge(check: &Check, ended: &supervise::Ended) -> (Verdict, String) {
    if ended.truncated() {
        return (
            Verdict::Unverifiable,
            "check output exceeded its cap".into(),
        );
    }
    if !ended.ending.success() {
        return (Verdict::Failed, format!("check ended: {:?}", ended.ending));
    }
    match &check.acceptance {
        Acceptance::ExitSuccess => (
            Verdict::Passed,
            "the host-prepared command exited successfully".into(),
        ),
        Acceptance::Suite {
            suite_digest,
            input_digest,
        } => {
            let Ok(evidence) = serde_json::from_str::<SuiteEvidence>(&ended.stdout.text) else {
                return (
                    Verdict::Unverifiable,
                    "suite output is missing or malformed".into(),
                );
            };
            if evidence.schema != SCHEMA
                || &evidence.suite_digest != suite_digest
                || &evidence.input_digest != input_digest
            {
                return (
                    Verdict::Unverifiable,
                    "suite evidence does not match the pinned identities".into(),
                );
            }
            (
                evidence.verdict,
                "typed suite verdict with matching identities".into(),
            )
        }
    }
}

/// Run serial checks with freshly verified capability approval and bounded output.
/// The candidate is read-only; temporary files live in supervisor-owned scratch.
/// The caller must bind `input_digest` to the independently inspected artifact.
pub async fn run(workspace: &Path, plan: &Plan, trust: &Trust) -> Result<Report, String> {
    plan.validate()?;
    let workspace = workspace.canonicalize().map_err(|e| e.to_string())?;
    let started = Instant::now();
    let before = Snapshot::observe(&workspace);
    if !before.is_complete() {
        return Err("candidate snapshot is incomplete; checks did not run".into());
    }
    let mut checks = Vec::new();
    for check in &plan.checks {
        if started.elapsed() >= Duration::from_secs(plan.seconds) {
            break;
        }
        let entry = Entry::load(&check.manifest, capability::Source::Operator)?;
        if entry.digest != check.manifest_digest
            || entry.manifest.transport != capability::SUBPROCESS
        {
            return Err("verification adapter changed or is not a subprocess capability".into());
        }
        let adapter = match trust.decide_verified(&entry, &workspace) {
            Verified::Approved(record) => record.adapter,
            Verified::Unconditional => {
                capability::resolve(&entry.manifest.detect.binary, &capability::search_dirs())
                    .ok_or("verification adapter is unavailable")?
            }
            Verified::Unapproved(reason) => return Err(reason),
        };
        let boundary = Boundary::readonly()
            .protecting(&workspace)
            .sealed(&entry.path)
            .sealed(&adapter)
            .owned_scratch_under(std::env::temp_dir())
            .build()
            .map_err(|e| e.to_string())?;
        let scratch = boundary
            .scratch()
            .ok_or("verification scratch is missing")?;
        let mut command = boundary
            .command(&adapter, &check.arguments)
            .map_err(|e| e.to_string())?;
        command
            .env_clear()
            .current_dir(&workspace)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", scratch)
            .env("TMPDIR", scratch)
            .env("TMP", scratch)
            .env("TEMP", scratch)
            .env("CARGO_TARGET_DIR", scratch.join("target"))
            .env("CARGO_BUILD_JOBS", "1")
            .env("RUST_TEST_THREADS", "1");
        for name in ["RUSTUP_HOME", "CARGO_HOME"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let Some(remaining) = Duration::from_secs(plan.seconds)
            .checked_sub(started.elapsed())
            .filter(|remaining| !remaining.is_zero())
        else {
            break;
        };
        let begun = Instant::now();
        let ended = Job::from_command(command)
            .bounded(
                Limits::within(remaining.min(Duration::from_secs(check.seconds)))
                    .keeping(check.output_bytes),
            )
            .run_holding(boundary.hold())
            .await;
        let (verdict, reason) = judge(check, &ended);
        checks.push(Checked {
            id: check.id.clone(),
            verdict,
            reason,
            elapsed_ms: begun.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            stdout_digest: atif::digest(&json!(ended.stdout.text)),
            stderr_digest: atif::digest(&json!(ended.stderr.text)),
            output_truncated: ended.truncated(),
        });
        if verdict != Verdict::Passed {
            break;
        }
    }
    let after = Snapshot::observe(&workspace);
    let verdict = if !before.is_complete()
        || !after.is_complete()
        || coder_boundary::compare(&before, &after).is_unverifiable()
    {
        Verdict::Unverifiable
    } else if !coder_boundary::compare(&before, &after).is_clean()
        || checks.iter().any(|check| check.verdict == Verdict::Failed)
    {
        Verdict::Failed
    } else if checks.len() != plan.checks.len()
        || checks.iter().any(|check| check.verdict != Verdict::Passed)
    {
        Verdict::Unverifiable
    } else {
        Verdict::Passed
    };
    Ok(Report {
        schema: SCHEMA.into(),
        plan_digest: plan.digest(),
        input_digest: plan.input_digest.clone(),
        before_snapshot: before.digest(),
        after_snapshot: after.digest(),
        verdict,
        checks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(
        command: &str,
        acceptance: Acceptance,
    ) -> (tempfile::TempDir, tempfile::TempDir, Plan) {
        let host = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        std::fs::write(workspace.path().join("candidate.txt"), "immutable\n").unwrap();
        let manifest = host.path().join("check.json");
        std::fs::write(&manifest, serde_json::to_vec(&json!({
            "v":1,"slug":"verification-fixture","name":"Verification fixture","transport":"subprocess",
            "detect":{"binary":"/bin/sh","version":["/bin/sh","--version"]},
            "enforces":[],"cannot_enforce":[],"sees_repository":true,
            "cost":"local","invoke":["/bin/sh"],"isolation":["directory"]
        })).unwrap()).unwrap();
        let entry = Entry::load(&manifest, capability::Source::Operator).unwrap();
        let plan = Plan {
            schema: SCHEMA.into(),
            input_digest: "candidate-digest".into(),
            seconds: 10,
            allow_unrestricted_reads: true,
            allow_network: true,
            checks: vec![Check {
                id: "fixture".into(),
                manifest,
                manifest_digest: entry.digest,
                arguments: vec!["-c".into(), command.into()],
                seconds: 3,
                output_bytes: 4096,
                acceptance,
            }],
        };
        (host, workspace, plan)
    }

    fn supported() -> bool {
        if crate::delegate::boundary_supported() {
            true
        } else {
            eprintln!("skipping: verification needs an enforcing filesystem boundary");
            false
        }
    }

    #[tokio::test]
    async fn success_failure_and_immutable_candidate_are_independent_evidence() {
        if !supported() {
            return;
        }
        let (_host, workspace, plan) = fixture("test -r candidate.txt", Acceptance::ExitSuccess);
        let result = run(workspace.path(), &plan, &Trust::everything())
            .await
            .unwrap();
        assert_eq!(result.verdict, Verdict::Passed);
        assert_eq!(result.before_snapshot, result.after_snapshot);
        let mut failure = plan.clone();
        failure.checks[0].arguments[1] = "exit 7".into();
        assert_eq!(
            run(workspace.path(), &failure, &Trust::everything())
                .await
                .unwrap()
                .verdict,
            Verdict::Failed
        );
        failure.checks[0].arguments[1] = "echo changed > candidate.txt".into();
        assert_eq!(
            run(workspace.path(), &failure, &Trust::everything())
                .await
                .unwrap()
                .verdict,
            Verdict::Failed
        );
        assert_eq!(
            std::fs::read_to_string(workspace.path().join("candidate.txt")).unwrap(),
            "immutable\n"
        );
    }

    #[tokio::test]
    async fn missing_stale_and_unverifiable_suite_evidence_never_pass() {
        if !supported() {
            return;
        }
        let acceptance = Acceptance::Suite {
            suite_digest: "suite-1".into(),
            input_digest: "candidate-digest".into(),
        };
        let (_host, workspace, mut plan) = fixture("true", acceptance);
        for (output, expected) in [
            ("", Verdict::Unverifiable),
            (
                r#"{"schema":"openagents.verification.v1","suite_digest":"stale","input_digest":"candidate-digest","verdict":"passed"}"#,
                Verdict::Unverifiable,
            ),
            (
                r#"{"schema":"openagents.verification.v1","suite_digest":"suite-1","input_digest":"candidate-digest","verdict":"unverifiable"}"#,
                Verdict::Unverifiable,
            ),
            (
                r#"{"schema":"openagents.verification.v1","suite_digest":"suite-1","input_digest":"candidate-digest","verdict":"passed"}"#,
                Verdict::Passed,
            ),
        ] {
            plan.checks[0].arguments[1] = format!("printf '%s' '{output}'");
            assert_eq!(
                run(workspace.path(), &plan, &Trust::everything())
                    .await
                    .unwrap()
                    .verdict,
                expected
            );
        }
    }

    #[tokio::test]
    async fn timeout_and_output_overflow_stop_later_checks() {
        if !supported() {
            return;
        }
        let (_host, workspace, mut plan) = fixture("sleep 10", Acceptance::ExitSuccess);
        plan.checks[0].seconds = 1;
        let mut second = plan.checks[0].clone();
        second.id = "second".into();
        second.arguments[1] = "true".into();
        plan.checks.push(second);
        let result = run(workspace.path(), &plan, &Trust::everything())
            .await
            .unwrap();
        assert_eq!(result.verdict, Verdict::Failed);
        assert_eq!(result.checks.len(), 1);
        plan.checks[0].arguments[1] = "printf 'output past bound'".into();
        plan.checks[0].output_bytes = 1;
        let result = run(workspace.path(), &plan, &Trust::everything())
            .await
            .unwrap();
        assert_eq!(result.verdict, Verdict::Unverifiable);
        assert!(result.checks[0].output_truncated);
        assert_eq!(result.checks.len(), 1);
    }

    #[tokio::test]
    async fn changed_or_unapproved_adapters_do_not_run() {
        if !supported() {
            return;
        }
        let (_host, workspace, mut plan) = fixture("true", Acceptance::ExitSuccess);
        assert!(run(workspace.path(), &plan, &Trust::empty()).await.is_err());
        plan.checks[0].manifest_digest = "wrong".into();
        assert!(
            run(workspace.path(), &plan, &Trust::everything())
                .await
                .unwrap_err()
                .contains("changed")
        );
    }

    #[tokio::test]
    async fn runtime_checks_require_authority_and_record_a_separate_verdict() {
        if !supported() {
            return;
        }
        let (_host, workspace, plan) = fixture("true", Acceptance::ExitSuccess);
        let survey = crate::Survey {
            capabilities: vec![],
            programs: crate::program::Registry::open(&[]),
            sources: crate::source::Registry::open(&[]),
            workspace: workspace.path().into(),
        };
        let runtime = crate::Runtime::using(survey, None).with_verification(
            workspace.path().into(),
            plan,
            Trust::everything(),
        );
        let program:crate::Program=serde_json::from_value(json!({"v":1,"slug":"verify-fixture","steps":[{"name":"gate","kind":"check","bounds":{"refuse_on":"gate_not_met"}}]})).unwrap();
        let inputs = crate::Inputs::read("Run the host-prepared check.", "");
        let refused = runtime
            .run(&program, &inputs, &crate::Grant::none(), None)
            .await;
        assert!(refused.verification.is_empty());
        assert_eq!(refused.stopped.unwrap().code, "unauthorized");
        let missing_effects = crate::Grant::selected(Some("verify-fixture"), Some("reads"));
        assert_eq!(
            runtime
                .run(&program, &inputs, &missing_effects, None)
                .await
                .stopped
                .unwrap()
                .code,
            "unauthorized"
        );
        let run = runtime
            .run(&program, &inputs, &crate::Grant::all(), None)
            .await;
        assert!(run.finished(), "{:?}", run.stopped);
        assert_eq!(run.verification.len(), 1);
        assert_eq!(run.verification[0].verdict, Verdict::Passed);
        assert!(run.delegations.is_empty());
        assert!(run.summary().contains("independent verification"));
    }

    #[test]
    fn unsupported_restrictions_and_missing_requirements_refuse() {
        let (_host, _workspace, mut plan) = fixture("true", Acceptance::ExitSuccess);
        plan.allow_network = false;
        assert!(plan.validate().is_err());
        plan.allow_network = true;
        plan.checks.clear();
        assert!(plan.validate().is_err());
    }
}
