use std::{
    io,
    net::IpAddr,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
};

use serde_json::json;
use sha2::{Digest, Sha256};
use tokio::{
    fs::{self, File, OpenOptions},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    net::TcpStream,
    time::timeout,
};

use crate::{
    domain::{parse_http_authorization_claim, parse_http_authorization_hash},
    store::{MediaDeleteOutcome, MediaRecord, StoreError},
};

use super::{
    GatewayConfig, GatewayError, MediaConfig,
    db::DbPool,
    rate::RateLimiter,
    server::unix_now,
    socket::{HttpHead, shutdown_http, write_http_bytes, write_http_head},
};

const MEDIA_IO_TIMEOUT: Duration = Duration::from_secs(30);
const MEDIA_UPLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const STALE_TEMPORARY_AGE: Duration = Duration::from_secs(3_600);
/// How old an unpublished upload registration must be before the sweep
/// releases it. A live upload finishes or fails inside
/// `MEDIA_UPLOAD_TIMEOUT`, so a registration this old belongs to a
/// connection task that was cancelled before it could release, in this
/// process or in one that has since exited.
pub const STALE_RESERVATION_AGE: Duration = Duration::from_secs(3_600);
/// How long a deleted blob's bytes stay under `.deleted/`. A backup that
/// dumps the database and then archives the media root within this window
/// finds every blob the dump references, live or retained. Read
/// `docs/protocol/media.md` before changing it.
pub const DELETED_RETENTION: Duration = Duration::from_secs(48 * 3_600);
const STREAM_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Clone)]
pub struct MediaStorage {
    root: Arc<PathBuf>,
    cloud_base_url: Option<Arc<str>>,
    next_temporary: Arc<AtomicU64>,
}

impl MediaStorage {
    pub async fn prepare(config: &MediaConfig) -> Result<Self, GatewayError> {
        fs::create_dir_all(config.root.join(".tmp")).await?;
        fs::create_dir_all(config.root.join(".deleted")).await?;
        let root = fs::canonicalize(&config.root).await?;
        if !fs::metadata(&root).await?.is_dir() {
            return Err(GatewayError::Config(
                "NOSTR_RELAY_MEDIA_ROOT must name a directory".into(),
            ));
        }
        clean_stale_temporary_files(&root).await?;
        clean_expired_deleted_blobs(&root).await?;
        Ok(Self {
            root: Arc::new(root),
            cloud_base_url: config
                .cloud_base_url
                .as_deref()
                .map(|value| Arc::<str>::from(value.trim_end_matches('/'))),
            next_temporary: Arc::new(AtomicU64::new(1)),
        })
    }

    fn blob_path(&self, record: &MediaRecord) -> PathBuf {
        if self.cloud_base_url.is_some() {
            self.root.join(&record.storage_key).join(format!(
                "{}.{}",
                record.sha256,
                extension_for_media_type(&record.media_type)
            ))
        } else {
            self.root
                .join(&record.sha256[..2])
                .join(format!("{}.{}", record.sha256, record.storage_key))
        }
    }

