//! Images bound to a task: the exact bytes a person attached, kept in the
//! task store beside the task and read back by digest.
//!
//! Two paths reach here and share every check. A chat on this computer
//! hands the bytes over directly ([`super::local::Local::start_with_images`]).
//! An enrolled device sends them first, chunk by chunk, with NIP-HOST
//! `artifact.put` ([`put`]); the host keeps them for that device only, under
//! `uploads/`, until a `task.create` from the same device names them
//! ([`adopt`]). Either way a task's image is a private file at
//! `media/<task>/<sha256 hex>` in the task store, and the task's intent
//! names it by digest, type, size, and name, so the execution grant (which
//! binds the intent's digest) admits exactly those bytes. The engine reads
//! them with [`load`], which checks the digest and the image type again.
//!
//! Bounds: PNG or JPEG only, at most 8 MiB an image and 4 a task
//! ([`coder_host::access::media`]); at most [`MAX_PENDING`] unfinished or
//! unclaimed uploads a device, each dropped after [`UPLOAD_TTL`] seconds.

use super::*;
/// The NIP-HOST image types and bounds ([`coder_host::access::media`]).
pub use coder_host::access::media as wire;
use coder_host::access::media::{self, ArtifactPut, ArtifactState, ImageRef};
use std::io::{Seek, SeekFrom};

/// Task images, under the task store.
pub const MEDIA_DIR: &str = "media";
/// Device uploads not yet bound to a task, under the task store.
pub const UPLOADS_DIR: &str = "uploads";
/// The most images one device may hold unclaimed at once.
pub const MAX_PENDING: usize = 8;
/// How long an unclaimed upload stays.
pub const UPLOAD_TTL: u64 = 24 * 60 * 60;

/// A name that is safe as one path component: a task or device identity.
fn component(value: &str) -> Result<&str, Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::UnsafePath);
    }
    Ok(value)
}

fn hex(digest: &str) -> Result<&str, Error> {
    media::hex(digest).map_err(|_| Error::InvalidCommand("an image digest is malformed"))
}

/// Check `bytes` against `reference`: exact size, digest, and type.
///
/// # Errors
/// [`Error::Corrupt`] when any differs.
pub fn verify(reference: &ImageRef, bytes: &[u8]) -> Result<(), Error> {
    reference
        .validate()
        .map_err(|_| Error::InvalidCommand("an image reference is out of bounds"))?;
    if bytes.len() as u64 != reference.size
        || media::digest(bytes) != reference.digest
        || media::sniff(bytes) != Some(reference.media_type.as_str())
    {
        return Err(Error::Corrupt(
            "image bytes differ from their digest or type",
        ));
    }
    Ok(())
}

fn task_dir(store: &Path, task: &str) -> Result<PathBuf, Error> {
    Ok(store.join(MEDIA_DIR).join(component(task)?))
}

/// Where `task`'s image `reference` is kept.
///
/// # Errors
/// The task ID or digest is not a safe name.
pub fn path(store: &Path, task: &str, reference: &ImageRef) -> Result<PathBuf, Error> {
    Ok(task_dir(store, task)?.join(hex(&reference.digest)?))
}

