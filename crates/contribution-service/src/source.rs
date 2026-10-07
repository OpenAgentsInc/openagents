//! Bounded exact reads from explicitly configured roots.

use crate::Result;
use gym::sales_evidence::{Reference, digest};
use nostr::domain::Event;
use serde::de::DeserializeOwned;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::{Component, Path, PathBuf},
};

pub fn path(root: &Path, relative: &str) -> Result<PathBuf> {
    if relative.is_empty() || relative.len() > 512 {
        return Err("invalid bounded contribution source path".into());
    }
    let mut out = root.to_owned();
    for part in Path::new(relative).components() {
        let Component::Normal(part) = part else {
            return Err("contribution sources must stay under their root".into());
        };
        out.push(part);
        if fs::symlink_metadata(&out)
            .map_err(|_| "contribution source is absent")?
            .file_type()
            .is_symlink()
        {
            return Err("symlink contribution sources are refused".into());
        }
    }
    Ok(out)
}
pub fn bytes(root: &Path, name: &str) -> Result<Vec<u8>> {
    let path = path(root, name)?;
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "contribution source is unavailable")?;
    if !f
        .metadata()
        .map_err(|_| "source metadata is unavailable")?
        .is_file()
    {
        return Err("contribution source must be a regular file".into());
    }
    let mut bytes = Vec::new();
    (&mut f)
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "contribution source cannot be read")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("contribution source exceeds 8 MiB".into());
    }
    Ok(bytes)
}
pub fn read(root: &Path, reference: &Reference) -> Result<Vec<u8>> {
    let bytes = bytes(root, &reference.path)?;
    if digest(&bytes) != reference.sha256 {
        return Err("contribution source changed".into());
    }
    Ok(bytes)
}
pub fn json<T: DeserializeOwned>(root: &Path, reference: &Reference) -> Result<T> {
    serde_json::from_slice(&read(root, reference)?)
        .map_err(|_| "invalid contribution source".into())
}
pub fn signed<T: DeserializeOwned>(bytes: &[u8], signer: &str, now: i64) -> Result<(Event, T)> {
    let event: Event =
        serde_json::from_slice(bytes).map_err(|_| "invalid signed contribution record")?;
    event
        .validate_crypto()
        .map_err(|_| "contribution signature is invalid")?;
    if event.pubkey != signer
        || event.kind != 1
        || !event.tags.is_empty()
        || i64::try_from(event.created_at).map_or(true, |at| at > now)
    {
        return Err("contribution record has an unadmitted signer or time".into());
    }
    let value =
        serde_json::from_str(&event.content).map_err(|_| "invalid signed contribution content")?;
    Ok((event, value))
}
pub fn artifact(root: &Path, reference: &Reference) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path(root, &reference.path)?)
        .map_err(|_| "artifact is unavailable")?;
    let meta = f
        .metadata()
        .map_err(|_| "artifact metadata is unavailable")?;
    if !meta.is_file() || meta.len() > 256 * 1024 * 1024 {
        return Err("artifact must be a regular file of at most 256 MiB".into());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut seen = 0u64;
    loop {
        let n = f.read(&mut buffer).map_err(|_| "artifact cannot be read")?;
        if n == 0 {
            break;
        }
        seen = seen.checked_add(n as u64).ok_or("artifact size overflow")?;
        if seen > 256 * 1024 * 1024 {
            return Err("artifact grew beyond its bound".into());
        }
        hash.update(&buffer[..n]);
    }
    if format!("{:x}", hash.finalize()) != reference.sha256 {
        return Err("artifact contents changed".into());
    }
    Ok(())
}
