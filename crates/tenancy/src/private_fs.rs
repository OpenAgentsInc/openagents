//! Owner-private files and directories, the custody the stores rest on.
//!
//! On Unix these helpers set and read mode bits, open flags, ownership, and
//! file identity exactly as the stores always have. Other platforms have no
//! equivalent, so every helper fails closed there: opens with a mode or
//! flags return an `Unsupported` error, and every metadata check reports
//! the file as not private.

use std::fs::{Metadata, OpenOptions};
use std::io;
use std::path::Path;

/// Open flags for [`flags`], zero outside Unix.
#[cfg(unix)]
pub(crate) use libc::{O_CLOEXEC, O_DIRECTORY, O_NOFOLLOW, O_NONBLOCK};
#[cfg(not(unix))]
pub(crate) const O_CLOEXEC: i32 = 0;
#[cfg(not(unix))]
pub(crate) const O_DIRECTORY: i32 = 0;
#[cfg(not(unix))]
pub(crate) const O_NOFOLLOW: i32 = 0;
#[cfg(not(unix))]
pub(crate) const O_NONBLOCK: i32 = 0;

#[cfg(not(unix))]
fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "private stores need Unix permissions",
    )
}

/// Set the mode a newly created file gets.
pub(crate) fn mode(options: &mut OpenOptions, mode: u32) -> io::Result<&mut OpenOptions> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        Ok(options.mode(mode))
    }
    #[cfg(not(unix))]
    {
        let _ = (options, mode);
        Err(unsupported())
    }
}

/// Add open flags such as [`O_NOFOLLOW`].
pub(crate) fn flags(options: &mut OpenOptions, flags: i32) -> io::Result<&mut OpenOptions> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        Ok(options.custom_flags(flags))
    }
    #[cfg(not(unix))]
    {
        let _ = (options, flags);
        Err(unsupported())
    }
}

/// Create a directory only its owner can reach.
pub(crate) fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(path)
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err(unsupported())
    }
}

/// Whether two metadata records name the same file. Never true outside Unix.
pub(crate) fn same_file(a: &Metadata, b: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.dev() == b.dev() && a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        false
    }
}

/// The file's hard link count, zero outside Unix.
pub(crate) fn nlink(meta: &Metadata) -> u64 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        meta.nlink()
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        0
    }
}

/// Whether none of the `mask` mode bits are set. Never true outside Unix.
pub(crate) fn mode_clear(meta: &Metadata, mask: u32) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & mask == 0
    }
    #[cfg(not(unix))]
    {
        let _ = (meta, mask);
        false
    }
}

/// Whether the effective user owns the file. Never true outside Unix.
pub(crate) fn owned(meta: &Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // SAFETY: `geteuid` has no preconditions and cannot fail.
        meta.uid() == unsafe { libc::geteuid() }
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        false
    }
}
