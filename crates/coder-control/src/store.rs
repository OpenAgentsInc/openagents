//! Private atomic state. A failed write poisons the open handle.
use crate::*;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub(crate) const MAX_BYTES: usize = 32 * 1024 * 1024;

#[cfg(test)]
std::thread_local! {pub(crate) static FAIL_NEXT_SAVE:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}

pub(crate) struct Store {
    directory: PathBuf,
    _lock: File,
    poisoned: bool,
}
impl Store {
    pub fn open(directory: &Path) -> Result<(Self, Option<Vec<u8>>)> {
        let fresh = match std::fs::DirBuilder::new().mode(0o700).create(directory) {
            Ok(()) => true,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => false,
            Err(e) => return Err(e.to_string()),
        };
        let metadata = std::fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err("control store requires an ordinary private directory".into());
        }
        let directory = directory.canonicalize().map_err(|e| e.to_string())?;
        if !fresh
            && (!directory.join("control.lock").exists()
                || !directory.join("control.json").exists())
        {
            return Err("control store is incomplete; do not replace its replay history".into());
        }
        let lock = private_open(&directory.join("control.lock"), fresh)?;
        lock.try_lock()
            .map_err(|_| "control store is busy".to_owned())?;
        lock.sync_all()
            .and_then(|_| File::open(&directory)?.sync_all())
            .map_err(|e| e.to_string())?;
        let bytes = if fresh {
            None
        } else {
            let mut bytes = vec![];
            private_open(&directory.join("control.json"), false)?
                .take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > MAX_BYTES {
                return Err("control store exceeds retention bound".into());
            }
            Some(bytes)
        };
        Ok((
            Self {
                directory,
                _lock: lock,
                poisoned: false,
            },
            bytes,
        ))
    }
    pub fn save(&mut self, value: &impl Serialize) -> Result<()> {
        self.ensure_healthy()?;
        self.poisoned = true;
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("control retention bound reached".into());
        }
        #[cfg(test)]
        if FAIL_NEXT_SAVE.with(|fault| fault.replace(false)) {
            return Err("synthetic control persistence failure".into());
        }
        let pending = self.directory.join(".control.pending");
        if pending.exists() {
            let _file = private_open(&pending, false)?;
            std::fs::remove_file(&pending).map_err(|e| e.to_string())?;
        }
        let mut file = private_open(&pending, true)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        std::fs::rename(&pending, self.directory.join("control.json"))
            .map_err(|e| e.to_string())?;
        File::open(&self.directory)
            .and_then(|file| file.sync_all())
            .map_err(|e| e.to_string())?;
        self.poisoned = false;
        Ok(())
    }
    pub fn ensure_healthy(&self) -> Result<()> {
        if self.poisoned {
            Err("control store requires reopening after an uncertain write".into())
        } else {
            Ok(())
        }
    }
}
fn private_open(path: &Path, create: bool) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(create)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.permissions().mode() & 0o077 != 0 {
        return Err("control files must be private singly linked regular files".into());
    }
    Ok(file)
}
