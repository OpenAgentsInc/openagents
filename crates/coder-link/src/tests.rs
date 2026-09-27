use crate::*;
use std::collections::BTreeSet;
use std::time::Duration;

fn policy() -> Policy {
    Policy {
        establish_timeout: Duration::from_secs(10),
        probe_timeout: Duration::from_secs(3),
        ladder: [1, 2, 4, 8].into_iter().map(Duration::from_secs).collect(),
        stable_after: Duration::from_secs(30),
        long_background: Duration::from_secs(300),
    }
}

/// One scripted input to a supervisor.
#[derive(Clone, Copy, Debug)]
enum Step {
    S(Signal),
    /// The current attempt succeeds.
    Up,
    /// The current attempt fails.
    Fail(Failure),
    /// The current connection ends.
    Lose(Failure),
    /// A report that an earlier attempt succeeded.
    Late(u64),
    /// The client caught up on the current connection.
    Data,
    /// The client caught up on a named connection.
    DataOn(u64),
    /// The subscription on the current connection failed.
    SubFail,
    /// Time passes, in milliseconds.
    Wait(u64),
    Tick,
}
use Step::*;

const C: Step = S(Signal::Connect);
const BG: Step = S(Signal::ApplicationBackground);
const ACTIVE: Step = S(Signal::ApplicationActive);
const RETRY: Step = S(Signal::RetryNow);
const CRED: Step = S(Signal::CredentialsChanged);
const NET_DOWN: Step = S(Signal::NetworkChanged { available: false });
const NET_UP: Step = S(Signal::NetworkChanged { available: true });
const UNREACHABLE: Step = Fail(Failure::Unreachable);

const fn secs(value: u64) -> Step {
    Wait(value * 1000)
}

struct Case {
    name: &'static str,
    steps: &'static [Step],
    phase: Phase,
    /// Commands from the last step.
    commands: &'static [Command],
    /// The last step is a report the supervisor must refuse as stale.
    stale: bool,
    step: Option<usize>,
    freshness: Option<Freshness>,
}

const fn case(name: &'static str, steps: &'static [Step], phase: Phase) -> Case {
    Case {
        name,
        steps,
        phase,
        commands: &[],
        stale: false,
        step: None,
        freshness: None,
    }
}

impl Case {
    const fn emits(mut self, commands: &'static [Command]) -> Self {
        self.commands = commands;
        self
    }
    const fn stale(mut self) -> Self {
        self.stale = true;
        self
    }
    const fn step(mut self, step: usize) -> Self {
        self.step = Some(step);
        self
    }
    const fn fresh(mut self, freshness: Freshness) -> Self {
        self.freshness = Some(freshness);
        self
    }
}

const fn open(id: u64) -> Command {
    Command::Open {
        attempt: AttemptId(id),
        stage: Stage::Establishing,
    }
}
const fn replace(id: u64) -> Command {
    Command::Open {
        attempt: AttemptId(id),
        stage: Stage::Replacing,
    }
}
const fn probe(id: u64, connection: u64) -> Command {
    Command::Probe {
        attempt: AttemptId(id),
        connection: ConnectionId(connection),
    }
}
const fn cancel(id: u64) -> Command {
    Command::Cancel(AttemptId(id))
}
const fn close(id: u64) -> Command {
    Command::Close(ConnectionId(id))
}
const fn backoff(ms: u64) -> Phase {
    Phase::Backoff { until: Moment(ms) }
}
const fn stale_at(as_of: Option<u64>, cause: StaleCause) -> Freshness {
    Freshness::Stale {
        as_of: match as_of {
            Some(ms) => Some(Moment(ms)),
            None => None,
        },
        cause,
    }
}
const ESTABLISHING: Phase = Phase::Connecting(Stage::Establishing);
const PROBING: Phase = Phase::Connecting(Stage::Probing);
const REPLACING: Phase = Phase::Connecting(Stage::Replacing);

