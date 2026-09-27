//! The per-host connection state machine.
//!
//! A supervisor never performs input or output. Each call takes the current
//! moment, updates the state, and returns the [`Command`] values the host must
//! carry out. Reports about attempts or connections that the supervisor has
//! already abandoned return [`StaleReport`] and change nothing.
use crate::clock::Moment;
use crate::ids::{AttemptId, ConnectionId};
use crate::policy::{Policy, PolicyError};

/// What an in-flight attempt is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    /// Opening a connection where none exists.
    Establishing,
    /// Checking that an existing connection still answers.
    Probing,
    /// Opening a new connection to replace one that is presumed stale.
    Replacing,
}

/// Why the supervisor stopped retrying. A blocked host waits for
/// [`Signal::RetryNow`], [`Signal::Connect`], or
/// [`Signal::CredentialsChanged`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BlockReason {
    /// The host refused this client's identity or proof.
    Authentication,
    /// The host revoked or expired this client's access.
    Revoked,
    /// The host speaks a protocol version this client does not support.
    Incompatible,
    /// The client's route or settings for this host are invalid.
    Configuration,
}

/// How an attempt or a connection ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Failure {
    /// The attempt did not finish before its deadline.
    Timeout,
    /// The host or relay could not be reached.
    Unreachable,
    /// The remote side closed the connection.
    Closed,
    /// The device has no usable network.
    NetworkUnavailable,
    /// A failure that retrying cannot fix.
    Blocked(BlockReason),
}

/// The transport health of one host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    /// Known and able to connect, but no connection is wanted.
    Available,
    /// A connection is wanted, and the device has no network. The supervisor
    /// waits for [`Signal::NetworkChanged`].
    Offline,
    /// An attempt is in flight.
    Connecting(Stage),
    /// Waiting after a failure. The supervisor tries again at `until`.
    Backoff {
        /// When the next attempt starts.
        until: Moment,
    },
    /// A connection is up.
    Connected,
    /// Retrying cannot help until a signal changes something.
    Blocked(BlockReason),
}

/// Why the data a client holds may be out of date.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StaleCause {
    /// A connection is up, and the client has not yet caught up on it.
    Syncing,
    /// No connection is up.
    TransportDown,
    /// The connection is up, and its subscription or catch-up failed.
    SubscriptionFailed,
}

/// How current the client's data for one host is. This is independent of
/// [`Phase`]: a connected host can hold stale data, and an offline host can
/// hold data that was current when the connection ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Freshness {
    /// The client has never caught up with this host and is not trying.
    Unknown,
    /// The client caught up at `as_of`, and the connection it used is still
    /// up.
    Current {
        /// When the client last caught up.
        as_of: Moment,
    },
    /// The client's data may be behind.
    Stale {
        /// When the client last caught up, if ever.
        as_of: Option<Moment>,
        /// Why the data may be behind.
        cause: StaleCause,
    },
}

/// An event from the application or platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Signal {
    /// The user wants this host connected.
    Connect,
    /// The user no longer wants this host connected.
    Disconnect,
    /// The user asked to try now instead of waiting.
    RetryNow,
    /// The device's network availability changed.
    NetworkChanged {
        /// Whether a network is now usable.
        available: bool,
    },
    /// The application moved to the background.
    ApplicationBackground,
    /// The application became active. After a background shorter than
    /// [`Policy::long_background`], the supervisor probes a connection; after
    /// a longer one, it replaces the connection.
    ApplicationActive,
    /// A credential that this host's route depends on changed.
    CredentialsChanged,
}

/// An outcome the transport reports back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Report {
    /// The attempt succeeded. For an establishing or replacing attempt, the
    /// new connection's ID is the attempt's ID.
    Established(AttemptId),
    /// The attempt failed.
    Failed(AttemptId, Failure),
    /// An established connection ended.
    ConnectionLost(ConnectionId, Failure),
    /// The client caught up on this connection.
    DataCurrent(ConnectionId),
    /// A subscription or catch-up on this connection failed. The transport
    /// stays up.
    SubscriptionFailed(ConnectionId),
}

