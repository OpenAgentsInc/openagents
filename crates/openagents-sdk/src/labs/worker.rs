//! Account-authorized enrollment and a separate job-only worker client.
//! Requests send once. Retain attempts and finish requests after response loss;
//! observing a started assignment does not grant permission to spawn it again.
use super::{CancellationToken, Error, JobRef, Request, WorkerRef, valid_id, valid_space, wire};
use crate::Deployment;
pub use lab_kit::worker::{
    Assignment, Capabilities, EffectiveRequirements, Enrolled, Lease, Placement, Usage, Worker,
    protocol,
};
use protocol::{Attempt, Available, Enroll, Finish, Poll, Work};
use reqwest::{Method, header};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fmt, future::Future, time::Duration};

const MAX_ENROLL: usize = 64 * 1024;
const MAX_WORK: usize = 12 * 1024 * 1024;
const MAX_OUTPUT: usize = 8 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
pub struct CredentialRecord {
    deployment: Deployment,
    token: String,
}
impl fmt::Debug for CredentialRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabWorkerCredential")
            .finish_non_exhaustive()
    }
}
/// A client whose credential authorizes only its worker's assigned work.
/// It cannot authenticate as an account, read documents, or choose an executable.
#[derive(Clone)]
pub struct Client {
    transport: crate::Client,
    record: CredentialRecord,
    invalidated: CancellationToken,
    timeout: Duration,
}
impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LabWorkerClient").finish_non_exhaustive()
    }
}
fn token_id(token: &str) -> Result<&str, Error> {
    let bytes = token.as_bytes();
    let hex = |b: &u8| b.is_ascii_digit() || (b'a'..=b'f').contains(b);
    if bytes.len() != 102
        || !bytes.starts_with(b"labw_")
        || bytes[37] != b'.'
        || !bytes[5..37].iter().all(hex)
        || !bytes[38..].iter().all(hex)
    {
        return Err(Error::InvalidInput);
    }
    Ok(&token[5..37])
}
impl crate::Client {
    /// Import a job-only key explicitly. The next worker request checks it with
    /// the service; this does not create an account session.
    pub fn labs_worker(&self, token: &str) -> Result<Client, Error> {
        token_id(token)?;
        Ok(Client {
            transport: self.clone(),
            record: CredentialRecord {
                deployment: self.deployment().clone(),
                token: token.into(),
            },
            invalidated: CancellationToken::new(),
            timeout: Duration::from_secs(30),
        })
    }
    pub fn restore_labs_worker(&self, record: &CredentialRecord) -> Result<Client, Error> {
        if record.deployment != *self.deployment() {
            return Err(crate::Error::WrongDeployment.into());
        }
        self.labs_worker(&record.token)
    }
}
impl super::Client {
    pub fn worker_ref(&self, space: &str, id: &str) -> Result<WorkerRef, Error> {
        valid_space(space)?;
        valid_id(id)?;
        Ok(WorkerRef {
            binding: self.snapshot()?.binding(),
            space: space.into(),
            id: id.into(),
        })
    }
    pub async fn enroll_worker(
        &self,
        request: &Request<Enroll>,
        cancel: &CancellationToken,
    ) -> Result<Enrolled, Error> {
        valid_space(&request.body.space)?;
        request
            .body
            .capabilities
            .validate()
            .map_err(|_| Error::InvalidInput)?;
        let snapshot = self.bound(&request.binding)?;
        let body = body(&request.body, MAX_ENROLL)?;
        let request = self.request(
            &snapshot,
            Method::POST,
            &["labs", "workers"],
            &[],
            Some(&body),
        )?;
        let enrolled: Enrolled = wire::guarded(&snapshot, cancel, async {
            let response = request.send().await.map_err(network)?;
            let bytes =
                crate::Client::read_limited(wire::checked(response).await?, MAX_ENROLL).await?;
            serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)
        })
        .await?;
        if token_id(&enrolled.token).map_err(|_| Error::InvalidResponse)? != enrolled.worker.id
            || enrolled.worker.account != snapshot.session.account().id
            || enrolled.worker.revoked
            || enrolled.worker.capabilities.validate().is_err()
        {
            return Err(Error::InvalidResponse);
        }
        Ok(enrolled)
    }
    pub async fn revoke_worker(
        &self,
        worker: &WorkerRef,
        cancel: &CancellationToken,
    ) -> Result<bool, Error> {
        valid_space(&worker.space)?;
        valid_id(&worker.id)?;
        let result: Revoked = self
            .json(
                &self.bound(&worker.binding)?,
                Method::DELETE,
                &["labs", "workers", &worker.id],
                &[("space", worker.space.clone())],
                None,
                None,
                cancel,
            )
            .await?;
        Ok(result.revoked)
    }
    pub async fn job_usage(
        &self,
        job: &JobRef,
        cancel: &CancellationToken,
    ) -> Result<Vec<Usage>, Error> {
        valid_space(&job.space)?;
        valid_id(&job.id)?;
        let result: Vec<Usage> = self
            .json(
                &self.bound(&job.binding)?,
                Method::GET,
                &["labs", "jobs", &job.id, "usage"],
                &[("space", job.space.clone())],
                None,
                None,
                cancel,
            )
            .await?;
        if result.iter().any(|row| row.job != job.id) {
            return Err(Error::InvalidResponse);
        }
        Ok(result)
    }
}
#[derive(Deserialize)]
struct Revoked {
    revoked: bool,
}
fn network(error: reqwest::Error) -> crate::Error {
    crate::Error::Network {
        timeout: error.is_timeout(),
    }
}
fn body<T: Serialize>(value: &T, limit: usize) -> Result<Vec<u8>, Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::InvalidInput)?;
    if bytes.len() > limit {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}
