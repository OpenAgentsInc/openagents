//! Where the counts are kept: a private Cloud Storage bucket per
//! environment (`OPENAGENTS_WEB_ANALYTICS_BUCKET`), or a directory for a
//! development server (`OPENAGENTS_WEB_ANALYTICS_DIR`).
//!
//! Objects are small JSON files of [`super::Row`]s:
//!
//! - `raw/YYYY-MM-DD/HH/INSTANCE.json`: one server instance's counts for one
//!   hour, rewritten whole at each flush. Only that instance writes it, so
//!   no lock or precondition is needed. Kept 30 days (bucket lifecycle).
//! - `daily/YYYY-MM-DD.json`: the sum of a day's raw files, per hour, made
//!   by [`super::rollup`]. Any instance may make it; the inputs are the same,
//!   so concurrent writers write the same sum. Kept 400 days (13 months).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::Deserialize;
use tokio::sync::Mutex;

const STORAGE_API: &str = "https://storage.googleapis.com";
const METADATA_TOKEN: &str =
    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";
/// The largest object read back.
const MAX_OBJECT: usize = 8 * 1024 * 1024;

/// Why storage failed. Never carries an object's contents.
#[derive(Debug)]
pub struct Error(pub &'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for Error {}

pub enum Store {
    Disk(PathBuf),
    Gcs(Gcs),
}

pub struct Gcs {
    bucket: String,
    client: Client,
    metadata: Client,
    token: Mutex<Option<(String, Instant)>>,
}

/// Whether `name` is an object name this module writes.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() < 200
        && !name.split('/').any(|part| part.is_empty() || part == "..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
}

impl Store {
    pub fn disk(directory: PathBuf) -> Self {
        Self::Disk(directory)
    }

    pub fn gcs(bucket: String) -> Result<Self, Error> {
        if bucket.is_empty()
            || bucket.len() > 222
            || !bucket.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-' | b'_')
            })
        {
            return Err(Error("The analytics bucket name is invalid."));
        }
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| Error("Analytics storage could not start."))?;
        let metadata = Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| Error("Analytics storage could not start."))?;
        Ok(Self::Gcs(Gcs {
            bucket,
            client,
            metadata,
            token: Mutex::new(None),
        }))
    }

    /// Writes `name` whole, replacing what was there.
    pub async fn put(&self, name: &str, bytes: Vec<u8>) -> Result<(), Error> {
        if !valid_name(name) {
            return Err(Error("The analytics object name is invalid."));
        }
        match self {
            Self::Disk(root) => {
                let path = root.join(name);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|_| Error("The analytics directory could not be made."))?;
                }
                let temporary = path.with_extension("json.tmp");
                tokio::fs::write(&temporary, bytes)
                    .await
                    .map_err(|_| Error("The analytics file could not be written."))?;
                tokio::fs::rename(&temporary, &path)
                    .await
                    .map_err(|_| Error("The analytics file could not be written."))
            }
            Self::Gcs(gcs) => gcs.put(name, bytes).await,
        }
    }

    /// The object `name`, or `None` when there is none.
    pub async fn get(&self, name: &str) -> Result<Option<Vec<u8>>, Error> {
        if !valid_name(name) {
            return Err(Error("The analytics object name is invalid."));
        }
        match self {
            Self::Disk(root) => match tokio::fs::read(root.join(name)).await {
                Ok(bytes) => Ok(Some(bytes)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(_) => Err(Error("The analytics file could not be read.")),
            },
            Self::Gcs(gcs) => gcs.get(name).await,
        }
    }

    /// The names of the objects under `prefix` (which ends in `/`).
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>, Error> {
        if !prefix.ends_with('/') || !valid_name(prefix.trim_end_matches('/')) {
            return Err(Error("The analytics prefix is invalid."));
        }
        match self {
            Self::Disk(root) => {
                let mut names = Vec::new();
                let mut pending = vec![prefix.trim_end_matches('/').to_owned()];
                while let Some(folder) = pending.pop() {
                    let Ok(mut entries) = tokio::fs::read_dir(root.join(&folder)).await else {
                        continue;
                    };
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let Some(file) = entry.file_name().to_str().map(str::to_owned) else {
                            continue;
                        };
                        let name = format!("{folder}/{file}");
                        match entry.file_type().await {
                            Ok(kind) if kind.is_dir() => pending.push(name),
                            Ok(_) if file.ends_with(".json") => names.push(name),
                            _ => {}
                        }
                    }
                }
                names.sort();
                Ok(names)
            }
            Self::Gcs(gcs) => gcs.list(prefix).await,
        }
    }
}