/// Transport work the host must carry out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Command {
    /// Open a connection. `stage` is [`Stage::Establishing`] or
    /// [`Stage::Replacing`].
    Open {
        /// The attempt to report on.
        attempt: AttemptId,
        /// Why the connection is opened.
        stage: Stage,
    },
    /// Check that `connection` still answers.
    Probe {
        /// The attempt to report on.
        attempt: AttemptId,
        /// The connection to check.
        connection: ConnectionId,
    },
    /// Abandon an attempt. Any later report about it is ignored.
    Cancel(AttemptId),
    /// Close a connection. Any later report about it is ignored.
    Close(ConnectionId),
}

/// A report about an attempt or connection that the supervisor no longer
/// tracks. It changed nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StaleReport;

/// A read-only view of one supervisor for projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Status {
    /// Transport health.
    pub phase: Phase,
    /// Data freshness, independent of `phase`.
    pub freshness: Freshness,
    /// Whether the host is switched on.
    pub enabled: bool,
    /// Whether the user wants the host connected.
    pub wanted: bool,
    /// Whether the supervisor believes a network is usable.
    pub network_available: bool,
    /// The next backoff ladder position.
    pub step: usize,
    /// The most recent failure since the last successful attempt.
    pub last_failure: Option<Failure>,
    /// The current connection, if one is up.
    pub connection: Option<ConnectionId>,
}

#[derive(Clone, Copy, Debug)]
struct Attempt {
    id: AttemptId,
    stage: Stage,
    deadline: Moment,
}

#[derive(Clone, Copy, Debug)]
struct Link {
    id: ConnectionId,
    since: Moment,
}

/// The connection owner for one host.
#[derive(Clone, Debug)]
pub struct Supervisor {
    policy: Policy,
    phase: Phase,
    attempt: Option<Attempt>,
    link: Option<Link>,
    enabled: bool,
    wanted: bool,
    network: bool,
    step: usize,
    next_id: u64,
    background_since: Option<Moment>,
    freshness: Freshness,
    last_failure: Option<Failure>,
}

impl Supervisor {
    /// Creates an idle, switched-on supervisor that assumes a usable network.
    pub fn new(policy: Policy) -> Result<Self, PolicyError> {
        policy.validate()?;
        Ok(Self {
            policy,
            phase: Phase::Available,
            attempt: None,
            link: None,
            enabled: true,
            wanted: false,
            network: true,
            step: 0,
            next_id: 1,
            background_since: None,
            freshness: Freshness::Unknown,
            last_failure: None,
        })
    }

    /// Returns the current state for a projection.
    pub fn status(&self) -> Status {
        Status {
            phase: self.phase,
            freshness: self.freshness,
            enabled: self.enabled,
            wanted: self.wanted,
            network_available: self.network,
            step: self.step,
            last_failure: self.last_failure,
            connection: self.link.map(|link| link.id),
        }
    }

    /// Returns the earliest moment at which [`Supervisor::tick`] can change
    /// the state, if any.
    pub fn next_deadline(&self) -> Option<Moment> {
        let attempt = self.attempt.map(|attempt| attempt.deadline);
        let backoff = match self.phase {
            Phase::Backoff { until } => Some(until),
            _ => None,
        };
        let stable = self
            .link
            .filter(|_| self.step > 0)
            .map(|link| link.since.after(self.policy.stable_after));
        [attempt, backoff, stable].into_iter().flatten().min()
    }

    /// Applies an application or platform signal.
    pub fn signal(&mut self, now: Moment, signal: Signal) -> Vec<Command> {
        let mut out = Vec::new();
        match signal {
            Signal::Connect => self.connect(now, &mut out),
            Signal::Disconnect => {
                self.wanted = false;
                self.stand_down(&mut out);
            }
            Signal::RetryNow => self.retry_now(now, &mut out),
            Signal::NetworkChanged { available } => self.network_changed(now, available, &mut out),
            Signal::ApplicationBackground => self.background_since = Some(now),
            Signal::ApplicationActive => self.active(now, &mut out),
            Signal::CredentialsChanged => self.credentials_changed(now, &mut out),
        }
        out
    }

