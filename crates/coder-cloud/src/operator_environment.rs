//! Project environment reads and reviewed effects (ENV-07).
//!
//! The operator answers the browser's environment panel from the same
//! private state it admits jobs from: `<state>/environments` (the ENV-01
//! records) and the verifier's retained evidence under
//! `<state>/environment-verify/evidence/<verify job>` ([`VERIFY_EVIDENCE`]).
//! Every read is admitted by the operator policy for that exact project,
//! opens records read-only, and creates no file, lock, or directory.
//!
//! Promote recomputes the candidate the reviewer saw and refuses unless its
//! digest is the displayed one; Select is the reviewed rollback. Both go
//! through the environment store's lease, selection fence, and request
//! replay, so a repeated original request returns its first result. Setup
//! steering is answered only when a setup owner is composed
//! ([`Operator::with_setup`]); otherwise the panel shows setup as
//! unavailable.

use super::*;
use coder_access::environment as view;
use coder_environment as env;
use coder_environment::evidence::read::{EvidenceReader, StreamCursor, StreamWindow};
use coder_environment::evidence::{EvidenceStatus, StreamName};

/// Where a composed verifier keeps its run evidence, under operator state.
pub const VERIFY_EVIDENCE: &str = "environment-verify/evidence";
/// The evidence ID of a verifier run's top-level record.
const VERIFY_RECORD: &str = "verify";
/// How long the review a browser Promote grants stays usable.
const REVIEW_MS: u64 = 10 * 60 * 1000;

/// A setup-session owner composed with the operator (ENV-03).
pub trait SetupSessions: Send + Sync {
    /// Sessions of one environment, newest first, at most
    /// [`view::MAX_ROWS`]. No side effects.
    fn sessions(&self, environment: &str) -> Result<Vec<view::Setup>>;
    /// Retain `text` as steering for `session` and wake it when it awaits
    /// input. Returns the session state after retention.
    fn steer(&self, environment: &str, session: &str, text: &str, now_ms: u64) -> Result<String>;
}

fn name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "unknown".into())
}
fn image(i: &env::ImageIdentity) -> view::Image {
    view::Image {
        image_id: i.image_id.clone(),
        snapshot_id: i.snapshot_id.clone(),
        manifest_digest: i.manifest_digest.clone(),
    }
}
fn steps<S: Serialize + Copy>(history: &[env::Step<S>]) -> Vec<view::Step> {
    let skip = history.len().saturating_sub(view::MAX_STEPS);
    history[skip..]
        .iter()
        .map(|s| view::Step {
            state: name(&s.state),
            at_ms: s.at_ms,
            reason: s.reason.clone(),
        })
        .collect()
}
fn last<T>(rows: &[T]) -> &[T] {
    &rows[rows.len().saturating_sub(view::MAX_ROWS)..]
}
/// The job view's copy of the version a job started with.
pub(super) fn pin(p: &env::VersionPin) -> view::Pin {
    view::Pin {
        environment: p.environment.clone(),
        version_id: p.version_id.clone(),
        number: p.number,
        selection_revision: p.selection_revision,
        recipe_revision: p.recipe_revision,
        source_revision: p.source.revision.clone(),
        image: image(&p.image),
        evidence_digest: p.evidence_digest.clone(),
    }
}
/// An identity the environment store admits, derived from one that may
/// use other characters.
fn opaque(value: &str) -> String {
    if env::valid_id(value) {
        value.into()
    } else {
        env::digest(value.as_bytes())
    }
}
fn store_error(e: env::store::StoreError) -> Code {
    use env::Refusal as R;
    use env::store::StoreError as E;
    match e {
        E::NotFound => Code::Forbidden,
        E::InvalidId => Code::Malformed,
        E::Busy => Code::Unavailable,
        E::Fence { .. } | E::Immutable | E::Ambiguous | E::Exists => Code::Conflict,
        E::Corrupt(_) | E::Io(_) => Code::Unavailable,
        E::Refused(r) => match r {
            R::StaleReview(_)
            | R::StaleDraft { .. }
            | R::StaleSelection { .. }
            | R::StaleBuild { .. }
            | R::ReviewExpired(_) => Code::Stale,
            R::UnknownVerification(_) | R::UnknownVersion(_) => Code::Forbidden,
            _ => Code::Conflict,
        },
    }
}

