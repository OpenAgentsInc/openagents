//! Shared Lab storage. Bind a store to an authorized space before using it.
//!
//! A write publishes immutable objects before atomically changing the space's
//! index. Interrupted writes can leave unreferenced objects, never a visible
//! version without its content. API admission remains the caller's job.

pub mod backend;
pub mod cad_legacy;
mod state;

use async_trait::async_trait;
use lab_kit::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use state::{DocumentRecord, State};
use std::{ops::Range, sync::Arc};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("This item could not be found.")]
    NotFound,
    #[error("This document changed. Reload it and try again.")]
    Conflict,
    #[error("This lab has reached its hourly limit. Try again later.")]
    RateLimited,
    #[error("This worker can no longer change the job.")]
    LeaseLost,
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Refused(#[from] Refusal),
    #[error("The lab's saved files could not be read.")]
    Corrupt,
    #[error("The lab's storage is unavailable.")]
    Storage(#[source] Box<dyn std::error::Error + Send + Sync>),
}
impl Error {
    pub fn storage(error: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Storage(Box::new(error))
    }
}
impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::storage(error)
    }
}
impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::Corrupt
    }
}

pub(crate) fn require(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}
pub(crate) fn text(value: &str, max: usize) -> Result<()> {
    require(
        !value.trim().is_empty() && value.len() <= max && !value.contains('\0'),
        "Enter a shorter, nonempty value.",
    )
}
pub(crate) fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// A checked change. Its fields are private so callers cannot bypass a kind's checks.
#[derive(Clone)]
pub struct CheckedDraft {
    kind: String,
    schema: String,
    limits: Limits,
    base: u64,
    document: Option<String>,
    base_fingerprint: Option<String>,
    draft: Draft,
}
impl CheckedDraft {
    pub fn content(&self) -> &Value {
        &self.draft.content
    }
    pub fn initial<K: LabKind>(
        schema: &str,
        content: Value,
        summary: &str,
        measurements: &Measurements,
    ) -> Result<Self> {
        let manifest = K::manifest();
        require(
            manifest.kind == K::KIND && manifest.schemas.iter().any(|s| s == schema),
            "Choose a document format this lab supports.",
        )?;
        let report = K::check(&content, measurements)?;
        Self::checked(
            manifest,
            schema.into(),
            0,
            Draft {
                content,
                summary: summary.into(),
                report,
                assets: vec![],
            },
        )
    }
    pub fn change<K: LabKind>(
        base: &Version,
        change: &Change,
        summary: &str,
        measurements: &Measurements,
    ) -> Result<Self> {
        let manifest = K::manifest();
        require(
            manifest.kind == K::KIND && manifest.schemas.contains(&base.schema),
            "Choose a document format this lab supports.",
        )?;
        require(
            serde_json::to_vec(change)?.len() as u64 <= manifest.limits.max_args_bytes,
            "This change is too large.",
        )?;
        let mut draft = K::apply(base, change, summary, measurements)?;
        draft.report = K::check(&draft.content, measurements)?;
        let mut checked = Self::checked(manifest, base.schema.clone(), base.number, draft)?;
        checked.document = Some(base.document.clone());
        checked.base_fingerprint = Some(digest(&serde_json::to_value(base)?));
        Ok(checked)
    }
    fn checked(manifest: KindManifest, schema: String, base: u64, draft: Draft) -> Result<Self> {
        text(&draft.summary, 2048)?;
        require(
            canonical_json(&draft.content).len() as u64 <= manifest.limits.max_doc_bytes,
            "This document is too large.",
        )?;
        require(
            draft.assets.len() <= 1024,
            "This document uses too many files.",
        )?;
        for asset in &draft.assets {
            text(&asset.location, 2048)?;
            require(
                valid_sha(&asset.sha256),
                "Choose a file with a valid checksum.",
            )?;
        }
        Ok(Self {
            kind: manifest.kind,
            schema,
            limits: manifest.limits,
            base,
            document: None,
            base_fingerprint: None,
            draft,
        })
    }
}
fn valid_sha(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<u64>,
}
pub use lab_kit::api::Event;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Lease {
    pub job: JobState,
    pub worker: String,
    pub attempt: u64,
    pub expires_unix_ms: u64,
    pub started: bool,
}
#[derive(Clone, Debug)]
pub struct ArtifactInput {
    pub version: Reference,
    pub role: ArtifactRole,
    pub media_type: String,
    pub class: AssetClass,
    pub no_training: bool,
}

