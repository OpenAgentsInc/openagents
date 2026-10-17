//! Job effects are fenced by the store lease; worker slots and result retries
//! survive service restarts in the same durable backend.
use crate::{
    Decision, EffectiveRequirements,
    authority::{Assignment, Authority, Worker},
    protocol::{Attempt, Finish, Work},
    select,
};
use base64::Engine;
use lab_kit::{ArtifactRole, JobState, JobStatus, KindManifest, digest};
use lab_store::{ArtifactInput, Error, LabStore, Result, Store};
use std::collections::BTreeMap;

pub struct Coordinator {
    pub authority: Authority,
}
impl Coordinator {
    fn scope(worker: &Worker, store: &Store) -> Result<()> {
        if store.namespace() != digest(&serde_json::to_value(&worker.space)?) {
            return Err(Error::NotFound);
        }
        Ok(())
    }
    pub async fn poll(
        &self,
        worker: &Worker,
        store: &Store,
        kinds: &BTreeMap<String, KindManifest>,
        now: u64,
    ) -> Result<Option<Work>> {
        Self::scope(worker, store)?;
        self.authority.observe(worker, now).await?;
        self.observe_losses(&worker.space, store, now).await?;
        if let Some(a) = self.authority.assignment(worker, now).await? {
            return self.work(worker, store, kinds, a, true).await.map(Some);
        }
        let available = self.authority.workers(&worker.space, now).await?;
        let mut after = 0;
        // A bounded poll never scans an entire account in one request.
        for _ in 0..10 {
            let page = store.queued_jobs(after, 100).await?;
            for job in page.items {
                let Some(kind) = kinds.get(&job.spec.kind) else {
                    continue;
                };
                let version = store
                    .version(&job.spec.version.document, job.spec.version.version)
                    .await?;
                let requirements = EffectiveRequirements::for_job(&job.spec, &version, kind)?;
                match select(&requirements, &available) {
                    Decision::Assigned { worker: id, .. } if id == worker.id => {
                        self.authority.reserve(worker, &job.id, now).await?;
                        let lease = match store.claim(&job.id, &worker.id, now, 30_000).await {
                            Ok(lease) => lease,
                            Err(e) => {
                                self.authority.release(worker, &job.id).await?;
                                if matches!(e, Error::Conflict) {
                                    continue;
                                }
                                return Err(e);
                            }
                        };
                        let assignment = self.authority.assign(worker, lease, now).await?;
                        return Ok(Some(Work {
                            assignment,
                            version,
                            requirements,
                        }));
                    }
                    Decision::Queued { message } => {
                        store.wait_for_worker(&job.id, &message, now).await?
                    }
                    _ => (),
                }
            }
            match page.next {
                Some(next) => after = next,
                None => break,
            }
        }
        Ok(None)
    }

    pub async fn observe_losses(
        &self,
        space: &lab_kit::SpaceRef,
        store: &Store,
        now: u64,
    ) -> Result<()> {
        if store.namespace() != digest(&serde_json::to_value(space)?) {
            return Err(Error::NotFound);
        }
        store.expire_leases(now).await?;
        for (worker, assignment) in self.authority.unsettled(space).await? {
            let job = store.job(&assignment.lease.job.id).await?;
            if assignment.lease.expires_unix_ms <= now || job.status != JobStatus::Running {
                // Finished results awaiting an acknowledgement still recover
                // through finish; this observation never changes the result.
                self.authority
                    .observe_loss(&worker, &assignment, job.status, now)
                    .await?;
            }
        }
        Ok(())
    }

