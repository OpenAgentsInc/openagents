//! Atomic space indexes and immutable, space-scoped objects.
use crate::{Error, Result, require, sha, valid_sha};
use async_trait::async_trait;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};
use tempfile::NamedTempFile;

pub const MAX_INDEX_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_OBJECT_BYTES: usize = 128 * 1024 * 1024;

#[async_trait]
pub trait Backend: Send + Sync {
    async fn load(&self, space: &str) -> Result<(u64, Option<Vec<u8>>)>;
    async fn compare_and_swap(&self, space: &str, generation: u64, bytes: Vec<u8>) -> Result<bool>;
    async fn put(&self, space: &str, hash: &str, bytes: Vec<u8>) -> Result<()>;
    async fn get(&self, space: &str, hash: &str, range: Option<Range<u64>>) -> Result<Vec<u8>>;
    async fn signed_url(
        &self,
        _space: &str,
        _hash: &str,
        _expires_seconds: u32,
    ) -> Result<Option<String>> {
        Ok(None)
    }
}
fn names(space: &str, hash: Option<&str>) -> Result<()> {
    require(
        valid_sha(space) && hash.is_none_or(valid_sha),
        "Choose a valid storage address.",
    )
}
fn regular(path: &Path) -> Result<()> {
    require(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "Choose a lab folder with ordinary files.",
    )
}
fn directory(path: &Path) -> Result<()> {
    if path.exists() || fs::symlink_metadata(path).is_ok() {
        require(
            fs::symlink_metadata(path)?.file_type().is_dir(),
            "Choose a lab folder with ordinary subfolders.",
        )?;
    } else {
        fs::create_dir(path)?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn read(path: &Path, max: usize) -> Result<Vec<u8>> {
    regular(path)?;
    let f = File::open(path)?;
    require(
        f.metadata()?.len() <= max as u64,
        "This saved file is too large.",
    )?;
    let mut bytes = vec![];
    f.take(max as u64 + 1).read_to_end(&mut bytes)?;
    require(bytes.len() <= max, "This saved file is too large.")?;
    Ok(bytes)
}
fn replace(path: &Path, bytes: &[u8]) -> Result<()> {
    if fs::symlink_metadata(path).is_ok() {
        regular(path)?;
    }
    let parent = path.parent().ok_or(Error::Corrupt)?;
    let mut f = NamedTempFile::new_in(parent)?;
    f.write_all(bytes)?;
    f.as_file().sync_all()?;
    f.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct Index {
    generation: u64,
    state: Value,
}

#[derive(Clone)]
pub struct FsBackend {
    root: PathBuf,
}
impl FsBackend {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        require(
            root.is_absolute(),
            "Choose an absolute folder for the lab's files.",
        )?;
        if fs::symlink_metadata(root).is_ok() {
            directory(root)?;
        } else {
            fs::create_dir_all(root)?;
            directory(root)?;
        }
        let root = fs::canonicalize(root)?;
        require(
            !root.ancestors().any(|p| p.join(".git").exists()),
            "Choose a lab folder outside the Git checkout.",
        )?;
        Ok(Self { root })
    }
    fn folder(&self, space: &str) -> Result<PathBuf> {
        names(space, None)?;
        let p = self.root.join(space);
        directory(&p)?;
        Ok(p)
    }
    fn locked(&self) -> Result<File> {
        let path = self.root.join("write.lock");
        if fs::symlink_metadata(&path).is_ok() {
            regular(&path)?;
        }
        let f = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        FileExt::lock_exclusive(&f)?;
        Ok(f)
    }
    fn index(&self, space: &str) -> Result<Option<Index>> {
        let path = self.folder(space)?.join("index.json");
        if fs::symlink_metadata(&path).is_err() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&read(
            &path,
            MAX_INDEX_BYTES + 1024,
        )?)?))
    }
    async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(Self) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let this = self.clone();
        tokio::task::spawn_blocking(move || f(this))
            .await
            .map_err(Error::storage)?
    }
}
#[async_trait]
impl Backend for FsBackend {
    async fn load(&self, space: &str) -> Result<(u64, Option<Vec<u8>>)> {
        let space = space.to_owned();
        self.blocking(move |this| {
            let _lock = this.locked()?;
            match this.index(&space)? {
                Some(i) => Ok((i.generation, Some(serde_json::to_vec(&i.state)?))),
                None => Ok((0, None)),
            }
        })
        .await
    }
    async fn compare_and_swap(&self, space: &str, generation: u64, bytes: Vec<u8>) -> Result<bool> {
        let space = space.to_owned();
        require(bytes.len() <= MAX_INDEX_BYTES, "This lab space is full.")?;
        self.blocking(move |this| {
            let _lock = this.locked()?;
            let current = this.index(&space)?.map_or(0, |i| i.generation);
            if current != generation {
                return Ok(false);
            }
            let i = Index {
                generation: generation.checked_add(1).ok_or(Error::Corrupt)?,
                state: serde_json::from_slice(&bytes)?,
            };
            replace(
                &this.folder(&space)?.join("index.json"),
                &serde_json::to_vec(&i)?,
            )?;
            Ok(true)
        })
        .await
    }
    async fn put(&self, space: &str, hash: &str, bytes: Vec<u8>) -> Result<()> {
        names(space, Some(hash))?;
        require(
            bytes.len() <= MAX_OBJECT_BYTES && sha(&bytes) == hash,
            "Choose a smaller file with the expected checksum.",
        )?;
        let space = space.to_owned();
        let hash = hash.to_owned();
        self.blocking(move |this| {
            let _lock = this.locked()?;
            let folder = this.folder(&space)?.join("objects");
            directory(&folder)?;
            let path = folder.join(&hash);
            if fs::symlink_metadata(&path).is_ok() {
                if sha(&read(&path, MAX_OBJECT_BYTES)?) != hash {
                    return Err(Error::Corrupt);
                }
                return Ok(());
            }
            replace(&path, &bytes)
        })
        .await
    }
    async fn get(&self, space: &str, hash: &str, range: Option<Range<u64>>) -> Result<Vec<u8>> {
        names(space, Some(hash))?;
        let space = space.to_owned();
        let hash = hash.to_owned();
        self.blocking(move |this| {
            let _lock = this.locked()?;
            let folder = this.folder(&space)?.join("objects");
            directory(&folder)?;
            let bytes = read(&folder.join(hash), MAX_OBJECT_BYTES)?;
            match range {
                Some(r) => {
                    require(
                        r.start < r.end && r.end <= bytes.len() as u64,
                        "Choose a byte range within the file.",
                    )?;
                    Ok(bytes[r.start as usize..r.end as usize].to_vec())
                }
                None => Ok(bytes),
            }
        })
        .await
    }
}