    /// Applies a transport report.
    pub fn report(&mut self, now: Moment, report: Report) -> Result<Vec<Command>, StaleReport> {
        let mut out = Vec::new();
        match report {
            Report::Established(id) => {
                let attempt = self.take_attempt(id)?;
                self.established(now, attempt, &mut out);
            }
            Report::Failed(id, failure) => {
                let attempt = self.take_attempt(id)?;
                self.attempt_failed(now, attempt.stage, failure, &mut out);
            }
            Report::ConnectionLost(id, failure) => {
                self.current_link(id)?;
                self.settle(now);
                self.link = None;
                self.last_failure = Some(failure);
                self.connection_lost(now, failure, &mut out);
            }
            Report::DataCurrent(id) => {
                self.current_link(id)?;
                self.freshness = Freshness::Current { as_of: now };
            }
            Report::SubscriptionFailed(id) => {
                self.current_link(id)?;
                self.freshness = Freshness::Stale {
                    as_of: self.as_of(),
                    cause: StaleCause::SubscriptionFailed,
                };
            }
        }
        Ok(out)
    }

    /// Applies deadlines that have passed by `now`: an attempt timeout, the
    /// end of a backoff, and the ladder reset after a stable connection.
    pub fn tick(&mut self, now: Moment) -> Vec<Command> {
        let mut out = Vec::new();
        if let Some(attempt) = self.attempt.filter(|attempt| now >= attempt.deadline) {
            self.attempt = None;
            out.push(Command::Cancel(attempt.id));
            self.attempt_failed(now, attempt.stage, Failure::Timeout, &mut out);
        } else if let Phase::Backoff { until } = self.phase
            && now >= until
        {
            self.start(now, &mut out);
        }
        self.settle(now);
        out
    }

    /// Stops all transport work and ignores signals until switched on. The
    /// user's wish to connect is kept.
    pub fn switch_off(&mut self) -> Vec<Command> {
        let mut out = Vec::new();
        self.enabled = false;
        self.stand_down(&mut out);
        out
    }

    /// Switches the host back on and connects if the user wants it.
    pub fn switch_on(&mut self, now: Moment) -> Vec<Command> {
        let mut out = Vec::new();
        if !self.enabled {
            self.enabled = true;
            if self.wanted {
                self.start(now, &mut out);
            }
        }
        out
    }

    /// Stops all transport work before the host is removed.
    pub fn shut_down(&mut self) -> Vec<Command> {
        let mut out = Vec::new();
        self.wanted = false;
        self.stand_down(&mut out);
        out
    }

    fn active_and_wanted(&self) -> bool {
        self.enabled && self.wanted
    }

    fn connect(&mut self, now: Moment, out: &mut Vec<Command>) {
        self.wanted = true;
        if !self.enabled {
            return;
        }
        match self.phase {
            Phase::Available | Phase::Blocked(_) => {
                self.step = 0;
                self.start(now, out);
            }
            Phase::Backoff { .. } => self.start(now, out),
            Phase::Offline | Phase::Connecting(_) | Phase::Connected => {}
        }
    }

    fn retry_now(&mut self, now: Moment, out: &mut Vec<Command>) {
        if !self.enabled {
            return;
        }
        self.wanted = true;
        match self.phase {
            Phase::Blocked(_) | Phase::Available => {
                self.step = 0;
                self.open(now, Stage::Establishing, out);
            }
            // The user may know better than the platform's network signal.
            Phase::Backoff { .. } | Phase::Offline => self.open(now, Stage::Establishing, out),
            Phase::Connected => self.probe(now, out),
            Phase::Connecting(_) => {}
        }
    }