    /// Remove the bytes of a registration that was released before it
    /// published. Only a blob installed on its way to publication can be
    /// here, so a missing file is not an error.
    pub(super) async fn remove_unpublished_blob(&self, record: &MediaRecord) -> io::Result<()> {
        match fs::remove_file(self.blob_path(record)).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    /// Where a deleted blob's bytes rest until `DELETED_RETENTION` passes.
    fn deleted_path(&self, record: &MediaRecord) -> PathBuf {
        self.root
            .join(".deleted")
            .join(format!("{}.{}", record.sha256, record.storage_key))
    }

    /// Take a blob out of service without destroying its bytes yet: the
    /// file moves to `.deleted/`, and retained files past their window go.
    async fn retire_blob(&self, record: &MediaRecord) -> io::Result<()> {
        let deleted_path = self.deleted_path(record);
        match fs::rename(self.blob_path(record), &deleted_path).await {
            Ok(()) => {
                // The retention window counts from deletion, not upload.
                tokio::task::spawn_blocking(move || {
                    std::fs::File::options()
                        .write(true)
                        .open(deleted_path)?
                        .set_modified(SystemTime::now())
                })
                .await
                .map_err(io::Error::other)??;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        clean_expired_deleted_blobs(&self.root).await
    }

    async fn temporary_file(&self) -> io::Result<(PathBuf, File)> {
        for _ in 0..16 {
            let sequence = self.next_temporary.fetch_add(1, Ordering::Relaxed);
            let path = self
                .root
                .join(".tmp")
                .join(format!("upload-{}-{sequence}", std::process::id()));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
                .await
            {
                Ok(file) => return Ok((path, file)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "unable to allocate media temporary file",
        ))
    }
}

pub fn is_media_request(head: &HttpHead) -> bool {
    head.path == "/upload" || parse_blob_path(&head.path).is_some()
}

pub async fn serve_media(
    mut stream: TcpStream,
    head: &HttpHead,
    config: &GatewayConfig,
    storage: &MediaStorage,
    db: &DbPool,
    rate: &RateLimiter,
    ip: IpAddr,
) -> Result<(), GatewayError> {
    if head.method == "OPTIONS" {
        return write_http_bytes(&mut stream, 204, "No Content", "text/plain", &[], &[]).await;
    }
    if head.path == "/upload" {
        return serve_upload(stream, head, config, storage, db, rate, ip).await;
    }
    let Some(sha256) = parse_blob_path(&head.path) else {
        return media_error(&mut stream, 404, "Not Found", "blob not found").await;
    };
    match head.method.as_str() {
        "GET" | "HEAD" => serve_blob(stream, head, storage, db, &sha256).await,
        "DELETE" => serve_delete(stream, head, config, storage, db, rate, ip, sha256).await,
        _ => media_error(&mut stream, 405, "Method Not Allowed", "method not allowed").await,
    }
}

/// A registration made before the body arrived. Releasing it undoes the
/// uploader's new ownership and an unpublished blob row nobody else owns,
/// so a body that never came or came wrong holds no quota. The guard also
/// holds the temporary file the body streams into: dropping it, including
/// when the connection task is cancelled, removes that file. A registration
/// the process never releases is swept once it is older than
/// [`STALE_RESERVATION_AGE`].
struct Reservation<'a> {
    db: &'a DbPool,
    pubkey: &'a str,
    sha256: &'a str,
    owned_before: bool,
    temporary_path: Option<PathBuf>,
}

impl Reservation<'_> {
    async fn release(&mut self) {
        self.remove_temporary();
        let _ = self
            .db
            .abandon_media(
                self.pubkey.to_owned(),
                self.sha256.to_owned(),
                self.owned_before,
            )
            .await;
    }

    fn remove_temporary(&mut self) {
        if let Some(path) = self.temporary_path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Drop for Reservation<'_> {
    fn drop(&mut self) {
        self.remove_temporary();
    }
}

async fn serve_upload(
    mut stream: TcpStream,
    head: &HttpHead,
    config: &GatewayConfig,
    storage: &MediaStorage,
    db: &DbPool,
    rate: &RateLimiter,
    ip: IpAddr,
) -> Result<(), GatewayError> {
    if head.method != "PUT" {
        return media_error(
            &mut stream,
            405,
            "Method Not Allowed",
            "uploads must use PUT",
        )
        .await;
    }
    if !rate.media_from_ip(ip) {
        return media_error(
            &mut stream,
            429,
            "Too Many Requests",
            "too many media requests; try again later",
        )
        .await;
    }
    let media_config = config
        .media
        .as_ref()
        .ok_or_else(|| GatewayError::Config("media configuration is missing".into()))?;
    let length = match request_content_length(head, media_config.max_blob_bytes) {
        Ok(length) => length,
        Err(message) if message == "upload exceeds configured blob limit" => {
            return media_error(&mut stream, 413, "Payload Too Large", message).await;
        }
        Err(message) => return media_error(&mut stream, 400, "Bad Request", message).await,
    };
    let media_type = match request_media_type(head) {
        Some(media_type) => media_type,
        None => {
            return media_error(
                &mut stream,
                400,
                "Bad Request",
                "Content-Type must be a lowercase MIME type",
            )
            .await;
        }
    };
    // Nothing of the body is read until the request has earned it: the
    // NIP-98 authorization must verify, name this URL and method, claim the
    // body's digest, and fit the uploader's rate and quota. The claimed
    // digest is what the body is held to once it has been read.
    let Some(authorization) = head.header("authorization") else {
        return unauthorized(&mut stream).await;
    };
    let absolute_url = config.absolute_http_url(&head.path)?;
    let claim =
        match parse_http_authorization_claim(authorization, "PUT", &absolute_url, unix_now()) {
            Ok(claim) => claim,
            Err(_) => return unauthorized(&mut stream).await,
        };
    let Some(sha256) = claim.payload_hash else {
        return unauthorized(&mut stream).await;
    };
    let auth = claim.auth;
    match head.header("x-sha-256") {
        Some(value) if value == sha256 => {}
        Some(value) if is_lower_hex_64(value) => {
            return media_error(
                &mut stream,
                400,
                "Bad Request",
                "the X-SHA-256 header does not match the digest in the authorization event",
            )
            .await;
        }
        Some(_) => {
            return media_error(
                &mut stream,
                400,
                "Bad Request",
                "the X-SHA-256 header must be a 64-character lowercase hex SHA-256 digest",
            )
            .await;
        }
        None => {}
    }
    if !rate.media_from_pubkey(&auth.pubkey) {
        return media_error(
            &mut stream,
            429,
            "Too Many Requests",
            "too many media requests; try again later",
        )
        .await;
    }
    let outcome = match db
        .register_media(
            auth.event_id,
            auth.pubkey.clone(),
            sha256.clone(),
            length,
            media_type,
            unix_now(),
            media_config.max_bytes_per_pubkey,
        )
        .await
    {
        Ok(outcome) => outcome,
        Err(StoreError::Media(message)) if message.contains("quota") => {
            return media_error(
                &mut stream,
                413,
                "Payload Too Large",
                "media quota exceeded",
            )
            .await;
        }
        Err(StoreError::Media(_)) => {
            return media_error(
                &mut stream,
                409,
                "Conflict",
                "this authorization event was already used; sign a new one",
            )
            .await;
        }
        Err(_) => {
            return media_error(
                &mut stream,
                503,
                "Service Unavailable",
                "the media database is unavailable",
            )
            .await;
        }
    };
    let mut reservation = Reservation {
        db,
        pubkey: &auth.pubkey,
        sha256: &sha256,
        owned_before: outcome.owned_before,
        temporary_path: None,
    };
    let (temporary_path, mut file) = match storage.temporary_file().await {
        Ok(temporary) => temporary,
        Err(_) => {
            reservation.release().await;
            return media_error(
                &mut stream,
                503,
                "Service Unavailable",
                "media storage is unavailable",
            )
            .await;
        }
    };
    reservation.temporary_path = Some(temporary_path.clone());
    let received = match timeout(
        MEDIA_UPLOAD_TIMEOUT,
        stream_upload(&mut stream, &mut file, length),
    )
    .await
    {
        Ok(Ok(received)) => received,
        Ok(Err(_)) | Err(_) => {
            reservation.release().await;
            return media_error(&mut stream, 400, "Bad Request", "incomplete upload body").await;
        }
    };
    drop(file);
    if received != sha256 {
        reservation.release().await;
        return media_error(
            &mut stream,
            400,
            "Bad Request",
            "the upload body does not match the SHA-256 digest in the authorization event",
        )
        .await;
    }
    let blob_path = storage.blob_path(&outcome.record);
    if fs::create_dir_all(blob_path.parent().unwrap_or(Path::new(".")))
        .await
        .is_err()
        || fs::rename(&temporary_path, &blob_path).await.is_err()
    {
        reservation.release().await;
        return media_error(
            &mut stream,
            503,
            "Service Unavailable",
            "media storage is unavailable; you can retry the upload",
        )
        .await;
    }
    reservation.temporary_path = None;
    match db.finalize_media(sha256.clone()).await {
        Ok(()) => {}
        Err(StoreError::Media(_)) => {
            let _ = fs::remove_file(&blob_path).await;
            return media_error(
                &mut stream,
                409,
                "Conflict",
                "the blob was deleted while the upload was in progress",
            )
            .await;
        }
        Err(_) => {
            if outcome.created {
                let _ = fs::remove_file(&blob_path).await;
            }
            reservation.release().await;
            return media_error(
                &mut stream,
                503,
                "Service Unavailable",
                "the upload did not finish; you can retry it",
            )
            .await;
        }
    }
    let url = media_url(config, &outcome.record)?;
    let body = serde_json::to_vec(&json!({
        "url": url,
        "sha256": outcome.record.sha256,
        "size": outcome.record.size,
        "type": outcome.record.media_type,
        "uploaded": outcome.record.uploaded_at,
        "nip94": [
            ["url", url],
            ["m", outcome.record.media_type],
            ["x", outcome.record.sha256],
            ["size", outcome.record.size.to_string()],
        ],
    }))
    .map_err(|error| GatewayError::Internal(format!("media response: {error}")))?;
    let (status, reason) = if outcome.created {
        (201, "Created")
    } else {
        (200, "OK")
    };
    write_http_bytes(
        &mut stream,
        status,
        reason,
        "application/json",
        &body,
        &[("X-SHA-256", outcome.record.sha256)],
    )
    .await
}

async fn serve_blob(
    mut stream: TcpStream,
    head: &HttpHead,
    storage: &MediaStorage,
    db: &DbPool,
    sha256: &str,
) -> Result<(), GatewayError> {
    let record = match db.media_blob(sha256.to_owned()).await {
        Ok(Some(record)) => record,
        Ok(None) => {
            return media_request_error(&mut stream, head, 404, "Not Found", "blob not found", &[])
                .await;
        }
        Err(_) => {
            return media_request_error(
                &mut stream,
                head,
                503,
                "Service Unavailable",
                "the media database is unavailable",
                &[],
            )
            .await;
        }
    };
    let extension = extension_for_media_type(&record.media_type);
    if let Some(base) = &storage.cloud_base_url {
        let location = format!(
            "{base}/{}/{}.{}",
            record.storage_key, record.sha256, extension
        );
        write_http_head(
            &mut stream,
            307,
            "Temporary Redirect",
            &record.media_type,
            0,
            &[("Location", location), ("X-SHA-256", record.sha256)],
        )
        .await?;
        return shutdown_http(&mut stream).await;
    }
    let mut file = match File::open(storage.blob_path(&record)).await {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return media_request_error(
                &mut stream,
                head,
                503,
                "Service Unavailable",
                "the stored file for this blob is missing or incomplete",
                &[],
            )
            .await;
        }
        Err(_) => {
            return media_request_error(
                &mut stream,
                head,
                503,
                "Service Unavailable",
                "media storage is unavailable",
                &[],
            )
            .await;
        }
    };
    if file.metadata().await?.len() != record.size {
        return media_request_error(
            &mut stream,
            head,
            503,
            "Service Unavailable",
            "the stored file for this blob is missing or incomplete",
            &[],
        )
        .await;
    }
    let range = match parse_range(head.header("range"), record.size) {
        Ok(range) => range,
        Err(()) => {
            return media_request_error(
                &mut stream,
                head,
                416,
                "Range Not Satisfiable",
                "range not satisfiable",
                &[("Content-Range", format!("bytes */{}", record.size))],
            )
            .await;
        }
    };
    let (start, end, status, reason) = match range {
        Some((start, end)) => (start, end, 206, "Partial Content"),
        None => (0, record.size.saturating_sub(1), 200, "OK"),
    };
    let content_length = if record.size == 0 { 0 } else { end - start + 1 };
    let mut headers = vec![
        ("Accept-Ranges", "bytes".to_owned()),
        ("ETag", format!("\"{}\"", record.sha256)),
        ("X-SHA-256", record.sha256.clone()),
        (
            "Cache-Control",
            "public, max-age=31536000, immutable".to_owned(),
        ),
    ];
    if range.is_some() {
        headers.push((
            "Content-Range",
            format!("bytes {start}-{end}/{}", record.size),
        ));
    }
    write_http_head(
        &mut stream,
        status,
        reason,
        &record.media_type,
        content_length,
        &headers,
    )
    .await?;
    if head.method == "HEAD" || content_length == 0 {
        return shutdown_http(&mut stream).await;
    }
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let mut remaining = content_length;
    let mut buffer = vec![0_u8; STREAM_BUFFER_BYTES];
    while remaining > 0 {
        let take = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = timeout(MEDIA_IO_TIMEOUT, file.read(&mut buffer[..take]))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "media read"))??;
        if read == 0 {
            return Err(
                io::Error::new(io::ErrorKind::UnexpectedEof, "media file shortened").into(),
            );
        }
        timeout(MEDIA_IO_TIMEOUT, stream.write_all(&buffer[..read]))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "media write"))??;
        remaining -= read as u64;
    }
    shutdown_http(&mut stream).await
}

