//! Held private files retain their original directory and physical identity.
use crate::{Error, Result};
use std::{
    fs::{File, OpenOptions},
    os::unix::{
        fs::{FileExt, MetadataExt, OpenOptionsExt},
        io::{AsRawFd, FromRawFd},
    },
    path::{Path, PathBuf},
};
pub(crate) struct Held {
    pub path: PathBuf,
    directory: File,
    pub file: File,
    digest: String,
}
fn private(m: &std::fs::Metadata, directory: bool) -> bool {
    m.uid() == unsafe { libc::geteuid() }
        && m.mode() & 0o077 == 0
        && if directory {
            m.is_dir()
        } else {
            m.is_file() && m.nlink() == 1
        }
}
fn same(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    a.dev() == b.dev() && a.ino() == b.ino()
}
impl Held {
    pub fn open(path: &Path, immutable: bool) -> Result<Self> {
        if !path.is_absolute() {
            return Err(Error::Denied);
        }
        let parent = path.parent().ok_or(Error::Denied)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent)?;
        if !private(&directory.metadata()?, true) {
            return Err(Error::Denied);
        }
        let name =
            std::ffi::CString::new(path.file_name().ok_or(Error::Denied)?.as_encoded_bytes())
                .map_err(|_| Error::Denied)?;
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(Error::Denied);
        }
        let file = unsafe { File::from_raw_fd(fd) };
        let mut held = Self {
            path: path.into(),
            directory,
            file,
            digest: String::new(),
        };
        held.check()?;
        if immutable {
            held.digest = crate::digest(&held.bytes(256 * 1024)?);
        }
        Ok(held)
    }
    pub fn check(&self) -> Result<()> {
        let d = self.directory.metadata()?;
        let f = self.file.metadata()?;
        let cd = std::fs::symlink_metadata(self.path.parent().ok_or(Error::Denied)?)?;
        let cf = std::fs::symlink_metadata(&self.path)?;
        if !private(&d, true)
            || !private(&cd, true)
            || !same(&d, &cd)
            || !private(&f, false)
            || !private(&cf, false)
            || !same(&f, &cf)
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    pub fn bytes(&self, max: usize) -> Result<Vec<u8>> {
        self.check()?;
        if self.file.metadata()?.len() > max as u64 {
            return Err(Error::Denied);
        }
        let mut bytes = vec![0; max + 1];
        let mut offset = 0;
        while offset < bytes.len() {
            let got = self.file.read_at(&mut bytes[offset..], offset as u64)?;
            if got == 0 {
                break;
            }
            offset += got;
        }
        if offset > max {
            return Err(Error::Denied);
        }
        bytes.truncate(offset);
        self.check()?;
        if !self.digest.is_empty() && crate::digest(&bytes) != self.digest {
            return Err(Error::Denied);
        }
        Ok(bytes)
    }
    pub fn token(&self) -> Result<String> {
        Ok(std::str::from_utf8(&self.bytes(4096)?)
            .map_err(|_| Error::Denied)?
            .trim()
            .to_owned())
    }
}
pub(crate) fn socket(path: &Path) -> Result<()> {
    use std::os::unix::fs::FileTypeExt;
    let m = std::fs::symlink_metadata(path)?;
    let p = std::fs::symlink_metadata(path.parent().ok_or(Error::Denied)?)?;
    if !private(&p, true)
        || !m.file_type().is_socket()
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
    {
        return Err(Error::Denied);
    }
    Ok(())
}
