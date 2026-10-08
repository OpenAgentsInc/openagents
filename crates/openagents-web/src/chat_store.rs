//! Private public-chat records shared by HTTP commands and event readers.
//!
//! The disk adapter uses an operating-system lock and an atomic rename. The
//! Cloud Storage adapter uses generation preconditions, so another replica
//! cannot replace a record that changed after it was read. Neither adapter
//! treats an HTTP connection as the owner of a running answer.

use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures_util::stream::{self, StreamExt, TryStreamExt};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

const SCHEMA: &str = "openagents.web.chat.v1";
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_LIST: usize = 256;
const STORAGE_API: &str = "https://storage.googleapis.com";
const METADATA_TOKEN: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Conversation {
    pub id: String,
    pub owner: String,
    pub revision: u64,
    pub title: String,
    pub messages: Vec<Message>,
    pub pending: Option<Pending>,
    pub requests: Vec<Request>,
    pub updated_unix: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Message {
    pub role: Role,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Role {
    User,
    Assistant,
    Tool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Pending {
    pub request_id: String,
    pub started_unix: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub job_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Request {
    pub id: String,
    pub digest: String,
    pub outcome: Outcome,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Outcome {
    Pending,
    Answered,
    Failed,
    Unknown,
}

/// A generation identifies the bytes that a caller read, not its authority.
#[derive(Clone, Debug)]
pub(crate) struct Loaded {
    pub conversation: Conversation,
    pub generation: String,
}

#[derive(Debug)]
pub enum Error {
    Conflict,
    Invalid(&'static str),
    Corrupt(&'static str),
    Unavailable(&'static str),
    Http(u16),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => formatter.write_str("The conversation changed. Read it again."),
            Self::Invalid(message) | Self::Corrupt(message) | Self::Unavailable(message) => {
                formatter.write_str(message)
            }
            Self::Http(status) => write!(formatter, "Conversation storage returned HTTP {status}."),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Clone)]
pub struct Store(Arc<Adapter>);

enum Adapter {
    Disk(PathBuf),
    Gcs(Gcs),
}

struct Gcs {
    bucket: String,
    prefix: String,
    client: Client,
    metadata: Client,
    token: Mutex<Option<Token>>,
}

struct Token {
    bearer: String,
    expires: Instant,
}

#[derive(Serialize, Deserialize)]
struct Record {
    schema: String,
    conversation: Conversation,
}

#[derive(Serialize, Deserialize)]
struct Active {
    schema: String,
    request_id: String,
    expires_unix: u64,
}

impl Store {
    /// Disk storage is suitable for a single machine and survives restarts.
    pub fn local(directory: PathBuf) -> Self {
        Self(Arc::new(Adapter::Disk(directory)))
    }

    /// Use a private bucket with object read, create, delete, and list rights.
    pub fn gcs(bucket: String, prefix: String) -> Result<Self, Error> {
        if bucket.is_empty()
            || bucket.len() > 222
            || !bucket.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'-' | b'_')
            })
        {
            return Err(Error::Invalid("The chat bucket name is invalid."));
        }
        let prefix = prefix.trim_matches('/').to_owned();
        if prefix.is_empty()
            || prefix.len() > 128
            || prefix
                .split('/')
                .any(|part| part.is_empty() || part == "..")
            || !prefix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_'))
        {
            return Err(Error::Invalid("The chat object prefix is invalid."));
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Unavailable("Conversation storage could not initialize."))?;
        let metadata = Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error::Unavailable("Cloud identity could not initialize."))?;
        Ok(Self(Arc::new(Adapter::Gcs(Gcs {
            bucket,
            prefix,
            client,
            metadata,
            token: Mutex::new(None),
        }))))
    }

    pub(crate) async fn load(&self, owner: &str, id: &str) -> Result<Option<Loaded>, Error> {
        validate_address(owner, id)?;
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let path = path(root, owner, id);
                let owner = owner.to_owned();
                let id = id.to_owned();
                blocking(move || read_disk(&path, &owner, &id)).await
            }
            Adapter::Gcs(gcs) => gcs.load(owner, id).await,
        }
    }

    /// Create succeeds once, including when two replicas race to create it.
    pub(crate) async fn create(&self, conversation: &Conversation) -> Result<Loaded, Error> {
        validate_address(&conversation.owner, &conversation.id)?;
        if conversation.revision != 1 {
            return Err(Error::Invalid("A new conversation must have revision 1."));
        }
        self.write(conversation, None).await
    }

    /// A mutation must preserve identity and advance the revision once.
    pub(crate) async fn compare_and_swap(
        &self,
        previous: &Loaded,
        conversation: &Conversation,
    ) -> Result<Loaded, Error> {
        validate_address(&conversation.owner, &conversation.id)?;
        if previous.conversation.owner != conversation.owner
            || previous.conversation.id != conversation.id
            || previous.conversation.revision.checked_add(1) != Some(conversation.revision)
        {
            return Err(Error::Invalid("The conversation mutation is invalid."));
        }
        self.write(conversation, Some(previous.generation.clone()))
            .await
    }

    async fn write(
        &self,
        conversation: &Conversation,
        previous: Option<String>,
    ) -> Result<Loaded, Error> {
        let bytes = encode(conversation)?;
        let generation = match self.0.as_ref() {
            Adapter::Disk(root) => {
                let path = path(root, &conversation.owner, &conversation.id);
                let owner = conversation.owner.clone();
                let id = conversation.id.clone();
                blocking(move || write_disk(&path, &owner, &id, &bytes, previous.as_deref()))
                    .await?
            }
            Adapter::Gcs(gcs) => {
                gcs.write(conversation, bytes, previous.as_deref().unwrap_or("0"))
                    .await?
            }
        };
        Ok(Loaded {
            conversation: conversation.clone(),
            generation,
        })
    }

    /// The list is private to one visitor. Fail explicitly if it exceeds 256.
    pub(crate) async fn list(&self, owner: &str) -> Result<Vec<Conversation>, Error> {
        validate_owner(owner)?;
        let ids = match self.0.as_ref() {
            Adapter::Disk(root) => {
                let directory = root.join(owner_digest(owner));
                blocking(move || {
                    let entries = match fs::read_dir(directory) {
                        Ok(entries) => entries,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            return Ok(Vec::new());
                        }
                        Err(_) => {
                            return Err(Error::Unavailable("The chat list could not be read."));
                        }
                    };
                    let mut ids = Vec::new();
                    for entry in entries {
                        let entry = entry
                            .map_err(|_| Error::Unavailable("The chat list could not be read."))?;
                        let name = entry.file_name();
                        let Some(id) = name.to_str().and_then(|name| name.strip_suffix(".json"))
                        else {
                            continue;
                        };
                        if valid_id(id) {
                            ids.push(id.to_owned());
                            if ids.len() > MAX_LIST {
                                return Err(Error::Unavailable("The chat list exceeds its limit."));
                            }
                        }
                    }
                    Ok(ids)
                })
                .await?
            }
            Adapter::Gcs(gcs) => gcs.list(owner).await?,
        };
        let mut conversations: Vec<Conversation> = stream::iter(ids)
            .map(|id| async move { self.load(owner, &id).await })
            .buffer_unordered(8)
            .try_filter_map(|loaded| async { Ok(loaded.map(|loaded| loaded.conversation)) })
            .try_collect()
            .await?;
        conversations.sort_by(|left, right| {
            right
                .updated_unix
                .cmp(&left.updated_unix)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(conversations)
    }

    /// Claim one answer per visitor across processes and replicas.
    ///
    /// A duplicate claim returns false, including a matching request identity.
    /// An expired claim can be replaced, but the old answer remains unknown
    /// until its conversation records that outcome. Do not replay that answer.
    pub(crate) async fn claim(
        &self,
        owner: &str,
        request_id: &str,
        expires_unix: u64,
    ) -> Result<bool, Error> {
        validate_address(owner, request_id)?;
        let now = now_unix();
        if expires_unix <= now || expires_unix > now.saturating_add(3600) {
            return Err(Error::Invalid("The answer lease expiration is invalid."));
        }
        let active = Active {
            schema: format!("{SCHEMA}.active"),
            request_id: request_id.to_owned(),
            expires_unix,
        };
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let directory = root.join(owner_digest(owner));
                blocking(move || {
                    create_directory(&directory)?;
                    let path = directory.join(".active.json");
                    let _lock = lock(&directory.join(".active.lock"))?;
                    if read_active_disk(&path)?.is_some_and(|claim| claim.expires_unix > now) {
                        return Ok(false);
                    }
                    let bytes = active_bytes(&active)?;
                    atomic_write(&path, &bytes)?;
                    Ok(true)
                })
                .await
            }
            Adapter::Gcs(gcs) => {
                let name = format!("{}.active.json", gcs.owner_prefix(owner));
                for _ in 0..4 {
                    let loaded = gcs.read_object(&name).await?;
                    let generation = match loaded {
                        Some((bytes, generation)) => {
                            if decode_active(&bytes)?.expires_unix > now {
                                return Ok(false);
                            }
                            generation
                        }
                        None => "0".to_owned(),
                    };
                    match gcs
                        .put_object(&name, active_bytes(&active)?, &generation)
                        .await
                    {
                        Ok(_) => return Ok(true),
                        Err(Error::Conflict) => continue,
                        Err(error) => return Err(error),
                    }
                }
                Err(Error::Conflict)
            }
        }
    }

    /// A completed answer can release its own lease, never a newer answer's.
    pub(crate) async fn release(&self, owner: &str, request_id: &str) -> Result<(), Error> {
        validate_address(owner, request_id)?;
        match self.0.as_ref() {
            Adapter::Disk(root) => {
                let directory = root.join(owner_digest(owner));
                let request_id = request_id.to_owned();
                blocking(move || {
                    create_directory(&directory)?;
                    let path = directory.join(".active.json");
                    let _lock = lock(&directory.join(".active.lock"))?;
                    let Some(mut active) = read_active_disk(&path)? else {
                        return Ok(());
                    };
                    if active.request_id == request_id {
                        active.request_id.clear();
                        active.expires_unix = 0;
                        atomic_write(&path, &active_bytes(&active)?)?;
                    }
                    Ok(())
                })
                .await
            }
            Adapter::Gcs(gcs) => {
                let name = format!("{}.active.json", gcs.owner_prefix(owner));
                for _ in 0..4 {
                    let Some((bytes, generation)) = gcs.read_object(&name).await? else {
                        return Ok(());
                    };
                    let mut active = decode_active(&bytes)?;
                    if active.request_id != request_id {
                        return Ok(());
                    }
                    active.request_id.clear();
                    active.expires_unix = 0;
                    match gcs
                        .put_object(&name, active_bytes(&active)?, &generation)
                        .await
                    {
                        Ok(_) => return Ok(()),
                        Err(Error::Conflict) => continue,
                        Err(error) => return Err(error),
                    }
                }
                Err(Error::Conflict)
            }
        }
    }
}

