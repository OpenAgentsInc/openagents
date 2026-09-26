//! Private short-lived store locks let local revocation serialize with reads.
use crate::{Error, ErrorCode, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_STORE: usize = 64 * 1024 * 1024;
pub(crate) struct Store {
    directory: PathBuf,
    _lock: File,
    poisoned: bool,
}
fn io(_: std::io::Error) -> Error {
    Error::new(
        ErrorCode::Unavailable,
        "private observer store is unavailable",
    )
}
pub(crate) fn private(path: &Path, create: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(io)?;
    let m = file.metadata().map_err(io)?;
    // SAFETY: geteuid has no pointers or preconditions.
    let uid = unsafe { libc::geteuid() };
    if !m.is_file() || m.nlink() != 1 || m.uid() != uid || m.permissions().mode() & 0o077 != 0 {
        return Err(Error::new(
            ErrorCode::Forbidden,
            "observer files must be private owned singly linked regular files",
        ));
    }
    Ok(file)
}
impl Store {
    pub fn open(directory: &Path, create: bool) -> Result<Self> {
        let fresh = if create {
            match std::fs::DirBuilder::new().mode(0o700).create(directory) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
                Err(e) => return Err(io(e)),
            }
        } else {
            false
        };
        let m = std::fs::symlink_metadata(directory).map_err(io)?;
        // SAFETY: geteuid has no pointers or preconditions.
        let uid = unsafe { libc::geteuid() };
        if !m.is_dir() || m.uid() != uid || m.permissions().mode() & 0o077 != 0 {
            return Err(Error::new(
                ErrorCode::Forbidden,
                "observer store must be an owned private directory",
            ));
        }
        let directory = directory.canonicalize().map_err(io)?;
        let lock = private(&directory.join("observer.lock"), fresh)?;
        lock.try_lock().map_err(|_| {
            Error::new(
                ErrorCode::Conflict,
                "observer store is busy; retry the local operation",
            )
        })?;
        Ok(Self {
            directory,
            _lock: lock,
            poisoned: false,
        })
    }
    pub fn key(&self, initialize: bool) -> Result<secp256k1::SecretKey> {
        let path = self.directory.join("host.key");
        if !path.exists() && initialize && !self.directory.join("observer.json").exists() {
            let key = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
            let mut file = private(&path, true)?;
            file.write_all(&key.secret_bytes())
                .and_then(|_| file.sync_all())
                .map_err(io)?;
            self.sync()?;
            return Ok(key);
        }
        let mut bytes = Vec::new();
        private(&path, false)?
            .take(33)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        let raw: [u8; 32] = bytes.try_into().map_err(|_| {
            Error::new(ErrorCode::Forbidden, "observer key file has invalid length")
        })?;
        secp256k1::SecretKey::from_byte_array(raw)
            .map_err(|_| Error::new(ErrorCode::Forbidden, "observer key file is invalid"))
    }
    pub fn load<T: DeserializeOwned>(&self) -> Result<Option<T>> {
        let path = self.directory.join("observer.json");
        if !path.exists() {
            return Ok(None);
        }
        let mut bytes = Vec::new();
        private(&path, false)?
            .take(MAX_STORE as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        if bytes.len() > MAX_STORE {
            return Err(Error::new(
                ErrorCode::Bounds,
                "observer retention limit exceeded",
            ));
        }
        let value = nostr::contracts::parse_strict_bounded(&bytes, MAX_STORE)
            .map_err(|_| Error::new(ErrorCode::Malformed, "observer store is malformed"))?;
        serde_json::from_value(value)
            .map(Some)
            .map_err(|_| Error::new(ErrorCode::Malformed, "observer store schema differs"))
    }
    pub fn save(&mut self, value: &impl Serialize) -> Result<()> {
        if self.poisoned {
            return Err(Error::new(
                ErrorCode::Unavailable,
                "reopen observer store after uncertain persistence",
            ));
        }
        self.poisoned = true;
        let bytes = serde_json::to_vec(value)
            .map_err(|_| Error::new(ErrorCode::Malformed, "observer store serialization failed"))?;
        if bytes.len() > MAX_STORE {
            return Err(Error::new(
                ErrorCode::Bounds,
                "observer store retention limit exceeded",
            ));
        }
        let pending = self.directory.join(".observer.pending");
        if pending.exists() {
            let _checked = private(&pending, false)?;
            std::fs::remove_file(&pending).map_err(io)?;
        }
        let mut file = private(&pending, true)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(io)?;
        std::fs::rename(pending, self.directory.join("observer.json")).map_err(io)?;
        self.sync()?;
        self.poisoned = false;
        Ok(())
    }
    fn sync(&self) -> Result<()> {
        File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(io)
    }
}