fn read_bounded(path: &Path, max: u64) -> Result<Option<Vec<u8>>, Error> {
    if !regular_or_absent(path)? {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    private_open(path, false, false)?
        .take(max + 1)
        .read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

/// Keep `bytes` as `task`'s image `reference`. Saving the same bytes again
/// changes nothing; a file left incomplete by a crash is written again.
///
/// # Errors
/// The bytes differ from the reference, or the store cannot be written.
pub fn save(store: &Path, task: &str, reference: &ImageRef, bytes: &[u8]) -> Result<(), Error> {
    verify(reference, bytes)?;
    let dir = task_dir(store, task)?;
    prepare_directory(&store.join(MEDIA_DIR))?;
    prepare_directory(&dir)?;
    let path = path(store, task, reference)?;
    if let Some(saved) = read_bounded(&path, reference.size)? {
        if saved == bytes {
            return Ok(());
        }
        std::fs::remove_file(&path)?;
    }
    let mut file = private_open(&path, true, true)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    sync_directory(&dir)?;
    Ok(())
}

/// `task`'s image `reference`, checked again against its digest and type.
///
/// # Errors
/// The image is missing or its bytes differ.
pub fn load(store: &Path, task: &str, reference: &ImageRef) -> Result<Vec<u8>, Error> {
    let bytes =
        read_bounded(&path(store, task, reference)?, reference.size)?.ok_or(Error::NotFound)?;
    verify(reference, &bytes)?;
    Ok(bytes)
}

/// The metadata an upload keeps beside its bytes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    media_type: String,
    size: u64,
    started_at: u64,
}

fn device_dir(store: &Path, device: &str) -> Result<PathBuf, Error> {
    Ok(store.join(UPLOADS_DIR).join(component(device)?))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// Drop `device`'s uploads older than [`UPLOAD_TTL`], and return how many
/// remain, by digest hex.
fn sweep(dir: &Path, now: u64) -> Result<Vec<String>, Error> {
    let mut kept = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(kept),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        let Some(hex) = name.strip_suffix(".meta") else {
            continue;
        };
        let meta = dir.join(&name);
        let pending: Option<Pending> =
            read_bounded(&meta, 4096)?.and_then(|bytes| serde_json::from_slice(&bytes).ok());
        match pending {
            Some(pending) if now.saturating_sub(pending.started_at) <= UPLOAD_TTL => {
                kept.push(hex.to_owned());
            }
            _ => {
                let _ = std::fs::remove_file(&meta);
                let _ = std::fs::remove_file(dir.join(format!("{hex}.part")));
            }
        }
    }
    Ok(kept)
}

/// Keep one chunk of `device`'s image and answer what is held. Chunks
/// arrive in order; a chunk the host already holds must carry the same
/// bytes and changes nothing, and one past the held bytes changes nothing
/// either, so the answer tells the device where to resume. When the last
/// byte arrives the whole image is checked against its digest and type, and
/// a mismatch drops it.
///
/// # Errors
/// The chunk is malformed, conflicts with held bytes or another upload of
/// the same digest, the device holds too many uploads, or the store cannot
/// be written.
pub fn put(store: &Path, device: &str, put: &ArtifactPut) -> Result<ArtifactState, Error> {
    let chunk = put
        .bytes()
        .map_err(|_| Error::InvalidCommand("an image chunk is malformed"))?;
    let hex = hex(&put.digest)?.to_owned();
    let dir = device_dir(store, device)?;
    let pending = Pending {
        media_type: put.media_type.clone(),
        size: put.size,
        started_at: now(),
    };
    let held = sweep(&dir, pending.started_at)?;
    let (meta, part) = (
        dir.join(format!("{hex}.meta")),
        dir.join(format!("{hex}.part")),
    );
    let state = |received: u64, complete: bool| ArtifactState {
        digest: put.digest.clone(),
        received,
        complete,
    };
    if !held.contains(&hex) {
        if held.len() >= MAX_PENDING {
            return Err(Error::LimitExceeded);
        }
        prepare_directory(&store.join(UPLOADS_DIR))?;
        prepare_directory(&dir)?;
        let _ = std::fs::remove_file(&part);
        private_open(&part, true, true)?.sync_all()?;
        let mut file = private_open(&meta, true, true)?;
        file.write_all(&serde_json::to_vec(&pending).map_err(|_| Error::UnsupportedSchema)?)?;
        file.sync_all()?;
    } else {
        let recorded: Pending = read_bounded(&meta, 4096)?
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or(Error::Corrupt("an upload's record is unreadable"))?;
        if recorded.media_type != put.media_type || recorded.size != put.size {
            return Err(Error::Conflict);
        }
    }
    let mut file = private_open(&part, false, true)?;
    let length = file.metadata()?.len();
    if length > put.size {
        return Err(Error::Corrupt("an upload holds more than its size"));
    }
    let end = put.offset + chunk.len() as u64;
    if end <= length {
        // A retry of a chunk already held: the same bytes, or a conflict.
        let mut saved = vec![0; chunk.len()];
        file.seek(SeekFrom::Start(put.offset))?;
        file.read_exact(&mut saved)?;
        if saved != chunk {
            return Err(Error::Conflict);
        }
    } else if put.offset == length {
        file.seek(SeekFrom::End(0))?;
        file.write_all(&chunk)?;
        file.sync_all()?;
    } else {
        // A gap: hold nothing new; the device resumes at `length`.
        return Ok(state(length, false));
    }
    let received = length.max(end);
    if received < put.size {
        return Ok(state(received, false));
    }
    let reference = ImageRef {
        digest: put.digest.clone(),
        media_type: put.media_type.clone(),
        size: put.size,
        name: String::new(),
    };
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(0))?;
    file.take(put.size + 1).read_to_end(&mut bytes)?;
    if verify(&reference, &bytes).is_err() {
        let _ = std::fs::remove_file(&part);
        let _ = std::fs::remove_file(&meta);
        return Err(Error::Corrupt(
            "image bytes differ from their digest or type",
        ));
    }
    Ok(state(received, true))
}