#[allow(clippy::too_many_arguments)]
async fn serve_delete(
    mut stream: TcpStream,
    head: &HttpHead,
    config: &GatewayConfig,
    storage: &MediaStorage,
    db: &DbPool,
    rate: &RateLimiter,
    ip: IpAddr,
    sha256: String,
) -> Result<(), GatewayError> {
    if !rate.media_from_ip(ip) {
        return media_error(
            &mut stream,
            429,
            "Too Many Requests",
            "too many media requests; try again later",
        )
        .await;
    }
    let Some(authorization) = head.header("authorization") else {
        return unauthorized(&mut stream).await;
    };
    let absolute_url = config.absolute_http_url(&head.path)?;
    let auth = match parse_http_authorization_hash(
        authorization,
        "DELETE",
        &absolute_url,
        None,
        unix_now(),
    ) {
        Ok(auth) => auth,
        Err(_) => return unauthorized(&mut stream).await,
    };
    if !rate.media_from_pubkey(&auth.pubkey) {
        return media_error(
            &mut stream,
            429,
            "Too Many Requests",
            "too many media requests; try again later",
        )
        .await;
    }
    match db
        .delete_media(auth.event_id, auth.pubkey, sha256.clone())
        .await
    {
        Ok(MediaDeleteOutcome::NotOwned) => {
            media_error(&mut stream, 404, "Not Found", "owned blob not found").await
        }
        Ok(MediaDeleteOutcome::OwnerRemoved) => {
            media_success(&mut stream, "ownership removed").await
        }
        Ok(MediaDeleteOutcome::BlobRemoved(record)) => {
            match storage.retire_blob(&record).await {
                Ok(()) => {}
                Err(_) => {
                    return media_error(
                        &mut stream,
                        503,
                        "Service Unavailable",
                        "blob record deleted; the relay operator must remove the stored file",
                    )
                    .await;
                }
            }
            media_success(&mut stream, "blob deleted").await
        }
        Err(StoreError::Media(_)) => {
            media_error(
                &mut stream,
                409,
                "Conflict",
                "this authorization event was already used; sign a new one",
            )
            .await
        }
        Err(_) => {
            media_error(
                &mut stream,
                503,
                "Service Unavailable",
                "the media database is unavailable",
            )
            .await
        }
    }
}