impl Gcs {
    async fn bearer(&self) -> Result<String, Error> {
        let mut cached = self.token.lock().await;
        if let Some((token, expires)) = cached.as_ref()
            && *expires > Instant::now()
        {
            return Ok(token.clone());
        }
        let response = self
            .metadata
            .get(METADATA_TOKEN)
            .header("Metadata-Flavor", "Google")
            .send()
            .await
            .map_err(|_| Error("The cloud identity is unavailable."))?;
        if !response.status().is_success() {
            return Err(Error("The cloud identity is unavailable."));
        }
        #[derive(Deserialize)]
        struct Credentials {
            access_token: String,
            expires_in: u64,
        }
        let credentials: Credentials = response
            .json()
            .await
            .map_err(|_| Error("The cloud identity response is invalid."))?;
        if credentials.access_token.is_empty() || credentials.expires_in <= 60 {
            return Err(Error("The cloud identity has expired."));
        }
        let expires = Instant::now()
            + Duration::from_secs(credentials.expires_in.saturating_sub(60).min(3600));
        *cached = Some((credentials.access_token.clone(), expires));
        Ok(credentials.access_token)
    }

    fn object_url(&self, name: &str) -> Result<url::Url, Error> {
        let mut url = url::Url::parse(STORAGE_API).map_err(|_| Error("bad endpoint"))?;
        url.path_segments_mut()
            .map_err(|_| Error("bad endpoint"))?
            .extend(["storage", "v1", "b", &self.bucket, "o", name]);
        Ok(url)
    }

    async fn put(&self, name: &str, bytes: Vec<u8>) -> Result<(), Error> {
        let response = self
            .client
            .post(format!(
                "{STORAGE_API}/upload/storage/v1/b/{}/o",
                self.bucket
            ))
            .query(&[("uploadType", "media"), ("name", name), ("fields", "name")])
            .bearer_auth(self.bearer().await?)
            .header("Content-Type", "application/json")
            .body(bytes)
            .send()
            .await
            .map_err(|_| Error("The analytics write did not finish."))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error("The analytics write was refused."))
        }
    }

    async fn get(&self, name: &str) -> Result<Option<Vec<u8>>, Error> {
        let response = self
            .client
            .get(self.object_url(name)?)
            .query(&[("alt", "media")])
            .bearer_auth(self.bearer().await?)
            .send()
            .await
            .map_err(|_| Error("The analytics read did not finish."))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(Error("The analytics read was refused."));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_OBJECT as u64)
        {
            return Err(Error("An analytics object is too large."));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|_| Error("The analytics read was interrupted."))?;
        if bytes.len() > MAX_OBJECT {
            return Err(Error("An analytics object is too large."));
        }
        Ok(Some(bytes.to_vec()))
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, Error> {
        let bearer = self.bearer().await?;
        let mut names = Vec::new();
        let mut page = String::new();
        loop {
            let response = self
                .client
                .get(format!("{STORAGE_API}/storage/v1/b/{}/o", self.bucket))
                .query(&[
                    ("prefix", prefix),
                    ("maxResults", "1000"),
                    ("fields", "items(name),nextPageToken"),
                    ("pageToken", page.as_str()),
                ])
                .bearer_auth(&bearer)
                .send()
                .await
                .map_err(|_| Error("The analytics list did not finish."))?;
            if !response.status().is_success() {
                return Err(Error("The analytics list was refused."));
            }
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
            let listed: Page = response
                .json()
                .await
                .map_err(|_| Error("The analytics list is invalid."))?;
            names.extend(
                listed
                    .items
                    .into_iter()
                    .map(|o| o.name)
                    .filter(|n| n.ends_with(".json")),
            );
            if listed.next.is_empty() || listed.next == page || names.len() > 100_000 {
                return Ok(names);
            }
            page = listed.next;
        }
    }
}
