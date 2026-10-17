//! Labs HTTP requests for kinds, tools, documents, versions, jobs, notes, and uploads.
use super::*;
use lab_kit::api::{Data, Deleted, ErrorBody, Saved};
use reqwest::{Method, Response, header};
use serde::{Serialize, de::DeserializeOwned};
use std::{future::Future, time::SystemTime};

pub(super) const MAX_JSON: usize = 16 * 1024 * 1024;
pub(super) const MAX_REQUEST: usize = 1024 * 1024;

pub(super) async fn guarded<T>(
    snapshot: &Snapshot,
    cancel: &CancellationToken,
    work: impl Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    tokio::select! { biased;
        _ = snapshot.invalidated.cancelled() => Err(Error::AccountChanged),
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = work => {
            if snapshot.invalidated.is_cancelled() { Err(Error::AccountChanged) }
            else if cancel.is_cancelled() { Err(Error::Cancelled) }
            else { result }
        }
    }
}
pub(super) fn retry_after(response: &Response) -> Option<Duration> {
    let value = response.headers().get(header::RETRY_AFTER)?.to_str().ok()?;
    value.parse().ok().map(Duration::from_secs).or_else(|| {
        httpdate::parse_http_date(value)
            .ok()?
            .duration_since(SystemTime::now())
            .ok()
    })
}
pub(super) async fn checked(response: Response) -> Result<Response, Error> {
    let status = response.status();
    if status.is_redirection() {
        return Err(crate::Error::Redirect.into());
    }
    if status.as_u16() == 402 {
        return Err(
            crate::Error::PaymentRequired(crate::PaymentRequired::from_headers(response.headers()))
                .into(),
        );
    }
    if status.is_success() {
        return Ok(response);
    }
    let retry_after = retry_after(&response);
    let bytes = crate::Client::read_limited(response, 64 * 1024).await?;
    let body: ErrorBody = serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)?;
    if body.error.code.is_empty() || body.error.code.len() > 128 || body.error.message.len() > 4096
    {
        return Err(Error::InvalidResponse);
    }
    Err(Error::Refused {
        status: status.as_u16(),
        refusal: Refusal::new(body.error.code, body.error.message),
        retry_after,
    })
}
fn body<T: Serialize>(value: &T) -> Result<Vec<u8>, Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::InvalidInput)?;
    if bytes.len() > MAX_REQUEST {
        return Err(Error::TooLarge);
    }
    Ok(bytes)
}
fn query(space: &str, page: Option<&PageOptions>) -> Result<Vec<(&'static str, String)>, Error> {
    valid_space(space)?;
    let mut query = vec![("space", space.into())];
    if let Some(page) = page {
        if page.limit == 0 || page.limit > 100 {
            return Err(Error::InvalidInput);
        }
        query.extend([
            ("after", page.after.to_string()),
            ("limit", page.limit.to_string()),
        ]);
        if let Some(kind) = &page.kind {
            valid_id(kind)?;
            query.push(("kind", kind.clone()));
        }
    }
    Ok(query)
}
impl Client {
    pub(super) fn url(
        &self,
        segments: &[&str],
        query: &[(&str, String)],
    ) -> Result<url::Url, Error> {
        let mut url = self.transport.deployment().api_base().clone();
        {
            let mut path = url.path_segments_mut().map_err(|_| Error::InvalidInput)?;
            path.pop_if_empty();
            for segment in segments {
                path.push(segment);
            }
        }
        if !query.is_empty() {
            url.query_pairs_mut()
                .extend_pairs(query.iter().map(|(k, v)| (*k, v.as_str())));
        }
        Ok(url)
    }
    pub(super) fn request(
        &self,
        snapshot: &Snapshot,
        method: Method,
        segments: &[&str],
        query: &[(&str, String)],
        body: Option<&[u8]>,
    ) -> Result<reqwest::RequestBuilder, Error> {
        let mut request = self
            .transport
            .http
            .request(method, self.url(segments, query)?)
            .bearer_auth(snapshot.session.token());
        if let Some(body) = body {
            request = request
                .header(header::CONTENT_TYPE, "application/json")
                .body(body.to_vec());
        }
        Ok(request)
    }
    pub(super) async fn json<T: DeserializeOwned>(
        &self,
        snapshot: &Snapshot,
        method: Method,
        segments: &[&str],
        query: &[(&str, String)],
        body: Option<&[u8]>,
        key: Option<&str>,
        cancel: &CancellationToken,
    ) -> Result<T, Error> {
        let mut request = self.request(snapshot, method, segments, query, body)?;
        if let Some(key) = key {
            request = request.header("Idempotency-Key", key);
        }
        guarded(snapshot, cancel, async {
            let response = request.send().await.map_err(|e| crate::Error::Network {
                timeout: e.is_timeout(),
            })?;
            let response = checked(response).await?;
            let bytes = crate::Client::read_limited(response, MAX_JSON).await?;
            serde_json::from_slice(&bytes).map_err(|_| Error::InvalidResponse)
        })
        .await
    }
    fn doc_snapshot(&self, document: &DocumentRef) -> Result<Snapshot, Error> {
        valid_space(&document.space)?;
        valid_id(&document.id)?;
        self.bound(&document.binding)
    }
    fn job_snapshot(&self, job: &JobRef) -> Result<Snapshot, Error> {
        valid_space(&job.space)?;
        valid_id(&job.id)?;
        self.bound(&job.binding)
    }
    /// Lists the lab kinds available in a space.
    pub async fn kinds(
        &self,
        space: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<KindManifest>, Error> {
        let response: Data<Vec<KindManifest>> = self
            .json(
                &self.snapshot()?,
                Method::GET,
                &["labs", "kinds"],
                &query(space, None)?,
                None,
                None,
                cancel,
            )
            .await?;
        Ok(response.data)
    }
    /// Lists the tools a lab kind offers in a space.
    pub async fn tools(
        &self,
        space: &str,
        kind: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<ToolDecl>, Error> {
        valid_id(kind)?;
        let response: Data<Vec<ToolDecl>> = self
            .json(
                &self.snapshot()?,
                Method::GET,
                &["labs", "kinds", kind, "tools"],
                &query(space, None)?,
                None,
                None,
                cancel,
            )
            .await?;
        Ok(response.data)
    }
    /// Calls a lab kind's tool and returns its result.
    pub async fn invoke(
        &self,
        kind: &str,
        name: &str,
        request: &Request<ToolCall>,
        cancel: &CancellationToken,
    ) -> Result<ToolResult, Error> {
        valid_id(kind)?;
        valid_id(name)?;
        valid_space(&request.body.space)?;
        self.json(
            &self.bound(&request.binding)?,
            Method::POST,
            &["labs", "kinds", kind, "tools", name],
            &[],
            Some(&body(&request.body)?),
            None,
            cancel,
        )
        .await
    }
    /// Returns a page of documents in a space.
    pub async fn documents(
        &self,
        space: &str,
        page: &PageOptions,
        cancel: &CancellationToken,
    ) -> Result<List<DocumentMeta>, Error> {
        self.json(
            &self.snapshot()?,
            Method::GET,
            &["labs", "documents"],
            &query(space, Some(page))?,
            None,
            None,
            cancel,
        )
        .await
    }
    /// Creates a document and returns its first version.
    pub async fn create_document(
        &self,
        request: &Request<CreateDocument>,
        cancel: &CancellationToken,
    ) -> Result<Version, Error> {
        valid_space(&request.body.space)?;
        valid_id(&request.body.id)?;
        valid_id(&request.body.kind)?;
        let version: Version = self
            .json(
                &self.bound(&request.binding)?,
                Method::POST,
                &["labs", "documents"],
                &[],
                Some(&body(&request.body)?),
                None,
                cancel,
            )
            .await?;
        check_version(&version, &request.body.id, None)?;
        Ok(version)
    }
    /// Returns a document's details.
    pub async fn document(
        &self,
        document: &DocumentRef,
        cancel: &CancellationToken,
    ) -> Result<DocumentMeta, Error> {
        let value: DocumentMeta = self
            .json(
                &self.doc_snapshot(document)?,
                Method::GET,
                &["labs", "documents", &document.id],
                &query(&document.space, None)?,
                None,
                None,
                cancel,
            )
            .await?;
        if value.id != document.id {
            return Err(Error::InvalidResponse);
        }
        Ok(value)
    }
    /// Deletes a document and returns whether it was deleted.
    pub async fn delete_document(
        &self,
        document: &DocumentRef,
        cancel: &CancellationToken,
    ) -> Result<bool, Error> {
        let value: Deleted = self
            .json(
                &self.doc_snapshot(document)?,
                Method::DELETE,
                &["labs", "documents", &document.id],
                &query(&document.space, None)?,
                None,
                None,
                cancel,
            )
            .await?;
        if value.id != document.id {
            return Err(Error::InvalidResponse);
        }
        Ok(value.deleted)
    }
    /// Returns a page of a document's versions.
    pub async fn versions(
        &self,
        document: &DocumentRef,
        page: &PageOptions,
        cancel: &CancellationToken,
    ) -> Result<List<Version>, Error> {
        let values: List<Version> = self
            .json(
                &self.doc_snapshot(document)?,
                Method::GET,
                &["labs", "documents", &document.id, "versions"],
                &query(&document.space, Some(page))?,
                None,
                None,
                cancel,
            )
            .await?;
        if values.data.len() > page.limit as usize {
            return Err(Error::InvalidResponse);
        }
        for value in &values.data {
            check_version(value, &document.id, None)?;
        }
        Ok(values)
    }
    /// Returns one version of a document by number.
    pub async fn version(
        &self,
        document: &DocumentRef,
        number: u64,
        cancel: &CancellationToken,
    ) -> Result<Version, Error> {
        if number == 0 {
            return Err(Error::InvalidInput);
        }
        let version: Version = self
            .json(
                &self.doc_snapshot(document)?,
                Method::GET,
                &[
                    "labs",
                    "documents",
                    &document.id,
                    "versions",
                    &number.to_string(),
                ],
                &query(&document.space, None)?,
                None,
                None,
                cancel,
            )
            .await?;
        check_version(&version, &document.id, Some(number))?;
        Ok(version)
    }
    /// Proposes a new version of a document and reports whether it was created, out of date, or refused.
    pub async fn edit(
        &self,
        document: &DocumentRef,
        request: &Request<EditDocument>,
        cancel: &CancellationToken,
    ) -> Result<EditResult, Error> {
        let snapshot = self.doc_snapshot(document)?;
        self.bound(&request.binding)?;
        if request.body.space != document.space || request.body.base == 0 {
            return Err(Error::InvalidInput);
        }
        let result: Result<Version, Error> = self
            .json(
                &snapshot,
                Method::POST,
                &["labs", "documents", &document.id, "versions"],
                &[],
                Some(&body(&request.body)?),
                None,
                cancel,
            )
            .await;
        match result {
            Ok(version) => {
                check_version(&version, &document.id, None)?;
                Ok(EditResult::Created(version))
            }
            Err(Error::Refused {
                status: 409,
                refusal,
                ..
            }) if matches!(refusal.code.as_str(), "conflict" | "stale_base") => {
                Ok(EditResult::Stale)
            }
            Err(Error::Refused {
                status: 422,
                refusal,
                ..
            }) => Ok(EditResult::Refused(refusal)),
            Err(e) => Err(e),
        }
    }
    /// Returns the card for one version of a document.
    pub async fn card(
        &self,
        document: &DocumentRef,
        number: u64,
        cancel: &CancellationToken,
    ) -> Result<Card, Error> {
        if number == 0 {
            return Err(Error::InvalidInput);
        }
        let card: Card = self
            .json(
                &self.doc_snapshot(document)?,
                Method::GET,
                &[
                    "labs",
                    "documents",
                    &document.id,
                    "versions",
                    &number.to_string(),
                    "card",
                ],
                &query(&document.space, None)?,
                None,
                None,
                cancel,
            )
            .await?;
        if card.document != document.id || card.version != number {
            return Err(Error::InvalidResponse);
        }
        Ok(card)
    }
    /// Starts a job, retrying with the same key after network errors or busy responses.
    pub async fn create_job(
        &self,
        request: &JobRequest,
        cancel: &CancellationToken,
    ) -> Result<JobState, Error> {
        if request.schema != "openagents.labs.job-request.v1" {
            return Err(Error::InvalidInput);
        }
        valid_space(&request.body().space)?;
        valid_id(&request.body().version.document)?;
        if request.body().version.version == 0 {
            return Err(Error::InvalidInput);
        }
        RequestKey::try_from(request.key.0.clone())?;
        let snapshot = self.bound(&request.request.binding)?;
        let bytes = body(request.body())?;
        let mut attempt = 0u8;
        loop {
            let result: Result<JobState, Error> = self
                .json(
                    &snapshot,
                    Method::POST,
                    &["labs", "jobs"],
                    &[],
                    Some(&bytes),
                    Some(request.key.as_str()),
                    cancel,
                )
                .await;
            match result {
                Ok(job) => {
                    valid_id(&job.id).map_err(|_| Error::InvalidResponse)?;
                    if job.spec.version != request.body().version {
                        return Err(Error::InvalidResponse);
                    }
                    return Ok(job);
                }
                Err(error) if attempt < self.retries.max_retries && retryable(&error) => {
                    let delay = error.retry_after().unwrap_or_else(|| {
                        self.retries
                            .initial_delay
                            .saturating_mul(1u32 << attempt)
                            .min(self.retries.max_delay)
                    });
                    if delay > self.retries.max_delay {
                        return Err(error);
                    }
                    guarded(&snapshot, cancel, async {
                        tokio::time::sleep(delay).await;
                        Ok(())
                    })
                    .await?;
                    attempt += 1;
                }
                Err(error) => return Err(error),
            }
        }
    }
    /// Returns a page of a document's jobs.
    pub async fn jobs(
        &self,
        document: &DocumentRef,
        page: &PageOptions,
        cancel: &CancellationToken,
    ) -> Result<List<JobState>, Error> {
        let mut query = query(&document.space, Some(page))?;
        query.push(("document", document.id.clone()));
        let jobs: List<JobState> = self
            .json(
                &self.doc_snapshot(document)?,
                Method::GET,
                &["labs", "jobs"],
                &query,
                None,
                None,
                cancel,
            )
            .await?;
        if jobs.data.len() > page.limit as usize
            || jobs
                .data
                .iter()
                .any(|job| job.spec.version.document != document.id)
        {
            return Err(Error::InvalidResponse);
        }
        Ok(jobs)
    }
    /// Returns a job's current state.
    pub async fn job(&self, job: &JobRef, cancel: &CancellationToken) -> Result<JobState, Error> {
        self.read_job(job, Method::GET, cancel).await
    }
    /// Cancels a job and returns its state.
    pub async fn cancel_job(
        &self,
        job: &JobRef,
        cancel: &CancellationToken,
    ) -> Result<JobState, Error> {
        self.read_job(job, Method::DELETE, cancel).await
    }
    async fn read_job(
        &self,
        job: &JobRef,
        method: Method,
        cancel: &CancellationToken,
    ) -> Result<JobState, Error> {
        let value: JobState = self
            .json(
                &self.job_snapshot(job)?,
                method,
                &["labs", "jobs", &job.id],
                &query(&job.space, None)?,
                None,
                None,
                cancel,
            )
            .await?;
        if value.id != job.id {
            return Err(Error::InvalidResponse);
        }
        Ok(value)
    }
    /// Checks a job until it finishes or the wait times out.
    pub async fn wait(
        &self,
        job: &JobRef,
        options: WaitOptions,
        cancel: &CancellationToken,
    ) -> Result<JobState, Error> {
        if options.timeout.is_zero()
            || options.timeout > Duration::from_secs(86400)
            || options.poll_interval < Duration::from_millis(10)
            || options.poll_interval > Duration::from_secs(60)
        {
            return Err(Error::InvalidInput);
        }
        let snapshot = self.job_snapshot(job)?;
        let deadline = tokio::time::Instant::now() + options.timeout;
        guarded(&snapshot, cancel, async {
            loop {
                let current = tokio::time::timeout_at(deadline, self.job(job, cancel))
                    .await
                    .map_err(|_| Error::WaitTimedOut)??;
                if current.status.finished() || current.status == JobStatus::Uncertain {
                    return Ok(current);
                }
                tokio::time::timeout_at(deadline, tokio::time::sleep(options.poll_interval))
                    .await
                    .map_err(|_| Error::WaitTimedOut)?;
            }
        })
        .await
    }
    /// Returns a page of a document's notes.
    pub async fn notes(
        &self,
        document: &DocumentRef,
        page: &PageOptions,
        cancel: &CancellationToken,
    ) -> Result<List<Note>, Error> {
        self.json(
            &self.doc_snapshot(document)?,
            Method::GET,
            &["labs", "documents", &document.id, "notes"],
            &query(&document.space, Some(page))?,
            None,
            None,
            cancel,
        )
        .await
    }
    /// Adds a note to a document version.
    pub async fn add_note(
        &self,
        document: &DocumentRef,
        request: &Request<AddNote>,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        self.bound(&request.binding)?;
        if document.space != request.body.space || request.body.version == 0 {
            return Err(Error::InvalidInput);
        }
        let result: Saved = self
            .json(
                &self.doc_snapshot(document)?,
                Method::POST,
                &["labs", "documents", &document.id, "notes"],
                &[],
                Some(&body(&request.body)?),
                None,
                cancel,
            )
            .await?;
        if !result.saved {
            return Err(Error::InvalidResponse);
        }
        Ok(())
    }
    /// Uploads a file to a space and returns its details.
    pub async fn upload(
        &self,
        request: &Request<Upload>,
        cancel: &CancellationToken,
    ) -> Result<Artifact, Error> {
        valid_space(&request.body.space)?;
        if request.body.class == AssetClass::PrivateLocal {
            return Err(Error::InvalidInput);
        }
        self.json(
            &self.bound(&request.binding)?,
            Method::POST,
            &["labs", "uploads"],
            &[],
            Some(&body(&request.body)?),
            None,
            cancel,
        )
        .await
    }
    /// Returns up to `limit` document events after a cursor, without a live stream.
    pub async fn events_since(
        &self,
        document: &DocumentRef,
        since: u64,
        limit: u16,
        cancel: &CancellationToken,
    ) -> Result<List<Event>, Error> {
        let mut query = query(
            &document.space,
            Some(&PageOptions {
                limit,
                ..PageOptions::default()
            }),
        )?;
        query.push(("since", since.to_string()));
        let events: List<Event> = self
            .json(
                &self.doc_snapshot(document)?,
                Method::GET,
                &["labs", "documents", &document.id, "events"],
                &query,
                None,
                None,
                cancel,
            )
            .await?;
        if events.data.len() > limit as usize {
            return Err(Error::InvalidResponse);
        }
        let mut cursor = since;
        for event in &events.data {
            if event.document != document.id || event.cursor <= cursor {
                return Err(Error::InvalidResponse);
            }
            cursor = event.cursor;
        }
        Ok(events)
    }
}
pub(super) fn retryable(error: &Error) -> bool {
    matches!(
        error,
        Error::Session(crate::Error::Network { .. })
            | Error::Refused {
                status: 408 | 429 | 500 | 502 | 503 | 504,
                ..
            }
    )
}
fn check_version(version: &Version, document: &str, number: Option<u64>) -> Result<(), Error> {
    if version.document != document
        || version.number == 0
        || number.is_some_and(|n| version.number != n)
        || version.content_sha256 != lab_kit::digest(&version.content)
    {
        return Err(Error::InvalidResponse);
    }
    Ok(())
}