/// Operations never accept a space argument: each instance is already scoped.
#[async_trait]
pub trait LabStore: Send + Sync {
    async fn documents(
        &self,
        kind: Option<&str>,
        after: u64,
        limit: u64,
    ) -> Result<Page<DocumentMeta>>;
    async fn document(&self, id: &str) -> Result<DocumentMeta>;
    async fn create(
        &self,
        id: &str,
        title: &str,
        author: &str,
        draft: CheckedDraft,
        now: u64,
    ) -> Result<Version>;
    async fn save(&self, id: &str, author: &str, draft: CheckedDraft, now: u64) -> Result<Version>;
    async fn delete(&self, id: &str, now: u64) -> Result<()>;
    async fn versions(&self, id: &str, after: u64, limit: u64) -> Result<Page<Version>>;
    async fn version(&self, id: &str, number: u64) -> Result<Version>;
    async fn put_artifact(
        &self,
        input: ArtifactInput,
        bytes: Vec<u8>,
        max_bytes: u64,
        now: u64,
    ) -> Result<Artifact>;
    async fn artifact(&self, id: &str) -> Result<Artifact>;
    async fn artifact_bytes(&self, id: &str, range: Option<Range<u64>>) -> Result<Vec<u8>>;
    async fn artifact_url(&self, id: &str, expires_seconds: u32) -> Result<Option<String>>;
    async fn create_job(
        &self,
        spec: JobSpec,
        manifest: &KindManifest,
        now: u64,
    ) -> Result<JobState>;
    async fn jobs(&self, document: &str, after: u64, limit: u64) -> Result<Page<JobState>>;
    async fn job(&self, id: &str) -> Result<JobState>;
    async fn version_jobs(
        &self,
        version: &Reference,
        after: u64,
        limit: u64,
    ) -> Result<Page<JobState>>;
    async fn claim(&self, id: &str, worker: &str, now: u64, duration_ms: u64) -> Result<Lease>;
    async fn start(&self, lease: &Lease, now: u64) -> Result<Lease>;
    async fn heartbeat(&self, lease: &Lease, now: u64, duration_ms: u64) -> Result<Lease>;
    async fn finish(
        &self,
        lease: &Lease,
        status: JobStatus,
        message: Option<String>,
        artifacts: Vec<String>,
        now: u64,
    ) -> Result<JobState>;
    async fn cancel(&self, id: &str, now: u64) -> Result<JobState>;
    async fn expire_leases(&self, now: u64) -> Result<usize>;
    async fn add_note(&self, note: Note) -> Result<()>;
    async fn notes(&self, document: &str, after: u64, limit: u64) -> Result<Page<Note>>;
    async fn events(&self, document: &str, since: u64, limit: u64) -> Result<Page<Event>>;
}