fn attempt(value: &Attempt) -> Result<(), Error> {
    valid_id(&value.job)?;
    if value.attempt == 0 {
        return Err(Error::InvalidInput);
    }
    Ok(())
}
fn work(value: &Work, expected: Option<&Attempt>) -> Result<(), Error> {
    let lease = &value.assignment.lease;
    valid_id(&lease.job.id).map_err(|_| Error::InvalidResponse)?;
    valid_id(&lease.worker).map_err(|_| Error::InvalidResponse)?;
    if lease.attempt == 0
        || value.version.number == 0
        || value.version.reference() != lease.job.spec.version
        || value.version.content_sha256 != lab_kit::digest(&value.version.content)
        || value.requirements.job != format!("{}/{}", lease.job.spec.kind, lease.job.spec.job_type)
        || lease.started != value.assignment.started_unix_ms.is_some()
        || expected.is_some_and(|expected| {
            expected.job != lease.job.id || expected.attempt != lease.attempt
        })
    {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}
impl Client {
    pub fn record(&self) -> CredentialRecord {
        self.record.clone()
    }
    pub fn worker_id(&self) -> &str {
        &self.record.token[5..37]
    }
    /// Stop this client's clones locally. Use `retire` to revoke the remote key,
    /// and remove any caller-owned saved credential separately.
    pub fn clear_credential(&self) {
        self.invalidated.cancel();
    }
    pub fn with_timeout(mut self, timeout: Duration) -> Result<Self, Error> {
        if timeout.is_zero() || timeout > Duration::from_secs(120) {
            return Err(Error::InvalidInput);
        }
        self.timeout = timeout;
        Ok(self)
    }
    async fn guarded<T>(
        &self,
        cancel: &CancellationToken,
        future: impl Future<Output = Result<T, Error>>,
    ) -> Result<T, Error> {
        tokio::select! {biased;
            _=self.invalidated.cancelled()=>Err(Error::Cancelled),
            _=cancel.cancelled()=>Err(Error::Cancelled),
            result=future=>if self.invalidated.is_cancelled() || cancel.is_cancelled() {Err(Error::Cancelled)} else {result}
        }
    }
    async fn send<T: DeserializeOwned>(
        &self,
        method: Method,
        segments: &[&str],
        body: Option<Vec<u8>>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        let mut url = self.record.deployment.api_base().clone();
        {
            let mut path = url.path_segments_mut().map_err(|_| Error::InvalidInput)?;
            path.pop_if_empty();
            path.push("labs").push("workers");
            for segment in segments {
                path.push(segment);
            }
        }
        let mut request = self
            .transport
            .http
            .request(method, url)
            .bearer_auth(&self.record.token)
            .timeout(self.timeout);
        if let Some(body) = body {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        self.guarded(cancel, async {
            let response = request.send().await.map_err(network)?;
            let bytes =
                crate::Client::read_limited(wire::checked(response).await?, MAX_WORK).await?;
            serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)
        })
        .await
    }
    pub async fn poll(
        &self,
        request: &Poll,
        cancel: &CancellationToken,
    ) -> Result<Available, Error> {
        if request.wait_seconds > 25 {
            return Err(Error::InvalidInput);
        }
        let response: Available = self
            .send(
                Method::POST,
                &["poll"],
                Some(body(request, MAX_ENROLL)?),
                cancel,
            )
            .await?;
        if let Some(value) = &response.work {
            work(value, None)?;
            if value.assignment.lease.worker != self.worker_id() {
                return Err(Error::InvalidResponse);
            }
        }
        Ok(response)
    }
    pub async fn start(
        &self,
        request: &Attempt,
        cancel: &CancellationToken,
    ) -> Result<Work, Error> {
        let work = self.update("start", request, cancel).await?;
        if !work.assignment.lease.started {
            return Err(Error::InvalidResponse);
        }
        Ok(work)
    }
    pub async fn heartbeat(
        &self,
        request: &Attempt,
        cancel: &CancellationToken,
    ) -> Result<Work, Error> {
        self.update("heartbeat", request, cancel).await
    }
    async fn update(
        &self,
        path: &str,
        request: &Attempt,
        cancel: &CancellationToken,
    ) -> Result<Work, Error> {
        attempt(request)?;
        let value: Work = self
            .send(
                Method::POST,
                &[path],
                Some(body(request, MAX_ENROLL)?),
                cancel,
            )
            .await?;
        work(&value, Some(request))?;
        if value.assignment.lease.worker != self.worker_id() {
            return Err(Error::InvalidResponse);
        }
        Ok(value)
    }
    /// Send a retained finish request once. An identical retry after response
    /// loss is safe; a response never grants permission to execute the job again.
    pub async fn finish(
        &self,
        request: &Finish,
        cancel: &CancellationToken,
    ) -> Result<lab_kit::JobState, Error> {
        attempt(&Attempt {
            job: request.job.clone(),
            attempt: request.attempt,
        })?;
        if !matches!(
            request.status,
            lab_kit::JobStatus::Done | lab_kit::JobStatus::Failed | lab_kit::JobStatus::Uncertain
        ) || request.outputs.len() > 16
            || (request.status != lab_kit::JobStatus::Done && !request.outputs.is_empty())
        {
            return Err(Error::InvalidInput);
        }
        use base64::Engine;
        let mut total = 0usize;
        for output in &request.outputs {
            if output.content_base64.len() > MAX_WORK {
                return Err(Error::TooLarge);
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&output.content_base64)
                .map_err(|_| Error::InvalidInput)?;
            total = total.checked_add(bytes.len()).ok_or(Error::TooLarge)?;
            if total > MAX_OUTPUT {
                return Err(Error::TooLarge);
            }
        }
        let value: lab_kit::JobState = self
            .send(
                Method::POST,
                &["finish"],
                Some(body(request, MAX_WORK)?),
                cancel,
            )
            .await?;
        if value.id != request.job || value.status != request.status {
            return Err(Error::InvalidResponse);
        }
        Ok(value)
    }
    pub async fn retire(&self, cancel: &CancellationToken) -> Result<bool, Error> {
        let result: Revoked = self
            .send(Method::DELETE, &[self.worker_id()], None, cancel)
            .await?;
        if result.revoked {
            self.clear_credential();
        }
        Ok(result.revoked)
    }
}
