//! One supervisor per host, driven by an injected clock and connector.
use crate::clock::{Clock, Moment};
use crate::ids::{AttemptId, ConnectionId, CredentialId, HostKey};
use crate::policy::{Policy, PolicyError};
use crate::supervisor::{Command, Report, Signal, Stage, StaleReport, Status, Supervisor};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The transport a registry drives. Implementations start work and return
/// immediately; outcomes come back later through [`Registry::report`].
///
/// A connector must not call back into the registry from these methods.
pub trait Connector {
    /// Starts opening a connection to `host`. `stage` is
    /// [`Stage::Establishing`] or [`Stage::Replacing`]; a replacement must not
    /// close the existing connection, which the registry closes after the
    /// replacement succeeds.
    fn open(&mut self, host: &HostKey, attempt: AttemptId, stage: Stage);
    /// Starts checking that `connection` still answers.
    fn probe(&mut self, host: &HostKey, attempt: AttemptId, connection: ConnectionId);
    /// Abandons an attempt. Its outcome no longer matters.
    fn cancel(&mut self, host: &HostKey, attempt: AttemptId);
    /// Closes a connection.
    fn close(&mut self, host: &HostKey, connection: ConnectionId);
}

/// What a host's route depends on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Route {
    /// The credentials the route uses, such as a device grant.
    pub credentials: BTreeSet<CredentialId>,
}

/// Why a registry call was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// The host key is not registered.
    Unknown,
    /// The host key is already registered.
    Duplicate,
    /// The report names an attempt or connection the host no longer tracks.
    Stale,
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unknown => "the host is not registered",
            Self::Duplicate => "the host is already registered",
            Self::Stale => "the report names a superseded attempt or connection",
        })
    }
}

impl std::error::Error for RegistryError {}

impl From<StaleReport> for RegistryError {
    fn from(_: StaleReport) -> Self {
        Self::Stale
    }
}

struct Entry {
    supervisor: Supervisor,
    route: Route,
}

/// The connection owners for every host a client knows.
pub struct Registry<C, K> {
    clock: C,
    connector: K,
    policy: Policy,
    hosts: BTreeMap<HostKey, Entry>,
}

impl<C: Clock, K: Connector> Registry<C, K> {
    /// Creates an empty registry. Every host it registers uses `policy`.
    pub fn new(clock: C, connector: K, policy: Policy) -> Result<Self, PolicyError> {
        policy.validate()?;
        Ok(Self {
            clock,
            connector,
            policy,
            hosts: BTreeMap::new(),
        })
    }

    /// Returns the connector, for inspection.
    pub fn connector(&self) -> &K {
        &self.connector
    }

    /// Returns the connector, for inspection or reconfiguration.
    pub fn connector_mut(&mut self) -> &mut K {
        &mut self.connector
    }

    /// Adds an idle host. Send [`Signal::Connect`] to connect it.
    pub fn register(&mut self, host: HostKey, route: Route) -> Result<(), RegistryError> {
        if self.hosts.contains_key(&host) {
            return Err(RegistryError::Duplicate);
        }
        let supervisor = Supervisor::new(self.policy.clone())
            .expect("the registry validated its policy on creation");
        self.hosts.insert(host, Entry { supervisor, route });
        Ok(())
    }

    /// Returns one host's status.
    pub fn status(&self, host: &HostKey) -> Option<Status> {
        self.hosts.get(host).map(|entry| entry.supervisor.status())
    }

    /// Returns every host's status in key order.
    pub fn statuses(&self) -> impl Iterator<Item = (&HostKey, Status)> {
        self.hosts
            .iter()
            .map(|(host, entry)| (host, entry.supervisor.status()))
    }

    /// Returns one host's route.
    pub fn route(&self, host: &HostKey) -> Option<&Route> {
        self.hosts.get(host).map(|entry| &entry.route)
    }