#[derive(Clone)]
pub struct Store {
    backend: Arc<dyn backend::Backend>,
    namespace: String,
}
impl Store {
    /// The hashed space address, for worker caches and rate limits.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }
    /// Bind a previously authorized legacy address without storing an account ID.
    /// A binding can be retried but never redirected to a different space.
    pub async fn bind_alias(&self, alias: &str) -> Result<()> {
        text(alias, 512)?;
        let key = digest(&serde_json::json!(["openagents.lab-store.alias.v1", alias]));
        let value = serde_json::to_vec(
            &serde_json::json!({"schema":"openagents.lab-store.alias.v1","space":self.namespace}),
        )?;
        for _ in 0..32 {
            let (generation, current) = self.backend.load(&key).await?;
            if let Some(current) = current {
                let current: Value = serde_json::from_slice(&current)?;
                require(
                    current["schema"] == "openagents.lab-store.alias.v1"
                        && current["space"] == self.namespace,
                    "This address belongs to another lab space.",
                )?;
                return Ok(());
            }
            if self
                .backend
                .compare_and_swap(&key, generation, value.clone())
                .await?
            {
                return Ok(());
            }
        }
        Err(Error::Conflict)
    }
    /// The caller must authorize `alias` before opening the space it names.
    pub async fn from_alias(
        backend: Arc<dyn backend::Backend>,
        alias: &str,
    ) -> Result<Option<Self>> {
        text(alias, 512)?;
        let key = digest(&serde_json::json!(["openagents.lab-store.alias.v1", alias]));
        let (_, current) = backend.load(&key).await?;
        let Some(current) = current else {
            return Ok(None);
        };
        let value: Value = serde_json::from_slice(&current)?;
        let namespace = value["space"]
            .as_str()
            .filter(|s| valid_sha(s))
            .ok_or(Error::Corrupt)?;
        require(
            value["schema"] == "openagents.lab-store.alias.v1",
            "This lab address couldn't be read.",
        )?;
        Ok(Some(Self {
            backend,
            namespace: namespace.into(),
        }))
    }
    /// Include deletion markers when deciding whether to fall back to an older store.
    pub async fn knows_document(&self, id: &str) -> Result<bool> {
        let (_, state) = self.state().await?;
        Ok(state.documents.contains_key(id))
    }
    pub async fn deleted_documents(&self, kind: &str) -> Result<Vec<String>> {
        let (_, state) = self.state().await?;
        Ok(state
            .documents
            .values()
            .filter(|d| d.deleted && d.meta.kind == kind)
            .map(|d| d.meta.id.clone())
            .collect())
    }
    /// Attach a kind's file name to an existing artifact in this version.
    /// The object address remains content-based and never uses the file name.
    pub async fn name_artifact(
        &self,
        version: &Reference,
        name: &str,
        artifact: &str,
    ) -> Result<()> {
        let key = file_key(version, name)?;
        self.update(|state| {
            state.reference(version)?;
            let value = state.artifacts.get(artifact).ok_or(Error::NotFound)?;
            require(
                value.version == *version,
                "Choose a file from this version.",
            )?;
            state.files.insert(key.clone(), artifact.into());
            Ok(())
        })
        .await
    }
    pub async fn named_artifact(&self, version: &Reference, name: &str) -> Result<Artifact> {
        let key = file_key(version, name)?;
        let (_, state) = self.state().await?;
        state.reference(version)?;
        let id = state.files.get(&key).ok_or(Error::NotFound)?;
        state.artifacts.get(id).cloned().ok_or(Error::Corrupt)
    }
    /// The extra entry lets a card refuse a history beyond its display limit.
    pub async fn version_artifacts(&self, version: &Reference) -> Result<Vec<Artifact>> {
        let (_, state) = self.state().await?;
        state.reference(version)?;
        Ok(state
            .artifacts
            .values()
            .filter(|a| a.version == *version)
            .take(1001)
            .cloned()
            .collect())
    }
    /// Bind a retry identity before admitting work. Reuse requires identical input.
    pub async fn reserve_request(&self, key: &str, input: &Value) -> Result<()> {
        text(key, 128)?;
        let key = sha(key.as_bytes());
        let fingerprint = digest(input);
        self.update(|s| {
            if let Some(prior) = s.requests.get(&key) {
                return if prior == &fingerprint {
                    Ok(())
                } else {
                    Err(Error::Conflict)
                };
            }
            require(
                s.requests.len() < 100_000,
                "This space has reached its request limit.",
            )?;
            s.requests.insert(key.clone(), fingerprint.clone());
            Ok(())
        })
        .await
    }
    /// Call only after authenticating and authorizing access to `space`.
    pub fn new(backend: Arc<dyn backend::Backend>, space: &SpaceRef) -> Result<Self> {
        match space {
            SpaceRef::Personal { account } => text(account, 256)?,
            SpaceRef::Project { project } => text(project, 256)?,
        }
        Ok(Self {
            backend,
            namespace: digest(&serde_json::to_value(space)?),
        })
    }
    async fn state(&self) -> Result<(u64, State)> {
        let (generation, bytes) = self.backend.load(&self.namespace).await?;
        let state: State = match bytes {
            Some(bytes) => serde_json::from_slice(&bytes)?,
            None => State::default(),
        };
        if state.schema != state::SCHEMA {
            return Err(Error::Corrupt);
        }
        Ok((generation, state))
    }
    async fn update<T>(&self, change: impl Fn(&mut State) -> Result<T> + Send + Sync) -> Result<T> {
        for _ in 0..32 {
            let (generation, mut state) = self.state().await?;
            let result = change(&mut state)?;
            let bytes = serde_json::to_vec(&state)?;
            require(
                bytes.len() <= backend::MAX_INDEX_BYTES,
                "This lab space is full.",
            )?;
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
    async fn publish(
        &self,
        id: &str,
        title: Option<&str>,
        author: &str,
        checked: CheckedDraft,
        now: u64,
    ) -> Result<Version> {
        text(id, 256)?;
        text(author, 256)?;
        require(
            checked
                .document
                .as_deref()
                .is_none_or(|source| source == id),
            "Apply this change to the document it came from.",
        )?;
        if let Some(title) = title {
            text(title, 512)?;
            require(
                checked.base == 0,
                "Start a new document with its first version.",
            )?;
        }
        let number = checked.base.checked_add(1).ok_or(Error::Conflict)?;
        require(
            number <= checked.limits.max_versions_per_doc,
            "This document has reached its version limit.",
        )?;
        let version = Version {
            document: id.into(),
            number,
            parent: (checked.base > 0).then_some(checked.base),
            author: author.into(),
            summary: checked.draft.summary.clone(),
            created_unix_ms: now,
            schema: checked.schema.clone(),
            content_sha256: digest(&checked.draft.content),
            content: checked.draft.content,
            report: checked.draft.report,
            assets: checked.draft.assets,
        };
        if let Some(fingerprint) = &checked.base_fingerprint {
            let saved = self.version(id, checked.base).await?;
            if digest(&serde_json::to_value(saved)?) != *fingerprint {
                return Err(Error::Conflict);
            }
        }
        let bytes = serde_json::to_vec(&version)?;
        let object = sha(&bytes);
        self.backend.put(&self.namespace, &object, bytes).await?;
        self.update(|state| {
            if let Some(title) = title {
                if state.documents.contains_key(id) {
                    return Err(Error::Conflict);
                }
                require(
                    (state
                        .documents
                        .values()
                        .filter(|d| d.meta.kind == checked.kind && !d.deleted)
                        .count() as u64)
                        < checked.limits.max_docs_per_space,
                    "This space has reached its document limit.",
                )?;
                state.documents.insert(
                    id.into(),
                    DocumentRecord {
                        meta: DocumentMeta {
                            id: id.into(),
                            kind: checked.kind.clone(),
                            schema: checked.schema.clone(),
                            title: title.into(),
                            created_unix_ms: now,
                            latest: 0,
                        },
                        deleted: false,
                        versions: Default::default(),
                    },
                );
            }
            let document = state.document_mut(id)?;
            if document.meta.latest != checked.base {
                return Err(Error::Conflict);
            }
            require(
                document.meta.kind == checked.kind && document.meta.schema == checked.schema,
                "Choose the same lab and format as this document.",
            )?;
            document.meta.latest = number;
            document.versions.insert(number, object.clone());
            state.event(id, now, json!({"type":"version", "version":number}))?;
            Ok(version.clone())
        })
        .await
    }
}

fn file_key(version: &Reference, name: &str) -> Result<String> {
    require(
        !name.is_empty()
            && name.len() <= 256
            && name.split('/').all(|s| {
                !s.is_empty()
                    && s != "."
                    && s != ".."
                    && s.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            }),
        "Choose a valid file name.",
    )?;
    Ok(serde_json::to_string(&(version, name))?)
}
fn page<T>(items: impl Iterator<Item = T>, after: u64, limit: u64) -> Result<Page<T>> {
    require(
        (1..=100).contains(&limit) && after <= usize::MAX as u64,
        "Choose a page size between 1 and 100.",
    )?;
    let mut items: Vec<_> = items
        .skip(after as usize)
        .take(limit as usize + 1)
        .collect();
    let next = if items.len() > limit as usize {
        items.pop();
        Some(after + limit)
    } else {
        None
    };
    Ok(Page { items, next })
}

#[async_trait]
impl LabStore for Store {
    async fn documents(
        &self,
        kind: Option<&str>,
        after: u64,
        limit: u64,
    ) -> Result<Page<DocumentMeta>> {
        let (_, state) = self.state().await?;
        page(
            state
                .documents
                .values()
                .filter(|d| !d.deleted && kind.is_none_or(|k| d.meta.kind == k))
                .map(|d| d.meta.clone()),
            after,
            limit,
        )
    }
    async fn document(&self, id: &str) -> Result<DocumentMeta> {
        Ok(self.state().await?.1.document(id)?.meta.clone())
    }
    async fn create(
        &self,
        id: &str,
        title: &str,
        author: &str,
        draft: CheckedDraft,
        now: u64,
    ) -> Result<Version> {
        self.publish(id, Some(title), author, draft, now).await
    }
    async fn save(&self, id: &str, author: &str, draft: CheckedDraft, now: u64) -> Result<Version> {
        require(draft.base > 0, "Choose an existing version to change.")?;
        self.publish(id, None, author, draft, now).await
    }
    async fn delete(&self, id: &str, now: u64) -> Result<()> {
        self.update(|s| {
            s.document_mut(id)?.deleted = true;
            // Deletion prevents new reads and work; already running work stays
            // uncertain until the worker's effect is separately observed.
            for j in s
                .jobs
                .values_mut()
                .filter(|j| j.state.spec.version.document == id)
            {
                if !j.state.status.finished() {
                    j.state.status = if j.lease.as_ref().is_some_and(|l| l.started) {
                        JobStatus::Uncertain
                    } else {
                        JobStatus::Cancelled
                    };
                    j.lease = None;
                }
            }
            s.event(id, now, json!({"type":"deleted"}))
        })
        .await
    }
    async fn versions(&self, id: &str, after: u64, limit: u64) -> Result<Page<Version>> {
        let (_, state) = self.state().await?;
        let p = page(state.document(id)?.versions.keys().copied(), after, limit)?;
        let mut items = vec![];
        for n in p.items {
            items.push(self.version(id, n).await?);
        }
        Ok(Page {
            items,
            next: p.next,
        })
    }
    async fn version(&self, id: &str, number: u64) -> Result<Version> {
        let (_, state) = self.state().await?;
        let object = state
            .document(id)?
            .versions
            .get(&number)
            .ok_or(Error::NotFound)?;
        let bytes = self.backend.get(&self.namespace, object, None).await?;
        if sha(&bytes) != *object {
            return Err(Error::Corrupt);
        }
        let v: Version = serde_json::from_slice(&bytes)?;
        if v.document != id || v.number != number || digest(&v.content) != v.content_sha256 {
            return Err(Error::Corrupt);
        }
        Ok(v)
    }
    async fn put_artifact(
        &self,
        input: ArtifactInput,
        bytes: Vec<u8>,
        max_bytes: u64,
        now: u64,
    ) -> Result<Artifact> {
        require(
            !bytes.is_empty() && bytes.len() as u64 <= max_bytes.min(128 * 1024 * 1024),
            "Choose a smaller file with content.",
        )?;
        text(&input.media_type, 128)?;
        require(
            input.class != AssetClass::PrivateLocal,
            "Keep this file on its computer.",
        )?;
        let version = self
            .version(&input.version.document, input.version.version)
            .await?;
        let hash = sha(&bytes);
        let no_training = input.no_training
            || input.class == AssetClass::NoTraining
            || version
                .assets
                .iter()
                .any(|a| a.class == AssetClass::NoTraining);
        let id = digest(
            &json!({"version": input.version,"sha256":hash,"role":input.role,"media":input.media_type,"class":input.class,"no_training":no_training}),
        );
        let artifact = Artifact {
            id: id.clone(),
            version: input.version,
            role: input.role,
            media_type: input.media_type,
            sha256: hash.clone(),
            size: bytes.len() as u64,
            class: input.class,
            no_training,
        };
        self.backend.put(&self.namespace, &hash, bytes).await?;
        self.update(|s| {
            s.document(&artifact.version.document)?;
            if s.artifacts.insert(id.clone(), artifact.clone()).is_none() {
                s.event(
                    &artifact.version.document,
                    now,
                    json!({"type":"artifact","id":id}),
                )?;
            }
            Ok(artifact.clone())
        })
        .await
    }
    async fn artifact(&self, id: &str) -> Result<Artifact> {
        let (_, s) = self.state().await?;
        let artifact = s.artifacts.get(id).ok_or(Error::NotFound)?;
        s.document(&artifact.version.document)?;
        Ok(artifact.clone())
    }
    async fn artifact_bytes(&self, id: &str, range: Option<Range<u64>>) -> Result<Vec<u8>> {
        let a = self.artifact(id).await?;
        if let Some(r) = &range {
            require(
                r.start < r.end && r.end <= a.size,
                "Choose a byte range within the file.",
            )?;
        }
        let bytes = self
            .backend
            .get(&self.namespace, &a.sha256, range.clone())
            .await?;
        if range.is_none() && (sha(&bytes) != a.sha256 || bytes.len() as u64 != a.size) {
            return Err(Error::Corrupt);
        }
        Ok(bytes)
    }
    async fn artifact_url(&self, id: &str, expires_seconds: u32) -> Result<Option<String>> {
        require(
            (1..=900).contains(&expires_seconds),
            "Choose a link lifetime of up to 15 minutes.",
        )?;
        let a = self.artifact(id).await?;
        self.backend
            .signed_url(&self.namespace, &a.sha256, expires_seconds)
            .await
    }
    async fn create_job(
        &self,
        spec: JobSpec,
        manifest: &KindManifest,
        now: u64,
    ) -> Result<JobState> {
        let v = self
            .version(&spec.version.document, spec.version.version)
            .await?;
        require(
            spec.kind == manifest.kind && manifest.job_types.contains(&spec.job_type),
            "Choose work this lab supports.",
        )?;
        require(
            canonical_json(&spec.params).len() as u64 <= manifest.limits.max_args_bytes,
            "These job settings are too large.",
        )?;
        require(
            !v.has_private_local() || spec.requirements.private_local,
            "Run this work on a computer that has its files.",
        )?;
        let id = digest(&json!({"version":spec.version, "key":spec.key(&v.content_sha256)}));
        self.update(|s| {
            let d = s.document(&spec.version.document)?;
            require(
                d.meta.kind == spec.kind,
                "Choose work from this document's lab.",
            )?;
            if let Some(job) = s.jobs.get(&id) {
                require(
                    job.state.spec == spec,
                    "Use the same settings when retrying a job.",
                )?;
                return Ok(job.state.clone());
            }
            if s.jobs
                .values()
                .filter(|j| {
                    j.state.spec.kind == spec.kind && now.saturating_sub(j.created) < 3_600_000
                })
                .count() as u64
                >= manifest.limits.runs_per_hour
            {
                return Err(Error::RateLimited);
            }
            let job = JobState {
                id: id.clone(),
                spec: spec.clone(),
                status: JobStatus::Queued,
                message: None,
                estimate_seconds: None,
                artifacts: vec![],
            };
            s.jobs.insert(
                id.clone(),
                state::JobRecord {
                    state: job.clone(),
                    created: now,
                    attempt: 0,
                    lease: None,
                    settled: None,
                },
            );
            s.event(
                &spec.version.document,
                now,
                json!({"type":"job","id":id,"status":"queued"}),
            )?;
            Ok(job)
        })
        .await
    }
    async fn jobs(&self, document: &str, after: u64, limit: u64) -> Result<Page<JobState>> {
        let (_, s) = self.state().await?;
        s.document(document)?;
        page(
            s.jobs
                .values()
                .filter(|j| j.state.spec.version.document == document)
                .map(|j| j.state.clone()),
            after,
            limit,
        )
    }
    async fn version_jobs(
        &self,
        version: &Reference,
        after: u64,
        limit: u64,
    ) -> Result<Page<JobState>> {
        let (_, state) = self.state().await?;
        state.reference(version)?;
        page(
            state
                .jobs
                .values()
                .filter(|j| j.state.spec.version == *version)
                .map(|j| j.state.clone()),
            after,
            limit,
        )
    }
    async fn job(&self, id: &str) -> Result<JobState> {
        let (_, s) = self.state().await?;
        let j = s.jobs.get(id).ok_or(Error::NotFound)?;
        s.document(&j.state.spec.version.document)?;
        Ok(j.state.clone())
    }
    async fn claim(&self, id: &str, worker: &str, now: u64, duration_ms: u64) -> Result<Lease> {
        text(worker, 256)?;
        lease_duration(duration_ms)?;
        self.update(|s| {
            s.expire(now)?;
            let document = s
                .jobs
                .get(id)
                .ok_or(Error::NotFound)?
                .state
                .spec
                .version
                .document
                .clone();
            s.document(&document)?;
            let j = s.jobs.get_mut(id).ok_or(Error::NotFound)?;
            if j.state.status != JobStatus::Queued {
                return Err(Error::Conflict);
            }
            j.attempt = j.attempt.checked_add(1).ok_or(Error::Corrupt)?;
            j.state.status = JobStatus::Running;
            let lease = Lease {
                job: j.state.clone(),
                worker: worker.into(),
                attempt: j.attempt,
                expires_unix_ms: now.checked_add(duration_ms).ok_or(Error::Corrupt)?,
                started: false,
            };
            j.lease = Some(lease.clone());
            s.event(
                &document,
                now,
                json!({"type":"job","id":id,"status":"running"}),
            )?;
            Ok(lease)
        })
        .await
    }
    async fn start(&self, lease: &Lease, now: u64) -> Result<Lease> {
        self.update(|s| {
            let j = s.leased(lease, now)?;
            let l = j.lease.as_mut().ok_or(Error::LeaseLost)?;
            l.started = true;
            Ok(l.clone())
        })
        .await
    }
    async fn heartbeat(&self, lease: &Lease, now: u64, duration_ms: u64) -> Result<Lease> {
        lease_duration(duration_ms)?;
        self.update(|s| {
            let j = s.leased(lease, now)?;
            let l = j.lease.as_mut().ok_or(Error::LeaseLost)?;
            l.expires_unix_ms = now.checked_add(duration_ms).ok_or(Error::Corrupt)?;
            Ok(l.clone())
        })
        .await
    }
    async fn finish(
        &self,
        lease: &Lease,
        status: JobStatus,
        message: Option<String>,
        artifacts: Vec<String>,
        now: u64,
    ) -> Result<JobState> {
        require(
            matches!(
                status,
                JobStatus::Done | JobStatus::Failed | JobStatus::Uncertain
            ),
            "Choose a finished job result.",
        )?;
        if let Some(m) = &message {
            text(m, 2048)?;
        }
        require(artifacts.len() <= 1024, "This job has too many outputs.")?;
        self.update(|s| {
            if let Some(j) = s.jobs.get(&lease.job.id) {
                if j.state.status == status
                    && j.state.message == message
                    && j.state.artifacts == artifacts
                    && j.settled
                        .as_ref()
                        .is_some_and(|l| l.worker == lease.worker && l.attempt == lease.attempt)
                {
                    s.document(&j.state.spec.version.document)?;
                    return Ok(j.state.clone());
                }
            }
            let version = s.leased(lease, now)?.state.spec.version.clone();
            for id in &artifacts {
                require(
                    s.artifacts.get(id).is_some_and(|a| a.version == version),
                    "Choose outputs from this job's version.",
                )?;
            }
            let j = s.leased(lease, now)?;
            require(
                j.lease.as_ref().is_some_and(|l| l.started),
                "Start the job before reporting its result.",
            )?;
            j.state.status = status;
            j.state.message = message.clone();
            j.state.artifacts = artifacts.clone();
            j.settled = j.lease.take();
            let result = j.state.clone();
            s.event(
                &version.document,
                now,
                json!({"type":"job","id":lease.job.id,"status":status}),
            )?;
            Ok(result)
        })
        .await
    }
    async fn cancel(&self, id: &str, now: u64) -> Result<JobState> {
        self.update(|s| {
            let document = s
                .jobs
                .get(id)
                .ok_or(Error::NotFound)?
                .state
                .spec
                .version
                .document
                .clone();
            s.document(&document)?;
            let j = s.jobs.get_mut(id).ok_or(Error::NotFound)?;
            if !j.state.status.finished() {
                j.state.status = if j.lease.as_ref().is_some_and(|l| l.started)
                    || j.state.status == JobStatus::Uncertain
                {
                    JobStatus::Uncertain
                } else {
                    JobStatus::Cancelled
                };
                j.lease = None;
                let status = j.state.status;
                let result = j.state.clone();
                s.event(
                    &document,
                    now,
                    json!({"type":"job","id":id,"status":status}),
                )?;
                Ok(result)
            } else {
                Ok(j.state.clone())
            }
        })
        .await
    }
    async fn expire_leases(&self, now: u64) -> Result<usize> {
        self.update(|s| s.expire(now)).await
    }
    async fn add_note(&self, note: Note) -> Result<()> {
        text(&note.text, 8192)?;
        text(&note.author, 256)?;
        match &note.locator {
            Locator::Time { seconds } => require(
                seconds.is_finite() && *seconds >= 0.0,
                "Choose a valid time.",
            )?,
            Locator::Region { x, y, w, h } => require(
                [x, y, w, h].iter().all(|v| v.is_finite() && **v >= 0.0) && *w > 0.0 && *h > 0.0,
                "Choose a valid region.",
            )?,
            Locator::Ref { reference } => text(reference, 1024)?,
            _ => (),
        }
        self.update(|s| {
            s.reference(&note.subject)?;
            require(
                s.notes.len() < 100_000,
                "This space has reached its note limit.",
            )?;
            s.notes.push(note.clone());
            s.event(
                &note.subject.document,
                note.created_unix_ms,
                json!({"type":"note"}),
            )
        })
        .await
    }
    async fn notes(&self, document: &str, after: u64, limit: u64) -> Result<Page<Note>> {
        let (_, s) = self.state().await?;
        s.document(document)?;
        page(
            s.notes
                .iter()
                .filter(|n| n.subject.document == document)
                .cloned(),
            after,
            limit,
        )
    }
    async fn events(&self, document: &str, since: u64, limit: u64) -> Result<Page<Event>> {
        require(
            (1..=100).contains(&limit),
            "Choose a page size between 1 and 100.",
        )?;
        let (_, s) = self.state().await?;
        s.document(document)?;
        let mut items: Vec<_> = s
            .events
            .iter()
            .filter(|e| e.document == document && e.cursor > since)
            .take(limit as usize + 1)
            .cloned()
            .collect();
        let more = items.len() > limit as usize;
        if more {
            items.pop();
        }
        let next = more.then(|| items.last().map(|e| e.cursor)).flatten();
        Ok(Page { items, next })
    }
}
fn lease_duration(duration_ms: u64) -> Result<()> {
    require(
        (1..=300_000).contains(&duration_ms),
        "Choose a job lease of up to five minutes.",
    )
}
