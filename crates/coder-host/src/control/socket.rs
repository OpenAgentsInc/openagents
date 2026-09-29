//! The control socket itself: a Unix socket, mode `0600`, in a directory of
//! mode `0700`, that serves only a peer whose user ID equals the host's.
//!
//! The peer's user ID comes from the kernel (`getpeereid` on macOS,
//! `SO_PEERCRED` on Linux, both through `UnixStream::peer_cred`), never
//! from anything the peer sends. A peer that fails the check is closed
//! before the host reads a byte from it.

use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use tokio::net::{UnixListener, UnixStream};

use crate::{Error, Result};

/// A bound control socket and the user ID it admits.
#[derive(Debug)]
pub struct Bound {
    pub(crate) listener: UnixListener,
    pub(crate) path: PathBuf,
    pub(crate) uid: u32,
}

impl Bound {
    /// Where the socket is.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// This process's effective user ID.
#[must_use]
pub fn own_uid() -> u32 {
    // SAFETY: geteuid has no pointers or preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

/// Bind the socket at `path`, admitting peers whose user ID is `uid`.
///
/// The directory is created with mode `0700`, or must already be a
/// directory this user owns; its mode is then set to `0700`. A socket file
/// left by a host that is gone is replaced; a socket another host still
/// answers on refuses the bind, so two hosts never share one socket.
///
/// # Errors
/// Refuses a directory another user owns, a path that is not a socket, a
/// live socket, or a failed bind.
pub async fn bind(path: &Path, uid: u32) -> Result<Bound> {
    let failed = |what: &str| Error::Config(format!("the control socket {what}"));
    // A Unix socket address holds at most 104 bytes on macOS and 108 on
    // Linux, terminator included.
    if path.as_os_str().len() >= 104 {
        return Err(failed("path is too long; choose a shorter one"));
    }
    let directory = path
        .parent()
        .ok_or_else(|| failed("path has no directory"))?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)
        .map_err(|_| failed("directory cannot be created"))?;
    let metadata =
        std::fs::symlink_metadata(directory).map_err(|_| failed("directory cannot be read"))?;
    if !metadata.is_dir() || metadata.uid() != own_uid() {
        return Err(failed("directory is not a directory this user owns"));
    }
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| failed("directory mode cannot be set"))?;
    match std::fs::symlink_metadata(path) {
        Ok(existing) => {
            use std::os::unix::fs::FileTypeExt;
            if !existing.file_type().is_socket() {
                return Err(failed("path holds something other than a socket"));
            }
            if UnixStream::connect(path).await.is_ok() {
                return Err(failed("is already served by another host"));
            }
            std::fs::remove_file(path).map_err(|_| failed("left by a stopped host stays"))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(failed("path cannot be read")),
    }
    let listener = UnixListener::bind(path).map_err(|_| failed("cannot bind"))?;
    // The directory already keeps other users out; the socket's own mode
    // says the same thing to anyone who looks.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|_| failed("mode cannot be set"))?;
    Ok(Bound {
        listener,
        path: path.to_owned(),
        uid,
    })
}

/// Whether a peer with user ID `peer` may use a socket that admits `uid`.
#[must_use]
pub fn admits(uid: u32, peer: Option<u32>) -> bool {
    peer == Some(uid)
}

/// The kernel's user ID for the peer, or `None` when it cannot be read.
#[must_use]
pub fn peer_uid(stream: &UnixStream) -> Option<u32> {
    stream.peer_cred().ok().map(|cred| cred.uid())
}