impl Gcs {
    async fn bearer(&self) -> Result<String, Error> {
        // Only credential refresh holds this lock. Record reads and CAS do not.
        let mut cached = self.token.lock().await;
        if let Some(token) = cached.as_ref()
            && token.expires > Instant::now()
        {
            return Ok(token.bearer.clone());
        }
        let response = self
            .metadata
            .get(METADATA_TOKEN)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|_| Error::Unavailable("The cloud identity is unavailable."))?;
        check_status(&response)?;
        #[derive(Deserialize)]
        struct Credentials {
            access_token: String,
            expires_in: u64,
        }
        let bytes = limited_body(response, 16 * 1024).await?;
        let credentials: Credentials = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Unavailable("The cloud identity response is invalid."))?;
        if credentials.access_token.is_empty() || credentials.expires_in <= 60 {
            return Err(Error::Unavailable("The cloud identity has expired."));
        }
        let bearer = credentials.access_token;
        *cached = Some(Token {
            bearer: bearer.clone(),
            expires: Instant::now()
                + Duration::from_secs(credentials.expires_in.saturating_sub(60).min(3600)),
        });
        Ok(bearer)
    }

    fn object(&self, owner: &str, id: &str) -> String {
        format!("{}{id}.json", self.owner_prefix(owner))
    }

    fn owner_prefix(&self, owner: &str) -> String {
        format!("{}/{}/", self.prefix, owner_digest(owner))
    }

    fn object_url(&self, object: &str) -> Result<url::Url, Error> {
        let mut url = url::Url::parse(STORAGE_API)
            .map_err(|_| Error::Unavailable("The storage endpoint is invalid."))?;
        url.path_segments_mut()
            .map_err(|_| Error::Unavailable("The storage endpoint is invalid."))?
            .extend(["storage", "v1", "b", &self.bucket, "o", object]);
        Ok(url)
    }

    async fn load(&self, owner: &str, id: &str) -> Result<Option<Loaded>, Error> {
        let Some((bytes, generation)) = self.read_object(&self.object(owner, id)).await? else {
            return Ok(None);
        };
        Ok(Some(Loaded {
            conversation: decode(&bytes, owner, id)?,
            generation,
        }))
    }

    async fn read_object(&self, name: &str) -> Result<Option<(Vec<u8>, String)>, Error> {
        for attempt in 0..3 {
            match self.read_object_once(name).await {
                Err(Error::Conflict) if attempt < 2 => {
                    // A bucket without versioning can replace the generation
                    // between metadata and media reads. Read the new pair;
                    // never relabel newer bytes with the older generation.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                result => return result,
            }
        }
        Err(Error::Conflict)
    }

    async fn read_object_once(&self, name: &str) -> Result<Option<(Vec<u8>, String)>, Error> {
        let bearer = self.bearer().await?;
        let url = self.object_url(name)?;
        let response = self
            .client
            .get(url.clone())
            .query(&[("fields", "generation")])
            .bearer_auth(&bearer)
            .send()
            .await
            .map_err(|_| Error::Unavailable("The conversation could not be read."))?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        check_status(&response)?;
        #[derive(Deserialize)]
        struct Object {
            generation: String,
        }
        let object: Object = serde_json::from_slice(&limited_body(response, 16 * 1024).await?)
            .map_err(|_| Error::Corrupt("The object generation is invalid."))?;
        if object.generation.is_empty()
            || !object.generation.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(Error::Corrupt("The object generation is invalid."));
        }
        let response = self
            .client
            .get(url)
            .query(&[("alt", "media"), ("generation", object.generation.as_str())])
            .bearer_auth(&bearer)
            .send()
            .await
            .map_err(|_| Error::Unavailable("The conversation could not be read."))?;
        // A generation replaced between metadata and media reads is a conflict,
        // not permission to return a newer record under the older generation.
        if response.status() == StatusCode::NOT_FOUND {
            return Err(Error::Conflict);
        }
        check_status(&response)?;
        let bytes = limited_body(response, MAX_RECORD_BYTES).await?;
        Ok(Some((bytes, object.generation)))
    }

    async fn write(
        &self,
        conversation: &Conversation,
        bytes: Vec<u8>,
        generation: &str,
    ) -> Result<String, Error> {
        self.put_object(
            &self.object(&conversation.owner, &conversation.id),
            bytes,
            generation,
        )
        .await
    }

    async fn put_object(
        &self,
        name: &str,
        bytes: Vec<u8>,
        generation: &str,
    ) -> Result<String, Error> {
        if generation.is_empty() || !generation.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::Invalid("The object generation is invalid."));
        }
        let response = self
            .client
            .post(format!(
                "{STORAGE_API}/upload/storage/v1/b/{}/o",
                self.bucket
            ))
            .query(&[
                ("uploadType", "media"),
                ("name", name),
                ("ifGenerationMatch", generation),
                ("fields", "generation"),
            ])
            .bearer_auth(self.bearer().await?)
            .header("Content-Type", "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|_| Error::Unavailable("The conversation write has an unknown outcome."))?;
        if response.status() == StatusCode::PRECONDITION_FAILED
            || response.status() == StatusCode::CONFLICT
        {
            return Err(Error::Conflict);
        }
        check_status(&response)?;
        #[derive(Deserialize)]
        struct Object {
            generation: String,
        }
        let object: Object = serde_json::from_slice(&limited_body(response, 16 * 1024).await?)
            .map_err(|_| Error::Corrupt("The object generation is invalid."))?;
        if object.generation.is_empty()
            || !object.generation.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(Error::Corrupt("The object generation is invalid."));
        }
        Ok(object.generation)
    }

    async fn list(&self, owner: &str) -> Result<Vec<String>, Error> {
        let prefix = self.owner_prefix(owner);
        let bearer = self.bearer().await?;
        let mut page = String::new();
        let mut ids = Vec::new();
        loop {
            let response = self
                .client
                .get(format!("{STORAGE_API}/storage/v1/b/{}/o", self.bucket))
                .query(&[
                    ("prefix", prefix.as_str()),
                    ("maxResults", "257"),
                    ("fields", "items(name),nextPageToken"),
                    ("pageToken", page.as_str()),
                ])
                .bearer_auth(&bearer)
                .send()
                .await
                .map_err(|_| Error::Unavailable("The chat list could not be read."))?;
            check_status(&response)?;
            #[derive(Deserialize)]
            struct Object {
                name: String,
            }
            #[derive(Deserialize)]
            struct Page {
                #[serde(default)]
                items: Vec<Object>,
                #[serde(default, rename = "nextPageToken")]
                next: String,
            }
            let listed: Page = serde_json::from_slice(&limited_body(response, 128 * 1024).await?)
                .map_err(|_| Error::Corrupt("The chat list is invalid."))?;
            for object in listed.items {
                if object.name == format!("{prefix}.active.json") {
                    continue;
                }
                let Some(id) = object
                    .name
                    .strip_prefix(&prefix)
                    .and_then(|name| name.strip_suffix(".json"))
                else {
                    return Err(Error::Corrupt("The chat list contains an invalid object."));
                };
                if !valid_id(id) {
                    return Err(Error::Corrupt("The chat list contains an invalid object."));
                }
                ids.push(id.to_owned());
                if ids.len() > MAX_LIST {
                    return Err(Error::Unavailable("The chat list exceeds its limit."));
                }
            }
            if listed.next.is_empty() {
                return Ok(ids);
            }
            if listed.next == page {
                return Err(Error::Corrupt("The chat list cursor did not advance."));
            }
            page = listed.next;
        }
    }
}

