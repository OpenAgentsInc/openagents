//! The control socket itself: a Unix socket, mode `0600`, in a directory of
//! mode `0700`, that serves only a peer whose user ID equals the host's.
//!
//! The peer's user ID comes from the kernel (`getpeereid` on macOS,
//! `SO_PEERCRED` on Linux, both through `UnixStream::peer_cred`), never
//! from anything the peer sends. A peer that fails the check is closed
//! before the host reads a byte from it.

use std::os::fd::{AsFd, AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
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

    /// A second descriptor for the listener, closed on `exec` until
    /// [`hand_over`] keeps it open for the next program.
    pub(crate) fn keep(&self) -> Option<OwnedFd> {
        self.listener.as_fd().try_clone_to_owned().ok()
    }
}

/// Keep `fd` open across `exec` and name it for the program that follows:
/// the value of [`super::HANDOVER_ENV`], `PID:FD`. The PID is this
/// process's, which `exec` keeps, so a child that inherits the variable
/// never takes a descriptor that is not its own.
pub(crate) fn hand_over(fd: OwnedFd) -> Option<String> {
    let raw = fd.into_raw_fd();
    // SAFETY: `raw` is an open descriptor this process owns; F_GETFD and
    // F_SETFD read and write only its flags.
    unsafe {
        let flags = libc::fcntl(raw, libc::F_GETFD);
        if flags < 0 || libc::fcntl(raw, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
            drop(OwnedFd::from_raw_fd(raw));
            return None;
        }
    }
    Some(format!("{}:{raw}", std::process::id()))
}

/// The listener the program this one replaced handed over for `path`
/// ([`hand_over`]), when [`super::HANDOVER_ENV`] names one for this
/// process and it is a listening socket bound there. It is closed on
/// `exec` again, until the next hand-over.
fn adopt(path: &Path) -> Option<UnixListener> {
    let value = std::env::var(super::HANDOVER_ENV).ok()?;
    let (pid, fd) = value.split_once(':')?;
    if pid.parse::<u32>().ok()? != std::process::id() {
        return None;
    }
    let raw: i32 = fd.parse().ok()?;
    // SAFETY: fstat writes only `stat`; a descriptor that is not open
    // fails it.
    let is_socket = unsafe {
        let mut stat: libc::stat = std::mem::zeroed();
        libc::fstat(raw, &raw mut stat) == 0 && (stat.st_mode & libc::S_IFMT) == libc::S_IFSOCK
    };
    if !is_socket {
        return None;
    }
    // SAFETY: the variable names this very process, so the descriptor is
    // the one the previous program left open for it, and nothing else in
    // this process owns it yet.
    let listener = unsafe { std::os::unix::net::UnixListener::from_raw_fd(raw) };
    let bound_here = listener
        .local_addr()
        .ok()
        .and_then(|address| address.as_pathname().map(Path::to_path_buf))
        .is_some_and(|bound| bound == path);
    if !bound_here {
        return None;
    }
    // SAFETY: as above; only the descriptor's flags change.
    unsafe {
        let flags = libc::fcntl(listener.as_raw_fd(), libc::F_GETFD);
        if flags >= 0 {
            libc::fcntl(
                listener.as_raw_fd(),
                libc::F_SETFD,
                flags | libc::FD_CLOEXEC,
            );
        }
    }
    listener.set_nonblocking(true).ok()?;
    UnixListener::from_std(listener).ok()
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
    // A restart for an update hands the bound socket over: clients that
    // connected while it started wait in the socket's queue.
    if let Some(listener) = adopt(path) {
        return Ok(Bound {
            listener,
            path: path.to_owned(),
            uid,
        });
    }
    // A Unix socket address holds at most 104 bytes on macOS and 108 on
    // Linux, terminator included.
    if path.as_os_str().len() >= 104 {
        return Err(failed(&format!(
            "path is too long ({} bytes; at most 103): pass --control-socket with a shorter path, \
             such as /tmp/openagents-host.sock",
            path.as_os_str().len()
        )));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A restart hands the bound socket to the next program: a client
    /// that connects after the old listener is gone, before the new one
    /// serves, waits in the queue and is accepted by the new one. The
    /// variable names this process; another PID's is refused.
    #[tokio::test]
    async fn a_handed_over_socket_keeps_its_queue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c/control.sock");
        let old = bind(&path, own_uid()).await.unwrap();
        let kept = old.keep().unwrap();
        drop(old);
        // The old program is gone; a client connects anyway.
        let early = UnixStream::connect(&path).await.unwrap();
        let value = hand_over(kept).unwrap();
        let (_, fd) = value.split_once(':').unwrap();
        // SAFETY: this test alone reads the variable, and sets it before
        // the bind below reads it.
        unsafe {
            std::env::set_var(super::super::HANDOVER_ENV, format!("1:{fd}"));
        }
        assert!(adopt(&path).is_none(), "another process's descriptor");
        unsafe {
            std::env::set_var(super::super::HANDOVER_ENV, &value);
        }
        let new = bind(&path, own_uid()).await.unwrap();
        unsafe {
            std::env::remove_var(super::super::HANDOVER_ENV);
        }
        let (accepted, _) = new.listener.accept().await.unwrap();
        assert_eq!(peer_uid(&accepted), Some(own_uid()));
        drop(early);
    }
}