const CASES: &[Case] = &[
    // Establishing a connection.
    case("connect opens an attempt", &[C], ESTABLISHING)
        .emits(&[open(1)])
        .fresh(Freshness::Unknown),
    case(
        "an established attempt connects",
        &[C, Up],
        Phase::Connected,
    )
    .step(0)
    .fresh(stale_at(None, StaleCause::Syncing)),
    case(
        "connect while connected does nothing",
        &[C, Up, C],
        Phase::Connected,
    ),
    case(
        "catching up makes data current",
        &[C, Up, secs(2), Data],
        Phase::Connected,
    )
    .fresh(Freshness::Current {
        as_of: Moment(2000),
    }),
    case(
        "a failed subscription does not show as reconnecting",
        &[C, Up, Data, secs(5), SubFail],
        Phase::Connected,
    )
    .fresh(stale_at(Some(0), StaleCause::SubscriptionFailed)),
    // The backoff ladder.
    case(
        "an unreachable host waits the first step",
        &[C, UNREACHABLE],
        backoff(1000),
    )
    .step(1),
    case(
        "the end of a backoff retries",
        &[C, UNREACHABLE, secs(1), Tick],
        ESTABLISHING,
    )
    .emits(&[open(2)]),
    case(
        "a tick before the end of a backoff waits",
        &[C, UNREACHABLE, Wait(999), Tick],
        backoff(1000),
    ),
    case(
        "consecutive failures climb the ladder",
        &[C, UNREACHABLE, secs(1), Tick, UNREACHABLE],
        backoff(3000),
    )
    .step(2),
    case(
        "the last ladder step is the cap",
        &[
            C,
            UNREACHABLE,
            secs(1),
            Tick,
            UNREACHABLE,
            secs(2),
            Tick,
            UNREACHABLE,
            secs(4),
            Tick,
            UNREACHABLE,
            secs(8),
            Tick,
            UNREACHABLE,
        ],
        backoff(23_000),
    )
    .step(5),
    case(
        "an establishment timeout cancels and backs off",
        &[C, secs(10), Tick],
        backoff(11_000),
    )
    .emits(&[cancel(1)])
    .step(1),
    case(
        "an attempt before its deadline continues",
        &[C, secs(9), Tick],
        ESTABLISHING,
    ),
    case(
        "the ladder resets after a stable connection",
        &[
            C,
            UNREACHABLE,
            secs(1),
            Tick,
            UNREACHABLE,
            secs(2),
            Tick,
            Up,
            secs(30),
            Tick,
        ],
        Phase::Connected,
    )
    .step(0),
    case(
        "a loss after a stable connection waits the first step",
        &[
            C,
            UNREACHABLE,
            secs(1),
            Tick,
            UNREACHABLE,
            secs(2),
            Tick,
            Up,
            secs(30),
            Lose(Failure::Closed),
        ],
        backoff(34_000),
    )
    .step(1),
    case(
        "a loss before the connection is stable keeps climbing",
        &[
            C,
            UNREACHABLE,
            secs(1),
            Tick,
            UNREACHABLE,
            secs(2),
            Tick,
            Up,
            secs(10),
            Lose(Failure::Closed),
        ],
        backoff(17_000),
    )
    .step(3),
    case(
        "a lost connection keeps its data as stale",
        &[C, Up, Data, secs(5), Lose(Failure::Closed)],
        backoff(6000),
    )
    .fresh(stale_at(Some(0), StaleCause::TransportDown)),
    case(
        "a report for a lost connection is stale",
        &[C, Up, Lose(Failure::Closed), DataOn(1)],
        backoff(1000),
    )
    .stale(),
    // Returning from the background.
    case(
        "returning from a short background probes",
        &[C, Up, BG, secs(60), ACTIVE],
        PROBING,
    )
    .emits(&[probe(2, 1)]),
    case(
        "a successful probe keeps the connection",
        &[C, Up, BG, secs(60), ACTIVE, Up],
        Phase::Connected,
    ),
    case(
        "a probe timeout reconnects without the first wait",
        &[C, Up, BG, secs(60), ACTIVE, secs(3), Tick],
        ESTABLISHING,
    )
    .emits(&[cancel(2), close(1), open(3)])
    .step(1),
    case(
        "a failure after a probe timeout waits the second step",
        &[C, Up, BG, secs(60), ACTIVE, secs(3), Tick, UNREACHABLE],
        backoff(65_000),
    )
    .step(2),
    case(
        "a failed probe reconnects without the first wait",
        &[C, Up, BG, secs(60), ACTIVE, Fail(Failure::Closed)],
        ESTABLISHING,
    )
    .emits(&[close(1), open(3)])
    .step(1),
    case(
        "returning from a long background replaces the connection",
        &[C, Up, BG, secs(300), ACTIVE],
        REPLACING,
    )
    .emits(&[replace(2)]),
    case(
        "a replacement closes the old connection",
        &[C, Up, Data, BG, secs(300), ACTIVE, Up],
        Phase::Connected,
    )
    .emits(&[close(1)])
    .fresh(stale_at(Some(0), StaleCause::Syncing)),
    case(
        "losing the old connection during a replacement keeps the attempt",
        &[C, Up, BG, secs(300), ACTIVE, Lose(Failure::Closed)],
        ESTABLISHING,
    ),
    case(
        "a failed replacement closes the old connection and backs off",
        &[C, Up, BG, secs(300), ACTIVE, UNREACHABLE],
        backoff(301_000),
    )
    .emits(&[close(1)]),
    case(
        "returning during a backoff retries",
        &[C, UNREACHABLE, BG, ACTIVE],
        ESTABLISHING,
    )
    .emits(&[open(2)]),
    // Blocked connections.
    case(
        "a blocked host never retries on its own",
        &[
            C,
            Fail(Failure::Blocked(BlockReason::Revoked)),
            secs(3600),
            Tick,
        ],
        Phase::Blocked(BlockReason::Revoked),
    ),
    case(
        "network and lifecycle signals do not unblock",
        &[
            C,
            Fail(Failure::Blocked(BlockReason::Authentication)),
            NET_DOWN,
            NET_UP,
            BG,
            secs(600),
            ACTIVE,
        ],
        Phase::Blocked(BlockReason::Authentication),
    ),
    case(
        "retry now leaves a blocked state",
        &[
            C,
            Fail(Failure::Blocked(BlockReason::Incompatible)),
            secs(60),
            Tick,
            RETRY,
        ],
        ESTABLISHING,
    )
    .emits(&[open(2)])
    .step(0),
    case(
        "connect leaves a blocked state",
        &[C, Fail(Failure::Blocked(BlockReason::Configuration)), C],
        ESTABLISHING,
    )
    .emits(&[open(2)]),
    case(
        "a revoked connection blocks",
        &[C, Up, Data, Lose(Failure::Blocked(BlockReason::Revoked))],
        Phase::Blocked(BlockReason::Revoked),
    )
    .fresh(stale_at(Some(0), StaleCause::TransportDown)),
    case(
        "a probe refused for authentication blocks",
        &[
            C,
            Up,
            ACTIVE,
            Fail(Failure::Blocked(BlockReason::Authentication)),
        ],
        Phase::Blocked(BlockReason::Authentication),
    )
    .emits(&[close(1)]),
    // Network changes.
    case(
        "an offline connect waits for the network",
        &[NET_DOWN, C],
        Phase::Offline,
    ),
    case(
        "an offline host ignores time",
        &[NET_DOWN, C, secs(600), Tick],
        Phase::Offline,
    ),
    case(
        "the network returning connects",
        &[NET_DOWN, C, NET_UP],
        ESTABLISHING,
    )
    .emits(&[open(1)]),
    case(
        "losing the network closes the connection",
        &[C, Up, NET_DOWN],
        Phase::Offline,
    )
    .emits(&[close(1)]),
    case(
        "losing the network cancels an attempt",
        &[C, NET_DOWN],
        Phase::Offline,
    )
    .emits(&[cancel(1)]),
    case(
        "an attempt that finds no network goes offline",
        &[C, Fail(Failure::NetworkUnavailable), secs(60), Tick],
        Phase::Offline,
    ),
    case(
        "a network change while connected probes",
        &[C, Up, NET_UP],
        PROBING,
    )
    .emits(&[probe(2, 1)]),
    case(
        "the network returning ends a backoff",
        &[C, UNREACHABLE, NET_UP],
        ESTABLISHING,
    )
    .emits(&[open(2)]),
    case(
        "retry now overrides an offline state",
        &[NET_DOWN, C, RETRY],
        ESTABLISHING,
    )
    .emits(&[open(1)]),
    case(
        "retry now ends a backoff and keeps the ladder",
        &[C, UNREACHABLE, RETRY],
        ESTABLISHING,
    )
    .emits(&[open(2)])
    .step(1),
    case("retry now while connected probes", &[C, Up, RETRY], PROBING).emits(&[probe(2, 1)]),
    // Credential changes.
    case(
        "credentials changed during an attempt restarts it",
        &[C, CRED],
        ESTABLISHING,
    )
    .emits(&[cancel(1), open(2)]),
    case(
        "the superseded attempt's success is stale",
        &[C, CRED, Late(1)],
        ESTABLISHING,
    )
    .stale(),
    case(
        "the new attempt after a credential change connects",
        &[C, CRED, Late(1), Up],
        Phase::Connected,
    ),
    case(
        "credentials changed while connected replaces",
        &[C, Up, CRED],
        REPLACING,
    )
    .emits(&[replace(2)]),
    case(
        "credentials changed during a probe replaces",
        &[C, Up, ACTIVE, CRED],
        REPLACING,
    )
    .emits(&[cancel(2), replace(3)]),
    case(
        "credentials changed during a replacement restarts it",
        &[C, Up, BG, secs(300), ACTIVE, CRED],
        REPLACING,
    )
    .emits(&[cancel(2), replace(3)]),
    case(
        "credentials changed unblocks",
        &[C, Fail(Failure::Blocked(BlockReason::Authentication)), CRED],
        ESTABLISHING,
    )
    .emits(&[open(2)]),
    case(
        "credentials changed ends a backoff and resets the ladder",
        &[C, UNREACHABLE, CRED],
        ESTABLISHING,
    )
    .emits(&[open(2)])
    .step(0),
    case(
        "credentials changed while offline waits",
        &[NET_DOWN, C, CRED],
        Phase::Offline,
    ),
    // Disconnecting and unwanted hosts.
    case(
        "disconnect closes the connection",
        &[C, Up, S(Signal::Disconnect)],
        Phase::Available,
    )
    .emits(&[close(1)]),
    case(
        "disconnect during a backoff stops retrying",
        &[C, UNREACHABLE, S(Signal::Disconnect), secs(5), Tick],
        Phase::Available,
    )
    .step(0),
    case(
        "an unwanted host ignores device signals",
        &[BG, secs(600), ACTIVE, NET_UP, CRED, Tick],
        Phase::Available,
    )
    .fresh(Freshness::Unknown),
];