/// Object implementations must make `put` create-only and verify an existing object.
#[async_trait]
pub trait Objects: Send + Sync {
    async fn put(&self, space: &str, hash: &str, bytes: Vec<u8>) -> Result<()>;
    async fn get(&self, space: &str, hash: &str, range: Option<Range<u64>>) -> Result<Vec<u8>>;
    async fn signed_url(
        &self,
        _space: &str,
        _hash: &str,
        _expires_seconds: u32,
    ) -> Result<Option<String>> {
        Ok(None)
    }
}

pub struct CloudBackend {
    pool: sqlx_postgres::PgPool,
    objects: Arc<dyn Objects>,
}
impl CloudBackend {
    /// `pool` is configured by the host, including its TLS and credential policy.
    pub async fn open(pool: sqlx_postgres::PgPool, objects: Arc<dyn Objects>) -> Result<Self> {
        sqlx_core::raw_sql::raw_sql(include_str!("../migrations/001_spaces.sql"))
            .execute(&pool)
            .await
            .map_err(Error::storage)?;
        Ok(Self { pool, objects })
    }
}
#[async_trait]
impl Backend for CloudBackend {
    async fn load(&self, space: &str) -> Result<(u64, Option<Vec<u8>>)> {
        names(space, None)?;
        let row: Option<(i64, Value)> = sqlx_core::query_as::query_as(
            "SELECT generation, state FROM lab_space_indexes WHERE space = $1",
        )
        .bind(space)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::storage)?;
        match row {
            Some((g, v)) => {
                let bytes = serde_json::to_vec(&v)?;
                if g < 1 || bytes.len() > MAX_INDEX_BYTES {
                    return Err(Error::Corrupt);
                }
                Ok((g as u64, Some(bytes)))
            }
            None => Ok((0, None)),
        }
    }
    async fn compare_and_swap(&self, space: &str, generation: u64, bytes: Vec<u8>) -> Result<bool> {
        names(space, None)?;
        require(
            bytes.len() <= MAX_INDEX_BYTES && generation < i64::MAX as u64,
            "This lab space is full.",
        )?;
        let state: Value = serde_json::from_slice(&bytes)?;
        let done = if generation == 0 {
            sqlx_core::query::query("INSERT INTO lab_space_indexes(space, generation, state) VALUES ($1, 1, $2) ON CONFLICT DO NOTHING").bind(space).bind(state).execute(&self.pool).await
        } else {
            sqlx_core::query::query("UPDATE lab_space_indexes SET generation = generation + 1, state = $3 WHERE space = $1 AND generation = $2").bind(space).bind(generation as i64).bind(state).execute(&self.pool).await
        }.map_err(Error::storage)?;
        Ok(done.rows_affected() == 1)
    }
    async fn put(&self, space: &str, hash: &str, bytes: Vec<u8>) -> Result<()> {
        names(space, Some(hash))?;
        require(
            bytes.len() <= MAX_OBJECT_BYTES && sha(&bytes) == hash,
            "Choose a smaller file with the expected checksum.",
        )?;
        self.objects.put(space, hash, bytes).await
    }
    async fn get(&self, space: &str, hash: &str, range: Option<Range<u64>>) -> Result<Vec<u8>> {
        names(space, Some(hash))?;
        self.objects.get(space, hash, range).await
    }
    async fn signed_url(&self, space: &str, hash: &str, seconds: u32) -> Result<Option<String>> {
        names(space, Some(hash))?;
        self.objects.signed_url(space, hash, seconds).await
    }
}