async fn clean_stale_temporary_files(root: &Path) -> Result<(), GatewayError> {
    let mut entries = fs::read_dir(root.join(".tmp")).await?;
    while let Some(entry) = entries.next_entry().await? {
        if !entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("upload-"))
        {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).await?;
        if metadata
            .modified()?
            .elapsed()
            .is_ok_and(|age| age >= STALE_TEMPORARY_AGE)
        {
            fs::remove_file(entry.path()).await?;
        }
    }
    Ok(())
}

async fn clean_expired_deleted_blobs(root: &Path) -> io::Result<()> {
    let mut entries = fs::read_dir(root.join(".deleted")).await?;
    while let Some(entry) = entries.next_entry().await? {
        let metadata = fs::symlink_metadata(entry.path()).await?;
        if metadata.is_file()
            && metadata
                .modified()?
                .elapsed()
                .is_ok_and(|age| age >= DELETED_RETENTION)
        {
            fs::remove_file(entry.path()).await?;
        }
    }
    Ok(())
}

async fn stream_upload(
    stream: &mut TcpStream,
    file: &mut File,
    length: u64,
) -> Result<String, io::Error> {
    let mut remaining = length;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; STREAM_BUFFER_BYTES];
    while remaining > 0 {
        let take = usize::try_from(remaining.min(buffer.len() as u64)).unwrap_or(buffer.len());
        let read = timeout(MEDIA_IO_TIMEOUT, stream.read(&mut buffer[..take]))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "media upload"))??;
        if read == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "incomplete media upload",
            ));
        }
        file.write_all(&buffer[..read]).await?;
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok(lower_hex(&hasher.finalize()))
}