fn run_case(case: &Case) {
    let mut supervisor = Supervisor::new(policy()).expect("valid policy");
    let mut now = Moment(0);
    let mut last = Vec::new();
    let mut stale = false;
    for (index, step) in case.steps.iter().enumerate() {
        let context = || format!("{}: step {index} ({step:?})", case.name);
        let attempt = || {
            supervisor
                .attempt_id()
                .unwrap_or_else(|| panic!("{}: no attempt", context()))
        };
        let connection = || {
            supervisor
                .status()
                .connection
                .unwrap_or_else(|| panic!("{}: no connection", context()))
        };
        let result = match *step {
            S(signal) => Ok(supervisor.signal(now, signal)),
            Step::Up => supervisor.report(now, Report::Established(attempt())),
            Fail(failure) => supervisor.report(now, Report::Failed(attempt(), failure)),
            Lose(failure) => supervisor.report(now, Report::ConnectionLost(connection(), failure)),
            Late(id) => supervisor.report(now, Report::Established(AttemptId(id))),
            Data => supervisor.report(now, Report::DataCurrent(connection())),
            DataOn(id) => supervisor.report(now, Report::DataCurrent(ConnectionId(id))),
            SubFail => supervisor.report(now, Report::SubscriptionFailed(connection())),
            Wait(ms) => {
                now = Moment(now.0 + ms);
                Ok(Vec::new())
            }
            Tick => Ok(supervisor.tick(now)),
        };
        supervisor.check();
        let last_step = index + 1 == case.steps.len();
        match result {
            Result::Ok(commands) => last = commands,
            Err(StaleReport) if last_step => {
                stale = true;
                last = Vec::new();
            }
            Err(StaleReport) if matches!(step, Late(_) | DataOn(_)) => last = Vec::new(),
            Err(StaleReport) => panic!("{}: unexpected stale report", context()),
        }
    }
    let status = supervisor.status();
    assert_eq!(status.phase, case.phase, "{}: phase", case.name);
    assert_eq!(last, case.commands, "{}: commands", case.name);
    assert_eq!(stale, case.stale, "{}: stale report", case.name);
    if let Some(step) = case.step {
        assert_eq!(status.step, step, "{}: ladder step", case.name);
    }
    if let Some(freshness) = case.freshness {
        assert_eq!(status.freshness, freshness, "{}: freshness", case.name);
    }
}

