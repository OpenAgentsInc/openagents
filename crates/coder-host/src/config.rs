//! What one resident host serves and where.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use coder_access::RelayPolicy;
use coder_reach::hints::Class;

use crate::{Error, Result};

/// The ready record the host service waits for during a trial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ready {
    /// Where to write the record, from `OPENAGENTS_HOST_READY_FILE`.
    pub file: PathBuf,
    /// The bundle identity to repeat, from `OPENAGENTS_HOST_VERSION`.
    pub version: String,
}

/// An extra direct endpoint the host advertises, such as a LAN or tailnet
/// address that forwards to its listener. The host cannot prove it; only a
/// device that completes a handshake through it can.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advertised {
    pub class: Class,
    /// `host:port`.
    pub address: String,
}

/// One host's configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// The `coder-access` store: the host key, owner, and grants.
    pub access: PathBuf,
    /// Relays the host serves. The first is the primary relay: grants and
    /// invitations name it, and the direct binding admits requests that
    /// name it.
    pub relays: Vec<String>,
    pub policy: RelayPolicy,
    /// The direct-channel listener. It must be a loopback address unless
    /// `allow_nonloopback` is set.
    pub listen: SocketAddr,
    /// Permit a listener on a non-loopback address, such as a LAN or
    /// tailnet interface.
    pub allow_nonloopback: bool,
    /// Advertise the listener's own address as a hint. A host behind a
    /// forwarder turns this off and advertises the forwarder instead.
    pub advertise_listener: bool,
    pub advertise: Vec<Advertised>,
    /// The NIP-REACH host generation. It increases on every start.
    pub generation: u64,
    /// Workspace labels a task or terminal may name, and their roots.
    pub workspaces: BTreeMap<String, PathBuf>,
    /// Write the service's ready record once the host serves.
    pub ready: Option<Ready>,
    /// Write the runtime record SSH launchers read, such as
    /// `~/.openagents/host/runtime`.
    pub runtime: Option<PathBuf>,
    /// How often presence and hints are republished without a change.
    pub presence_every: Duration,
    /// How often an open channel rechecks its grant without traffic.
    pub recheck_every: Duration,
    /// How long a direct handshake may take.
    pub handshake_timeout: Duration,
}

impl Config {
    /// A loopback host with default periods and no workspaces.
    #[must_use]
    pub fn new(access: PathBuf, relays: Vec<String>, generation: u64) -> Self {
        Self {
            access,
            relays,
            policy: RelayPolicy::Production,
            listen: SocketAddr::from(([127, 0, 0, 1], 0)),
            allow_nonloopback: false,
            advertise_listener: true,
            advertise: Vec::new(),
            generation,
            workspaces: BTreeMap::new(),
            ready: None,
            runtime: None,
            presence_every: Duration::from_secs(60),
            recheck_every: Duration::from_millis(500),
            handshake_timeout: Duration::from_secs(10),
        }
    }

    /// The primary relay.
    ///
    /// # Errors
    /// Refuses a configuration without a relay.
    pub fn primary(&self) -> Result<&str> {
        self.relays
            .first()
            .map(String::as_str)
            .ok_or_else(|| Error::Config("serve needs at least one relay".into()))
    }

    /// Check the configuration before anything binds or publishes.
    ///
    /// # Errors
    /// Refuses a missing relay, a relay the policy refuses, a non-loopback
    /// listener without permission, a bad workspace, or a zero period.
    pub fn validate(&self) -> Result<()> {
        self.primary()?;
        if self.relays.len() > coder_reach::directory::MAX_RELAYS {
            return Err(Error::Config("serve takes at most eight relays".into()));
        }
        for relay in &self.relays {
            self.policy
                .validate(relay)
                .map_err(|_| Error::Config("a relay is not allowed by the relay policy".into()))?;
        }
        if !self.listen.ip().is_loopback() && !self.allow_nonloopback {
            return Err(Error::Config(
                "the listener is loopback only unless --allow-nonloopback is given".into(),
            ));
        }
        for (label, root) in &self.workspaces {
            if label.is_empty()
                || label.len() > 128
                || label.chars().any(char::is_control)
                || !root.is_absolute()
            {
                return Err(Error::Config(
                    "a workspace needs a short label and an absolute root".into(),
                ));
            }
        }
        if self.presence_every.is_zero()
            || self.recheck_every.is_zero()
            || self.handshake_timeout.is_zero()
        {
            return Err(Error::Config("periods must be greater than zero".into()));
        }
        Ok(())
    }
}
