//! Project dispatch uses the existing Coder program runtime and executor boundary.
//!
//! A returned answer is a reviewable result, not acceptance of an issue.

use std::path::{Path, PathBuf};
use std::time::Duration;

use coder::{Bounds, Inputs, Isolation, Program, Recorder, Runtime, Survey, Task};
use serde::{Deserialize, Serialize};
use serde_json::json;
use supervise::{Job, Limits};

pub mod artifact;
pub mod controller;
pub mod discovery;
pub mod github;
pub mod gym_suite;
pub mod poll;
pub mod reservations;
pub mod semantics;

/// An operator-prepared task. Tracker text cannot populate its authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub id: String,
    pub base: String,
    pub prompt: String,
    pub writes: bool,
    pub expected_text: Option<String>,
    pub minutes: u64,
}

impl Assignment {
    /// Reject malformed or unbounded input before the runtime probes executors.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || self.id.len() > 128
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(
                "task ID must contain 1–128 ASCII letters, digits, hyphens, or underscores".into(),
            );
        }
        if self.base.len() != 40 || !self.base.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("task base must be a full Git SHA-1 commit ID".into());
        }
        if self.prompt.trim().is_empty() || self.prompt.len() > 64 * 1024 {
            return Err("task prompt must contain 1–65536 bytes".into());
        }
        if !(1..=60).contains(&self.minutes) {
            return Err("task deadline must be between 1 and 60 minutes".into());
        }
        if self
            .expected_text
            .as_ref()
            .is_some_and(|s| s.trim().is_empty() || s.len() > 4096)
        {
            return Err("expected text must contain 1–4096 bytes when supplied".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(&self) -> String {
        atif::digest(&json!(self))
    }
}

/// Execution evidence. Artifact review and integration remain separate transitions.
#[derive(Debug, Serialize, Deserialize)]
pub struct DispatchReport {
    pub schema: String,
    pub task_id: String,
    pub input_digest: String,
    pub base: String,
    pub program_digest: String,
    pub trace: PathBuf,
    pub answered: bool,
    pub execution_status: Option<String>,
    pub text_matched: Option<bool>,
    pub refusal: Option<String>,
    pub retained_worktree: Option<PathBuf>,
    pub elapsed_ms: u64,
    pub executor_cost_usd: Option<f64>,
    pub artifact_verified: bool,
}

/// Run a bounded Git observation through the shared subprocess supervisor.
pub async fn git(repository: &Path, args: &[&str]) -> Result<String, String> {
    let result = Job::new("git")
        .arg("--no-pager")
        .args(args)
        .in_directory(repository)
        .bounded(Limits::within(Duration::from_secs(30)).keeping(4 * 1024 * 1024))
        .run()
        .await;
    if result.ending.code() != Some(0) || result.truncated() {
        return Err(format!(
            "bounded Git observation failed: {:?}",
            result.ending
        ));
    }
    Ok(result.stdout.text)
}

/// Execute one already admitted task. The caller owns the capacity reservation.
///
/// The survey's approved capabilities and the runtime's program permit remain
/// authoritative. This adapter does not use a model to select a program: the
/// operator explicitly selected the project workflow.
pub async fn dispatch(
    repository: &Path,
    survey: Survey,
    assignment: &Assignment,
    trace_path: &Path,
) -> Result<DispatchReport, String> {
    assignment.validate()?;
    protect_evidence(repository, trace_path, &survey)?;
    let base = git(repository, &["rev-parse", "HEAD"]).await?;
    if base.trim() != assignment.base {
        return Err(
            "repository base changed before dispatch; refresh and review the assignment".into(),
        );
    }
    let mut program: Program = serde_json::from_str(include_str!("project-task.json"))
        .map_err(|error| error.to_string())?;
    program
        .steps
        .last_mut()
        .ok_or("project program has no steps")?
        .bounds
        .insert("minutes".into(), json!(assignment.minutes));
    program.validate()?;
    let program_digest = atif::digest(&json!(program));
    let runtime = Runtime::using(survey, Some(repository));
    let mut trace = Recorder::at(
        trace_path,
        "unknown",
        "devin-local",
        &repository.display().to_string(),
    )?;
    trace.note(&format!(
        "Project task {} at base {}; input {}.",
        assignment.id,
        assignment.base,
        assignment.digest()
    ));
    let task = Task {
        prompt: assignment.prompt.clone(),
        purpose: format!("Implement or inspect project task {}.", assignment.id),
        reads: None,
        expected: assignment.expected_text.clone(),
        bounds: Bounds::minutes(assignment.minutes),
        isolation: Isolation::Worktree,
        writes: assignment.writes,
    };
    let inputs = Inputs {
        request: format!("Run the operator-admitted project task {}.", assignment.id),
        tasks: vec![task],
        executor: "devin-local".into(),
    };
    let grant = coder::program_authority::Grant::operator(None);
    let run = runtime
        .run(&program, &inputs, &grant, Some(&mut trace))
        .await;
    trace.finish("ended");
    if let Some(error) = trace.failure() {
        return Err(format!("execution evidence is incomplete: {error}"));
    }
    let delegation = run.delegations.first();
    Ok(DispatchReport {
        schema: "openagents.project-dispatch.v1".into(),
        task_id: assignment.id.clone(),
        input_digest: assignment.digest(),
        base: assignment.base.clone(),
        program_digest,
        trace: trace_path.to_path_buf(),
        answered: run.finished()
            && run.delegations.len() == 1
            && delegation.is_some_and(|d| d.answered()),
        execution_status: delegation.map(|d| d.status.to_string()),
        text_matched: delegation.and_then(|d| d.correct()),
        refusal: run.stopped.map(|r| format!("{}: {}", r.code, r.reason)),
        retained_worktree: delegation.and_then(|d| d.retained.clone()),
        elapsed_ms: delegation.map_or(0, |d| {
            d.elapsed.as_millis().min(u128::from(u64::MAX)) as u64
        }),
        executor_cost_usd: None,
        artifact_verified: false,
    })
}

/// Evidence is host-owned state, outside checkout and executor write grants.
pub fn protect_evidence(repository: &Path, path: &Path, survey: &Survey) -> Result<(), String> {
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err("host-owned execution state cannot be a symlink".into());
    }
    let repository = repository.canonicalize().map_err(|e| e.to_string())?;
    let parent = path
        .parent()
        .ok_or("evidence path has no parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if parent.starts_with(&repository) {
        return Err("execution evidence must be outside the delegated repository".into());
    }
    if let Some(executor) = survey.executor("devin-local") {
        for writable in executor.policy().writable() {
            let writable = writable.canonicalize().map_err(|e| e.to_string())?;
            if parent.starts_with(writable) {
                return Err("execution evidence is inside an executor write grant".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task() -> Assignment {
        Assignment {
            id: "9504-authority".into(),
            base: "a".repeat(40),
            prompt: "Inspect a public file.".into(),
            writes: false,
            expected_text: Some("done".into()),
            minutes: 5,
        }
    }

    #[test]
    fn task_identity_covers_effects_and_acceptance() {
        let baseline = task();
        let mut changed = baseline.clone();
        changed.writes = true;
        assert_ne!(baseline.digest(), changed.digest());
        changed = baseline.clone();
        changed.expected_text = Some("different".into());
        assert_ne!(baseline.digest(), changed.digest());
    }

    #[test]
    fn invalid_deadlines_and_path_like_ids_refuse() {
        for id in ["", "../outside", "a/b", "a\nsecond"] {
            let mut candidate = task();
            candidate.id = id.into();
            assert!(candidate.validate().is_err());
        }
        for minutes in [0, 61, u64::MAX] {
            let mut candidate = task();
            candidate.minutes = minutes;
            assert!(candidate.validate().is_err());
        }
    }

    #[test]
    fn dispatch_program_is_one_task_and_has_no_model_authority() {
        let program: Program = serde_json::from_str(include_str!("project-task.json")).unwrap();
        program.validate().unwrap();
        assert_eq!(program.steps[0].bounds["max_results"], 1);
        assert_eq!(program.steps[2].bounds["concurrent_max"], 1);
        assert!(program.steps.iter().all(|s| s.question.is_none()));
    }

    #[tokio::test]
    async fn changed_base_refuses_before_opening_execution_evidence() {
        let repository = tempfile::tempdir().unwrap();
        git(repository.path(), &["init", "--quiet"]).await.unwrap();
        git(
            repository.path(),
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "--quiet",
                "-m",
                "Fixture",
            ],
        )
        .await
        .unwrap();
        let survey = Survey {
            capabilities: vec![],
            programs: coder::program::Registry::open(&[]),
            sources: coder::source::Registry::open(&[]),
            workspace: repository.path().into(),
        };
        let evidence = tempfile::tempdir().unwrap();
        let trace = evidence.path().join("must-not-exist.jsonl");
        let error = dispatch(repository.path(), survey, &task(), &trace)
            .await
            .unwrap_err();
        assert!(error.contains("base changed"));
        assert!(!trace.exists());
    }
}