#[test]
fn every_transition_in_the_table() {
    let mut names = BTreeSet::new();
    for case in CASES {
        assert!(names.insert(case.name), "duplicate case {}", case.name);
        run_case(case);
    }
}

#[test]
fn next_deadline_follows_the_state() {
    let mut supervisor = Supervisor::new(policy()).unwrap();
    assert_eq!(supervisor.next_deadline(), None);
    supervisor.signal(Moment(0), Signal::Connect);
    assert_eq!(supervisor.next_deadline(), Some(Moment(10_000)));
    let attempt = supervisor.attempt_id().unwrap();
    supervisor
        .report(Moment(500), Report::Failed(attempt, Failure::Unreachable))
        .unwrap();
    assert_eq!(supervisor.next_deadline(), Some(Moment(1500)));
    supervisor.tick(Moment(1500));
    let attempt = supervisor.attempt_id().unwrap();
    supervisor
        .report(Moment(1600), Report::Established(attempt))
        .unwrap();
    // The stable-connection reset is still pending.
    assert_eq!(supervisor.next_deadline(), Some(Moment(31_600)));
    supervisor.tick(Moment(31_600));
    assert_eq!(supervisor.status().step, 0);
    assert_eq!(supervisor.next_deadline(), None);
}

#[test]
fn switched_off_host_keeps_its_wish_and_ignores_signals() {
    let mut supervisor = Supervisor::new(policy()).unwrap();
    supervisor.signal(Moment(0), Signal::Connect);
    let attempt = supervisor.attempt_id().unwrap();
    supervisor
        .report(Moment(0), Report::Established(attempt))
        .unwrap();
    assert_eq!(supervisor.switch_off(), vec![close(1)]);
    for signal in [
        Signal::ApplicationActive,
        Signal::NetworkChanged { available: true },
        Signal::RetryNow,
        Signal::CredentialsChanged,
        Signal::Connect,
    ] {
        assert!(
            supervisor.signal(Moment(1), signal).is_empty(),
            "{signal:?}"
        );
    }
    let status = supervisor.status();
    assert_eq!(status.phase, Phase::Available);
    assert!(!status.enabled && status.wanted);
    assert_eq!(supervisor.switch_on(Moment(2)), vec![open(2)]);
    assert!(supervisor.switch_on(Moment(2)).is_empty());
}