/// Bind `device`'s complete upload `reference` to `task`: verify it, keep it
/// in the task's media, and drop the upload. A task that already holds the
/// image (a retried `task.create`) needs no upload.
///
/// # Errors
/// [`Error::NotFound`] when neither the task nor the device holds the
/// complete image; otherwise as [`save`].
pub fn adopt(store: &Path, device: &str, task: &str, reference: &ImageRef) -> Result<(), Error> {
    if load(store, task, reference).is_ok() {
        return Ok(());
    }
    let dir = device_dir(store, device)?;
    let hex = hex(&reference.digest)?;
    let (meta, part) = (
        dir.join(format!("{hex}.meta")),
        dir.join(format!("{hex}.part")),
    );
    let bytes = read_bounded(&part, reference.size)?.ok_or(Error::NotFound)?;
    if bytes.len() as u64 != reference.size {
        return Err(Error::NotFound);
    }
    save(store, task, reference, &bytes)?;
    let _ = std::fs::remove_file(&part);
    let _ = std::fs::remove_file(&meta);
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use coder_host::access::media::{CHUNK_BYTES, Upload};
    use std::sync::Arc;

    fn png(extra: usize) -> Arc<Vec<u8>> {
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        bytes.extend((0..extra).map(|i| (i * 7 % 256) as u8));
        Arc::new(bytes)
    }

    fn private_root() -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let root = tempfile::tempdir().unwrap();
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        root
    }

    #[test]
    fn chunks_reach_the_task_as_exact_bytes_and_retries_change_nothing() {
        let root = private_root();
        let store = root.path();
        let device = "d".repeat(64);
        let upload = Upload::new("shot.png", png(80_000)).unwrap();
        let chunks = upload.chunks(0);
        // Out of order: nothing is held past the gap.
        let gap = put(store, &device, &chunks[1]).unwrap();
        assert_eq!((gap.received, gap.complete), (0, false));
        for chunk in &chunks {
            let state = put(store, &device, chunk).unwrap();
            // A retry of the same chunk answers the same.
            assert_eq!(put(store, &device, chunk).unwrap(), state);
        }
        let last = put(store, &device, chunks.last().unwrap()).unwrap();
        assert!(last.complete);
        assert_eq!(last.received, upload.reference.size);
        // A different chunk at a held offset conflicts.
        let mut other = chunks[0].clone();
        other.data = other.data.replace('A', "B");
        if other.data != chunks[0].data {
            assert!(put(store, &device, &other).is_err());
        }
        // Another device holds nothing of it.
        let task = "a".repeat(64);
        assert!(matches!(
            adopt(store, &"e".repeat(64), &task, &upload.reference),
            Err(Error::NotFound)
        ));
        adopt(store, &device, &task, &upload.reference).unwrap();
        assert_eq!(
            load(store, &task, &upload.reference).unwrap(),
            *upload.bytes
        );
        // A retried create finds the task's copy; the upload is gone.
        adopt(store, &device, &task, &upload.reference).unwrap();
        assert!(
            !store
                .join(UPLOADS_DIR)
                .join(&device)
                .join(format!(
                    "{}.part",
                    media::hex(&upload.reference.digest).unwrap()
                ))
                .exists()
        );
    }

    #[test]
    fn a_digest_or_type_mismatch_is_dropped_and_bounds_hold() {
        let root = private_root();
        let store = root.path();
        let device = "d".repeat(64);
        let upload = Upload::new("shot.png", png(10)).unwrap();
        let mut chunk = upload.chunks(0).remove(0);
        chunk.digest = media::digest(b"something else");
        assert!(put(store, &device, &chunk).is_err());
        // The task copy is checked on every read.
        let task = "t".repeat(8);
        save(store, &task, &upload.reference, &upload.bytes).unwrap();
        let path = path(store, &task, &upload.reference).unwrap();
        std::fs::write(&path, b"\x89PNG\r\n\x1a\nchanged").unwrap();
        assert!(load(store, &task, &upload.reference).is_err());
        assert!(save(store, "../escape", &upload.reference, &upload.bytes).is_err());
        // At most MAX_PENDING unclaimed uploads a device.
        for index in 0..MAX_PENDING {
            let bytes = png(index + 1);
            let upload = Upload::new("x", bytes).unwrap();
            let first = upload.chunks(0).remove(0);
            put(store, &device, &first).unwrap();
        }
        let extra = Upload::new("x", png(CHUNK_BYTES as usize)).unwrap();
        assert!(matches!(
            put(store, &device, &extra.chunks(0)[0]),
            Err(Error::LimitExceeded)
        ));
    }
}
