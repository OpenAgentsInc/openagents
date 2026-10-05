//! Bounded local diagnostics with no transport credentials or player payloads.
use super::net::{AdmissionStats, Stats, Timing};
use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::{Notify, watch};

pub const SCHEMA: &str = "verse.host.operations.v1";
pub const CLIENTS: usize = super::net::admission::CONNECTIONS;
pub const RECORDS: usize = 128;
pub const STATUS_BYTES: usize = 256 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub package_version: String,
    pub source_revision: String,
    pub wire_version: u16,
}
impl Build {
    pub fn validate(&self) -> Result<(), String> {
        if self.wire_version != super::wire::VERSION
            || self.package_version.is_empty()
            || self.package_version.len() > 128
            || self.source_revision.is_empty()
            || self.source_revision.len() > 128
            || self.package_version.chars().any(char::is_control)
            || self.source_revision.chars().any(char::is_control)
        {
            return Err("Operator build identity is incompatible or invalid".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Starting,
    Running,
    Draining,
    Stopped,
    Failed,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Starting,
    Draining,
    Stopped,
    ConnectionCapacity,
    StorageBackpressure,
    ReceiptBackpressure,
    WriterStalled,
    WorkBudget,
    SimulationOverBudget,
    TransportFailure,
    AuthorityFailure,
    StorageFailure,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Distribution {
    pub observations: u64,
    pub p95_upper_bound_ms: Option<f64>,
    pub p99_upper_bound_ms: Option<f64>,
    pub maximum_ms: f64,
    pub total_ms: f64,
}
impl From<&Timing> for Distribution {
    fn from(t: &Timing) -> Self {
        Self {
            observations: t.count,
            p95_upper_bound_ms: t.percentile(0.95).map(|s| s * 1000.),
            p99_upper_bound_ms: t.percentile(0.99).map(|s| s * 1000.),
            maximum_ms: t.maximum_seconds * 1000.,
            total_ms: t.total_seconds * 1000.,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    pub connections: usize,
    pub pre_auth_connections: usize,
    pub request_queue: usize,
    pub writer_copies: usize,
    pub held_reply_bytes: usize,
    pub receipt_staging_bytes: usize,
    pub client_observations: usize,
    pub diagnostic_records: usize,
    pub status_bytes: usize,
}
impl Default for Budgets {
    fn default() -> Self {
        Self {
            connections: CLIENTS,
            pre_auth_connections: super::net::admission::PRE_AUTH,
            request_queue: super::net::QUEUE,
            writer_copies: 2,
            held_reply_bytes: super::net::REPLY_BYTES * 2,
            receipt_staging_bytes: super::rewards::history::PENDING_BYTES,
            client_observations: CLIENTS,
            diagnostic_records: RECORDS,
            status_bytes: STATUS_BYTES,
        }
    }
}
/// IDs identify this process's connections, not accounts or network addresses.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Client {
    pub connection: u64,
    pub authenticated: bool,
    pub requests: u64,
    pub received_payload_bytes: u64,
    pub sent_payload_bytes: u64,
    pub idle_ms: u64,
    pub delivered_tick: Option<u64>,
    pub delivered_tick_lag: Option<u64>,
    pub last_delivery_ms_ago: Option<u64>,
    pub work_refusals: u64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub admission: AdmissionStats,
    pub ticks: u64,
    pub requests: u64,
    pub dropped_seconds: f64,
    pub simulation: Distribution,
    pub read_projection: Distribution,
    pub storage_commit: Distribution,
    pub journal_sync: Distribution,
    pub request_queue: usize,
    pub request_queue_peak: usize,
    pub writer_queue: usize,
    pub writer_queue_peak: usize,
    pub held_reply_bytes: usize,
    pub held_reply_bytes_peak: usize,
    pub storage_refusals: u64,
    pub storage_paused_seconds: f64,
    pub durable_commits: u64,
    pub durable_revision: u64,
    pub pending_commit_age_ms: Option<u64>,
    pub actor_count: usize,
    pub live_hostiles: usize,
    pub motor_recovery_blocks: u64,
    pub navigation_refusals: u64,
}
impl Metrics {
    pub(super) fn capture(stats: &Stats) -> Self {
        Self {
            admission: stats.admission.clone(),
            ticks: stats.ticks,
            requests: stats.requests,
            dropped_seconds: stats.dropped_seconds,
            simulation: (&stats.simulation).into(),
            read_projection: (&stats.read_projection).into(),
            storage_commit: (&stats.commits).into(),
            journal_sync: (&stats.journal_sync).into(),
            request_queue_peak: stats.request_queue_peak,
            writer_queue_peak: stats.writer_queue_peak,
            held_reply_bytes_peak: stats.held_reply_bytes_peak,
            storage_refusals: stats.storage_refusals,
            storage_paused_seconds: stats.storage_paused_seconds,
            durable_commits: stats.checkpoint_commits,
            durable_revision: stats.durable_revision,
            ..Default::default()
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub sequence: u64,
    pub elapsed_ms: u64,
    pub phase: Phase,
    pub reasons: Vec<Reason>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: String,
    pub build: Build,
    pub instance: u64,
    pub content: Option<[u8; 32]>,
    pub sampled_unix_ms: u64,
    pub elapsed_ms: u64,
    pub phase: Phase,
    pub live: bool,
    pub ready: bool,
    pub reasons: Vec<Reason>,
    pub metrics: Metrics,
    pub budgets: Budgets,
    pub clients: Vec<Client>,
    pub records: Vec<Record>,
    pub omitted_records: u64,
}
impl Snapshot {
    pub fn validate(&self) -> Result<(), String> {
        self.build.validate()?;
        if self.schema != SCHEMA
            || self.instance == 0
            || self.clients.len() > CLIENTS
            || self.records.len() > RECORDS
            || self.reasons.len() > 16
            || self.records.iter().any(|r| r.reasons.len() > 16)
            || self
                .metrics
                .admission
                .active
                .checked_add(self.metrics.admission.pending)
                .is_none_or(|n| n > CLIENTS)
        {
            return Err("Operator snapshot is incompatible or exceeds its budget".into());
        }
        Ok(())
    }
}
/// Readers receive owned snapshots and cannot hold the publisher's channel lock.
pub struct Observer {
    receiver: watch::Receiver<Snapshot>,
}
impl Observer {
    pub fn latest(&self) -> Snapshot {
        self.receiver.borrow().clone()
    }
    pub async fn changed(&mut self) -> Result<Snapshot, tokio::sync::watch::error::RecvError> {
        self.receiver.changed().await?;
        Ok(self.latest())
    }
}
/// A latest-value channel never queues diagnostics behind a slow observer.
#[derive(Clone)]
pub struct Monitor {
    sender: watch::Sender<Snapshot>,
    drain: Arc<AtomicBool>,
    wake: Arc<Notify>,
}
impl Monitor {
    pub fn new(build: Build, instance: u64, content: Option<[u8; 32]>) -> Result<Self, String> {
        build.validate()?;
        if instance == 0 {
            return Err("Operator instance identity is invalid".into());
        }
        let (sender, _) = watch::channel(Snapshot {
            schema: SCHEMA.into(),
            build,
            instance,
            content,
            sampled_unix_ms: 0,
            elapsed_ms: 0,
            phase: Phase::Starting,
            live: false,
            ready: false,
            reasons: vec![Reason::Starting],
            metrics: Default::default(),
            budgets: Default::default(),
            clients: vec![],
            records: vec![],
            omitted_records: 0,
        });
        Ok(Self {
            sender,
            drain: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(Notify::new()),
        })
    }
    pub fn subscribe(&self) -> Observer {
        Observer {
            receiver: self.sender.subscribe(),
        }
    }
    pub fn snapshot(&self) -> Snapshot {
        self.sender.borrow().clone()
    }
    /// Stops new admission through the existing ordered service drain.
    pub fn request_drain(&self) {
        self.drain.store(true, Ordering::Release);
        self.wake.notify_one();
    }
    pub(super) async fn draining(&self) {
        loop {
            let notified = self.wake.notified();
            if self.drain.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    }
    pub fn phase(&self, phase: Phase, reason: Reason, start: std::time::Instant) {
        let mut snapshot = self.snapshot();
        snapshot.phase = phase;
        snapshot.live = matches!(phase, Phase::Running | Phase::Draining);
        snapshot.ready = phase == Phase::Running;
        snapshot.reasons = vec![reason];
        stamp(&mut snapshot, start);
        self.publish(snapshot);
    }
    pub(super) fn running(
        &self,
        gateway: &super::auth::Gateway,
        stats: &Stats,
        start: std::time::Instant,
        requests: usize,
        writes: usize,
        bytes: usize,
        oldest: Option<std::time::Duration>,
        history: bool,
        clients: Vec<Client>,
    ) {
        let mut snapshot = self.snapshot();
        let mut reasons = vec![];
        if stats.admission.active + stats.admission.pending >= CLIENTS {
            reasons.push(Reason::ConnectionCapacity);
        }
        if writes >= 2 {
            reasons.push(Reason::StorageBackpressure);
        }
        if !history {
            reasons.push(Reason::ReceiptBackpressure);
        }
        if oldest.is_some_and(|age| age >= std::time::Duration::from_secs(1)) {
            reasons.push(Reason::WriterStalled);
        }
        let previous = &snapshot.metrics.admission;
        if stats.admission.principal_work_refusals > previous.principal_work_refusals
            || stats.admission.aggregate_work_refusals > previous.aggregate_work_refusals
        {
            reasons.push(Reason::WorkBudget);
        }
        if stats
            .simulation
            .percentile(0.99)
            .is_some_and(|seconds| seconds > 1. / 30.)
        {
            reasons.push(Reason::SimulationOverBudget);
        }
        snapshot.phase = Phase::Running;
        snapshot.live = true;
        snapshot.ready = !reasons.iter().any(|r| {
            matches!(
                r,
                Reason::ConnectionCapacity
                    | Reason::StorageBackpressure
                    | Reason::ReceiptBackpressure
                    | Reason::WriterStalled
            )
        });
        snapshot.reasons = reasons;
        snapshot.metrics = Metrics::capture(stats);
        snapshot.metrics.request_queue = requests;
        snapshot.metrics.writer_queue = writes;
        snapshot.metrics.held_reply_bytes = bytes;
        snapshot.metrics.pending_commit_age_ms = oldest.map(millis);
        // Scene actors are bounded by content admission. Avoid projecting the render frame.
        snapshot.metrics.actor_count = gateway.game().scene.actors.len();
        snapshot.metrics.live_hostiles = gateway.game().simulation.live_hostiles();
        snapshot.metrics.motor_recovery_blocks = gateway.game().motor_recovery.blocks;
        snapshot.metrics.navigation_refusals = gateway.game().navigation_budget_refusals;
        snapshot.clients = clients;
        stamp(&mut snapshot, start);
        self.publish(snapshot);
    }
    pub(super) fn terminal(&self, exit: &super::net::Exit, start: std::time::Instant) {
        let mut snapshot = self.snapshot();
        snapshot.phase = if exit.failure.is_some() {
            Phase::Failed
        } else {
            Phase::Stopped
        };
        snapshot.live = false;
        snapshot.ready = false;
        snapshot.metrics = Metrics::capture(&exit.stats);
        snapshot.metrics.actor_count = exit.gateway.game().scene.actors.len();
        snapshot.metrics.live_hostiles = exit.gateway.game().simulation.live_hostiles();
        snapshot.metrics.motor_recovery_blocks = exit.gateway.game().motor_recovery.blocks;
        snapshot.metrics.navigation_refusals = exit.gateway.game().navigation_budget_refusals;
        snapshot.clients.clear();
        snapshot.reasons = vec![match exit.failure.as_deref() {
            None => Reason::Stopped,
            Some(error)
                if error.contains("storage")
                    || error.contains("checkpoint")
                    || error.contains("journal")
                    || error.contains("history") =>
            {
                Reason::StorageFailure
            }
            Some(error) if error.contains("listener") || error.contains("dispatch queue") => {
                Reason::TransportFailure
            }
            _ => Reason::AuthorityFailure,
        }];
        stamp(&mut snapshot, start);
        self.publish(snapshot);
    }
    pub(super) fn publish(&self, mut snapshot: Snapshot) {
        snapshot.clients.truncate(CLIENTS);
        let mut previous = self.sender.borrow().clone();
        if previous.phase != snapshot.phase || previous.reasons != snapshot.reasons {
            let sequence = previous
                .records
                .last()
                .map_or(1, |r| r.sequence.saturating_add(1));
            previous.records.push(Record {
                sequence,
                elapsed_ms: snapshot.elapsed_ms,
                phase: snapshot.phase,
                reasons: snapshot.reasons.clone(),
            });
            if previous.records.len() > RECORDS {
                previous.records.remove(0);
                previous.omitted_records = previous.omitted_records.saturating_add(1);
            }
        }
        snapshot.records = previous.records;
        snapshot.omitted_records = previous.omitted_records;
        self.sender.send_replace(snapshot);
    }
}

pub(super) fn millis(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
fn stamp(snapshot: &mut Snapshot, start: std::time::Instant) {
    snapshot.elapsed_ms = millis(start.elapsed());
    snapshot.sampled_unix_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(millis)
        .unwrap_or(0);
}
#[cfg(test)]
mod tests {
    use super::*;
    fn monitor() -> Monitor {
        Monitor::new(
            Build {
                package_version: "test".into(),
                source_revision: "scratch".into(),
                wire_version: super::super::wire::VERSION,
            },
            120,
            Some([8; 32]),
        )
        .unwrap()
    }
    #[test]
    fn slow_observer_keeps_only_latest_bounded_state() {
        let m = monitor();
        let observer = m.subscribe();
        let initial = observer.latest();
        for n in 0..1000 {
            m.phase(
                if n % 2 == 0 {
                    Phase::Running
                } else {
                    Phase::Draining
                },
                Reason::Draining,
                std::time::Instant::now(),
            );
        }
        let snapshot = observer.latest();
        assert_eq!(initial.phase, Phase::Starting);
        assert_eq!(snapshot.phase, Phase::Draining);
        assert_eq!(snapshot.records.len(), RECORDS);
        assert_eq!(snapshot.omitted_records, 872);
        assert!(serde_json::to_vec(&snapshot).unwrap().len() < STATUS_BYTES);
    }
    #[tokio::test]
    async fn drain_requested_before_wait_is_not_lost() {
        let m = monitor();
        m.request_drain();
        tokio::time::timeout(std::time::Duration::from_millis(50), m.draining())
            .await
            .unwrap();
    }
    #[test]
    fn incompatible_build_cannot_start_monitor() {
        let mut build = monitor().snapshot().build;
        build.wire_version += 1;
        assert!(Monitor::new(build, 120, None).is_err());
    }
}