fn request_content_length(head: &HttpHead, max_bytes: usize) -> Result<u64, &'static str> {
    if head.header("transfer-encoding").is_some() {
        return Err("chunked uploads are not supported");
    }
    let length = head
        .header("content-length")
        .ok_or("Content-Length is required")?
        .parse::<u64>()
        .map_err(|_| "invalid Content-Length")?;
    if length > max_bytes as u64 {
        return Err("upload exceeds configured blob limit");
    }
    Ok(length)
}

fn request_media_type(head: &HttpHead) -> Option<String> {
    let value = head
        .header("content-type")
        .unwrap_or("application/octet-stream")
        .split(';')
        .next()?
        .trim()
        .to_ascii_lowercase();
    valid_media_type(&value).then_some(value)
}

fn valid_media_type(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 127
        && value.split_once('/').is_some_and(|(top, subtype)| {
            !top.is_empty()
                && !subtype.is_empty()
                && top.bytes().all(media_type_byte)
                && subtype.bytes().all(media_type_byte)
        })
}

fn media_type_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"!#$&^_.+-".contains(&byte)
}

fn parse_blob_path(path: &str) -> Option<String> {
    let value = path.strip_prefix('/')?;
    if value.len() < 64
        || !value.as_bytes()[..64]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return None;
    }
    let (sha256, suffix) = value.split_at(64);
    if !suffix.is_empty()
        && (!suffix.starts_with('.')
            || suffix.len() > 17
            || suffix[1..].is_empty()
            || !suffix[1..].bytes().all(|byte| byte.is_ascii_alphanumeric()))
    {
        return None;
    }
    Some(sha256.to_owned())
}

