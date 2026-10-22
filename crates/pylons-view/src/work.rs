//! Private, bounded execution summaries for the staff Pylon work view.
//!
//! Publishers supply reviewed summaries of observed actions. These records
//! carry no prompts, code, raw tool output, or model thoughts.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SCHEMA: &str = "openagents.pylon.work.v1";
pub const PATH: &str = "/v1/pylons/work";
pub const MAX_STEPS: usize = 32;
pub const MAX_FILES: usize = 64;
pub const MAX_CHECKS: usize = 16;
pub const MAX_BODY_BYTES: usize = 48 * 1024;
pub const STALE_SECONDS: u64 = 120;
pub const FUTURE_SECONDS: u64 = 30;

/// A private GET response, including the absence of a work report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkView {
    pub snapshot: Option<WorkSnapshot>,
    pub stale: bool,
}

/// One session's latest observed activity for one exact Pylon address.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkSnapshot {
    pub schema: String,
    pub pylon: String,
    pub session_id: String,
    pub session_started_at: u64,
    pub sequence: u64,
    pub observed_at: u64,
    pub issue: Option<u64>,
    pub title: String,
    pub phase: WorkPhase,
    pub current: Option<WorkStep>,
    pub turns: u32,
    pub calls: u32,
    pub changed: u32,
    pub files: Vec<String>,
    pub checks: Vec<WorkCheck>,
    pub steps: Vec<WorkStep>,
    pub answer: Option<WorkAnswer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkPhase {
    Queued,
    Working,
    Checking,
    Done,
    Failed,
}

/// A short label and reviewed summary of an actual action, never raw output.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkStep {
    pub sequence: u64,
    pub at: u64,
    pub kind: WorkStepKind,
    pub label: String,
    pub path: Option<String>,
    pub summary: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStepKind {
    Read,
    Write,
    Command,
    Check,
    Answer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkCheck {
    pub label: String,
    /// One of `passed`, `failed`, `running`, or `pending`.
    pub status: String,
    pub summary: String,
}

/// The actual request and serving identity observed by the bridge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkAnswer {
    pub request: String,
    pub provider: String,
    pub model: String,
    pub profile: String,
    pub lossy: bool,
}

impl WorkSnapshot {
    /// Validate the bounded, reviewed summary before saving or displaying it.
    ///
    /// # Errors
    /// Returns a fixed reason when the report is malformed or too old to publish.
    pub fn validate(&self, now: u64) -> Result<(), &'static str> {
        let provider = provider(&self.pylon).ok_or("invalid Pylon address")?;
        if self.schema != SCHEMA
            || !identifier(&self.session_id, 96)
            || self.sequence == 0
            || self.session_started_at == 0
            || self.session_started_at > self.observed_at
            || self.observed_at > now.saturating_add(FUTURE_SECONDS)
            || now.saturating_sub(self.observed_at) > STALE_SECONDS
            || self.issue == Some(0)
            || !safe_text(&self.title, 160, false)
        {
            return Err("invalid work session");
        }
        if self.steps.len() > MAX_STEPS
            || self.files.len() > MAX_FILES
            || self.checks.len() > MAX_CHECKS
        {
            return Err("work report exceeds its limits");
        }
        let mut files = BTreeSet::new();
        if self.files.iter().any(|p| !safe_path(p) || !files.insert(p)) {
            return Err("invalid work file list");
        }
        for check in &self.checks {
            if !safe_text(&check.label, 96, false)
                || !safe_text(&check.summary, 240, true)
                || !matches!(
                    check.status.as_str(),
                    "passed" | "failed" | "running" | "pending"
                )
            {
                return Err("invalid work check");
            }
        }
        let mut previous = None;
        for step in &self.steps {
            self.validate_step(step)?;
            if previous.is_some_and(|(sequence, at)| step.sequence <= sequence || step.at < at) {
                return Err("work steps are not ordered");
            }
            previous = Some((step.sequence, step.at));
        }
        if let Some(step) = &self.current {
            self.validate_step(step)?;
        }
        if let Some(answer) = &self.answer
            && (!hex_id(&answer.request)
                || answer.provider != provider
                || !safe_text(&answer.model, 160, false)
                || !identifier(&answer.profile, 96))
        {
            return Err("invalid serving identity");
        }
        Ok(())
    }

    fn validate_step(&self, step: &WorkStep) -> Result<(), &'static str> {
        if step.sequence == 0
            || step.at < self.session_started_at
            || step.at > self.observed_at
            || !safe_text(&step.label, 96, false)
            || !safe_text(&step.summary, 240, true)
            || step.path.as_deref().is_some_and(|p| !safe_path(p))
        {
            return Err("invalid work step");
        }
        Ok(())
    }

    /// A new session must begin later. A running session keeps its start time
    /// and advances its sequence without moving its observation clock backward.
    #[must_use]
    pub fn is_after(&self, previous: &Self) -> bool {
        self.pylon == previous.pylon
            && self.observed_at >= previous.observed_at
            && if self.session_id == previous.session_id {
                self.session_started_at == previous.session_started_at
                    && self.sequence > previous.sequence
            } else {
                self.session_started_at > previous.session_started_at
            }
    }
}

impl WorkView {
    #[must_use]
    pub fn at(snapshot: Option<WorkSnapshot>, now: u64) -> Self {
        let stale = snapshot
            .as_ref()
            .is_some_and(|s| now.saturating_sub(s.observed_at) > STALE_SECONDS);
        Self { snapshot, stale }
    }
}