impl Operator {
    /// Compose a setup-session owner for the panel's setup chat.
    pub fn with_setup(self, setup: Arc<dyn SetupSessions>) -> std::result::Result<Self, String> {
        self.0
            .setup
            .set(setup)
            .map_err(|_| "A setup owner is already composed.".to_string())?;
        Ok(self)
    }
    fn environment_admitted(&self, device: &str, workspace: &str, project: &str) -> Result<()> {
        let policy = self.0.policy.read()?;
        if !policy
            .operators
            .iter()
            .any(|a| a.device == device && a.workspace == workspace && a.project == project)
        {
            return Err(Code::Forbidden);
        }
        Ok(())
    }
    fn environment_store(&self) -> Result<Option<env::store::Store>> {
        let root = self.0.root.join("environments");
        if fs::symlink_metadata(&root).is_err() {
            return Ok(None);
        }
        private(&root, true)?;
        Ok(Some(env::store::Store::under(root)))
    }
    fn environment_record(
        &self,
        workspace: &str,
        project: &str,
        id: &str,
    ) -> Result<(env::store::Store, env::Environment)> {
        let store = self.environment_store()?.ok_or(Code::Forbidden)?;
        let record = store.read(id).map_err(store_error)?;
        if record.project.workspace != workspace || record.project.project != project {
            return Err(Code::Forbidden);
        }
        Ok((store, record))
    }

    pub(super) fn environment_read(&self, device: &str, q: &view::Query) -> Result<view::View> {
        self.environment_admitted(device, &q.workspace, &q.project)?;
        let records = match self.environment_store()? {
            Some(store) => store.list().map_err(store_error)?,
            None => vec![],
        };
        let mine: Vec<_> = records
            .into_iter()
            .filter(|e| e.project.workspace == q.workspace && e.project.project == q.project)
            .collect();
        let environments = last(&mine)
            .iter()
            .rev()
            .map(|e| view::Listed {
                id: e.id.clone(),
                retired: e.retired_ms.is_some(),
                active: e.selection.active.clone(),
            })
            .collect::<Vec<_>>();
        let chosen = match &q.environment {
            Some(id) => Some(
                mine.iter()
                    .find(|e| e.id == *id)
                    .filter(|e| environments.iter().any(|l| l.id == e.id))
                    .ok_or(Code::Forbidden)?,
            ),
            None => mine
                .iter()
                .rev()
                .find(|e| e.retired_ms.is_none())
                .or(mine.last()),
        };
        let detail = chosen
            .map(|e| self.environment_detail(e, q.before))
            .transpose()?
            .map(Box::new);
        Ok(view::View {
            workspace: q.workspace.clone(),
            project: q.project.clone(),
            environments,
            detail,
        })
    }

