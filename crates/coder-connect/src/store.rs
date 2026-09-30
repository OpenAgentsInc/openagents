//! Private short-lived store locks let local revocation serialize with reads.
//! Other host crates reuse this store under their own file name.
use crate::{Error, ErrorCode, Result};
use serde::{Serialize, de::DeserializeOwned};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const MAX_STORE: usize = 64 * 1024 * 1024;
/// A state file's device, inode, length, and modification and change times.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stamp([u64; 7]);
pub struct Store {
    directory: PathBuf,
    name: &'static str,
    _lock: File,
    poisoned: bool,
}
fn io(_: std::io::Error) -> Error {
    Error::new(
        ErrorCode::Unavailable,
        "private observer store is unavailable",
    )
}
#[cfg(unix)]
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
/// The Windows form of the same rule: opened without following a link, a
/// regular file with one link, owned by this user, and admitting no one else
/// ([`private_fs::is_private`]). A file created here inherits its private
/// directory's owner-only DACL.
#[cfg(windows)]
pub(crate) fn private(path: &Path, create: bool) -> Result<File> {
    let file = private_fs::nofollow(OpenOptions::new().read(true).write(true).create_new(create))
        .open(path)
        .map_err(io)?;
    checked(file)
}
/// Refuses `file` unless it is a private, singly linked regular file.
#[cfg(windows)]
fn checked(file: File) -> Result<File> {
    let m = file.metadata().map_err(io)?;
    let links = private_fs::identity(&file).map_err(io)?.links;
    if !m.is_file() || links != 1 || !private_fs::is_private(&file).map_err(io)? {
        return Err(Error::new(
            ErrorCode::Forbidden,
            "observer files must be private owned singly linked regular files",
        ));
    }
    Ok(file)
}
/// Open a private append-only file, creating it when `create` is set; `None`
/// when it does not exist and is not created. The same ownership, link, and
/// mode checks as [`private`] apply.
#[cfg(unix)]
pub(crate) fn private_append(path: &Path, create: bool) -> Result<Option<File>> {
    let file = match OpenOptions::new()
        .read(true)
        .append(true)
        .create(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(e) if !create && e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    let m = file.metadata().map_err(io)?;
    // SAFETY: geteuid has no pointers or preconditions.
    let uid = unsafe { libc::geteuid() };
    if !m.is_file() || m.nlink() != 1 || m.uid() != uid || m.permissions().mode() & 0o077 != 0 {
        return Err(Error::new(
            ErrorCode::Forbidden,
            "observer files must be private owned singly linked regular files",
        ));
    }
    Ok(Some(file))
}
/// The Windows form of [`private_append`], under the rule of [`private`].
#[cfg(windows)]
pub(crate) fn private_append(path: &Path, create: bool) -> Result<Option<File>> {
    let file = match private_fs::nofollow(OpenOptions::new().read(true).append(true).create(create))
        .open(path)
    {
        Ok(file) => file,
        Err(e) if !create && e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io(e)),
    };
    checked(file).map(Some)
}
impl Store {
    pub fn open(directory: &Path, create: bool) -> Result<Self> {
        Self::open_named(directory, "observer", create)
    }
    /// Open a private store whose state, lock, and pending files use `name`.
    pub fn open_named(directory: &Path, name: &'static str, create: bool) -> Result<Self> {
        let fresh = if create {
            match create_private_dir(directory) {
                Ok(()) => true,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
                Err(e) => return Err(io(e)),
            }
        } else {
            false
        };
        if !private_dir(directory)? {
            return Err(Error::new(
                ErrorCode::Forbidden,
                "observer store must be an owned private directory",
            ));
        }
        let directory = directory.canonicalize().map_err(io)?;
        let lock = private(&directory.join(format!("{name}.lock")), fresh)?;
        lock.try_lock().map_err(|_| {
            Error::new(
                ErrorCode::Conflict,
                "observer store is busy; retry the local operation",
            )
        })?;
        Ok(Self {
            directory,
            name,
            _lock: lock,
            poisoned: false,
        })
    }
    pub fn key(&self, initialize: bool) -> Result<secp256k1::SecretKey> {
        let path = self.directory.join("host.key");
        if !path.exists() && initialize && !self.state_path().exists() {
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
        let path = self.state_path();
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
        let pending = self.directory.join(format!(".{}.pending", self.name));
        if pending.exists() {
            let _checked = private(&pending, false)?;
            std::fs::remove_file(&pending).map_err(io)?;
        }
        let mut file = private(&pending, true)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(io)?;
        std::fs::rename(pending, self.state_path()).map_err(io)?;
        self.sync()?;
        self.poisoned = false;
        Ok(())
    }
    /// The state file's identity, which changes with every save and with any
    /// other writer's replacement: a reader that validated this exact file
    /// can reuse what it validated while the stamp is unchanged.
    #[cfg(windows)]
    pub fn stamp(&self) -> Option<Stamp> {
        let (id, m) = private_fs::identity_of(&self.state_path()).ok()?;
        m.is_file().then_some(Stamp([
            u64::from(id.volume),
            id.index,
            id.size,
            id.written,
            0,
            id.created,
            u64::from(id.links),
        ]))
    }
    /// The state file's identity, which changes with every save and with any
    /// other writer's replacement: a reader that validated this exact file
    /// can reuse what it validated while the stamp is unchanged.
    #[cfg(unix)]
    pub fn stamp(&self) -> Option<Stamp> {
        let m = std::fs::symlink_metadata(self.state_path()).ok()?;
        Some(Stamp([
            m.dev(),
            m.ino(),
            m.len(),
            m.mtime() as u64,
            m.mtime_nsec() as u64,
            m.ctime() as u64,
            m.ctime_nsec() as u64,
        ]))
    }
    /// The canonical store directory.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    fn state_path(&self) -> PathBuf {
        self.directory.join(format!("{}.json", self.name))
    }
    #[cfg(unix)]
    fn sync(&self) -> Result<()> {
        File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(io)
    }
    /// Windows cannot flush a directory; NTFS journals the rename itself.
    #[cfg(windows)]
    #[allow(clippy::unnecessary_wraps)]
    fn sync(&self) -> Result<()> {
        Ok(())
    }
}
/// The identity of the open `file` whose metadata is `m`: its device and
/// inode, or on Windows its volume and file index.
#[cfg(unix)]
#[allow(clippy::unnecessary_wraps)]
pub(crate) fn file_id(_file: &File, m: &std::fs::Metadata) -> Result<(u64, u64)> {
    Ok((m.dev(), m.ino()))
}
/// The identity of the open `file` whose metadata is `m`: its device and
/// inode, or on Windows its volume and file index.
#[cfg(windows)]
pub(crate) fn file_id(file: &File, _m: &std::fs::Metadata) -> Result<(u64, u64)> {
    let id = private_fs::identity(file).map_err(io)?;
    Ok((u64::from(id.volume), id.index))
}
/// Creates `directory` for this user alone: mode `0700`, or an owner-only
/// DACL on Windows.
#[cfg(unix)]
pub(crate) fn create_private_dir(directory: &Path) -> std::io::Result<()> {
    std::fs::DirBuilder::new().mode(0o700).create(directory)
}
/// Creates `directory` for this user alone: mode `0700`, or an owner-only
/// DACL on Windows.
#[cfg(windows)]
pub(crate) fn create_private_dir(directory: &Path) -> std::io::Result<()> {
    private_fs::create_dir(directory)
}
/// Whether `directory` (not followed through a link) is a directory this
/// user owns that no one else may use.
#[cfg(unix)]
pub(crate) fn private_dir(directory: &Path) -> Result<bool> {
    let m = std::fs::symlink_metadata(directory).map_err(io)?;
    // SAFETY: geteuid has no pointers or preconditions.
    let uid = unsafe { libc::geteuid() };
    Ok(m.is_dir() && m.uid() == uid && m.permissions().mode() & 0o077 == 0)
}
/// Whether `directory` (not followed through a link) is a directory this
/// user owns that no one else may use.
#[cfg(windows)]
pub(crate) fn private_dir(directory: &Path) -> Result<bool> {
    let object = private_fs::open_dir(directory).map_err(io)?;
    let m = object.metadata().map_err(io)?;
    Ok(m.is_dir() && private_fs::is_private(&object).map_err(io)?)
}