#[test]
fn policy_validation() {
    assert_eq!(Policy::default().validate(), Result::Ok(()));
    let zero = Policy {
        probe_timeout: Duration::ZERO,
        ..policy()
    };
    assert_eq!(zero.validate(), Err(PolicyError::ZeroTimeout));
    let empty = Policy {
        ladder: Vec::new(),
        ..policy()
    };
    assert_eq!(empty.validate(), Err(PolicyError::Ladder));
    let decreasing = Policy {
        ladder: vec![Duration::from_secs(2), Duration::from_secs(1)],
        ..policy()
    };
    assert_eq!(decreasing.validate(), Err(PolicyError::Decreasing));
    assert!(Supervisor::new(decreasing).is_err());
}

#[test]
fn identifiers_are_bounded_visible_ascii() {
    assert!(HostKey::new("a1b2").is_ok());
    assert_eq!(HostKey::new(""), Err(InvalidId));
    assert_eq!(HostKey::new("has space"), Err(InvalidId));
    assert_eq!(HostKey::new("x".repeat(129)), Err(InvalidId));
    assert!(CredentialId::new("grant:1").is_ok());
    assert_eq!(CredentialId::new("tab\t"), Err(InvalidId));
}

#[test]
fn manual_clock_is_shared_and_saturates() {
    let clock = ManualClock::new(Moment(5));
    let other = clock.clone();
    clock.advance(Duration::from_millis(10));
    assert_eq!(other.now(), Moment(15));
    clock.advance(Duration::MAX);
    assert_eq!(other.now(), Moment(u64::MAX));
    assert_eq!(Moment(3).since(Moment(9)), Duration::ZERO);
}