    /// Sends a signal to one host.
    pub fn signal(&mut self, host: &HostKey, signal: Signal) -> Result<(), RegistryError> {
        let now = self.clock.now();
        let entry = self.hosts.get_mut(host).ok_or(RegistryError::Unknown)?;
        let commands = entry.supervisor.signal(now, signal);
        run(&mut self.connector, host, commands);
        Ok(())
    }

    /// Sends a device-wide signal, such as a network change or an application
    /// lifecycle change, to every host.
    pub fn signal_all(&mut self, signal: Signal) {
        let now = self.clock.now();
        for (host, entry) in &mut self.hosts {
            let commands = entry.supervisor.signal(now, signal);
            run(&mut self.connector, host, commands);
        }
    }

    /// Applies a transport report for one host.
    pub fn report(&mut self, host: &HostKey, report: Report) -> Result<(), RegistryError> {
        let now = self.clock.now();
        let entry = self.hosts.get_mut(host).ok_or(RegistryError::Unknown)?;
        let commands = entry.supervisor.report(now, report)?;
        run(&mut self.connector, host, commands);
        Ok(())
    }

    /// Applies every deadline that has passed.
    pub fn tick(&mut self) {
        let now = self.clock.now();
        for (host, entry) in &mut self.hosts {
            let commands = entry.supervisor.tick(now);
            run(&mut self.connector, host, commands);
        }
    }

    /// Returns when [`Registry::tick`] next needs to run, if ever.
    pub fn next_deadline(&self) -> Option<Moment> {
        self.hosts
            .values()
            .filter_map(|entry| entry.supervisor.next_deadline())
            .min()
    }

    /// Stops a host's transport work without forgetting it. Its route,
    /// credentials, and cached data remain.
    pub fn switch_off(&mut self, host: &HostKey) -> Result<(), RegistryError> {
        let entry = self.hosts.get_mut(host).ok_or(RegistryError::Unknown)?;
        let commands = entry.supervisor.switch_off();
        run(&mut self.connector, host, commands);
        Ok(())
    }

    /// Switches a host back on.
    pub fn switch_on(&mut self, host: &HostKey) -> Result<(), RegistryError> {
        let now = self.clock.now();
        let entry = self.hosts.get_mut(host).ok_or(RegistryError::Unknown)?;
        let commands = entry.supervisor.switch_on(now);
        run(&mut self.connector, host, commands);
        Ok(())
    }

    /// Stops a host's transport work, forgets it, and then calls `forget` so
    /// the caller clears the host's cached projections and credentials. Later
    /// reports for the host return [`RegistryError::Unknown`].
    pub fn remove(
        &mut self,
        host: &HostKey,
        forget: impl FnOnce(&HostKey, &Route),
    ) -> Result<(), RegistryError> {
        let mut entry = self.hosts.remove(host).ok_or(RegistryError::Unknown)?;
        let commands = entry.supervisor.shut_down();
        run(&mut self.connector, host, commands);
        forget(host, &entry.route);
        Ok(())
    }

    /// Sends [`Signal::CredentialsChanged`] to every host whose route depends
    /// on `credential`, and returns those hosts. Other hosts are untouched.
    pub fn credentials_changed(&mut self, credential: &CredentialId) -> Vec<HostKey> {
        let now = self.clock.now();
        let mut affected = Vec::new();
        for (host, entry) in &mut self.hosts {
            if entry.route.credentials.contains(credential) {
                let commands = entry.supervisor.signal(now, Signal::CredentialsChanged);
                run(&mut self.connector, host, commands);
                affected.push(host.clone());
            }
        }
        affected
    }
}

fn run<K: Connector>(connector: &mut K, host: &HostKey, commands: Vec<Command>) {
    for command in commands {
        match command {
            Command::Open { attempt, stage } => connector.open(host, attempt, stage),
            Command::Probe {
                attempt,
                connection,
            } => connector.probe(host, attempt, connection),
            Command::Cancel(attempt) => connector.cancel(host, attempt),
            Command::Close(connection) => connector.close(host, connection),
        }
    }
}