fn parse_range(value: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(value) = value else {
        return Ok(None);
    };
    let range = value.strip_prefix("bytes=").ok_or(())?;
    if range.contains(',') || size == 0 {
        return Err(());
    }
    let (start, end) = range.split_once('-').ok_or(())?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().map_err(|_| ())?;
        if suffix == 0 {
            return Err(());
        }
        return Ok(Some((size.saturating_sub(suffix), size - 1)));
    }
    let start = start.parse::<u64>().map_err(|_| ())?;
    if start >= size {
        return Err(());
    }
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().map_err(|_| ())?.min(size - 1)
    };
    if end < start {
        return Err(());
    }
    Ok(Some((start, end)))
}

fn media_url(config: &GatewayConfig, record: &MediaRecord) -> Result<String, GatewayError> {
    config.absolute_http_url(&format!(
        "/{}.{}",
        record.sha256,
        extension_for_media_type(&record.media_type)
    ))
}

fn extension_for_media_type(media_type: &str) -> &'static str {
    match media_type {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mpeg" => "mp3",
        "audio/ogg" => "ogg",
        "audio/wav" => "wav",
        "application/pdf" => "pdf",
        "application/json" => "json",
        "application/zip" => "zip",
        "text/plain" => "txt",
        _ => "bin",
    }
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