fn check_status(response: &Response) -> Result<(), Error> {
    if response.status().is_success() {
        Ok(())
    } else {
        // Provider response bodies can contain record names. Do not report them.
        Err(Error::Http(response.status().as_u16()))
    }
}

async fn limited_body(mut response: Response, limit: usize) -> Result<Vec<u8>, Error> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(Error::Corrupt(
            "The storage response exceeds its size limit.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Error::Unavailable("The storage response was interrupted."))?
    {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(Error::Corrupt(
                "The storage response exceeds its size limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn encode(conversation: &Conversation) -> Result<Vec<u8>, Error> {
    let bytes = serde_json::to_vec(&Record {
        schema: SCHEMA.to_owned(),
        conversation: conversation.clone(),
    })
    .map_err(|_| Error::Invalid("The conversation could not be encoded."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::Invalid(
            "The conversation exceeds its retained size limit.",
        ));
    }
    Ok(bytes)
}

fn decode(bytes: &[u8], owner: &str, id: &str) -> Result<Conversation, Error> {
    let record: Record = serde_json::from_slice(bytes)
        .map_err(|_| Error::Corrupt("The retained conversation is invalid."))?;
    if record.schema != SCHEMA
        || record.conversation.owner != owner
        || record.conversation.id != id
        || record.conversation.revision == 0
    {
        return Err(Error::Corrupt(
            "The retained conversation identity is invalid.",
        ));
    }
    Ok(record.conversation)
}

async fn blocking<T, F>(operation: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|_| Error::Unavailable("Conversation storage stopped unexpectedly."))?
}

fn path(root: &Path, owner: &str, id: &str) -> PathBuf {
    root.join(owner_digest(owner)).join(format!("{id}.json"))
}

fn read_disk(path: &Path, owner: &str, id: &str) -> Result<Option<Loaded>, Error> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable("The conversation could not be read.")),
    };
    let mut bytes = Vec::new();
    file.take((MAX_RECORD_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable("The conversation could not be read."))?;
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(Error::Corrupt(
            "The retained conversation exceeds its size limit.",
        ));
    }
    Ok(Some(Loaded {
        conversation: decode(&bytes, owner, id)?,
        generation: digest(&bytes),
    }))
}