// Registry behavior.

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Open(String, u64, Stage),
    Probe(String, u64, u64),
    Cancel(String, u64),
    Close(String, u64),
}

#[derive(Default)]
struct Recorder(Vec<Call>);

impl Connector for Recorder {
    fn open(&mut self, host: &HostKey, attempt: AttemptId, stage: Stage) {
        self.0.push(Call::Open(host.to_string(), attempt.0, stage));
    }
    fn probe(&mut self, host: &HostKey, attempt: AttemptId, connection: ConnectionId) {
        self.0
            .push(Call::Probe(host.to_string(), attempt.0, connection.0));
    }
    fn cancel(&mut self, host: &HostKey, attempt: AttemptId) {
        self.0.push(Call::Cancel(host.to_string(), attempt.0));
    }
    fn close(&mut self, host: &HostKey, connection: ConnectionId) {
        self.0.push(Call::Close(host.to_string(), connection.0));
    }
}

fn host(name: &str) -> HostKey {
    HostKey::new(name).unwrap()
}

fn credential(name: &str) -> CredentialId {
    CredentialId::new(name).unwrap()
}

fn route(credentials: &[&str]) -> Route {
    Route {
        credentials: credentials.iter().map(|name| credential(name)).collect(),
    }
}

fn registry() -> (ManualClock, Registry<ManualClock, Recorder>) {
    let clock = ManualClock::default();
    let registry = Registry::new(clock.clone(), Recorder::default(), policy()).unwrap();
    (clock, registry)
}

fn drain(registry: &mut Registry<ManualClock, Recorder>) -> Vec<Call> {
    std::mem::take(&mut registry.connector_mut().0)
}

#[test]
fn remove_during_backoff_forgets_and_stops_retrying() {
    let (clock, mut registry) = registry();
    let a = host("a");
    registry.register(a.clone(), route(&["grant-a"])).unwrap();
    registry.signal(&a, Signal::Connect).unwrap();
    registry
        .report(&a, Report::Failed(AttemptId(1), Failure::Unreachable))
        .unwrap();
    assert_eq!(registry.status(&a).unwrap().phase, backoff(1000));
    drain(&mut registry);

    let mut forgotten = Vec::new();
    registry
        .remove(&a, |key, route| {
            forgotten.push((key.clone(), route.credentials.clone()))
        })
        .unwrap();
    assert_eq!(
        forgotten,
        vec![(a.clone(), BTreeSet::from([credential("grant-a")]))]
    );
    // A backoff holds no attempt or connection, so there is nothing to stop.
    assert!(drain(&mut registry).is_empty());

    clock.advance(Duration::from_secs(5));
    registry.tick();
    assert!(drain(&mut registry).is_empty(), "a removed host retried");
    assert_eq!(registry.next_deadline(), None);
    assert_eq!(registry.status(&a), None);
    assert_eq!(
        registry.report(&a, Report::Established(AttemptId(2))),
        Err(RegistryError::Unknown)
    );
    assert_eq!(registry.remove(&a, |_, _| {}), Err(RegistryError::Unknown));
}

#[test]
fn remove_while_connected_closes_before_forgetting() {
    let (_, mut registry) = registry();
    let a = host("a");
    registry.register(a.clone(), Route::default()).unwrap();
    registry.signal(&a, Signal::Connect).unwrap();
    registry
        .report(&a, Report::Established(AttemptId(1)))
        .unwrap();
    drain(&mut registry);
    let mut calls_at_forget = None;
    let result = registry.remove(&a, |_, _| calls_at_forget = Some(()));
    result.unwrap();
    assert!(calls_at_forget.is_some());
    assert_eq!(drain(&mut registry), vec![Call::Close("a".into(), 1)]);
}

