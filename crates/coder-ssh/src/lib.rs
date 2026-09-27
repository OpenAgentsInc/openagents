//! Install, start or adopt, and reach a Coder host over SSH.
//!
//! You name an SSH destination: a `Host` alias from your SSH configuration,
//! `user@host`, or anything else the system `ssh` binary accepts. A
//! [`Launcher`] then does four things, each through a separate `ssh`
//! invocation:
//!
//! 1. [`Launcher::up`] sends a fixed POSIX `sh` script on standard input.
//!    The script detects the remote operating system and architecture,
//!    installs the pinned release archive for that platform into
//!    `~/.openagents/ssh-host/` when it is missing, and then adopts a host
//!    that already runs for that user or starts one.
//! 2. [`Launcher::connect`] forwards a reserved local loopback port to the
//!    host's loopback port with `ssh -N -L`.
//! 3. [`Launcher::invite`] runs the host's local invitation command and
//!    returns the invitation to the caller, who redeems it.
//! 4. [`Launcher::remove`] stops a host that this launcher started, and only
//!    detaches from a host that it adopted.
//!
//! # Ownership
//!
//! The remote machine records who started a host. A host this launcher
//! started is [`Ownership::Managed`]; a host that was already running is
//! [`Ownership::External`]. Only an explicit [`Launcher::remove`], or a
//! [`Launcher::up`] whose release or serve arguments changed, stops a
//! managed host, and nothing in this crate stops an external one. Dropping a
//! [`Launcher`], a [`Host`], or a [`Tunnel`], a tunnel that dies, and a
//! client process that exits without cleanup all leave the remote host
//! running. The host outlives the connection on purpose: a phone, another
//! computer, or the next terminal session can reach the same host later.
//!
//! # Transport
//!
//! This crate runs the system `ssh` binary directly, never through a local
//! shell, and uses no SSH library. It resolves a destination with `ssh -G`
//! and disables connection sharing (`ControlMaster=no`) for its own
//! connections, so a multiplexed master that another program owns cannot
//! carry Coder's traffic or outlive it. Password and passphrase prompts go
//! through a one-shot askpass helper that reads each answer from a named
//! pipe, never from an environment variable, and that is removed when the
//! invocation ends. See [`Prompter`].
//!
//! Every call blocks. An asynchronous host runs them on a blocking thread.
//!
//! # Enrollment
//!
//! SSH authorizes enrollment once: [`Launcher::invite`] returns an
//! [`Invitation`] that the caller redeems through the host's access
//! contract. After that, the host's grant governs access, not the SSH login.

#[cfg(not(unix))]
compile_error!("coder-ssh runs the system ssh binary through Unix process groups");

mod askpass;
mod error;
mod launcher;
mod protocol;
mod ssh;
mod tunnel;

pub use askpass::{Prompter, Secret};
pub use error::Error;
pub use launcher::{
    Arch, Artifact, Host, Install, Invitation, Launcher, Os, Ownership, Release, Removal, Runner,
    Start,
};
pub use ssh::{Destination, Resolved};
pub use tunnel::Tunnel;

/// The fixed script sent to `sh -s` on the remote machine.
pub const REMOTE_SCRIPT: &str = include_str!("remote.sh");

/// The schema line a resident host writes to `~/.openagents/host/runtime`.
pub const RUNTIME_SCHEMA: &str = "openagents.coder.host-runtime.v1";

/// The schema line of the remote record that marks a host as launcher-owned.
pub const MANAGED_SCHEMA: &str = "openagents.coder.ssh-managed.v1";