    fn environment_detail(
        &self,
        e: &env::Environment,
        before: Option<u64>,
    ) -> Result<view::Detail> {
        let builds = last(&e.builds)
            .iter()
            .rev()
            .map(|b| view::Build {
                id: b.id.clone(),
                recipe_revision: b.recipe_revision,
                recipe_digest: b.recipe_digest.clone(),
                state: name(&b.state),
                stale: e.is_stale(b),
                unresolved: b.unresolved.as_ref().map(|u| u.reason.clone()),
                job: b.run.as_ref().map(|r| r.cloud_job.clone()),
                image: b.image.as_ref().map(image),
                steps: steps(&b.history),
                created_ms: b.created_ms,
            })
            .collect();
        let verifications = last(&e.verifications)
            .iter()
            .rev()
            .map(|v| {
                let candidate = e.propose(&v.id).ok().map(|c| view::Candidate {
                    digest: c.digest(),
                    recipe_revision: c.recipe_revision,
                    recipe_digest: c.recipe_digest.clone(),
                    source_revision: c.source.revision.clone(),
                    image: image(&c.image),
                    plan_digest: c.plan_digest.clone(),
                    evidence_digest: c.evidence_digest.clone(),
                });
                view::Verification {
                    id: v.id.clone(),
                    build_id: v.build_id.clone(),
                    state: name(&v.state),
                    unresolved: v.unresolved.as_ref().map(|u| u.reason.clone()),
                    job: v.run.as_ref().map(|r| r.cloud_job.clone()),
                    evidence_status: v.evidence_status.as_ref().map(name),
                    evidence_digest: v.evidence_digest.clone(),
                    evidence_readable: self.verify_evidence_dir(v).is_some(),
                    candidate,
                    steps: steps(&v.history),
                    created_ms: v.created_ms,
                }
            })
            .collect();
        let history = e.history(before, view::MAX_ROWS + 1);
        let history_before = (history.len() > view::MAX_ROWS)
            .then(|| history.get(view::MAX_ROWS - 1).map(|h| h.number))
            .flatten();
        let history = history
            .into_iter()
            .take(view::MAX_ROWS)
            .map(|h| view::Version {
                id: h.version_id,
                number: h.number,
                created_ms: h.created_ms,
                recipe_revision: h.recipe_revision,
                source_revision: h.source_revision,
                image: image(&h.image),
                evidence_digest: h.evidence_digest,
                selected: h.selected,
                reviewer: h.reviewer,
                selected_at: last(&h.selected_at).to_vec(),
            })
            .collect();
        let changes = last(&e.selections)
            .iter()
            .rev()
            .map(|c| view::Change {
                revision: c.revision,
                kind: name(&c.kind),
                version_id: c.version_id.clone(),
                previous: c.previous.clone(),
                at_ms: c.at_ms,
            })
            .collect();
        let setup = match self.0.setup.get() {
            Some(owner) => {
                let mut rows = owner.sessions(&e.id)?;
                rows.truncate(view::MAX_ROWS);
                Some(rows)
            }
            None => None,
        };
        let draft = e.draft();
        Ok(view::Detail {
            id: e.id.clone(),
            revision: e.revision,
            retired: e.retired_ms.is_some(),
            source: view::Source {
                repository: e.source.repository.clone(),
                revision: e.source.revision.clone(),
                digest: e.source.digest.clone(),
            },
            draft_revision: draft.revision,
            draft_digest: draft.digest.clone(),
            recipes: last(&e.recipes)
                .iter()
                .rev()
                .map(|r| view::Recipe {
                    revision: r.revision,
                    digest: r.digest.clone(),
                    created_ms: r.created_ms,
                })
                .collect(),
            builds,
            verifications,
            selection_revision: e.selection.revision,
            active: e.selection.active.clone(),
            history,
            history_before,
            changes,
            setup,
        })
    }

    /// The retained run evidence of one verification, when present.
    fn verify_evidence_dir(&self, v: &env::VerificationAttempt) -> Option<PathBuf> {
        let job = v.run.as_ref()?.task.as_deref()?;
        if !env::valid_id(job) {
            return None;
        }
        let dir = self.0.root.join(VERIFY_EVIDENCE).join(job);
        fs::symlink_metadata(dir.join("events.jsonl"))
            .is_ok_and(|m| m.is_file())
            .then_some(dir)
    }

