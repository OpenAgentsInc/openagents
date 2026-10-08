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
    /// Seal a passed verification into an immutable version.
    SaveVersion {
        request_id: String,
        verification_id: String,
        expected_draft_revision: u64,
    },
    /// Select a saved version for later project work.
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
    Failed {
        reason: String,
    },
    Incomplete {
        reason: String,
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
    Selected {
        version_id: String,
        selection_revision: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        previous: Option<String>,
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
        Command::SaveVersion {
            request_id,
            verification_id,
            expected_draft_revision,
        } => save_version(
            &mut next,
            request_id,
            verification_id,
            *expected_draft_revision,
            now_ms,
        )?,
        Command::Select {
            request_id: _,
            expected_selection_revision,
            version_id,
        } => select(&mut next, *expected_selection_revision, version_id)?,
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
                }
                Next::To(VerificationState::Passed, None)
            }
        }
        VerificationObservation::Failed { reason: r } => {
            Next::To(VerificationState::Failed, Some(reason(r)?))
        }
        VerificationObservation::Incomplete { reason: r } => {
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
    verification_id: &str,
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
    let v = env
        .verification(verification_id)
        .ok_or_else(|| Refusal::UnknownVerification(verification_id.into()))?;
    if let Some(saved) = env
        .versions
        .iter()
        .find(|s| s.verification_id == verification_id)
    {
        return Err(Refusal::AlreadySaved(saved.id.clone()));
    }
    let (VerificationState::Passed, Some(evidence_digest)) = (v.state, &v.evidence_digest) else {
        return Err(Refusal::NotPassed(verification_id.into()));
    };
    let verification_run = v
        .run
        .clone()
        .ok_or_else(|| Refusal::NotLinked(verification_id.into()))?;
    let b = env
        .build(&v.build_id)
        .ok_or_else(|| Refusal::UnknownBuild(v.build_id.clone()))?;
    let build_run = b
        .run
        .clone()
        .ok_or_else(|| Refusal::NotLinked(b.id.clone()))?;
    if b.recipe_revision != env.draft_revision {
        return Err(Refusal::StaleBuild {
            build_recipe: b.recipe_revision,
            draft: env.draft_revision,
        });
    }
    if b.image.as_ref() != Some(&v.image) {
        return Err(Refusal::ImageConflict(b.id.clone()));
    }
    if env.versions.len() >= MAX_VERSIONS {
        return Err(Refusal::Limit("The environment retains too many versions."));
    }
    let recipe = &env
        .recipe(b.recipe_revision)
        .ok_or(Refusal::Invalid(
            "The build names a missing recipe revision.",
        ))?
        .recipe;
    let number = env.versions.len() as u64 + 1;
    let version = EnvironmentVersion {
        id: version_id(number),
        number,
        request_id: request_id.into(),
        parent: env.versions.last().map(|p| p.id.clone()),
        recipe_revision: b.recipe_revision,
        recipe_digest: b.recipe_digest.clone(),
        source: b.source.clone(),
        base: recipe.base.clone(),
        runtime: recipe.runtime.clone(),
        image: v.image.clone(),
        build_id: b.id.clone(),
        build_run,
        verification_id: v.id.clone(),
        verification_run,
        plan_digest: v.plan_digest.clone(),
        evidence_digest: evidence_digest.clone(),
        created_ms: now_ms,
    };
    let version_id = version.id.clone();
    env.versions.push(version);
    Ok(changed(Effect::VersionSaved { version_id }))
}

fn select(env: &mut Environment, expected: u64, version_id: &str) -> Result<Done, Refusal> {
    live(env)?;
    if expected != env.selection.revision {
        return Err(Refusal::StaleSelection {
            expected,
            current: env.selection.revision,
        });
    }
    if env.version(version_id).is_none() {
        return Err(Refusal::UnknownVersion(version_id.into()));
    }
    let previous = env.selection.active.replace(version_id.into());
    env.selection.revision += 1;
    Ok(changed(Effect::Selected {
        version_id: version_id.into(),
        selection_revision: env.selection.revision,
        previous,
    }))
}