fn write_disk(
    path: &Path,
    owner: &str,
    id: &str,
    bytes: &[u8],
    previous: Option<&str>,
) -> Result<String, Error> {
    let directory = path
        .parent()
        .ok_or(Error::Invalid("The chat directory is invalid."))?;
    create_directory(directory)?;
    let _lock = lock(&path.with_extension("lock"))?;
    let current = read_disk(path, owner, id)?;
    match (previous, current.as_ref()) {
        (None, None) => {}
        (Some(expected), Some(loaded)) if expected == loaded.generation => {}
        _ => return Err(Error::Conflict),
    }
    atomic_write(path, bytes)
}

fn create_directory(directory: &Path) -> Result<(), Error> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(directory)
        .map_err(|_| Error::Unavailable("The chat directory could not be created."))
}

fn read_active_disk(path: &Path) -> Result<Option<Active>, Error> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Unavailable("The answer lease could not be read.")),
    };
    let mut bytes = Vec::new();
    file.take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Unavailable("The answer lease could not be read."))?;
    if bytes.len() > 4096 {
        return Err(Error::Corrupt(
            "The retained answer lease exceeds its size limit.",
        ));
    }
    Ok(Some(decode_active(&bytes)?))
}

fn active_bytes(active: &Active) -> Result<Vec<u8>, Error> {
    serde_json::to_vec(active).map_err(|_| Error::Invalid("The answer lease is invalid."))
}

