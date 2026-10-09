//! Reviewed promotion, rollback, saved history, and job pins (ENV-06).
//!
//! A [`Candidate`] is exactly what Save would record for one passed
//! verification: recipe revision, source pin, base and runtime pins, the
//! sealed image and its manifest digest, the build and verifier run links,
//! and the evidence digest. [`Environment::propose`] computes it without
//! side effects; a reviewer approves that displayed candidate as a
//! [`Review`]. Save and promotion recompute the candidate and refuse with
//! [`crate::Refusal::StaleReview`] when any field changed since display.
//!
//! Save creates an immutable [`crate::EnvironmentVersion`]; selection is a
//! separate fenced pointer with its own retained [`SelectionChange`]
//! history. Rollback selects an earlier saved version and never rewrites
//! one. New jobs copy a [`VersionPin`] once at admission; later selection
//! changes never reach a job that already holds its pin.

use crate::*;
use serde::{Deserialize, Serialize};

pub const MAX_SELECTIONS: usize = 1024;
/// The longest a review grant stays usable.
pub const MAX_REVIEW_MS: u64 = 24 * 60 * 60 * 1000;

/// Everything a reviewer must see before a version can be saved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub environment: String,
    pub verification_id: String,
    pub build_id: String,
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub source: SourcePin,
    pub base: ImagePin,
    pub runtime: ArtifactPin,
    pub image: ImageIdentity,
    pub build_run: RunLink,
    pub verification_run: RunLink,
    pub plan_digest: String,
    pub evidence_digest: String,
}
impl Candidate {
    pub fn digest(&self) -> String {
        digest(&serde_json::to_vec(self).expect("candidate encodes"))
    }
    /// The first field of `self` that differs from `current`.
    pub fn changed_field(&self, current: &Candidate) -> Option<&'static str> {
        let fields: [(&'static str, bool); 13] = [
            ("environment", self.environment == current.environment),
            (
                "verification",
                self.verification_id == current.verification_id,
            ),
            ("build", self.build_id == current.build_id),
            (
                "recipe_revision",
                self.recipe_revision == current.recipe_revision,
            ),
            ("recipe_digest", self.recipe_digest == current.recipe_digest),
            ("source", self.source == current.source),
            ("base", self.base == current.base),
            ("runtime", self.runtime == current.runtime),
            ("image", self.image == current.image),
            ("build_run", self.build_run == current.build_run),
            (
                "verification_run",
                self.verification_run == current.verification_run,
            ),
            ("plan", self.plan_digest == current.plan_digest),
            ("evidence", self.evidence_digest == current.evidence_digest),
        ];
        fields.into_iter().find(|(_, same)| !same).map(|(f, _)| f)
    }
}

/// An explicit review grant naming one exact displayed candidate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    /// Grant identity; one review saves at most one version.
    pub id: String,
    pub actor: String,
    pub candidate: Candidate,
    pub granted_ms: u64,
    pub expires_ms: u64,
}
impl Review {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !valid_id(&self.id) || !valid_id(&self.actor) {
            return Err("A review needs opaque review and reviewer identities.");
        }
        if self.expires_ms <= self.granted_ms || self.expires_ms - self.granted_ms > MAX_REVIEW_MS {
            return Err("A review needs a bounded validity window.");
        }
        Ok(())
    }
    pub fn stamp(&self) -> ReviewStamp {
        ReviewStamp {
            id: self.id.clone(),
            actor: self.actor.clone(),
            candidate_digest: self.candidate.digest(),
            granted_ms: self.granted_ms,
        }
    }
}

/// The review retained on a saved version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewStamp {
    pub id: String,
    pub actor: String,
    pub candidate_digest: String,
    pub granted_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SelectionKind {
    /// A reviewed save that also moved the selection.
    Promoted,
    /// Selected a version newer than the previous selection.
    Selected,
    /// Selected an earlier saved version.
    RolledBack,
}

/// One retained move of the project's selected-version pointer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionChange {
    pub revision: u64,
    pub kind: SelectionKind,
    pub version_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<String>,
    pub request_id: String,
    pub at_ms: u64,
}

/// What a job retains about the environment it started with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionPin {
    pub environment: String,
    pub version_id: String,
    pub number: u64,
    pub selection_revision: u64,
    pub recipe_revision: u64,
    pub recipe_digest: String,
    pub source: SourcePin,
    pub base: ImagePin,
    pub runtime: ArtifactPin,
    pub image: ImageIdentity,
    pub plan_digest: String,
    pub evidence_digest: String,
}

