//! Linux guest execution. Call `restrict` only in the transfer subprocess.

use crate::{FileError, MAX_BYTES, digest, validate_path};
use std::ffi::CString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::fs::MetadataExt;

fn errno() -> FileError {
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ENOENT | libc::ENOTDIR) => FileError::Missing,
        Some(libc::ELOOP | libc::EXDEV | libc::EACCES | libc::EPERM) => FileError::Escape,
        Some(libc::EAGAIN | libc::ESTALE) => FileError::Changed,
        Some(libc::EEXIST) => FileError::Exists,
        Some(libc::ENOSYS | libc::EOPNOTSUPP) => FileError::Unsupported,
        _ => FileError::Unavailable,
    }
}
fn cstr(s: &str) -> Result<CString, FileError> {
    CString::new(s).map_err(|_| FileError::Invalid)
}
fn owned(fd: i32) -> Result<OwnedFd, FileError> {
    if fd < 0 {
        Err(errno())
    } else {
        // SAFETY: A successful syscall returns a new descriptor owned here.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}
#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}
fn open_beneath(root: RawFd, path: &str, flags: i32) -> Result<OwnedFd, FileError> {
    let path = cstr(path)?;
    let how = OpenHow {
        flags: (flags | libc::O_CLOEXEC) as u64,
        mode: 0,
        // NO_XDEV, NO_MAGICLINKS, NO_SYMLINKS, BENEATH.
        resolve: 0x01 | 0x02 | 0x04 | 0x08,
    };
    // SAFETY: The path and fixed-layout argument live through the syscall.
    owned(unsafe {
        libc::syscall(
            libc::SYS_openat2,
            root,
            path.as_ptr(),
            &how,
            size_of::<OpenHow>(),
        ) as i32
    })
}

/// Open the fixed workspace beneath the explicitly supplied guest home.
pub fn workspace(home: &str, create: bool) -> Result<OwnedFd, FileError> {
    let home = cstr(home)?;
    // SAFETY: `home` is a valid C string, with no output pointers.
    let parent = owned(unsafe {
        libc::open(
            home.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    })?;
    if create {
        // SAFETY: The parent descriptor is valid; the name is a constant C string.
        let r = unsafe { libc::mkdirat(parent.as_raw_fd(), c"workspace".as_ptr(), 0o700) };
        if r != 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EEXIST) {
            return Err(errno());
        }
    }
    open_beneath(
        parent.as_raw_fd(),
        "workspace",
        libc::O_PATH | libc::O_DIRECTORY,
    )
}

#[repr(C)]
struct RulesetAttr {
    handled_access_fs: u64,
}
#[repr(C, packed)]
struct PathBeneathAttr {
    allowed_access: u64,
    parent_fd: i32,
}

/// Restrict all later file opens and mutations, including renamed directories.
/// Fail closed unless the kernel supports Landlock ABI3 and openat2.
pub fn restrict(root: RawFd) -> Result<(), FileError> {
    // SAFETY: The version query takes no pointers.
    let abi = unsafe { libc::syscall(libc::SYS_landlock_create_ruleset, 0, 0, 1) };
    if abi < 3 {
        return Err(FileError::Unsupported);
    }
    let rules = RulesetAttr {
        handled_access_fs: (1 << 15) - 1,
    };
    // SAFETY: The structure has the kernel ABI layout and remains live.
    let fd = owned(unsafe {
        libc::syscall(
            libc::SYS_landlock_create_ruleset,
            &rules,
            size_of::<RulesetAttr>(),
            0,
        ) as i32
    })
    .map_err(|_| FileError::Unsupported)?;
    // Read/write regular files and directories; create, replace, and remove files.
    // The REFER rule prevents a moved directory from granting access elsewhere.
    let path = PathBeneathAttr {
        allowed_access: (1 << 1)
            | (1 << 2)
            | (1 << 3)
            | (1 << 5)
            | (1 << 8)
            | (1 << 13)
            | (1 << 14),
        parent_fd: root,
    };
    // SAFETY: The kernel reads the fixed-layout path rule; fd is owned here.
    if unsafe { libc::syscall(libc::SYS_landlock_add_rule, fd.as_raw_fd(), 1, &path, 0) } != 0 {
        return Err(FileError::Unsupported);
    }
    // SAFETY: No pointer arguments; this only narrows the current subprocess.
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0
        || unsafe { libc::syscall(libc::SYS_landlock_restrict_self, fd.as_raw_fd(), 0) } != 0
    {
        return Err(FileError::Unsupported);
    }
    Ok(())
}

fn regular(root: RawFd, path: &str) -> Result<File, FileError> {
    let fd = open_beneath(root, path, libc::O_PATH)?;
    let probe = File::from(fd);
    let m = probe.metadata().map_err(|_| FileError::Unavailable)?;
    if !m.is_file() {
        return Err(FileError::Special);
    }
    // Reopen only the pinned regular inode. Never open a device or FIFO for I/O.
    let file = File::open(format!("/proc/self/fd/{}", probe.as_raw_fd())).map_err(|_| errno())?;
    let actual = file.metadata().map_err(|_| FileError::Unavailable)?;
    if (m.dev(), m.ino()) != (actual.dev(), actual.ino()) {
        return Err(FileError::Changed);
    }
    Ok(file)
}
fn signature(m: &std::fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
pub fn read(root: RawFd, path: &str) -> Result<Vec<u8>, FileError> {
    validate_path(path)?;
    let mut file = regular(root, path)?;
    // A read lease excludes existing writable opens and delays new writers.
    // SIGIO requests a lease break; the final GETLEASE check refuses that read.
    // The helper is a single-threaded subprocess, so changing its signal action
    // cannot affect the API server or another transfer.
    // SAFETY: SIG_IGN is a defined signal action; fcntl uses an owned descriptor.
    unsafe {
        libc::signal(libc::SIGIO, libc::SIG_IGN);
    }
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLEASE, libc::F_RDLCK) } != 0 {
        return Err(
            if std::io::Error::last_os_error().raw_os_error() == Some(libc::EAGAIN) {
                FileError::Changed
            } else {
                FileError::Unsupported
            },
        );
    }
    let before = file.metadata().map_err(|_| FileError::Unavailable)?;
    if before.len() > MAX_BYTES as u64 {
        return Err(FileError::TooLarge);
    }
    let mut bytes = Vec::with_capacity(before.len() as usize);
    Read::by_ref(&mut file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| FileError::Unavailable)?;
    if bytes.len() > MAX_BYTES {
        return Err(FileError::TooLarge);
    }
    let after = file.metadata().map_err(|_| FileError::Unavailable)?;
    // SAFETY: Querying the lease does not mutate memory or filesystem data.
    let lease = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLEASE) };
    if lease != libc::F_RDLCK
        || signature(&before) != signature(&after)
        || bytes.len() as u64 != before.len()
    {
        return Err(FileError::Changed);
    }
    Ok(bytes)
}

