//! Durable job-only credentials. Account sign-in and current membership remain
//! the API's responsibility; no account bearer is stored or sent to a worker.
use crate::Capabilities;
use lab_kit::{SpaceRef, digest};
use lab_store::{Error, Lease, Result, backend::Backend};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub job: String,
    pub attempt: u64,
    pub worker: String,
    pub account: String,
    pub space: SpaceRef,
    pub placement: crate::Placement,
    pub started_unix_ms: Option<u64>,
    pub observed_until_unix_ms: u64,
    /// Time between service observations, not CPU usage or provider billable time.
    pub observed_wall_ms: Option<u64>,
    pub outcome: lab_kit::JobStatus,
    pub charge: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Completion {
    pub result: lab_kit::JobState,
    pub fingerprint: String,
    pub usage: Usage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worker {
    pub id: String,
    pub account: String,
    pub space: SpaceRef,
    pub capabilities: Capabilities,
    pub expires_unix_ms: u64,
    pub revoked: bool,
    /// The issuing app/login-key session reference, never its bearer token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_session: Option<String>,
}

/// Returned once to the enrolling account. Debug output never contains the key.
#[derive(Serialize, Deserialize)]
pub struct Enrolled {
    pub worker: Worker,
    pub token: String,
}
impl std::fmt::Debug for Enrolled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Enrolled")
            .field("worker", &self.worker)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assignment {
    pub lease: Lease,
    /// The service records this before returning permission to execute.
    pub started_unix_ms: Option<u64>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Record {
    worker: Worker,
    token_hash: String,
    last_seen: u64,
    reservation: Option<Reservation>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Reservation {
    job: String,
    expires_unix_ms: u64,
    assignment: Option<Assignment>,
}
#[derive(Default, Serialize, Deserialize)]
struct Index {
    workers: BTreeMap<String, Record>,
    #[serde(default)]
    completed: BTreeMap<String, Completion>,
    #[serde(default)]
    assignments: BTreeMap<String, Assignment>,
    #[serde(default)]
    usage: BTreeMap<String, Usage>,
}

#[derive(Clone)]
pub struct Authority {
    backend: Arc<dyn Backend>,
    namespace: String,
}
impl Authority {
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            namespace: digest(&json!({"labs_worker_credentials":1})),
        }
    }
    async fn read(&self) -> Result<(u64, Index)> {
        let (generation, bytes) = self.backend.load(&self.namespace).await?;
        Ok((
            generation,
            match bytes {
                Some(b) => serde_json::from_slice(&b)?,
                None => Index::default(),
            },
        ))
    }
    async fn update<T>(&self, f: impl Fn(&mut Index) -> Result<T> + Send + Sync) -> Result<T> {
        for _ in 0..32 {
            let (generation, mut index) = self.read().await?;
            let result = f(&mut index)?;
            let bytes = serde_json::to_vec(&index)?;
            if bytes.len() > lab_store::backend::MAX_INDEX_BYTES {
                return Err(Error::RateLimited);
            }
            if self
                .backend
                .compare_and_swap(&self.namespace, generation, bytes)
                .await?
            {
                return Ok(result);
            }
            tokio::task::yield_now().await;
        }
        Err(Error::Conflict)
    }

    /// `account` and `space` must come from current account authorization.
    /// One credential admits jobs in one named space and no other account APIs.
    pub async fn enroll(
        &self,
        account: &str,
        space: SpaceRef,
        capabilities: Capabilities,
        now: u64,
        lifetime_ms: u64,
    ) -> Result<Enrolled> {
        self.enroll_inner(account, space, capabilities, now, lifetime_ms, None)
            .await
    }

    pub async fn enroll_for_session(
        &self,
        account: &str,
        space: SpaceRef,
        capabilities: Capabilities,
        now: u64,
        lifetime_ms: u64,
        session: &str,
    ) -> Result<Enrolled> {
        self.enroll_inner(
            account,
            space,
            capabilities,
            now,
            lifetime_ms,
            Some(session.into()),
        )
        .await
    }

    async fn enroll_inner(
        &self,
        account: &str,
        space: SpaceRef,
        mut capabilities: Capabilities,
        now: u64,
        lifetime_ms: u64,
        issuer_session: Option<String>,
    ) -> Result<Enrolled> {
        if account.is_empty()
            || account.len() > 256
            || !(60_000..=86_400_000).contains(&lifetime_ms)
        {
            return Err(Error::Invalid(
                "Choose a worker lifetime from one minute to one day.".into(),
            ));
        }
        if matches!(&space,SpaceRef::Personal{account:owner} if owner!=account) {
            return Err(Error::NotFound);
        }
        let id = Uuid::new_v4().simple().to_string();
        capabilities.worker = id.clone();
        capabilities.validate()?;
        let worker = Worker {
            id: id.clone(),
            account: account.into(),
            space,
            capabilities,
            expires_unix_ms: now.checked_add(lifetime_ms).ok_or(Error::Corrupt)?,
            revoked: false,
            issuer_session,
        };
        let token = format!(
            "labw_{id}.{}{}",
            Uuid::new_v4().simple(),
            Uuid::new_v4().simple()
        );
        let record = Record {
            worker: worker.clone(),
            token_hash: digest(&json!(token)),
            last_seen: now,
            reservation: None,
        };
        self.update(|i| {
            if i.workers.len() >= 10_000
                || i.workers
                    .values()
                    .filter(|r| {
                        r.worker.account == account
                            && !r.worker.revoked
                            && r.worker.expires_unix_ms > now
                    })
                    .count()
                    >= 32
            {
                return Err(Error::RateLimited);
            }
            i.workers.insert(id.clone(), record.clone());
            Ok(())
        })
        .await?;
        Ok(Enrolled { worker, token })
    }

    pub async fn authenticate(&self, token: &str, now: u64) -> Result<Worker> {
        let id = token
            .strip_prefix("labw_")
            .and_then(|s| s.split_once('.'))
            .filter(|(id, secret)| id.len() == 32 && secret.len() == 64)
            .map(|(id, _)| id)
            .ok_or(Error::LeaseLost)?;
        let hash = digest(&json!(token));
        self.update(|i| {
            let r = i.workers.get_mut(id).ok_or(Error::LeaseLost)?;
            // Both values have fixed length; do not return a mismatch position.
            let diff = r
                .token_hash
                .bytes()
                .zip(hash.bytes())
                .fold(0_u8, |a, (b, c)| a | (b ^ c));
            if diff != 0
                || r.token_hash.len() != hash.len()
                || r.worker.revoked
                || now >= r.worker.expires_unix_ms
            {
                return Err(Error::LeaseLost);
            }
            r.last_seen = now;
            Ok(r.worker.clone())
        })
        .await
    }

    pub async fn revoke(&self, account: &str, id: &str) -> Result<()> {
        self.update(|i| {
            let r = i
                .workers
                .get_mut(id)
                .filter(|r| r.worker.account == account)
                .ok_or(Error::NotFound)?;
            r.worker.revoked = true;
            Ok(())
        })
        .await
    }

    /// Refresh only from an authenticated account-service answer. A worker request
    /// cannot supply this structure; immutable enrollment fields must agree.
    pub async fn observe(&self, worker: &Worker, now: u64) -> Result<()> {
        worker.capabilities.validate()?;
        if worker.id != worker.capabilities.worker
            || worker.revoked
            || worker.expires_unix_ms <= now
        {
            return Err(Error::LeaseLost);
        }
        self.update(|i| {
            match i.workers.get_mut(&worker.id) {
                Some(r) => {
                    if r.worker != *worker {
                        return Err(Error::LeaseLost);
                    }
                    r.last_seen = now;
                }
                None => {
                    if i.workers.len() >= 10_000 {
                        return Err(Error::RateLimited);
                    }
                    i.workers.insert(
                        worker.id.clone(),
                        Record {
                            worker: worker.clone(),
                            token_hash: String::new(),
                            last_seen: now,
                            reservation: None,
                        },
                    );
                }
            }
            Ok(())
        })
        .await
    }
    pub async fn workers(&self, space: &SpaceRef, now: u64) -> Result<Vec<Capabilities>> {
        let (_, i) = self.read().await?;
        Ok(i.workers
            .values()
            .filter(|r| {
                !r.worker.revoked
                    && r.worker.expires_unix_ms > now
                    && r.worker.space == *space
                    && now.saturating_sub(r.last_seen) <= 30_000
                    && r.reservation
                        .as_ref()
                        .is_none_or(|a| a.expires_unix_ms <= now)
            })
            .map(|r| r.worker.capabilities.clone())
            .collect())
    }

    /// Reserve the worker's one slot before changing the job's independent index.
    /// A crash between those writes consumes only a bounded unused reservation.
    pub async fn reserve(&self, worker: &Worker, job: &str, now: u64) -> Result<()> {
        self.update(|i| {
            let r = i.workers.get_mut(&worker.id).ok_or(Error::LeaseLost)?;
            if r.worker.revoked || r.worker.expires_unix_ms <= now {
                return Err(Error::LeaseLost);
            }
            if r.reservation
                .as_ref()
                .is_some_and(|a| a.expires_unix_ms > now)
            {
                return Err(Error::Conflict);
            }
            r.reservation = Some(Reservation {
                job: job.into(),
                expires_unix_ms: now.saturating_add(30_000),
                assignment: None,
            });
            Ok(())
        })
        .await
    }
    pub async fn assign(&self, worker: &Worker, lease: Lease, now: u64) -> Result<Assignment> {
        self.update(|i| {
            let r = i.workers.get_mut(&worker.id).ok_or(Error::LeaseLost)?;
            if r.worker.revoked || r.worker.expires_unix_ms <= now || lease.worker != worker.id {
                return Err(Error::LeaseLost);
            }
            let a = r
                .reservation
                .as_mut()
                .filter(|a| a.job == lease.job.id && a.expires_unix_ms > now)
                .ok_or(Error::LeaseLost)?;
            let assignment = Assignment {
                lease: lease.clone(),
                started_unix_ms: a.assignment.as_ref().and_then(|a| a.started_unix_ms),
            };
            a.expires_unix_ms = lease.expires_unix_ms;
            a.assignment = Some(assignment.clone());
            i.assignments.insert(
                digest(&json!([worker.id, lease.job.id, lease.attempt])),
                assignment.clone(),
            );
            Ok(assignment)
        })
        .await
    }
    pub async fn assignment(&self, worker: &Worker, now: u64) -> Result<Option<Assignment>> {
        let (_, i) = self.read().await?;
        let r = i.workers.get(&worker.id).ok_or(Error::LeaseLost)?;
        if r.worker.revoked || r.worker.expires_unix_ms <= now {
            return Err(Error::LeaseLost);
        }
        Ok(r.reservation
            .as_ref()
            .filter(|r| r.expires_unix_ms > now)
            .and_then(|r| r.assignment.clone()))
    }
    pub async fn started(&self, worker: &Worker, lease: Lease, now: u64) -> Result<Assignment> {
        self.update(|i| {
            let r = i.workers.get_mut(&worker.id).ok_or(Error::LeaseLost)?;
            if r.worker.revoked || r.worker.expires_unix_ms <= now {
                return Err(Error::LeaseLost);
            }
            let a = r
                .reservation
                .as_mut()
                .and_then(|r| r.assignment.as_mut())
                .ok_or(Error::LeaseLost)?;
            if a.lease.job.id != lease.job.id
                || a.lease.attempt != lease.attempt
                || lease.worker != worker.id
                || !lease.started
            {
                return Err(Error::LeaseLost);
            }
            a.lease = lease.clone();
            a.started_unix_ms.get_or_insert(now);
            let result = a.clone();
            i.assignments.insert(
                digest(&json!([worker.id, lease.job.id, lease.attempt])),
                result.clone(),
            );
            Ok(result)
        })
        .await
    }

    pub async fn attempt(
        &self,
        worker: &Worker,
        job: &str,
        attempt: u64,
    ) -> Result<Option<Assignment>> {
        let (_, i) = self.read().await?;
        Ok(i.assignments
            .get(&digest(&json!([worker.id, job, attempt])))
            .cloned())
    }
    pub async fn release(&self, worker: &Worker, job: &str) -> Result<()> {
        self.update(|i| {
            let r = i.workers.get_mut(&worker.id).ok_or(Error::LeaseLost)?;
            if r.reservation.as_ref().is_some_and(|a| a.job == job) {
                r.reservation = None;
            }
            Ok(())
        })
        .await
    }

    pub async fn completion(
        &self,
        worker: &Worker,
        job: &str,
        attempt: u64,
    ) -> Result<Option<Completion>> {
        let (_, i) = self.read().await?;
        Ok(i.completed
            .get(&digest(&json!([worker.id, job, attempt])))
            .cloned())
    }

    pub async fn complete(
        &self,
        worker: &Worker,
        assignment: &Assignment,
        result: lab_kit::JobState,
        fingerprint: &str,
        now: u64,
    ) -> Result<()> {
        let key = digest(&json!([worker.id, result.id, assignment.lease.attempt]));
        let completed = Completion {
            usage: Usage {
                job: result.id.clone(),
                attempt: assignment.lease.attempt,
                worker: worker.id.clone(),
                account: worker.account.clone(),
                space: worker.space.clone(),
                placement: worker.capabilities.placement,
                started_unix_ms: assignment.started_unix_ms,
                observed_until_unix_ms: now,
                observed_wall_ms: assignment.started_unix_ms.map(|s| now.saturating_sub(s)),
                outcome: result.status,
                charge: None,
            },
            result,
            fingerprint: fingerprint.into(),
        };
        self.update(|i| {
            if let Some(prior) = i.completed.get(&key) {
                return if prior.fingerprint == fingerprint {
                    Ok(())
                } else {
                    Err(Error::Conflict)
                };
            }
            let r = i.workers.get_mut(&worker.id).ok_or(Error::LeaseLost)?;
            if r.reservation.as_ref().is_some_and(|r| {
                r.job == completed.result.id
                    && r.assignment
                        .as_ref()
                        .is_some_and(|a| a.lease.attempt == assignment.lease.attempt)
            }) {
                r.reservation = None;
            }
            i.assignments.remove(&key);
            i.usage.insert(key.clone(), completed.usage.clone());
            i.completed.insert(key.clone(), completed.clone());
            Ok(())
        })
        .await
    }

    /// Retained attempts allow the service to account for a worker that never
    /// reconnects. A receipt records an unknown duration rather than inventing
    /// CPU time, a price, or a successful result.
    pub async fn unsettled(&self, space: &SpaceRef) -> Result<Vec<(Worker, Assignment)>> {
        let (_, i) = self.read().await?;
        Ok(i.assignments
            .iter()
            .filter(|(key, _)| !i.usage.contains_key(*key))
            .filter_map(|(_, a)| {
                i.workers
                    .get(&a.lease.worker)
                    .filter(|r| r.worker.space == *space)
                    .map(|r| (r.worker.clone(), a.clone()))
            })
            .take(1000)
            .collect())
    }
    pub async fn observe_loss(
        &self,
        worker: &Worker,
        assignment: &Assignment,
        status: lab_kit::JobStatus,
        now: u64,
    ) -> Result<()> {
        let key = digest(&json!([
            worker.id,
            assignment.lease.job.id,
            assignment.lease.attempt
        ]));
        let usage = Usage {
            job: assignment.lease.job.id.clone(),
            attempt: assignment.lease.attempt,
            worker: worker.id.clone(),
            account: worker.account.clone(),
            space: worker.space.clone(),
            placement: worker.capabilities.placement,
            started_unix_ms: assignment.started_unix_ms,
            observed_until_unix_ms: now,
            observed_wall_ms: None,
            outcome: status,
            charge: None,
        };
        self.update(|i| {
            i.usage.entry(key.clone()).or_insert(usage.clone());
            Ok(())
        })
        .await
    }

    /// The caller must first authorize this space and job through the account API.
    pub async fn usage(&self, space: &SpaceRef, job: &str) -> Result<Vec<Usage>> {
        let (_, i) = self.read().await?;
        Ok(i.usage
            .values()
            .filter(|u| u.space == *space && u.job == job)
            .cloned()
            .collect())
    }
}