    fn network_changed(&mut self, now: Moment, available: bool, out: &mut Vec<Command>) {
        self.network = available;
        if !self.active_and_wanted() {
            return;
        }
        match (available, self.phase) {
            (false, Phase::Connecting(_) | Phase::Backoff { .. } | Phase::Connected) => {
                self.tear_down(out);
                self.phase = Phase::Offline;
            }
            (true, Phase::Offline) => {
                self.step = 0;
                self.open(now, Stage::Establishing, out);
            }
            (true, Phase::Backoff { .. }) => self.open(now, Stage::Establishing, out),
            // A new network can break an existing path.
            (true, Phase::Connected) => self.probe(now, out),
            _ => {}
        }
    }

    fn active(&mut self, now: Moment, out: &mut Vec<Command>) {
        let long = self
            .background_since
            .take()
            .is_some_and(|since| now.since(since) >= self.policy.long_background);
        if !self.active_and_wanted() {
            return;
        }
        match self.phase {
            Phase::Connected if long => self.open(now, Stage::Replacing, out),
            Phase::Connected => self.probe(now, out),
            // Timers may not have run while suspended.
            Phase::Backoff { .. } => self.open(now, Stage::Establishing, out),
            _ => {}
        }
    }

    fn credentials_changed(&mut self, now: Moment, out: &mut Vec<Command>) {
        if !self.active_and_wanted() {
            return;
        }
        match self.phase {
            Phase::Blocked(_) | Phase::Backoff { .. } => {
                self.step = 0;
                self.start(now, out);
            }
            Phase::Connecting(_) => {
                // The attempt in flight used the old credential.
                if let Some(attempt) = self.attempt.take() {
                    out.push(Command::Cancel(attempt.id));
                }
                let stage = if self.link.is_some() {
                    Stage::Replacing
                } else {
                    Stage::Establishing
                };
                self.open(now, stage, out);
            }
            Phase::Connected => self.open(now, Stage::Replacing, out),
            Phase::Available | Phase::Offline => {}
        }
    }

    fn established(&mut self, now: Moment, attempt: Attempt, out: &mut Vec<Command>) {
        self.last_failure = None;
        self.phase = Phase::Connected;
        if attempt.stage == Stage::Probing {
            return;
        }
        if let Some(old) = self.link.take() {
            out.push(Command::Close(old.id));
        }
        self.link = Some(Link {
            id: ConnectionId(attempt.id.0),
            since: now,
        });
        self.freshness = Freshness::Stale {
            as_of: self.as_of(),
            cause: StaleCause::Syncing,
        };
    }

    fn attempt_failed(
        &mut self,
        now: Moment,
        stage: Stage,
        failure: Failure,
        out: &mut Vec<Command>,
    ) {
        self.last_failure = Some(failure);
        if stage == Stage::Probing && retryable(failure) {
            self.probe_failed(now, out);
        } else {
            self.failed(now, failure, out);
        }
    }

    fn connection_lost(&mut self, now: Moment, failure: Failure, out: &mut Vec<Command>) {
        match self.attempt {
            Some(Attempt {
                stage: Stage::Replacing,
                ..
            }) if retryable(failure) => {
                // The replacement continues as an ordinary attempt.
                if let Some(attempt) = self.attempt.as_mut() {
                    attempt.stage = Stage::Establishing;
                }
                self.phase = Phase::Connecting(Stage::Establishing);
                self.mark_transport_down();
            }
            Some(Attempt {
                stage: Stage::Probing,
                ..
            }) if retryable(failure) => self.probe_failed(now, out),
            _ => self.failed(now, failure, out),
        }
    }

    /// A probe failure means the connection went stale, which is normal after
    /// a suspension. Reconnect without the first wait; a further failure waits
    /// the second ladder step.
    fn probe_failed(&mut self, now: Moment, out: &mut Vec<Command>) {
        self.tear_down(out);
        self.step = self.step.max(1);
        self.start(now, out);
    }

    fn failed(&mut self, now: Moment, failure: Failure, out: &mut Vec<Command>) {
        self.tear_down(out);
        match failure {
            Failure::Blocked(reason) => self.phase = Phase::Blocked(reason),
            Failure::NetworkUnavailable => {
                self.network = false;
                self.phase = Phase::Offline;
            }
            Failure::Timeout | Failure::Unreachable | Failure::Closed => {
                self.phase = Phase::Backoff {
                    until: now.after(self.policy.delay(self.step)),
                };
                self.step = self.step.saturating_add(1);
            }
        }
    }