/// Hosts can use their existing metadata or Secret Manager token provider.
/// Tokens are requested per operation and never persisted by this crate.
#[async_trait]
pub trait Bearer: Send + Sync {
    async fn token(&self) -> Result<String>;
}

pub struct GcsObjects {
    client: reqwest::Client,
    base: reqwest::Url,
    bucket: String,
    bearer: Arc<dyn Bearer>,
    signing_account: Option<String>,
}
impl GcsObjects {
    pub fn new(
        bucket: &str,
        bearer: Arc<dyn Bearer>,
        signing_account: Option<String>,
    ) -> Result<Self> {
        Self::at(
            "https://storage.googleapis.com/",
            bucket,
            bearer,
            signing_account,
        )
    }
    /// An explicit loopback endpoint permits an isolated storage emulator.
    pub fn emulator(base: &str, bucket: &str, bearer: Arc<dyn Bearer>) -> Result<Self> {
        let url = reqwest::Url::parse(base).map_err(Error::storage)?;
        require(
            url.scheme() == "http"
                && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")),
            "Use a loopback storage emulator.",
        )?;
        Self::at(base, bucket, bearer, None)
    }
    fn at(
        base: &str,
        bucket: &str,
        bearer: Arc<dyn Bearer>,
        signing_account: Option<String>,
    ) -> Result<Self> {
        require(
            !bucket.is_empty()
                && bucket.len() <= 222
                && bucket
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.".contains(&b)),
            "Choose a valid storage bucket.",
        )?;
        if let Some(account) = &signing_account {
            crate::text(account, 256)?;
            require(
                account.ends_with(".iam.gserviceaccount.com") && !account.contains('/'),
                "Choose a service account for file links.",
            )?;
        }
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(45))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .map_err(Error::storage)?,
            base: reqwest::Url::parse(base).map_err(Error::storage)?,
            bucket: bucket.into(),
            bearer,
            signing_account,
        })
    }
    fn url(&self, prefix: &str, space: &str, hash: &str) -> Result<reqwest::Url> {
        names(space, Some(hash))?;
        let mut url = self.base.clone();
        {
            let mut path = url.path_segments_mut().map_err(|_| Error::Corrupt)?;
            path.clear();
            for segment in prefix.split('/') {
                path.push(segment);
            }
            path.push("b").push(&self.bucket).push("o");
            if prefix == "download/storage/v1" {
                path.push(&format!("labs/{space}/{hash}"));
            }
        }
        Ok(url)
    }
    async fn response_bytes(mut response: reqwest::Response, limit: usize) -> Result<Vec<u8>> {
        require(
            response.content_length().is_none_or(|n| n <= limit as u64),
            "This saved file is too large.",
        )?;
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(Error::storage)? {
            require(
                bytes.len().saturating_add(chunk.len()) <= limit,
                "This saved file is too large.",
            )?;
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}
#[async_trait]
impl Objects for GcsObjects {
    async fn put(&self, space: &str, hash: &str, bytes: Vec<u8>) -> Result<()> {
        names(space, Some(hash))?;
        require(
            bytes.len() <= MAX_OBJECT_BYTES && sha(&bytes) == hash,
            "Choose a smaller file with the expected checksum.",
        )?;
        let mut url = self.url("upload/storage/v1", space, hash)?;
        url.query_pairs_mut()
            .append_pair("uploadType", "media")
            .append_pair("ifGenerationMatch", "0")
            .append_pair("name", &format!("labs/{space}/{hash}"));
        let response = self
            .client
            .post(url)
            .bearer_auth(self.bearer.token().await?)
            .header("Content-Type", "application/octet-stream")
            .body(bytes)
            .send()
            .await
            .map_err(Error::storage)?;
        if response.status() == reqwest::StatusCode::PRECONDITION_FAILED {
            let old = self.get(space, hash, None).await?;
            if sha(&old) != hash {
                return Err(Error::Corrupt);
            }
            return Ok(());
        }
        response.error_for_status().map_err(Error::storage)?;
        Ok(())
    }
    async fn get(&self, space: &str, hash: &str, range: Option<Range<u64>>) -> Result<Vec<u8>> {
        let mut url = self.url("download/storage/v1", space, hash)?;
        url.query_pairs_mut().append_pair("alt", "media");
        let mut request = self.client.get(url).bearer_auth(self.bearer.token().await?);
        if let Some(r) = &range {
            require(
                r.start < r.end && r.end - r.start <= MAX_OBJECT_BYTES as u64,
                "Choose a byte range within the file.",
            )?;
            request = request.header("Range", format!("bytes={}-{}", r.start, r.end - 1));
        }
        let response = request
            .send()
            .await
            .map_err(Error::storage)?
            .error_for_status()
            .map_err(Error::storage)?;
        if range.is_some() && response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(Error::Corrupt);
        }
        let limit = range
            .as_ref()
            .map_or(MAX_OBJECT_BYTES, |r| (r.end - r.start) as usize);
        let bytes = Self::response_bytes(response, limit).await?;
        if range
            .as_ref()
            .is_some_and(|r| bytes.len() as u64 != r.end - r.start)
            || (range.is_none() && sha(&bytes) != hash)
        {
            return Err(Error::Corrupt);
        }
        Ok(bytes)
    }
    async fn signed_url(&self, space: &str, hash: &str, seconds: u32) -> Result<Option<String>> {
        use base64::Engine;
        names(space, Some(hash))?;
        require(
            (1..=900).contains(&seconds),
            "Choose a link lifetime of up to 15 minutes.",
        )?;
        let Some(account) = &self.signing_account else {
            return Ok(None);
        };
        let expires = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(Error::storage)?
            .as_secs()
            + seconds as u64;
        let resource = format!("/{}/labs/{space}/{hash}", self.bucket);
        let payload = base64::engine::general_purpose::STANDARD
            .encode(format!("GET\n\n\n{expires}\n{resource}"));
        let response = self.client.post(format!("https://iamcredentials.googleapis.com/v1/projects/-/serviceAccounts/{account}:signBlob")).bearer_auth(self.bearer.token().await?).json(&serde_json::json!({"payload":payload})).send().await.map_err(Error::storage)?.error_for_status().map_err(Error::storage)?;
        let response: Value =
            serde_json::from_slice(&Self::response_bytes(response, 16384).await?)?;
        let signature = response
            .get("signedBlob")
            .and_then(Value::as_str)
            .ok_or(Error::Corrupt)?;
        let mut url = reqwest::Url::parse(&format!("https://storage.googleapis.com{resource}"))
            .map_err(Error::storage)?;
        url.query_pairs_mut()
            .append_pair("GoogleAccessId", account)
            .append_pair("Expires", &expires.to_string())
            .append_pair("Signature", signature);
        Ok(Some(url.into()))
    }
}
