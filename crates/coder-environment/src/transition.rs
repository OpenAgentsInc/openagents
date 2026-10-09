//! Pure environment transitions with explicit typed outcomes.
//!
//! [`apply`] never performs I/O. It returns the next record and the effect,
//! a replay of an earlier accepted request, or a typed [`Refusal`]. Callers
//! retain the returned record (see [`crate::store`]) before acting on it.

use crate::*;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    /// Replace the draft recipe; fenced by the expected draft revision.
    UpdateRecipe {
        expected_draft_revision: u64,
        recipe: Recipe,
    },
    /// Freeze the current draft into a build attempt.
    StartBuild {
        request_id: String,
        expected_draft_revision: u64,
    },
    ObserveBuild {
        build_id: String,
        observation: BuildObservation,
    },
    /// Verify one sealed build image against its frozen plan.
    StartVerification {
        request_id: String,
        build_id: String,
        plan_digest: String,
    },
    ObserveVerification {
        verification_id: String,
        observation: VerificationObservation,
    },
    /// Seal a reviewed, passed verification into an immutable version.
    /// The review names the exact candidate it approved; any change since
    /// it was displayed refuses the save. Selection is not changed.
    SaveVersion { request_id: String, review: Review },
    /// Reviewed Save plus a fenced selection update in one retained
    /// change: the new immutable version and the moved pointer are
    /// separate records, and both or neither are retained.
    Promote {
        request_id: String,
        expected_selection_revision: u64,
        review: Review,
    },
    /// Select a saved version for later project work. Selecting an earlier
    /// version is a rollback; no version is ever rewritten.
    Select {
        request_id: String,
        expected_selection_revision: u64,
        version_id: String,
    },
    /// Stop new drafts, builds, saves, and selections. In-flight attempts can
    /// still be observed and reconciled.
    Retire,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum BuildObservation {
    Linked {
        run: RunLink,
    },
    Progress {
        state: BuildState,
    },
    ImageReady {
        image: ImageIdentity,
    },
    Failed {
        reason: String,
    },
    Cancelled,
    /// The owner could not learn what happened (lost reply, restart).
    Unknown {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum VerificationObservation {
    Linked {
        run: RunLink,
    },
    Progress {
        state: VerificationState,
    },
    /// The checks passed. `evidence` is the sealed status of the evidence
    /// manifest `evidence_digest` names ([`crate::evidence::Sealed`]); only
    /// complete evidence lets the verification pass and a version save.
    Passed {
        evidence_digest: String,
        evidence: crate::evidence::EvidenceStatus,
    },
    /// The checks failed. `evidence` cites the sealed record of the run
    /// when one was sealed.
    Failed {
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<crate::evidence::Sealed>,
    },
    Incomplete {
        reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        evidence: Option<crate::evidence::Sealed>,
    },
    Cancelled,
    Unknown {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    RecipeRevised {
        revision: u64,
        digest: String,
    },
    BuildRequested {
        build_id: String,
        recipe_revision: u64,
    },
    BuildObserved {
        build_id: String,
        state: BuildState,
    },
    VerificationRequested {
        verification_id: String,
        build_id: String,
    },
    VerificationObserved {
        verification_id: String,
        state: VerificationState,
    },
    VersionSaved {
        version_id: String,
    },
    Promoted {
        version_id: String,
        selection_revision: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous: Option<String>,
    },
    Selected {
        version_id: String,
        selection_revision: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous: Option<String>,
        change: SelectionKind,
    },
    Retired,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// Retain this record, then report the effect.
    Changed(Box<Environment>, Effect),
    /// Already true or already accepted; nothing to retain.
    Replayed(Effect),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Invalid(&'static str),
    StaleDraft {
        expected: u64,
        current: u64,
    },
    StaleSelection {
        expected: u64,
        current: u64,
    },
    /// The build was made from a recipe revision the draft has moved past.
    StaleBuild {
        build_recipe: u64,
        draft: u64,
    },
    RequestConflict(String),
    UnknownBuild(String),
    UnknownVerification(String),
    UnknownVersion(String),
    /// An attempt's outcome is unknown; reconcile it first.
    Unresolved(String),
    Terminal(String),
    Regression(String),
    RunConflict(String),
    ImageConflict(String),
    BuildNotReady(String),
    PlanMismatch,
    NotPassed(String),
    NotLinked(String),
    AlreadySaved(String),
    /// The reviewed candidate differs from the current one in this field.
    StaleReview(&'static str),
    /// The review grant's validity window has passed.
    ReviewExpired(String),
    /// The review grant already saved a version.
    ReviewUsed {
        review: String,
        version: String,
    },
    Retired,
    Limit(&'static str),
}
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(m) | Self::Limit(m) => f.write_str(m),
            Self::StaleDraft { expected, current } => write!(
                f,
                "The draft is at revision {current}, not the expected {expected}."
            ),
            Self::StaleSelection { expected, current } => write!(
                f,
                "The project selection is at revision {current}, not the expected {expected}."
            ),
            Self::StaleBuild {
                build_recipe,
                draft,
            } => write!(
                f,
                "The build used recipe revision {build_recipe}; the draft is at {draft}."
            ),
            Self::RequestConflict(id) => {
                write!(
                    f,
                    "Request {id} was already used for a different operation."
                )
            }
            Self::UnknownBuild(id) => write!(f, "No build {id}."),
            Self::UnknownVerification(id) => write!(f, "No verification {id}."),
            Self::UnknownVersion(id) => write!(f, "No version {id}."),
            Self::Unresolved(id) => {
                write!(f, "The outcome of {id} is unknown; reconcile it first.")
            }
            Self::Terminal(id) => write!(f, "{id} has already finished."),
            Self::Regression(id) => write!(f, "{id} cannot move backward."),
            Self::RunConflict(id) => write!(f, "{id} is already linked to another run."),
            Self::ImageConflict(id) => write!(f, "{id} already sealed a different image."),
            Self::BuildNotReady(id) => write!(f, "Build {id} has no sealed image."),
            Self::PlanMismatch => f.write_str("The plan differs from the recipe's frozen plan."),
            Self::NotPassed(id) => write!(f, "Verification {id} has not passed."),
            Self::NotLinked(id) => write!(f, "{id} has no linked run."),
            Self::AlreadySaved(id) => write!(f, "This verification was already saved as {id}."),
            Self::StaleReview(field) => write!(
                f,
                "The reviewed candidate's {field} changed since it was displayed; review it again."
            ),
            Self::ReviewExpired(id) => write!(f, "Review {id} has expired; review it again."),
            Self::ReviewUsed { review, version } => {
                write!(f, "Review {review} already saved {version}.")
            }
            Self::Retired => f.write_str("The environment is retired."),
        }
    }
}
impl std::error::Error for Refusal {}

/// Apply one command to `env` at `now_ms`.
pub fn apply(env: &Environment, command: &Command, now_ms: u64) -> Result<Applied, Refusal> {
    if let Some(id) = request_id(command) {
        if !valid_id(id) {
            return Err(Refusal::Invalid("Request IDs must be opaque identities."));
        }
        if let Some(entry) = env.requests.get(id) {
            return if entry.fingerprint == fingerprint(command) {
                Ok(Applied::Replayed(entry.effect.clone()))
            } else {
                Err(Refusal::RequestConflict(id.into()))
            };
        }
        if env.requests.len() >= MAX_REQUESTS {
            return Err(Refusal::Limit("The environment retains too many requests."));
        }
    }
    let mut next = env.clone();
    let outcome = match command {
        Command::UpdateRecipe {
            expected_draft_revision,
            recipe,
        } => update_recipe(&mut next, *expected_draft_revision, recipe, now_ms)?,
        Command::StartBuild {
            request_id,
            expected_draft_revision,
        } => start_build(&mut next, request_id, *expected_draft_revision, now_ms)?,
        Command::ObserveBuild {
            build_id,
            observation,
        } => observe_build(&mut next, build_id, observation, now_ms)?,
        Command::StartVerification {
            request_id,
            build_id,
            plan_digest,
        } => start_verification(&mut next, request_id, build_id, plan_digest, now_ms)?,
        Command::ObserveVerification {
            verification_id,
            observation,
        } => observe_verification(&mut next, verification_id, observation, now_ms)?,
        Command::SaveVersion { request_id, review } => {
            let version_id = save_version(&mut next, request_id, review, now_ms)?;
            changed(Effect::VersionSaved { version_id })
        }
        Command::Promote {
            request_id,
            expected_selection_revision,
            review,
        } => {
            live(&next)?;
            fence_selection(&next, *expected_selection_revision)?;
            let version_id = save_version(&mut next, request_id, review, now_ms)?;
            let (selection_revision, previous) = move_selection(
                &mut next,
                &version_id,
                SelectionKind::Promoted,
                request_id,
                now_ms,
            )?;
            changed(Effect::Promoted {
                version_id,
                selection_revision,
                previous,
            })
        }
        Command::Select {
            request_id,
            expected_selection_revision,
            version_id,
        } => select(
            &mut next,
            *expected_selection_revision,
            version_id,
            request_id,
            now_ms,
        )?,
        Command::Retire => {
            if env.retired_ms.is_some() {
                Done::Same(Effect::Retired)
            } else {
                next.retired_ms = Some(now_ms);
                changed(Effect::Retired)
            }
        }
    };
    Ok(match outcome {
        Done::Changed(effect) => {
            if let Some(id) = request_id(command) {
                next.requests.insert(
                    id.into(),
                    RequestEntry {
                        fingerprint: fingerprint(command),
                        effect: effect.clone(),
                    },
                );
            }
            next.revision += 1;
            Applied::Changed(Box::new(next), effect)
        }
        Done::Same(effect) => Applied::Replayed(effect),
    })
}

/// What an inner transition did to the working copy.
enum Done {
    Changed(Effect),
    Same(Effect),
}
fn changed(effect: Effect) -> Done {
    Done::Changed(effect)
}

fn request_id(command: &Command) -> Option<&str> {
    match command {
        Command::StartBuild { request_id, .. }
        | Command::StartVerification { request_id, .. }
        | Command::SaveVersion { request_id, .. }
        | Command::Promote { request_id, .. }
        | Command::Select { request_id, .. } => Some(request_id),
        _ => None,
    }
}
fn fingerprint(command: &Command) -> String {
    digest(&serde_json::to_vec(command).expect("command encodes"))
}
fn live(env: &Environment) -> Result<(), Refusal> {
    if env.retired_ms.is_some() {
        Err(Refusal::Retired)
    } else {
        Ok(())
    }
}
fn reason(text: &str) -> Result<String, Refusal> {
    if text.is_empty() || text.len() > MAX_REASON_BYTES {
        return Err(Refusal::Invalid("A reason needs 1 to 1024 bytes."));
    }
    Ok(text.into())
}

fn update_recipe(
    env: &mut Environment,
    expected: u64,
    recipe: &Recipe,
    now_ms: u64,
) -> Result<Done, Refusal> {
    live(env)?;
    recipe.validate().map_err(Refusal::Invalid)?;
    let digest = recipe.digest();
    let current = env.draft_revision;
    let draft = env.draft();
    // A lost reply to the update that produced the current draft replays.
    if (expected == current || expected + 1 == current) && draft.digest == digest {
        return Ok(Done::Same(Effect::RecipeRevised {
            revision: current,
            digest,
        }));
    }
    if expected != current {
        return Err(Refusal::StaleDraft { expected, current });
    }
    if env.recipes.len() >= MAX_RECIPE_REVISIONS {
        return Err(Refusal::Limit(
            "The environment retains too many recipe revisions.",
        ));
    }
    let revision = current + 1;
    let parent_digest = Some(draft.digest.clone());
    env.recipes.push(RecipeRevision {
        revision,
        digest: digest.clone(),
        parent_digest,
        recipe: recipe.clone(),
        created_ms: now_ms,
    });
    env.draft_revision = revision;
    Ok(changed(Effect::RecipeRevised { revision, digest }))
}

fn start_build(
    env: &mut Environment,
    request_id: &str,
    expected: u64,
    now_ms: u64,
) -> Result<Done, Refusal> {
    live(env)?;
    if expected != env.draft_revision {
        return Err(Refusal::StaleDraft {
            expected,
            current: env.draft_revision,
        });
    }
    // Never start another install while an earlier one may still be running.
    if let Some(b) = env.builds.iter().find(|b| b.unresolved.is_some()) {
        return Err(Refusal::Unresolved(b.id.clone()));
    }
    if env.builds.len() >= MAX_ATTEMPTS {
        return Err(Refusal::Limit("The environment retains too many builds."));
    }
    let draft = env.draft();
    let build_id = format!("build-{}", env.builds.len() + 1);
    let build = BuildAttempt {
        id: build_id.clone(),
        request_id: request_id.into(),
        recipe_revision: draft.revision,
        recipe_digest: draft.digest.clone(),
        source: env.source.clone(),
        run: None,
        state: BuildState::Requested,
        unresolved: None,
        image: None,
        history: vec![Step {
            state: BuildState::Requested,
            at_ms: now_ms,
            reason: None,
        }],
        created_ms: now_ms,
    };
    let recipe_revision = build.recipe_revision;
    env.builds.push(build);
    Ok(changed(Effect::BuildRequested {
        build_id,
        recipe_revision,
    }))
}

enum Next<S> {
    To(S, Option<String>),
    Unknown(String),
}

/// Shared lifecycle rule: forward only, terminal states are final, unknown
/// outcomes park the attempt until a definite observation reconciles it.
fn advance<S: Stage>(
    id: &str,
    state: &mut S,
    unresolved: &mut Option<Unresolved<S>>,
    history: &mut Vec<Step<S>>,
    next: Next<S>,
    now_ms: u64,
) -> Result<bool, Refusal> {
    let (to, why) = match next {
        Next::Unknown(why) => {
            if state.terminal() {
                return Err(Refusal::Terminal(id.into()));
            }
            if unresolved.is_some() {
                return Ok(false);
            }
            *unresolved = Some(Unresolved {
                reason: why.clone(),
                prior: *state,
                since_ms: now_ms,
            });
            (S::RECONCILE, Some(why))
        }
        Next::To(to, why) => {
            if *state == to {
                return Ok(false);
            }
            if state.terminal() {
                return Err(Refusal::Terminal(id.into()));
            }
            let floor = unresolved.as_ref().map_or(*state, |u| u.prior);
            if to == S::RECONCILE || to.rank() < floor.rank() {
                return Err(Refusal::Regression(id.into()));
            }
            *unresolved = None;
            (to, why)
        }
    };
    if history.len() >= MAX_HISTORY {
        return Err(Refusal::Limit("The attempt retains too many steps."));
    }
    *state = to;
    history.push(Step {
        state: to,
        at_ms: now_ms,
        reason: why,
    });
    Ok(true)
}

fn link(id: &str, slot: &mut Option<RunLink>, run: &RunLink) -> Result<bool, Refusal> {
    if !valid_id(&run.cloud_job) || run.task.as_deref().is_some_and(|t| !valid_id(t)) {
        return Err(Refusal::Invalid("Run links need opaque job and task IDs."));
    }
    match slot {
        Some(existing) if existing == run => Ok(false),
        Some(_) => Err(Refusal::RunConflict(id.into())),
        None => {
            *slot = Some(run.clone());
            Ok(true)
        }
    }
}

fn observe_build(
    env: &mut Environment,
    build_id: &str,
    observation: &BuildObservation,
    now_ms: u64,
) -> Result<Done, Refusal> {
    let b = env
        .builds
        .iter_mut()
        .find(|b| b.id == build_id)
        .ok_or_else(|| Refusal::UnknownBuild(build_id.into()))?;
    let next = match observation {
        BuildObservation::Linked { run } => {
            let changed_link = link(build_id, &mut b.run, run)?;
            let effect = Effect::BuildObserved {
                build_id: build_id.into(),
                state: b.state,
            };
            return Ok(if changed_link {
                changed(effect)
            } else {
                Done::Same(effect)
            });
        }
        BuildObservation::Progress { state } => {
            if state.terminal() || *state == BuildState::NeedsReconciliation {
                return Err(Refusal::Invalid(
                    "Progress names a running build state; use the outcome observations.",
                ));
            }
            Next::To(*state, None)
        }
        BuildObservation::ImageReady { image } => {
            image.validate().map_err(Refusal::Invalid)?;
            match &b.image {
                Some(existing) if existing != image => {
                    return Err(Refusal::ImageConflict(build_id.into()));
                }
                _ => {}
            }
            if !b.state.terminal() {
                b.image = Some(image.clone());
            }
            Next::To(BuildState::Ready, None)
        }
        BuildObservation::Failed { reason: r } => Next::To(BuildState::Failed, Some(reason(r)?)),
        BuildObservation::Cancelled => Next::To(BuildState::Cancelled, None),
        BuildObservation::Unknown { reason: r } => Next::Unknown(reason(r)?),
    };
    let did = advance(
        build_id,
        &mut b.state,
        &mut b.unresolved,
        &mut b.history,
        next,
        now_ms,
    )?;
    let effect = Effect::BuildObserved {
        build_id: build_id.into(),
        state: b.state,
    };
    Ok(if did {
        changed(effect)
    } else {
        Done::Same(effect)
    })
}

fn start_verification(
    env: &mut Environment,
    request_id: &str,
    build_id: &str,
    plan_digest: &str,
    now_ms: u64,
) -> Result<Done, Refusal> {
    live(env)?;
    let build = env
        .build(build_id)
        .ok_or_else(|| Refusal::UnknownBuild(build_id.into()))?;
    if build.unresolved.is_some() {
        return Err(Refusal::Unresolved(build_id.into()));
    }
    // A recipe edit stales every earlier build: verify the current one.
    if env.is_stale(build) {
        return Err(Refusal::StaleBuild {
            build_recipe: build.recipe_revision,
            draft: env.draft_revision,
        });
    }
    let image = match (build.state, &build.image) {
        (BuildState::Ready, Some(image)) => image.clone(),
        _ => return Err(Refusal::BuildNotReady(build_id.into())),
    };
    let recipe = env.recipe(build.recipe_revision).ok_or(Refusal::Invalid(
        "The build names a missing recipe revision.",
    ))?;
    if recipe.recipe.qualification.plan_digest != plan_digest {
        return Err(Refusal::PlanMismatch);
    }
    if let Some(v) = env
        .verifications
        .iter()
        .find(|v| v.build_id == build_id && v.unresolved.is_some())
    {
        return Err(Refusal::Unresolved(v.id.clone()));
    }
    if env.verifications.len() >= MAX_ATTEMPTS {
        return Err(Refusal::Limit(
            "The environment retains too many verifications.",
        ));
    }
    let verification_id = format!("verify-{}", env.verifications.len() + 1);
    env.verifications.push(VerificationAttempt {
        id: verification_id.clone(),
        request_id: request_id.into(),
        build_id: build_id.into(),
        image,
        plan_digest: plan_digest.into(),
        run: None,
        state: VerificationState::Requested,
        unresolved: None,
        evidence_digest: None,
        evidence_status: None,
        history: vec![Step {
            state: VerificationState::Requested,
            at_ms: now_ms,
            reason: None,
        }],
        created_ms: now_ms,
    });
    Ok(changed(Effect::VerificationRequested {
        verification_id,
        build_id: build_id.into(),
    }))
}

/// Retain the evidence a non-passing verdict cites, before it is final.
fn cite(
    v: &mut VerificationAttempt,
    evidence: Option<&crate::evidence::Sealed>,
) -> Result<(), Refusal> {
    let Some(sealed) = evidence else {
        return Ok(());
    };
    if !valid_digest(&sealed.digest) {
        return Err(Refusal::Invalid("The cited evidence digest is invalid."));
    }
    if !v.state.terminal() {
        v.evidence_digest = Some(sealed.digest.clone());
        v.evidence_status = Some(sealed.status);
    }
    Ok(())
}

fn observe_verification(
    env: &mut Environment,
    verification_id: &str,
    observation: &VerificationObservation,
    now_ms: u64,
) -> Result<Done, Refusal> {
    let v = env
        .verifications
        .iter_mut()
        .find(|v| v.id == verification_id)
        .ok_or_else(|| Refusal::UnknownVerification(verification_id.into()))?;
    let next = match observation {
        VerificationObservation::Linked { run } => {
            let changed_link = link(verification_id, &mut v.run, run)?;
            let effect = Effect::VerificationObserved {
                verification_id: verification_id.into(),
                state: v.state,
            };
            return Ok(if changed_link {
                changed(effect)
            } else {
                Done::Same(effect)
            });
        }
        VerificationObservation::Progress { state } => {
            if state.terminal() || *state == VerificationState::NeedsReconciliation {
                return Err(Refusal::Invalid(
                    "Progress names a running verification state; use the verdict observations.",
                ));
            }
            Next::To(*state, None)
        }
        VerificationObservation::Passed {
            evidence_digest,
            evidence,
        } => {
            if !valid_digest(evidence_digest) {
                return Err(Refusal::Invalid(
                    "A passed verdict needs its evidence digest.",
                ));
            }
            if !evidence.complete() {
                // Passing checks with incomplete evidence is an explicit
                // incomplete result: it never passes, so it never saves.
                if !v.state.terminal() {
                    v.evidence_digest = Some(evidence_digest.clone());
                    v.evidence_status = Some(*evidence);
                }
                Next::To(
                    VerificationState::Incomplete,
                    Some(
                        "The checks passed, but their evidence is incomplete; a version needs complete evidence."
                            .into(),
                    ),
                )
            } else {
                if v.state == VerificationState::Passed
                    && v.evidence_digest.as_deref() != Some(evidence_digest)
                {
                    return Err(Refusal::Terminal(verification_id.into()));
                }
                if !v.state.terminal() {
                    v.evidence_digest = Some(evidence_digest.clone());
                    v.evidence_status = Some(*evidence);
                }
                Next::To(VerificationState::Passed, None)
            }
        }
        VerificationObservation::Failed {
            reason: r,
            evidence,
        } => {
            cite(v, evidence.as_ref())?;
            Next::To(VerificationState::Failed, Some(reason(r)?))
        }
        VerificationObservation::Incomplete {
            reason: r,
            evidence,
        } => {
            cite(v, evidence.as_ref())?;
            Next::To(VerificationState::Incomplete, Some(reason(r)?))
        }
        VerificationObservation::Cancelled => Next::To(VerificationState::Cancelled, None),
        VerificationObservation::Unknown { reason: r } => Next::Unknown(reason(r)?),
    };
    let did = advance(
        verification_id,
        &mut v.state,
        &mut v.unresolved,
        &mut v.history,
        next,
        now_ms,
    )?;
    let effect = Effect::VerificationObserved {
        verification_id: verification_id.into(),
        state: v.state,
    };
    Ok(if did {
        changed(effect)
    } else {
        Done::Same(effect)
    })
}

fn save_version(
    env: &mut Environment,
    request_id: &str,
    review: &Review,
    now_ms: u64,
) -> Result<String, Refusal> {
    live(env)?;
    review.validate().map_err(Refusal::Invalid)?;
    if now_ms > review.expires_ms {
        return Err(Refusal::ReviewExpired(review.id.clone()));
    }
    if review.candidate.environment != env.id {
        return Err(Refusal::StaleReview("environment"));
    }
    if let Some(v) = env
        .versions
        .iter()
        .find(|v| v.review.as_ref().is_some_and(|r| r.id == review.id))
    {
        return Err(Refusal::ReviewUsed {
            review: review.id.clone(),
            version: v.id.clone(),
        });
    }
    if review.candidate.recipe_revision != env.draft_revision {
        return Err(Refusal::StaleDraft {
            expected: review.candidate.recipe_revision,
            current: env.draft_revision,
        });
    }
    let current = env.propose(&review.candidate.verification_id)?;
    if let Some(field) = review.candidate.changed_field(&current) {
        return Err(Refusal::StaleReview(field));
    }
    if env.versions.len() >= MAX_VERSIONS {
        return Err(Refusal::Limit("The environment retains too many versions."));
    }
    let number = env.versions.len() as u64 + 1;
    let version = EnvironmentVersion {
        id: version_id(number),
        number,
        request_id: request_id.into(),
        parent: env.versions.last().map(|p| p.id.clone()),
        recipe_revision: current.recipe_revision,
        recipe_digest: current.recipe_digest,
        source: current.source,
        base: current.base,
        runtime: current.runtime,
        image: current.image,
        build_id: current.build_id,
        build_run: current.build_run,
        verification_id: current.verification_id,
        verification_run: current.verification_run,
        plan_digest: current.plan_digest,
        evidence_digest: current.evidence_digest,
        review: Some(review.stamp()),
        created_ms: now_ms,
    };
    let id = version.id.clone();
    env.versions.push(version);
    Ok(id)
}

fn fence_selection(env: &Environment, expected: u64) -> Result<(), Refusal> {
    if expected != env.selection.revision {
        return Err(Refusal::StaleSelection {
            expected,
            current: env.selection.revision,
        });
    }
    Ok(())
}

fn move_selection(
    env: &mut Environment,
    version_id: &str,
    kind: SelectionKind,
    request_id: &str,
    now_ms: u64,
) -> Result<(u64, Option<String>), Refusal> {
    if env.selections.len() >= crate::promotion::MAX_SELECTIONS {
        return Err(Refusal::Limit(
            "The environment retains too many selection changes.",
        ));
    }
    let previous = env.selection.active.replace(version_id.into());
    env.selection.revision += 1;
    env.selections.push(SelectionChange {
        revision: env.selection.revision,
        kind,
        version_id: version_id.into(),
        previous: previous.clone(),
        request_id: request_id.into(),
        at_ms: now_ms,
    });
    Ok((env.selection.revision, previous))
}

fn select(
    env: &mut Environment,
    expected: u64,
    version_id: &str,
    request_id: &str,
    now_ms: u64,
) -> Result<Done, Refusal> {
    live(env)?;
    fence_selection(env, expected)?;
    let target = env
        .version(version_id)
        .ok_or_else(|| Refusal::UnknownVersion(version_id.into()))?
        .number;
    let kind = match env.active() {
        Some(active) if target < active.number => SelectionKind::RolledBack,
        _ => SelectionKind::Selected,
    };
    let (selection_revision, previous) = move_selection(env, version_id, kind, request_id, now_ms)?;
    Ok(changed(Effect::Selected {
        version_id: version_id.into(),
        selection_revision,
        previous,
        change: kind,
    }))
}