    fn start(&mut self, now: Moment, out: &mut Vec<Command>) {
        if self.network {
            self.open(now, Stage::Establishing, out);
        } else {
            self.phase = Phase::Offline;
        }
    }

    fn open(&mut self, now: Moment, stage: Stage, out: &mut Vec<Command>) {
        let id = self.allocate();
        self.attempt = Some(Attempt {
            id,
            stage,
            deadline: now.after(self.policy.establish_timeout),
        });
        self.phase = Phase::Connecting(stage);
        out.push(Command::Open { attempt: id, stage });
    }

    fn probe(&mut self, now: Moment, out: &mut Vec<Command>) {
        let Some(link) = self.link else {
            return;
        };
        let id = self.allocate();
        self.attempt = Some(Attempt {
            id,
            stage: Stage::Probing,
            deadline: now.after(self.policy.probe_timeout),
        });
        self.phase = Phase::Connecting(Stage::Probing);
        out.push(Command::Probe {
            attempt: id,
            connection: link.id,
        });
    }

    fn allocate(&mut self) -> AttemptId {
        let id = AttemptId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    fn stand_down(&mut self, out: &mut Vec<Command>) {
        self.tear_down(out);
        self.phase = Phase::Available;
        self.step = 0;
    }

    fn tear_down(&mut self, out: &mut Vec<Command>) {
        if let Some(attempt) = self.attempt.take() {
            out.push(Command::Cancel(attempt.id));
        }
        if let Some(link) = self.link.take() {
            out.push(Command::Close(link.id));
        }
        self.mark_transport_down();
    }

    fn mark_transport_down(&mut self) {
        if self.freshness != Freshness::Unknown {
            self.freshness = Freshness::Stale {
                as_of: self.as_of(),
                cause: StaleCause::TransportDown,
            };
        }
    }

    fn settle(&mut self, now: Moment) {
        if self
            .link
            .is_some_and(|link| now.since(link.since) >= self.policy.stable_after)
        {
            self.step = 0;
        }
    }

    fn as_of(&self) -> Option<Moment> {
        match self.freshness {
            Freshness::Unknown => None,
            Freshness::Current { as_of } => Some(as_of),
            Freshness::Stale { as_of, .. } => as_of,
        }
    }

    fn take_attempt(&mut self, id: AttemptId) -> Result<Attempt, StaleReport> {
        match self.attempt {
            Some(attempt) if attempt.id == id => {
                self.attempt = None;
                Ok(attempt)
            }
            _ => Err(StaleReport),
        }
    }

    fn current_link(&self, id: ConnectionId) -> Result<(), StaleReport> {
        match self.link {
            Some(link) if link.id == id => Ok(()),
            _ => Err(StaleReport),
        }
    }

    #[cfg(test)]
    pub(crate) fn attempt_id(&self) -> Option<AttemptId> {
        self.attempt.map(|attempt| attempt.id)
    }

    /// Checks the internal invariants. Tests call it after every step.
    #[cfg(test)]
    pub(crate) fn check(&self) {
        match self.phase {
            Phase::Connecting(stage) => {
                let attempt = self.attempt.expect("connecting without an attempt");
                assert_eq!(attempt.stage, stage, "phase and attempt stage differ");
                if stage == Stage::Establishing {
                    assert!(self.link.is_none(), "establishing beside a connection");
                } else {
                    assert!(self.link.is_some(), "{stage:?} without a connection");
                }
            }
            Phase::Connected => {
                assert!(self.attempt.is_none(), "connected with an attempt");
                assert!(self.link.is_some(), "connected without a connection");
            }
            _ => {
                assert!(self.attempt.is_none(), "{:?} with an attempt", self.phase);
                assert!(self.link.is_none(), "{:?} with a connection", self.phase);
            }
        }
    }
}

fn retryable(failure: Failure) -> bool {
    matches!(
        failure,
        Failure::Timeout | Failure::Unreachable | Failure::Closed
    )
}
