//! What can go wrong, in terms a person at the client can act on.

use std::fmt;

/// Why an SSH host operation did not complete.
#[derive(Debug)]
pub enum Error {
    /// The destination is not one this crate passes to `ssh`.
    InvalidDestination(String),
    /// The release does not describe a usable pinned artifact set.
    InvalidRelease(String),
    /// The runner arguments cannot be sent safely.
    InvalidRunner(String),
    /// The `ssh` binary could not be started.
    Spawn(String),
    /// `ssh` itself failed: the connection, authentication, or the remote
    /// shell. `detail` is the end of its diagnostic output.
    Ssh {
        /// The exit code of the `ssh` process, when it had one.
        code: Option<i32>,
        /// The last part of `ssh`'s standard error.
        detail: String,
    },
    /// An operation ran past its wall-clock bound and was stopped.
    TimedOut(&'static str),
    /// The remote operating system or architecture has no release build.
    Unsupported {
        /// The remote `uname -s` value.
        os: String,
        /// The remote `uname -m` value.
        arch: String,
    },
    /// The release has no archive for the remote platform.
    NoArtifact {
        /// The remote operating system.
        os: String,
        /// The remote architecture.
        arch: String,
    },
    /// The local archive does not match its pinned digest, so it was not
    /// sent.
    LocalChecksumMismatch(String),
    /// The archive that arrived on the remote machine does not match its
    /// pinned digest. Nothing was installed.
    ChecksumMismatch,
    /// The archive could not be extracted or holds no `coder` binary.
    BadArchive,
    /// The extracted binary did not answer `--version`, so it was not
    /// installed.
    BinaryRejected,
    /// The remote machine lacks a tool the script needs.
    MissingTool(String),
    /// Another launcher holds the remote installation lock.
    Busy,
    /// The host process exited or did not report its port in time.
    HostDidNotStart,
    /// The pinned version is not installed on the remote machine.
    NotInstalled,
    /// The invitation command failed or printed something other than one
    /// invitation line.
    Invitation(String),
    /// The remote script reported an error this crate does not name.
    Remote(String),
    /// The remote output did not follow the script's result format.
    Protocol(String),
    /// A local file or process operation failed.
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidDestination(why) => write!(f, "invalid SSH destination: {why}"),
            Error::InvalidRelease(why) => write!(f, "invalid release: {why}"),
            Error::InvalidRunner(why) => write!(f, "invalid runner arguments: {why}"),
            Error::Spawn(why) => write!(f, "could not start ssh: {why}"),
            Error::Ssh { code, detail } => {
                match code {
                    Some(code) => write!(f, "ssh exited with code {code}")?,
                    None => write!(f, "ssh stopped without an exit code")?,
                }
                if detail.is_empty() {
                    Ok(())
                } else {
                    write!(f, ": {detail}")
                }
            }
            Error::TimedOut(what) => write!(f, "{what} ran past its time limit and was stopped"),
            Error::Unsupported { os, arch } => {
                write!(f, "no Coder build supports the remote platform {os} {arch}")
            }
            Error::NoArtifact { os, arch } => {
                write!(f, "the release has no archive for {os} {arch}")
            }
            Error::LocalChecksumMismatch(path) => {
                write!(
                    f,
                    "the local archive {path} does not match its pinned SHA-256"
                )
            }
            Error::ChecksumMismatch => write!(
                f,
                "the archive received by the remote machine does not match its pinned SHA-256; nothing was installed"
            ),
            Error::BadArchive => write!(f, "the archive holds no usable coder binary"),
            Error::BinaryRejected => {
                write!(
                    f,
                    "the extracted coder binary did not run on the remote machine"
                )
            }
            Error::MissingTool(tool) => write!(f, "the remote machine lacks {tool}"),
            Error::Busy => write!(f, "another launcher holds the remote installation lock"),
            Error::HostDidNotStart => {
                write!(
                    f,
                    "the remote host exited or did not report its port in time"
                )
            }
            Error::NotInstalled => write!(
                f,
                "the pinned version is not installed on the remote machine"
            ),
            Error::Invitation(why) => write!(f, "the invitation command failed: {why}"),
            Error::Remote(code) => write!(f, "the remote script failed: {code}"),
            Error::Protocol(why) => write!(f, "unexpected remote output: {why}"),
            Error::Io(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Io(error)
    }
}
