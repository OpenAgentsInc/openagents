//! Private client references, never a second balance or payment ledger.
use super::{Error, Result};
use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

pub(super) fn path_check(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
        return Err(Error::Private(
            "an absolute path without parent traversal is required",
        ));
    }
    let mut prefix = PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        match fs::symlink_metadata(&prefix) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::Private("symlinks are not admitted"));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn owned(meta: &fs::Metadata, directory: bool) -> bool {
    meta.uid() == unsafe { libc::geteuid() }
        && meta.mode() & 0o077 == 0
        && if directory {
            meta.is_dir()
        } else {
            meta.is_file() && meta.nlink() == 1
        }
}
pub(super) fn read(path: &Path, max: u64) -> Result<Vec<u8>> {
    path_check(path)?;
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let m = f.metadata()?;
    if !owned(&m, false) || m.len() > max {
        return Err(Error::Private(
            "the file must be private, owned, unshared, and bounded",
        ));
    }
    let mut bytes = vec![];
    Read::by_ref(&mut f).take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(Error::Private("the file grew beyond its bound"));
    }
    Ok(bytes)
}
fn create(path: &Path) -> Result<File> {
    path_check(path)?;
    let f = match OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)?,
        Err(e) => return Err(e.into()),
    };
    if !owned(&f.metadata()?, false) {
        return Err(Error::Private("existing shared files cannot be adopted"));
    }
    Ok(f)
}
pub(super) struct Custody {
    path: PathBuf,
    file: File,
    directory: bool,
}
impl Custody {
    fn check(&self) -> Result<()> {
        path_check(&self.path)?;
        let current = fs::symlink_metadata(&self.path)?;
        let original = self.file.metadata()?;
        if !owned(&current, self.directory)
            || current.dev() != original.dev()
            || current.ino() != original.ino()
        {
            return Err(Error::Private(
                "private client state was replaced or shared",
            ));
        }
        Ok(())
    }
}
pub(super) struct StateFile {
    directory: Custody,
    lock: Custody,
    record: Custody,
}
impl StateFile {
    pub fn open(root: &Path) -> Result<Self> {
        path_check(root)?;
        if !root.exists() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(root)?;
        }
        let directory = Custody {
            path: root.into(),
            file: File::open(root)?,
            directory: true,
        };
        directory.check()?;
        let path = root.join("client.lock");
        let lock = Custody {
            file: create(&path)?,
            path,
            directory: false,
        };
        lock.file
            .try_lock_exclusive()
            .map_err(|_| Error::Private("another retail client owns this state"))?;
        let path = root.join("references.json");
        let record = Custody {
            file: create(&path)?,
            path,
            directory: false,
        };
        let state = Self {
            directory,
            lock,
            record,
        };
        state.check()?;
        Ok(state)
    }
    pub fn check(&self) -> Result<()> {
        self.directory.check()?;
        self.lock.check()?;
        self.record.check()
    }
    pub fn load(&self) -> Result<Option<super::State>> {
        self.check()?;
        if self.record.file.metadata()?.len() == 0 {
            return Ok(None);
        }
        let bytes = read(&self.record.path, 2 * 1024 * 1024)?;
        let value = serde_json::from_slice(&bytes)?;
        self.check()?;
        Ok(Some(value))
    }
    pub fn save(&mut self, state: &super::State) -> Result<()> {
        self.check()?;
        let bytes = serde_json::to_vec(state)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(Error::Private("client references exceed their bound"));
        }
        let path = self.directory.path.join("references.next");
        if path.exists() {
            let _ = read(&path, 2 * 1024 * 1024)?;
            fs::remove_file(&path)?;
        }
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(&path)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        self.check()?;
        fs::rename(&path, &self.record.path)?;
        self.record.file = f;
        self.directory.file.sync_all()?;
        self.check()
    }
}