async fn unauthorized(stream: &mut TcpStream) -> Result<(), GatewayError> {
    let body = serde_json::to_vec(&json!({ "message": "invalid NIP-98 authorization" }))
        .map_err(|error| GatewayError::Internal(format!("media response: {error}")))?;
    write_http_bytes(
        stream,
        401,
        "Unauthorized",
        "application/json",
        &body,
        &[("WWW-Authenticate", "Nostr".to_owned())],
    )
    .await
}

async fn media_success(stream: &mut TcpStream, message: &str) -> Result<(), GatewayError> {
    let body = serde_json::to_vec(&json!({ "message": message }))
        .map_err(|error| GatewayError::Internal(format!("media response: {error}")))?;
    write_http_bytes(stream, 200, "OK", "application/json", &body, &[]).await
}

async fn media_error(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    message: &str,
) -> Result<(), GatewayError> {
    let body = serde_json::to_vec(&json!({ "message": message }))
        .map_err(|error| GatewayError::Internal(format!("media response: {error}")))?;
    write_http_bytes(stream, status, reason, "application/json", &body, &[]).await
}

async fn media_request_error(
    stream: &mut TcpStream,
    head: &HttpHead,
    status: u16,
    reason: &str,
    message: &str,
    extra_headers: &[(&str, String)],
) -> Result<(), GatewayError> {
    if head.method != "HEAD" {
        let body = serde_json::to_vec(&json!({ "message": message }))
            .map_err(|error| GatewayError::Internal(format!("media response: {error}")))?;
        return write_http_bytes(
            stream,
            status,
            reason,
            "application/json",
            &body,
            extra_headers,
        )
        .await;
    }
    let content_length = serde_json::to_vec(&json!({ "message": message }))
        .map_err(|error| GatewayError::Internal(format!("media response: {error}")))?
        .len() as u64;
    write_http_head(
        stream,
        status,
        reason,
        "application/json",
        content_length,
        extra_headers,
    )
    .await?;
    shutdown_http(stream).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_paths_mime_and_ranges_are_bounded() {
        let hash = "a".repeat(64);
        assert_eq!(parse_blob_path(&format!("/{hash}.jpg")), Some(hash.clone()));
        assert_eq!(parse_blob_path(&format!("/{hash}")), Some(hash));
        assert!(parse_blob_path("/../blob").is_none());
        assert!(valid_media_type("image/jpeg"));
        assert!(!valid_media_type("image/jpeg; charset=utf-8"));
        assert_eq!(parse_range(Some("bytes=2-4"), 10), Ok(Some((2, 4))));
        assert_eq!(parse_range(Some("bytes=-3"), 10), Ok(Some((7, 9))));
        assert_eq!(parse_range(Some("bytes=10-"), 10), Err(()));

        let local = MediaStorage {
            root: Arc::new(PathBuf::from("/media")),
            cloud_base_url: None,
            next_temporary: Arc::new(AtomicU64::new(1)),
        };
        let record = MediaRecord {
            sha256: "a".repeat(64),
            storage_key: "b".repeat(64),
            size: 1,
            media_type: "image/jpeg".into(),
            uploaded_at: 1,
        };
        assert_eq!(
            local.blob_path(&record),
            PathBuf::from(format!("/media/aa/{}.{}", "a".repeat(64), "b".repeat(64)))
        );
        let cloud = MediaStorage {
            cloud_base_url: Some(Arc::from("https://media.example")),
            ..local
        };
        assert_eq!(
            cloud.blob_path(&record),
            PathBuf::from(format!("/media/{}/{}.jpg", "b".repeat(64), "a".repeat(64)))
        );
    }
}
