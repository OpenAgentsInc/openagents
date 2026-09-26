use super::digest;
use crate::{Error, Harness};
use std::ffi::{CStr, CString, OsString};
use std::fs::{File, Metadata, OpenOptions};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

pub const MAX_ENTRIES: usize = 100_000;
pub const MAX_SOURCE_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PARSE_BYTES: usize = 256 * 1024;

pub struct Root {
    pub harness: Harness,
    pub id: String,
    directory: File,
}

impl Root {
    pub fn open(harness: Harness, path: PathBuf) -> Result<Self, Error> {
        if !path.is_absolute()
            || std::fs::symlink_metadata(&path)
                .map_err(|_| Error::InvalidRoot)?
                .file_type()
                .is_symlink()
        {
            return Err(Error::InvalidRoot);
        }
        let canonical = path.canonicalize().map_err(|_| Error::InvalidRoot)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_DIRECTORY)
            .open(&canonical)
            .map_err(|_| Error::InvalidRoot)?;
        let meta = directory.metadata().map_err(|_| Error::InvalidRoot)?;
        if !meta.is_dir() {
            return Err(Error::InvalidRoot);
        }
        let mut identity = canonical.as_os_str().as_bytes().to_vec();
        identity.extend_from_slice(
            format!("\0{:?}\0{}:{}", harness, meta.dev(), meta.ino()).as_bytes(),
        );
        Ok(Self {
            harness,
            id: digest(&identity),
            directory,
        })
    }

    pub fn source_id(&self, relative: &Path) -> String {
        let mut bytes = self.id.as_bytes().to_vec();
        bytes.push(0);
        bytes.extend_from_slice(relative.as_os_str().as_bytes());
        digest(&bytes)
    }

    pub fn open_file(&self, relative: &Path) -> Result<File, Error> {
        let file = self.open_relative(relative, false)?;
        let meta = file.metadata().map_err(|_| Error::SourceUnreadable)?;
        if !meta.is_file() {
            return Err(Error::SourceUnreadable);
        }
        Ok(file)
    }

    pub fn open_dir(&self, relative: &Path) -> Result<File, Error> {
        self.open_relative(relative, true)
    }

    fn open_relative(&self, relative: &Path, directory: bool) -> Result<File, Error> {
        let parts: Vec<_> = relative.components().collect();
        if parts.is_empty()
            || parts.len() > 16
            || parts.iter().any(|p| !matches!(p, Component::Normal(_)))
        {
            return Err(Error::InvalidRequest);
        }
        let mut current = self
            .directory
            .try_clone()
            .map_err(|_| Error::SourceUnreadable)?;
        for (index, part) in parts.iter().enumerate() {
            let Component::Normal(name) = part else {
                return Err(Error::InvalidRequest);
            };
            let name = CString::new(name.as_bytes()).map_err(|_| Error::InvalidRequest)?;
            let flags = libc::O_RDONLY
                | libc::O_CLOEXEC
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | if index + 1 < parts.len() || directory {
                    libc::O_DIRECTORY
                } else {
                    0
                };
            // SAFETY: current is a live directory descriptor and name is a
            // NUL-terminated single path component. No symlink is followed.
            let fd = unsafe { libc::openat(current.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                let err = std::io::Error::last_os_error();
                return Err(if err.kind() == std::io::ErrorKind::NotFound {
                    Error::SourceMissing
                } else {
                    Error::SourceUnreadable
                });
            }
            // SAFETY: openat returned a newly owned descriptor.
            current = unsafe { File::from_raw_fd(fd) };
        }
        Ok(current)
    }
}

/// Enumerate names through the held directory descriptor, not a mutable path.
pub fn names(directory: &File) -> Result<Vec<OsString>, Error> {
    let fd = directory
        .try_clone()
        .map_err(|_| Error::SourceUnreadable)?
        .into_raw_fd();
    // SAFETY: fd is owned here; fdopendir takes ownership on success.
    let ptr = unsafe { libc::fdopendir(fd) };
    if ptr.is_null() {
        // SAFETY: fdopendir failed without taking ownership of fd.
        unsafe {
            libc::close(fd);
        }
        return Err(Error::SourceUnreadable);
    }
    struct Directory(*mut libc::DIR);
    impl Drop for Directory {
        fn drop(&mut self) {
            // SAFETY: this wrapper uniquely owns the successful fdopendir.
            unsafe {
                libc::closedir(self.0);
            }
        }
    }
    let dir = Directory(ptr);
    let mut out = Vec::new();
    loop {
        // SAFETY: the platform errno slot is thread-local and valid here.
        unsafe {
            *errno_slot() = 0;
        }
        // SAFETY: readdir uses the live stream, exclusively within this call.
        let item = unsafe { libc::readdir(dir.0) };
        if item.is_null() {
            // SAFETY: this reads the same thread-local errno slot.
            if unsafe { *errno_slot() } != 0 {
                return Err(Error::SourceUnreadable);
            }
            break;
        }
        // SAFETY: d_name is a NUL-terminated name valid until the next readdir.
        let name = unsafe { CStr::from_ptr((*item).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if out.len() >= MAX_ENTRIES {
            return Err(Error::ResourceLimit);
        }
        out.push(OsString::from_vec(name.to_vec()));
    }
    out.sort();
    Ok(out)
}

fn errno_slot() -> *mut libc::c_int {
    #[cfg(target_os = "linux")]
    // SAFETY: libc returns this thread's errno storage.
    unsafe {
        libc::__errno_location()
    }
    #[cfg(target_os = "macos")]
    // SAFETY: Darwin exposes their thread-local errno through __error.
    unsafe {
        libc::__error()
    }
}

pub fn incarnation(metadata: &Metadata) -> String {
    let created = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|t| t.as_nanos());
    digest(format!("{}:{}:{created:?}", metadata.dev(), metadata.ino()).as_bytes())
}

pub enum Kind {
    Directory,
    File,
    Symlink,
    Other,
}

pub fn kind(directory: &File, name: &std::ffi::OsStr) -> Result<Kind, Error> {
    let name = CString::new(name.as_bytes()).map_err(|_| Error::InvalidRequest)?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    // SAFETY: the descriptor and C string are live, and fstatat initializes
    // stat on success. The final symlink is inspected rather than followed.
    if unsafe {
        libc::fstatat(
            directory.as_raw_fd(),
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        return Err(Error::SourceUnreadable);
    }
    // SAFETY: fstatat succeeded and initialized the structure.
    let mode = unsafe { stat.assume_init() }.st_mode & libc::S_IFMT;
    Ok(match mode {
        libc::S_IFDIR => Kind::Directory,
        libc::S_IFREG => Kind::File,
        libc::S_IFLNK => Kind::Symlink,
        _ => Kind::Other,
    })
}