    async fn work(
        &self,
        worker: &Worker,
        store: &Store,
        kinds: &BTreeMap<String, KindManifest>,
        assignment: Assignment,
        require_running: bool,
    ) -> Result<Work> {
        Self::scope(worker, store)?;
        let job = store.job(&assignment.lease.job.id).await?;
        let kind = kinds.get(&job.spec.kind).ok_or(Error::LeaseLost)?;
        let version = store
            .version(&job.spec.version.document, job.spec.version.version)
            .await?;
        let requirements = EffectiveRequirements::for_job(&job.spec, &version, kind)?;
        if job.spec != assignment.lease.job.spec
            || (require_running && job.status != JobStatus::Running)
            || (job.status == JobStatus::Running && !requirements.eligible(&worker.capabilities))
        {
            return Err(Error::LeaseLost);
        }
        Ok(Work {
            assignment,
            version,
            requirements,
        })
    }
    async fn proof(
        &self,
        worker: &Worker,
        store: &Store,
        kinds: &BTreeMap<String, KindManifest>,
        attempt: &Attempt,
        now: u64,
    ) -> Result<Work> {
        self.authority.observe(worker, now).await?;
        let a = self
            .authority
            .assignment(worker, now)
            .await?
            .filter(|a| a.lease.job.id == attempt.job && a.lease.attempt == attempt.attempt)
            .ok_or(Error::LeaseLost)?;
        self.work(worker, store, kinds, a, true).await
    }
    pub async fn start(
        &self,
        worker: &Worker,
        store: &Store,
        kinds: &BTreeMap<String, KindManifest>,
        attempt: &Attempt,
        now: u64,
    ) -> Result<Work> {
        let mut work = self.proof(worker, store, kinds, attempt, now).await?;
        let lease = store.start(&work.assignment.lease, now).await?;
        work.assignment = self.authority.started(worker, lease, now).await?;
        Ok(work)
    }
    pub async fn heartbeat(
        &self,
        worker: &Worker,
        store: &Store,
        kinds: &BTreeMap<String, KindManifest>,
        attempt: &Attempt,
        now: u64,
    ) -> Result<Work> {
        let mut work = self.proof(worker, store, kinds, attempt, now).await?;
        let lease = store.heartbeat(&work.assignment.lease, now, 30_000).await?;
        work.assignment = self.authority.assign(worker, lease, now).await?;
        Ok(work)
    }
    pub async fn finish(
        &self,
        worker: &Worker,
        store: &Store,
        kinds: &BTreeMap<String, KindManifest>,
        request: &Finish,
        now: u64,
    ) -> Result<JobState> {
        Self::scope(worker, store)?;
        self.authority.observe(worker, now).await?;
        let fingerprint = digest(&serde_json::to_value(request)?);
        if let Some(completed) = self
            .authority
            .completion(worker, &request.job, request.attempt)
            .await?
        {
            return if completed.fingerprint == fingerprint {
                Ok(completed.result)
            } else {
                Err(Error::Conflict)
            };
        }
        let assignment = self
            .authority
            .attempt(worker, &request.job, request.attempt)
            .await?
            .ok_or(Error::LeaseLost)?;
        let work = self.work(worker, store, kinds, assignment, false).await?;
        if !work.assignment.lease.started
            || !matches!(
                request.status,
                JobStatus::Done | JobStatus::Failed | JobStatus::Uncertain
            )
            || request.outputs.len() > 16
            || (request.status != JobStatus::Done && !request.outputs.is_empty())
        {
            return Err(Error::Invalid(
                "Finish started work with its completed result.".into(),
            ));
        }
        let mut artifacts = Vec::new();
        let mut total = 0usize;
        for output in &request.outputs {
            if work.requirements.private_local && output.role == ArtifactRole::Source {
                return Err(Error::Invalid(
                    "Keep private source files on their computer.".into(),
                ));
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&output.content_base64)
                .map_err(|_| Error::Invalid("Choose a valid output file.".into()))?;
            total = total.saturating_add(bytes.len());
            if total > 8 * 1024 * 1024 {
                return Err(Error::Invalid("Choose smaller output files.".into()));
            }
            let artifact = store
                .put_job_artifact(
                    &work.assignment.lease,
                    ArtifactInput {
                        version: work.version.reference(),
                        role: output.role,
                        media_type: output.media_type.clone(),
                        class: if work.requirements.no_training {
                            lab_kit::AssetClass::NoTraining
                        } else {
                            lab_kit::AssetClass::Public
                        },
                        no_training: work.requirements.no_training,
                    },
                    bytes,
                    8 * 1024 * 1024,
                    now,
                )
                .await?;
            artifacts.push(artifact.id);
        }
        let result = store
            .finish(&work.assignment.lease, request.status, None, artifacts, now)
            .await?;
        self.authority
            .complete(worker, &work.assignment, result.clone(), &fingerprint, now)
            .await?;
        Ok(result)
    }
}
