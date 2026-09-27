//! Small filesystem helpers with the durability rules every record here
//! follows: private directories, bounded reads that refuse symbolic links,
//! and atomic replacement followed by a directory sync.

use std::fs::{self, File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest as _, Sha256};

use crate::{Error, Result};

/// The largest record this crate reads: a configuration, a state record,
/// a descriptor, or a request.
pub const RECORD_MAX: u64 = 1024 * 1024;

/// Creates `path` as a private directory, or checks that an existing one is
/// a private ordinary directory owned by this user.
pub fn private_dir(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if !meta.file_type().is_dir() {
                return Err(Error::refused(format!(
                    "{} is not an ordinary directory",
                    path.display()
                )));
            }
            if meta.mode() & 0o077 != 0 {
                return Err(Error::refused(format!(
                    "{} must not be readable or writable by other users",
                    path.display()
                )));
            }
            // SAFETY: `getuid` takes no arguments and cannot fail.
            if meta.uid() != unsafe { libc::getuid() } {
                return Err(Error::refused(format!(
                    "{} belongs to another user",
                    path.display()
                )));
            }
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(path)?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
            if let Some(parent) = path.parent() {
                sync_dir(parent)?;
            }
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

/// Reads an ordinary file of at most `max` bytes, refusing a symbolic link,
/// a device, a FIFO, or a hard-linked file.
pub fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let meta = file.metadata()?;
    // A link count of zero is a complete record that an atomic write
    // replaced after this open; more than one is a hard link, refused.
    if !meta.file_type().is_file() || meta.nlink() > 1 {
        return Err(Error::refused(format!(
            "{} is not an ordinary file without hard links",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::refused(format!(
            "{} exceeds its {max}-byte limit",
            path.display()
        )));
    }
    Ok(bytes)
}

/// Reads a record, or `None` when the file does not exist.
pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match read_bounded(path, RECORD_MAX) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Replaces `path` with `bytes` atomically: a private temporary file in the
/// same directory, synced, renamed over the target, and the directory
/// synced after the rename.
pub fn atomic_write(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::refused("a record path has no parent directory"))?;
    let name = path
        .file_name()
        .ok_or_else(|| Error::refused("a record path has no file name"))?
        .to_string_lossy()
        .into_owned();
    let temporary = parent.join(format!(".{name}.pending-{}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        sync_dir(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Flushes a directory's entries, so a rename in it survives a crash.
pub fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

/// The SHA-256 of `bytes`, in lowercase hexadecimal.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The SHA-256 of an ordinary file's contents, streamed.
pub fn sha256_file(path: &Path) -> Result<String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(Error::refused(format!(
            "{} is not an ordinary file",
            path.display()
        )));
    }
    let mut hasher = Sha256::new();
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Whether `value` is a 64-character lowercase hexadecimal digest.
#[must_use]
pub fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Milliseconds since the Unix epoch.
#[must_use]
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Removes a file, treating a missing file as already removed.
pub fn remove_file_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Removes a directory tree, treating a missing tree as already removed.
pub fn remove_tree_if_present(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => Ok(fs::remove_dir_all(path)?),
        Ok(_) => Ok(fs::remove_file(path)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