    pub(super) fn environment_evidence(
        &self,
        device: &str,
        q: &view::EvidenceQuery,
    ) -> Result<view::EvidencePage> {
        self.environment_admitted(device, &q.workspace, &q.project)?;
        let (_, record) = self.environment_record(&q.workspace, &q.project, &q.environment)?;
        let attempt = record
            .verification(&q.verification)
            .ok_or(Code::Forbidden)?;
        let dir = self.verify_evidence_dir(attempt).ok_or(Code::Unavailable)?;
        let top = EvidenceReader::open(dir, VERIFY_RECORD).map_err(|_| Code::Unavailable)?;
        let reader = match &q.child {
            Some(child) => top.child(child).map_err(|_| Code::Stale)?,
            None => top,
        };
        let summary = reader.summary();
        let mut gaps: Vec<String> = summary
            .gaps
            .iter()
            .map(|g| {
                let mut s = serde_json::to_string(g).unwrap_or_else(|_| "unknown gap".into());
                s.truncate(512);
                s
            })
            .collect();
        let sealed_digest = summary.sealed.as_ref().map(|s| s.digest.clone());
        if q.child.is_none()
            && attempt.evidence_digest.is_some()
            && attempt.evidence_digest != sealed_digest
        {
            gaps.insert(
                0,
                "The retained record does not match the digest the attempt cites.".into(),
            );
        }
        let chunk = match (&q.call, q.stream) {
            (Some(call), Some(stream)) => {
                let stream_name = match stream {
                    view::Stream::Stdout => StreamName::Stdout,
                    view::Stream::Stderr => StreamName::Stderr,
                };
                let window = match &q.cursor {
                    None => StreamWindow::Oldest,
                    Some(c) => {
                        if c.evidence_id != reader.id() {
                            return Err(Code::Stale);
                        }
                        StreamWindow::After(StreamCursor {
                            evidence_id: c.evidence_id.clone(),
                            call: c.call.clone(),
                            stream: stream_name,
                            offset: c.offset,
                            chunk_seq: c.chunk_seq,
                            chunk_digest: c.chunk_digest.clone(),
                        })
                    }
                };
                let page = reader
                    .stream(call, stream_name, window, u64::from(q.limit))
                    .map_err(|e| match e {
                        env::evidence::EvidenceError::Cursor(_) => Code::Stale,
                        env::evidence::EvidenceError::Invalid(_)
                        | env::evidence::EvidenceError::UnknownCall(_) => Code::Forbidden,
                        _ => Code::Unavailable,
                    })?;
                if page.bytes.len() > q.limit as usize {
                    return Err(Code::Bounds);
                }
                for g in &page.gaps {
                    let mut s = serde_json::to_string(g).unwrap_or_else(|_| "unknown gap".into());
                    s.truncate(512);
                    if !gaps.contains(&s) {
                        gaps.push(s);
                    }
                }
                use base64::Engine;
                Some(view::Chunk {
                    call: call.clone(),
                    stream,
                    start: page.start,
                    data: base64::engine::general_purpose::STANDARD.encode(&page.bytes),
                    digest: page.digest.clone(),
                    length: page.length,
                    closed: page.state.is_some(),
                    next: page.more_newer.then(|| view::EvidenceCursor {
                        evidence_id: page.newer.evidence_id.clone(),
                        call: page.newer.call.clone(),
                        stream,
                        offset: page.newer.offset,
                        chunk_seq: page.newer.chunk_seq,
                        chunk_digest: page.newer.chunk_digest.clone(),
                    }),
                })
            }
            _ => None,
        };
        let calls_omitted = summary.calls.len() > view::MAX_CALLS;
        let gaps_omitted = gaps.len() > view::MAX_GAPS;
        gaps.truncate(view::MAX_GAPS);
        Ok(view::EvidencePage {
            workspace: q.workspace.clone(),
            project: q.project.clone(),
            environment: q.environment.clone(),
            verification: q.verification.clone(),
            child: q.child.clone(),
            evidence_id: reader.id().into(),
            status: name(&summary.status),
            complete: summary.complete
                && matches!(
                    summary.status,
                    EvidenceStatus::Complete | EvidenceStatus::CompleteWithRedactions
                ),
            sealed_digest,
            head_seq: summary.head.seq,
            calls: summary
                .calls
                .iter()
                .take(view::MAX_CALLS)
                .map(|c| view::Call {
                    call: c.call.clone(),
                    tool: c.tool.chars().take(128).collect(),
                    outcome: name(&c.outcome),
                    stdout_bytes: c.stdout.length,
                    stderr_bytes: c.stderr.length,
                })
                .collect(),
            calls_omitted,
            children: summary
                .children
                .iter()
                .take(view::MAX_CALLS)
                .map(|c| c.evidence_id.clone())
                .collect(),
            gaps,
            gaps_omitted,
            chunk,
        })
    }