#[test]
fn credential_sweep_affects_only_dependent_routes() {
    let (_, mut registry) = registry();
    for (name, credentials) in [("a", &["x"][..]), ("b", &["y"][..]), ("c", &["x", "y"][..])] {
        let key = host(name);
        registry.register(key.clone(), route(credentials)).unwrap();
        registry.signal(&key, Signal::Connect).unwrap();
        registry
            .report(&key, Report::Established(AttemptId(1)))
            .unwrap();
    }
    let idle = host("d");
    registry.register(idle.clone(), route(&["x"])).unwrap();
    drain(&mut registry);

    let affected = registry.credentials_changed(&credential("x"));
    assert_eq!(affected, vec![host("a"), host("c"), idle.clone()]);
    assert_eq!(
        drain(&mut registry),
        vec![
            Call::Open("a".into(), 2, Stage::Replacing),
            Call::Open("c".into(), 2, Stage::Replacing),
        ]
    );
    assert_eq!(registry.status(&host("b")).unwrap().phase, Phase::Connected);
    assert_eq!(registry.status(&idle).unwrap().phase, Phase::Available);
    assert!(registry.credentials_changed(&credential("z")).is_empty());
}

#[test]
fn switch_off_keeps_the_host_registered() {
    let (_, mut registry) = registry();
    let a = host("a");
    registry.register(a.clone(), route(&["x"])).unwrap();
    registry.signal(&a, Signal::Connect).unwrap();
    registry
        .report(&a, Report::Established(AttemptId(1)))
        .unwrap();
    registry.switch_off(&a).unwrap();
    registry.signal_all(Signal::NetworkChanged { available: true });
    registry.credentials_changed(&credential("x"));
    assert_eq!(
        drain(&mut registry),
        vec![
            Call::Open("a".into(), 1, Stage::Establishing),
            Call::Close("a".into(), 1),
        ]
    );
    assert_eq!(registry.route(&a), Some(&route(&["x"])));
    registry.switch_on(&a).unwrap();
    assert_eq!(
        drain(&mut registry),
        vec![Call::Open("a".into(), 2, Stage::Establishing)]
    );
}

#[test]
fn device_signals_reach_every_host_through_the_clock() {
    let (clock, mut registry) = registry();
    let (a, b) = (host("a"), host("b"));
    for key in [&a, &b] {
        registry.register(key.clone(), Route::default()).unwrap();
        registry.signal(key, Signal::Connect).unwrap();
        registry
            .report(key, Report::Established(AttemptId(1)))
            .unwrap();
    }
    drain(&mut registry);
    registry.signal_all(Signal::ApplicationBackground);
    clock.advance(Duration::from_secs(301));
    registry.signal_all(Signal::ApplicationActive);
    assert_eq!(
        drain(&mut registry),
        vec![
            Call::Open("a".into(), 2, Stage::Replacing),
            Call::Open("b".into(), 2, Stage::Replacing),
        ]
    );
    assert_eq!(registry.next_deadline(), Some(Moment(311_000)));
    clock.advance(Duration::from_secs(10));
    registry.tick();
    assert_eq!(
        drain(&mut registry),
        vec![
            Call::Cancel("a".into(), 2),
            Call::Close("a".into(), 1),
            Call::Cancel("b".into(), 2),
            Call::Close("b".into(), 1),
        ]
    );
    let statuses: Vec<_> = registry
        .statuses()
        .map(|(_, status)| status.phase)
        .collect();
    assert_eq!(statuses, vec![backoff(312_000), backoff(312_000)]);
}

#[test]
fn registry_refusals() {
    let (_, mut registry) = registry();
    let a = host("a");
    registry.register(a.clone(), Route::default()).unwrap();
    assert_eq!(
        registry.register(a.clone(), Route::default()),
        Err(RegistryError::Duplicate)
    );
    assert_eq!(
        registry.signal(&host("b"), Signal::Connect),
        Err(RegistryError::Unknown)
    );
    registry.signal(&a, Signal::Connect).unwrap();
    assert_eq!(
        registry.report(&a, Report::Established(AttemptId(9))),
        Err(RegistryError::Stale)
    );
    assert!(
        Registry::new(
            ManualClock::default(),
            Recorder::default(),
            Policy {
                ladder: Vec::new(),
                ..policy()
            }
        )
        .is_err()
    );
}
