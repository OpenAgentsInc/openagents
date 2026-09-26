//! Source enumeration and reads anchored to an admitted directory descriptor.
use crate::{ErrorCode, Result, error};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::{
    ffi::{OsStrExt, OsStringExt},
    fs::{MetadataExt, OpenOptionsExt},
};
use std::{
    ffi::{CStr, CString, OsString},
    fs::{File, OpenOptions},
    path::{Component, Path, PathBuf},
};
pub(crate) struct Anchor {
    root: PathBuf,
    directory: File,
}
impl Anchor {
    pub fn new(root: &Path, device: u64, inode: u64) -> Result<Self> {
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC)
            .open(root)
            .map_err(|_| error(ErrorCode::SourceChanged, "Gym root is unavailable"))?;
        let m = directory
            .metadata()
            .map_err(|_| error(ErrorCode::SourceChanged, "Gym root metadata unavailable"))?;
        if m.dev() != device || m.ino() != inode {
            return Err(error(ErrorCode::SourceChanged, "Gym root was replaced"));
        }
        Ok(Self {
            root: root.into(),
            directory,
        })
    }
    pub fn open(&self, path: &Path, directory: bool) -> std::io::Result<File> {
        let relative = path
            .strip_prefix(&self.root)
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
        let parts = relative.components().collect::<Vec<_>>();
        if parts.len() > 8 || parts.iter().any(|p| !matches!(p, Component::Normal(_))) {
            return Err(std::io::ErrorKind::PermissionDenied.into());
        }
        let mut current = self.directory.try_clone()?;
        for (index, part) in parts.iter().enumerate() {
            let Component::Normal(name) = part else {
                return Err(std::io::ErrorKind::PermissionDenied.into());
            };
            let name = CString::new(name.as_bytes())
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::PermissionDenied))?;
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if directory || index + 1 < parts.len() {
                    libc::O_DIRECTORY
                } else {
                    0
                };
            // SAFETY: the descriptor is live and name is one NUL-terminated component.
            let fd = unsafe { libc::openat(current.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(std::io::Error::last_os_error());
            }
            // SAFETY: openat returned a new owned descriptor.
            current = unsafe { File::from_raw_fd(fd) };
        }
        Ok(current)
    }
    pub fn names(&self, path: &Path, cap: usize) -> std::io::Result<(Vec<OsString>, bool)> {
        let fd = self.open(path, true)?.into_raw_fd();
        // SAFETY: fdopendir takes ownership on success.
        let ptr = unsafe { libc::fdopendir(fd) };
        if ptr.is_null() {
            unsafe { libc::close(fd) };
            return Err(std::io::Error::last_os_error());
        }
        struct Directory(*mut libc::DIR);
        impl Drop for Directory {
            fn drop(&mut self) {
                unsafe { libc::closedir(self.0) };
            }
        }
        let dir = Directory(ptr);
        let mut names = Vec::new();
        let mut complete = true;
        loop {
            // SAFETY: the live stream is accessed only by this function.
            let entry = unsafe { libc::readdir(dir.0) };
            if entry.is_null() {
                break;
            }
            // SAFETY: d_name is terminated and valid until the next readdir.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            if names.len() >= cap {
                complete = false;
                break;
            }
            names.push(OsString::from_vec(name.to_vec()));
        }
        Ok((names, complete))
    }
}