    pub(super) fn environment_effect(
        &self,
        request: &str,
        principal: &Principal,
        op: &Operation,
    ) -> Result<view::Accepted> {
        dto::alias(request).map_err(|e| e.code)?;
        let (workspace, project, environment) = match op {
            Operation::EnvironmentPromote { intent } => {
                (&intent.workspace, &intent.project, &intent.environment)
            }
            Operation::EnvironmentSelect { intent } => {
                (&intent.workspace, &intent.project, &intent.environment)
            }
            Operation::EnvironmentSteer { intent } => {
                (&intent.workspace, &intent.project, &intent.environment)
            }
            _ => return Err(Code::Unsupported),
        };
        self.environment_admitted(&principal.device, workspace, project)?;
        let (store, record) = self.environment_record(workspace, project, environment)?;
        let now = crate::now_ms();
        let request_id = opaque(request);
        let accepted = |action: &str, state: String, version, selection_revision| view::Accepted {
            request: request.into(),
            workspace: workspace.clone(),
            project: project.clone(),
            environment: environment.clone(),
            action: action.into(),
            state,
            version,
            selection_revision,
        };
        let effect = |applied: env::Applied| match applied {
            env::Applied::Changed(_, effect) | env::Applied::Replayed(effect) => effect,
        };
        // A repeated original request returns its first result. The review a
        // Promote grants is stamped when first applied, so the replay reads
        // the retained effect rather than re-granting it.
        if let Some(entry) = record.requests.get(&request_id) {
            return match (&entry.effect, op) {
                (
                    env::Effect::Promoted {
                        version_id,
                        selection_revision,
                        ..
                    },
                    Operation::EnvironmentPromote { intent },
                ) if record
                    .version(version_id)
                    .is_some_and(|v| v.verification_id == intent.verification) =>
                {
                    Ok(accepted(
                        "promote",
                        "promoted".into(),
                        Some(version_id.clone()),
                        Some(*selection_revision),
                    ))
                }
                (
                    env::Effect::Selected {
                        version_id,
                        selection_revision,
                        change,
                        ..
                    },
                    Operation::EnvironmentSelect { intent },
                ) if *version_id == intent.version => Ok(accepted(
                    "select",
                    name(change),
                    Some(version_id.clone()),
                    Some(*selection_revision),
                )),
                _ => Err(Code::Conflict),
            };
        }
        match op {
            Operation::EnvironmentPromote { intent } => {
                let candidate = record
                    .propose(&intent.verification)
                    .map_err(|r| store_error(env::store::StoreError::Refused(r)))?;
                if candidate.digest() != intent.candidate_digest {
                    return Err(Code::Stale);
                }
                let review = env::Review {
                    id: env::digest(format!("review:{request}").as_bytes()),
                    actor: opaque(&principal.device),
                    candidate,
                    granted_ms: now,
                    expires_ms: now + REVIEW_MS,
                };
                let applied = store
                    .apply(
                        environment,
                        &env::Command::Promote {
                            request_id,
                            expected_selection_revision: intent.expected_selection_revision,
                            review,
                        },
                        now,
                    )
                    .map_err(store_error)?;
                match effect(applied) {
                    env::Effect::Promoted {
                        version_id,
                        selection_revision,
                        ..
                    } => Ok(accepted(
                        "promote",
                        "promoted".into(),
                        Some(version_id),
                        Some(selection_revision),
                    )),
                    _ => Err(Code::Conflict),
                }
            }
            Operation::EnvironmentSelect { intent } => {
                let applied = store
                    .apply(
                        environment,
                        &env::Command::Select {
                            request_id,
                            expected_selection_revision: intent.expected_selection_revision,
                            version_id: intent.version.clone(),
                        },
                        now,
                    )
                    .map_err(store_error)?;
                match effect(applied) {
                    env::Effect::Selected {
                        version_id,
                        selection_revision,
                        change,
                        ..
                    } => Ok(accepted(
                        "select",
                        name(&change),
                        Some(version_id),
                        Some(selection_revision),
                    )),
                    _ => Err(Code::Conflict),
                }
            }
            Operation::EnvironmentSteer { intent } => {
                let owner = self.0.setup.get().ok_or(Code::Unavailable)?;
                let state = owner.steer(environment, &intent.session, &intent.text, now)?;
                Ok(accepted("steer", state, None, None))
            }
            _ => Err(Code::Unsupported),
        }
    }
}