/// One saved version in history order, newest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryRow {
    pub version_id: String,
    pub number: u64,
    pub created_ms: u64,
    pub recipe_revision: u64,
    pub source_revision: String,
    pub image: ImageIdentity,
    pub evidence_digest: String,
    pub selected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewer: Option<String>,
    /// Selection revisions at which this version became selected.
    pub selected_at: Vec<u64>,
}

impl Environment {
    /// The exact candidate a reviewed Save of `verification_id` would
    /// record. No side effects.
    pub fn propose(&self, verification_id: &str) -> Result<Candidate, Refusal> {
        if self.retired_ms.is_some() {
            return Err(Refusal::Retired);
        }
        let v = self
            .verification(verification_id)
            .ok_or_else(|| Refusal::UnknownVerification(verification_id.into()))?;
        if let Some(saved) = self
            .versions
            .iter()
            .find(|s| s.verification_id == verification_id)
        {
            return Err(Refusal::AlreadySaved(saved.id.clone()));
        }
        let (VerificationState::Passed, Some(evidence_digest), Some(status)) =
            (v.state, &v.evidence_digest, v.evidence_status)
        else {
            return Err(Refusal::NotPassed(verification_id.into()));
        };
        if !status.complete() {
            return Err(Refusal::NotPassed(verification_id.into()));
        }
        let verification_run = v
            .run
            .clone()
            .ok_or_else(|| Refusal::NotLinked(verification_id.into()))?;
        let b = self
            .build(&v.build_id)
            .ok_or_else(|| Refusal::UnknownBuild(v.build_id.clone()))?;
        let build_run = b
            .run
            .clone()
            .ok_or_else(|| Refusal::NotLinked(b.id.clone()))?;
        if self.is_stale(b) {
            return Err(Refusal::StaleBuild {
                build_recipe: b.recipe_revision,
                draft: self.draft_revision,
            });
        }
        if b.image.as_ref() != Some(&v.image) {
            return Err(Refusal::ImageConflict(b.id.clone()));
        }
        let recipe = &self
            .recipe(b.recipe_revision)
            .ok_or(Refusal::Invalid(
                "The build names a missing recipe revision.",
            ))?
            .recipe;
        Ok(Candidate {
            environment: self.id.clone(),
            verification_id: v.id.clone(),
            build_id: b.id.clone(),
            recipe_revision: b.recipe_revision,
            recipe_digest: b.recipe_digest.clone(),
            source: b.source.clone(),
            base: recipe.base.clone(),
            runtime: recipe.runtime.clone(),
            image: v.image.clone(),
            build_run,
            verification_run,
            plan_digest: v.plan_digest.clone(),
            evidence_digest: evidence_digest.clone(),
        })
    }

    /// The exact version a job admitted now starts with, or `None` when no
    /// version is selected or the environment is retired. No side effects.
    pub fn pin(&self) -> Option<VersionPin> {
        if self.retired_ms.is_some() {
            return None;
        }
        let v = self.active()?;
        Some(VersionPin {
            environment: self.id.clone(),
            version_id: v.id.clone(),
            number: v.number,
            selection_revision: self.selection.revision,
            recipe_revision: v.recipe_revision,
            recipe_digest: v.recipe_digest.clone(),
            source: v.source.clone(),
            base: v.base.clone(),
            runtime: v.runtime.clone(),
            image: v.image.clone(),
            plan_digest: v.plan_digest.clone(),
            evidence_digest: v.evidence_digest.clone(),
        })
    }

    /// Saved versions newest first, at most `limit` rows older than
    /// version number `before` (exclusive) when given. No side effects.
    pub fn history(&self, before: Option<u64>, limit: usize) -> Vec<HistoryRow> {
        self.versions
            .iter()
            .rev()
            .filter(|v| before.is_none_or(|b| v.number < b))
            .take(limit.min(MAX_VERSIONS))
            .map(|v| HistoryRow {
                version_id: v.id.clone(),
                number: v.number,
                created_ms: v.created_ms,
                recipe_revision: v.recipe_revision,
                source_revision: v.source.revision.clone(),
                image: v.image.clone(),
                evidence_digest: v.evidence_digest.clone(),
                selected: self.selection.active.as_deref() == Some(v.id.as_str()),
                reviewer: v.review.as_ref().map(|r| r.actor.clone()),
                selected_at: self
                    .selections
                    .iter()
                    .filter(|s| s.version_id == v.id)
                    .map(|s| s.revision)
                    .collect(),
            })
            .collect()
    }
}