/// Return the producer from an exact NIP-PYLON address.
#[must_use]
pub fn provider(address: &str) -> Option<&str> {
    let mut parts = address.split(':');
    let (kind, key, slug) = (parts.next()?, parts.next()?, parts.next()?);
    (kind == "30200" && hex_id(key) && identifier(slug, 96) && parts.next().is_none())
        .then_some(key)
}

fn hex_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn safe_text(value: &str, max: usize, empty: bool) -> bool {
    let lower = value.to_ascii_lowercase();
    value.len() <= max
        && (empty || !value.trim().is_empty())
        && !value.chars().any(char::is_control)
        && !["nsec1", "oak_", "sk-proj-", "bearer ", "-----begin", "```"]
            .iter()
            .any(|p| lower.contains(p))
}

fn safe_path(value: &str) -> bool {
    safe_text(value, 240, false)
        && !value.starts_with('/')
        && !value.contains(['\\', ':'])
        && value.split('/').all(|part| {
            let lower = part.to_ascii_lowercase();
            !part.is_empty()
                && part != "."
                && part != ".."
                && !lower.starts_with(".env")
                && !lower.contains("secret")
                && !lower.contains("credential")
                && !matches!(
                    lower.as_str(),
                    "provider.key" | "id_rsa" | "id_ed25519" | "google_compute_engine"
                )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn snapshot() -> WorkSnapshot {
        WorkSnapshot {
            schema: SCHEMA.into(),
            pylon: format!("30200:{}:maya-1", "11".repeat(32)),
            session_id: "workspace-1".into(),
            session_started_at: 1000,
            sequence: 2,
            observed_at: 1010,
            issue: Some(12524),
            title: "Show current Pylon work".into(),
            phase: WorkPhase::Working,
            current: None,
            turns: 1,
            calls: 2,
            changed: 1,
            files: vec!["crates/pylons-view/src/work.rs".into()],
            checks: vec![WorkCheck {
                label: "Unit tests".into(),
                status: "running".into(),
                summary: String::new(),
            }],
            steps: vec![WorkStep {
                sequence: 1,
                at: 1005,
                kind: WorkStepKind::Read,
                label: "Read file".into(),
                path: Some("Cargo.toml".into()),
                summary: "Checked crate dependencies.".into(),
            }],
            answer: Some(WorkAnswer {
                request: "22".repeat(32),
                provider: "11".repeat(32),
                model: "deepseek-v4.1-flash".into(),
                profile: "consumer-q".into(),
                lossy: true,
            }),
        }
    }

    #[test]
    fn bounds_and_serving_identity_are_checked() {
        let good = snapshot();
        assert_eq!(good.validate(1010), Ok(()));
        let mut bad = good.clone();
        bad.steps = vec![good.steps[0].clone(); MAX_STEPS + 1];
        assert!(bad.validate(1010).is_err());
        let mut bad = good.clone();
        bad.files = vec!["Cargo.toml".into(); MAX_FILES + 1];
        assert!(bad.validate(1010).is_err());
        let mut bad = good.clone();
        bad.answer.as_mut().unwrap().provider = "33".repeat(32);
        assert!(bad.validate(1010).is_err());
        let mut bad = good;
        bad.title = "x".repeat(161);
        assert!(bad.validate(1010).is_err());
    }

    #[test]
    fn private_paths_and_raw_or_secret_fields_are_refused() {
        for path in [
            "../Cargo.toml",
            "/Users/owner/project.rs",
            ".env",
            ".env.production",
            ".secrets/key",
            "provider.key",
        ] {
            let mut bad = snapshot();
            bad.files = vec![path.into()];
            assert!(bad.validate(1010).is_err(), "{path}");
        }
        let mut value = serde_json::to_value(snapshot()).unwrap();
        for field in ["thought", "prompt", "raw_output", "workspace"] {
            value[field] = serde_json::json!("unreviewed");
            assert!(serde_json::from_value::<WorkSnapshot>(value.clone()).is_err());
            value.as_object_mut().unwrap().remove(field);
        }
        let mut bad = snapshot();
        bad.steps[0].summary = "Bearer private-token".into();
        assert!(bad.validate(1010).is_err());
    }

    #[test]
    fn sessions_sequences_and_step_timestamps_do_not_move_backwards() {
        let previous = snapshot();
        let mut next = previous.clone();
        next.sequence += 1;
        assert!(next.is_after(&previous));
        assert!(!previous.is_after(&previous));
        next.observed_at = 1009;
        assert!(!next.is_after(&previous));
        next = previous.clone();
        next.session_id = "replacement".into();
        assert!(!next.is_after(&previous));
        next.session_started_at = 1001;
        assert!(next.is_after(&previous));
        let mut bad = previous;
        bad.steps.push(bad.steps[0].clone());
        assert!(bad.validate(1010).is_err());
        bad.steps.pop();
        bad.steps[0].at = 999;
        assert!(bad.validate(1010).is_err());
    }

    #[test]
    fn stale_reports_stay_visible_and_new_stale_publications_fail() {
        let snapshot = snapshot();
        assert!(snapshot.validate(1131).is_err());
        assert!(WorkView::at(Some(snapshot.clone()), 1131).stale);
        assert!(!WorkView::at(Some(snapshot), 1130).stale);
        assert_eq!(WorkView::at(None, 2000), WorkView::default());
    }
}