/// Write an already length- and digest-checked body, then publish it atomically.
/// Existing parent directories are required. Repeating a create-only upload of
/// identical bytes succeeds; another file at that path returns `Exists`.
pub fn write(root: RawFd, path: &str, bytes: &[u8], overwrite: bool) -> Result<(), FileError> {
    validate_path(path)?;
    if bytes.len() > MAX_BYTES {
        return Err(FileError::TooLarge);
    }
    let (parent, name) = path.rsplit_once('/').unwrap_or((".", path));
    let dir = open_beneath(root, parent, libc::O_RDONLY | libc::O_DIRECTORY)?;
    match regular(root, path) {
        Ok(_) if !overwrite => {
            return if read(root, path)? == bytes {
                Ok(())
            } else {
                Err(FileError::Exists)
            };
        }
        Ok(_) | Err(FileError::Missing) => {}
        Err(e) => return Err(e),
    }
    let mut random = [0u8; 16];
    // SAFETY: The buffer is writable for its full declared length.
    if unsafe { libc::getrandom(random.as_mut_ptr().cast(), random.len(), 0) }
        != random.len() as isize
    {
        return Err(FileError::Unavailable);
    }
    let suffix = digest(&random);
    let temp = cstr(&format!(".oa-upload-{}", &suffix[..32]))?;
    // SAFETY: Directory is open and the temporary name has no separators.
    let fd = owned(unsafe {
        libc::openat(
            dir.as_raw_fd(),
            temp.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    })?;
    let result = (|| {
        let mut file = File::from(fd);
        // A private random name plus an exclusive lease protects the new inode
        // while it is filled. Verify that publication names that same inode.
        // SAFETY: These operations only affect this helper and its owned file.
        unsafe {
            libc::signal(libc::SIGIO, libc::SIG_IGN);
        }
        if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLEASE, libc::F_WRLCK) } != 0 {
            return Err(FileError::Changed);
        }
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| FileError::Unavailable)?;
        let name = cstr(name)?;
        // SAFETY: Both names are fixed components beneath the same open parent.
        // Landlock checks its current ancestry if a command moved the directory.
        let r = unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                dir.as_raw_fd(),
                temp.as_ptr(),
                dir.as_raw_fd(),
                name.as_ptr(),
                if overwrite { 0 } else { libc::RENAME_NOREPLACE },
            )
        };
        if r != 0 {
            let e = errno();
            if e == FileError::Exists && !overwrite && read(root, path)? == bytes {
                return Ok(());
            }
            return Err(e);
        }
        // SAFETY: Flush the containing directory after the atomic publication.
        if unsafe { libc::fsync(dir.as_raw_fd()) } != 0 {
            return Err(FileError::WriteUncertain);
        }
        let published = open_beneath(root, path, libc::O_PATH).map_err(|_| FileError::Changed)?;
        let actual = File::from(published)
            .metadata()
            .map_err(|_| FileError::Changed)?;
        let written = file.metadata().map_err(|_| FileError::Changed)?;
        // SAFETY: A query on our still-open file descriptor.
        let lease = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETLEASE) };
        if (actual.dev(), actual.ino()) != (written.dev(), written.ino()) || lease != libc::F_WRLCK
        {
            return Err(FileError::Changed);
        }
        Ok(())
    })();
    // SAFETY: This removes only our random temporary entry, if it still exists.
    unsafe { libc::unlinkat(dir.as_raw_fd(), temp.as_ptr(), 0) };
    result
}