fn decode_active(bytes: &[u8]) -> Result<Active, Error> {
    let active: Active = serde_json::from_slice(bytes)
        .map_err(|_| Error::Corrupt("The retained answer lease is invalid."))?;
    if active.schema != format!("{SCHEMA}.active")
        || (!active.request_id.is_empty() && !valid_id(&active.request_id))
        || (active.request_id.is_empty() && active.expires_unix != 0)
    {
        return Err(Error::Corrupt(
            "The retained answer lease identity is invalid.",
        ));
    }
    Ok(active)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<String, Error> {
    let directory = path
        .parent()
        .ok_or(Error::Invalid("The chat directory is invalid."))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(Error::Invalid("The chat file name is invalid."))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| Error::Unavailable("The system clock is unavailable."))?
        .as_nanos();
    let temporary = directory.join(format!(".{name}.{}.{stamp}.tmp", std::process::id()));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|_| Error::Unavailable("The conversation could not be staged."))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::Unavailable("The conversation could not be retained."))?;
        fs::rename(&temporary, path)
            .map_err(|_| Error::Unavailable("The conversation write has an unknown outcome."))?;
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(|_| Error::Unavailable("The conversation write has an unknown outcome."))?;
        Ok(digest(bytes))
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

/// The kernel releases this lock if a process exits before writing a record.
#[cfg(unix)]
fn lock(path: &Path) -> Result<File, Error> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .map_err(|_| Error::Unavailable("The conversation lock could not be opened."))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // SAFETY: The descriptor belongs to the retained File and remains open.
        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 {
            return Ok(file);
        }
        let error = std::io::Error::last_os_error();
        if !matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
        ) || Instant::now() >= deadline
        {
            return Err(Error::Unavailable(
                "The conversation lock could not be acquired.",
            ));
        }
        // This wait runs only in spawn_blocking, never on an async executor.
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(unix))]
fn lock(_path: &Path) -> Result<File, Error> {
    Err(Error::Unavailable(
        "Local conversation storage requires operating-system file locks.",
    ))
}

fn validate_address(owner: &str, id: &str) -> Result<(), Error> {
    validate_owner(owner)?;
    if !valid_id(id) {
        return Err(Error::Invalid("The conversation ID is invalid."));
    }
    Ok(())
}

fn validate_owner(owner: &str) -> Result<(), Error> {
    if owner.len() != 32 || !owner.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::Invalid("The visitor identity is invalid."));
    }
    Ok(())
}

fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

fn owner_digest(owner: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(b"openagents.web.chat.owner.v1\0");
    hash.update(owner.as_bytes());
    format!("{:x}", hash.finalize())
}

pub(crate) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
