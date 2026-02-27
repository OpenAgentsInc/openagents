use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use nostr::{
    KIND_AGENT_PROFILE, KIND_AGENT_SCHEDULE, KIND_AGENT_STATE, KIND_CREDIT_DEFAULT_NOTICE,
    KIND_CREDIT_ENVELOPE, KIND_CREDIT_INTENT, KIND_CREDIT_OFFER, KIND_CREDIT_SETTLEMENT,
    KIND_CREDIT_SPEND_AUTH, KIND_SKILL_MANIFEST, KIND_SKILL_SEARCH_REQUEST,
    KIND_SKILL_SEARCH_RESULT, KIND_SKILL_VERSION_LOG, KIND_TICK_REQUEST, KIND_TICK_RESULT,
};

const SA_LANE_POLL: Duration = Duration::from_millis(120);
const SKL_LANE_POLL: Duration = Duration::from_millis(120);
const AC_LANE_POLL: Duration = Duration::from_millis(120);
const SA_CONNECT_DELAY: Duration = Duration::from_millis(900);
const SKL_TRUST_DELAY: Duration = Duration::from_millis(420);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeLane {
    SaLifecycle,
    SklDiscoveryTrust,
    AcCredit,
}

impl RuntimeLane {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SaLifecycle => "sa_lifecycle",
            Self::SklDiscoveryTrust => "skl_discovery_trust",
            Self::AcCredit => "ac_credit",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCommandStatus {
    Accepted,
    Rejected,
    Retryable,
}

impl RuntimeCommandStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Retryable => "retryable",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCommandErrorClass {
    Validation,
    Dependency,
    Transport,
}

impl RuntimeCommandErrorClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Dependency => "dependency",
            Self::Transport => "transport",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCommandError {
    pub class: RuntimeCommandErrorClass,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeCommandKind {
    SetRunnerOnline,
    PublishAgentProfile,
    PublishAgentState,
    ConfigureAgentSchedule,
    PublishTickRequest,
    PublishSkillManifest,
    PublishSkillVersionLog,
    SubmitSkillSearch,
    PublishCreditIntent,
    PublishCreditOffer,
    PublishCreditEnvelope,
    PublishCreditSpendAuth,
    PublishCreditSettlement,
    PublishCreditDefault,
}

impl RuntimeCommandKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SetRunnerOnline => "SetRunnerOnline",
            Self::PublishAgentProfile => "PublishAgentProfile",
            Self::PublishAgentState => "PublishAgentState",
            Self::ConfigureAgentSchedule => "ConfigureAgentSchedule",
            Self::PublishTickRequest => "PublishTickRequest",
            Self::PublishSkillManifest => "PublishSkillManifest",
            Self::PublishSkillVersionLog => "PublishSkillVersionLog",
            Self::SubmitSkillSearch => "SubmitSkillSearch",
            Self::PublishCreditIntent => "PublishCreditIntent",
            Self::PublishCreditOffer => "PublishCreditOffer",
            Self::PublishCreditEnvelope => "PublishCreditEnvelope",
            Self::PublishCreditSpendAuth => "PublishCreditSpendAuth",
            Self::PublishCreditSettlement => "PublishCreditSettlement",
            Self::PublishCreditDefault => "PublishCreditDefault",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeCommandResponse {
    pub command_seq: u64,
    pub lane: RuntimeLane,
    pub command: RuntimeCommandKind,
    pub status: RuntimeCommandStatus,
    pub event_id: Option<String>,
    pub error: Option<RuntimeCommandError>,
}

impl RuntimeCommandResponse {
    fn accepted(
        command_seq: u64,
        lane: RuntimeLane,
        command: RuntimeCommandKind,
        event_id: String,
    ) -> Self {
        Self {
            command_seq,
            lane,
            command,
            status: RuntimeCommandStatus::Accepted,
            event_id: Some(event_id),
            error: None,
        }
    }

    fn rejected(
        command_seq: u64,
        lane: RuntimeLane,
        command: RuntimeCommandKind,
        class: RuntimeCommandErrorClass,
        message: impl Into<String>,
    ) -> Self {
        Self {
            command_seq,
            lane,
            command,
            status: RuntimeCommandStatus::Rejected,
            event_id: None,
            error: Some(RuntimeCommandError {
                class,
                message: message.into(),
            }),
        }
    }

    fn retryable(
        command_seq: u64,
        lane: RuntimeLane,
        command: RuntimeCommandKind,
        class: RuntimeCommandErrorClass,
        message: impl Into<String>,
    ) -> Self {
        Self {
            command_seq,
            lane,
            command,
            status: RuntimeCommandStatus::Retryable,
            event_id: None,
            error: Some(RuntimeCommandError {
                class,
                message: message.into(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaRunnerMode {
    Offline,
    Connecting,
    Online,
}

impl SaRunnerMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Online => "online",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SaLaneSnapshot {
    pub mode: SaRunnerMode,
    pub mode_changed_at: Instant,
    pub connect_until: Option<Instant>,
    pub online_since: Option<Instant>,
    pub last_heartbeat_at: Option<Instant>,
    pub heartbeat_seconds: u64,
    pub queue_depth: u32,
    pub tick_count: u64,
    pub last_result: Option<String>,
    pub degraded_reason_code: Option<String>,
    pub last_error_detail: Option<String>,
    pub profile_event_id: Option<String>,
    pub state_event_id: Option<String>,
    pub schedule_event_id: Option<String>,
    pub last_tick_request_event_id: Option<String>,
    pub last_tick_result_event_id: Option<String>,
}

impl Default for SaLaneSnapshot {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            mode: SaRunnerMode::Offline,
            mode_changed_at: now,
            connect_until: None,
            online_since: None,
            last_heartbeat_at: None,
            heartbeat_seconds: 30,
            queue_depth: 0,
            tick_count: 0,
            last_result: None,
            degraded_reason_code: None,
            last_error_detail: None,
            profile_event_id: None,
            state_event_id: None,
            schedule_event_id: None,
            last_tick_request_event_id: None,
            last_tick_result_event_id: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SkillTrustTier {
    Unknown,
    Provisional,
    Trusted,
}

impl SkillTrustTier {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Provisional => "provisional",
            Self::Trusted => "trusted",
        }
    }
}

#[derive(Clone, Debug)]
pub struct SklLaneSnapshot {
    pub trust_tier: SkillTrustTier,
    pub manifest_a: Option<String>,
    pub manifest_event_id: Option<String>,
    pub version_log_event_id: Option<String>,
    pub search_result_event_id: Option<String>,
    pub revocation_event_id: Option<String>,
    pub kill_switch_active: bool,
    pub last_error: Option<String>,
}

impl Default for SklLaneSnapshot {
    fn default() -> Self {
        Self {
            trust_tier: SkillTrustTier::Unknown,
            manifest_a: None,
            manifest_event_id: None,
            version_log_event_id: None,
            search_result_event_id: None,
            revocation_event_id: None,
            kill_switch_active: false,
            last_error: None,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct AcLaneSnapshot {
    pub credit_available: bool,
    pub available_credit_sats: u64,
    pub intent_event_id: Option<String>,
    pub offer_event_id: Option<String>,
    pub envelope_event_id: Option<String>,
    pub spend_auth_event_id: Option<String>,
    pub settlement_event_id: Option<String>,
    pub default_event_id: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum SaLifecycleCommand {
    SetRunnerOnline {
        online: bool,
    },
    PublishAgentProfile {
        display_name: String,
        about: String,
        version: String,
    },
    PublishAgentState {
        encrypted_state_ref: String,
    },
    ConfigureAgentSchedule {
        heartbeat_seconds: u64,
    },
    PublishTickRequest {
        reason: String,
    },
}

#[derive(Clone, Debug)]
pub enum SklDiscoveryTrustCommand {
    PublishSkillManifest {
        skill_slug: String,
        version: String,
    },
    PublishSkillVersionLog {
        skill_slug: String,
        version: String,
        summary: String,
    },
    SubmitSkillSearch {
        query: String,
        limit: u32,
    },
}

#[derive(Clone, Debug)]
pub enum AcCreditCommand {
    PublishCreditIntent {
        scope: String,
        request_type: String,
        payload: String,
        skill_scope_id: Option<String>,
        credit_envelope_ref: Option<String>,
        requested_sats: u64,
        timeout_seconds: u64,
    },
    PublishCreditOffer {
        intent_event_id: String,
        offered_sats: u64,
    },
    PublishCreditEnvelope {
        offer_event_id: String,
        cap_sats: u64,
    },
    PublishCreditSpendAuth {
        envelope_event_id: String,
        job_id: String,
        spend_sats: u64,
    },
    PublishCreditSettlement {
        envelope_event_id: String,
        result_event_id: String,
        payment_pointer: String,
    },
    PublishCreditDefault {
        envelope_event_id: String,
        reason: String,
    },
}

#[derive(Clone, Debug)]
pub enum SaLaneUpdate {
    Snapshot(Box<SaLaneSnapshot>),
    CommandResponse(RuntimeCommandResponse),
}

#[derive(Clone, Debug)]
pub enum SklLaneUpdate {
    Snapshot(Box<SklLaneSnapshot>),
    CommandResponse(RuntimeCommandResponse),
}

#[derive(Clone, Debug)]
pub enum AcLaneUpdate {
    Snapshot(Box<AcLaneSnapshot>),
    CommandResponse(RuntimeCommandResponse),
}

struct SequencedSaCommand {
    command_seq: u64,
    command: SaLifecycleCommand,
}

struct SequencedSklCommand {
    command_seq: u64,
    command: SklDiscoveryTrustCommand,
}

struct SequencedAcCommand {
    command_seq: u64,
    command: AcCreditCommand,
}

pub struct SaLaneWorker {
    command_tx: Sender<SequencedSaCommand>,
    update_rx: Receiver<SaLaneUpdate>,
}

impl SaLaneWorker {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<SequencedSaCommand>();
        let (update_tx, update_rx) = mpsc::channel::<SaLaneUpdate>();
        std::thread::spawn(move || run_sa_lane_loop(command_rx, update_tx));
        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue(&self, command_seq: u64, command: SaLifecycleCommand) -> Result<(), String> {
        self.command_tx
            .send(SequencedSaCommand {
                command_seq,
                command,
            })
            .map_err(|error| format!("SA lane offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<SaLaneUpdate> {
        drain_updates(&self.update_rx)
    }
}

pub struct SklLaneWorker {
    command_tx: Sender<SequencedSklCommand>,
    update_rx: Receiver<SklLaneUpdate>,
}

impl SklLaneWorker {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<SequencedSklCommand>();
        let (update_tx, update_rx) = mpsc::channel::<SklLaneUpdate>();
        std::thread::spawn(move || run_skl_lane_loop(command_rx, update_tx));
        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue(
        &self,
        command_seq: u64,
        command: SklDiscoveryTrustCommand,
    ) -> Result<(), String> {
        self.command_tx
            .send(SequencedSklCommand {
                command_seq,
                command,
            })
            .map_err(|error| format!("SKL lane offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<SklLaneUpdate> {
        drain_updates(&self.update_rx)
    }
}

pub struct AcLaneWorker {
    command_tx: Sender<SequencedAcCommand>,
    update_rx: Receiver<AcLaneUpdate>,
}

impl AcLaneWorker {
    pub fn spawn() -> Self {
        let (command_tx, command_rx) = mpsc::channel::<SequencedAcCommand>();
        let (update_tx, update_rx) = mpsc::channel::<AcLaneUpdate>();
        std::thread::spawn(move || run_ac_lane_loop(command_rx, update_tx));
        Self {
            command_tx,
            update_rx,
        }
    }

    pub fn enqueue(&self, command_seq: u64, command: AcCreditCommand) -> Result<(), String> {
        self.command_tx
            .send(SequencedAcCommand {
                command_seq,
                command,
            })
            .map_err(|error| format!("AC lane offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<AcLaneUpdate> {
        drain_updates(&self.update_rx)
    }
}

fn drain_updates<T>(receiver: &Receiver<T>) -> Vec<T> {
    let mut updates = Vec::new();
    while let Ok(update) = receiver.try_recv() {
        updates.push(update);
    }
    updates
}

fn run_sa_lane_loop(command_rx: Receiver<SequencedSaCommand>, update_tx: Sender<SaLaneUpdate>) {
    let mut snapshot = SaLaneSnapshot::default();
    let mut next_event_seq: u64 = 1;
    let mut last_tick_at: Option<Instant> = None;

    loop {
        match command_rx.recv_timeout(SA_LANE_POLL) {
            Ok(envelope) => {
                let mut snapshot_changed = false;
                let response = handle_sa_command(
                    &mut snapshot,
                    &mut next_event_seq,
                    envelope,
                    &mut snapshot_changed,
                );
                if snapshot_changed {
                    let _ = update_tx.send(SaLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                }
                let _ = update_tx.send(SaLaneUpdate::CommandResponse(response));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let now = Instant::now();
        let mut snapshot_changed = false;
        if snapshot.mode == SaRunnerMode::Connecting
            && snapshot.connect_until.is_some_and(|until| now >= until)
        {
            snapshot.mode = SaRunnerMode::Online;
            snapshot.mode_changed_at = now;
            snapshot.connect_until = None;
            snapshot.online_since = Some(now);
            snapshot.last_heartbeat_at = Some(now);
            snapshot.last_result = Some("SA runner online".to_string());
            snapshot.last_error_detail = None;
            snapshot.degraded_reason_code = None;
            snapshot_changed = true;
        }

        if snapshot.mode == SaRunnerMode::Online {
            let heartbeat_every = Duration::from_secs(snapshot.heartbeat_seconds.max(1));
            let should_tick =
                last_tick_at.is_none_or(|last| now.duration_since(last) >= heartbeat_every);
            if should_tick {
                last_tick_at = Some(now);
                snapshot.tick_count = snapshot.tick_count.saturating_add(1);
                snapshot.last_heartbeat_at = Some(now);
                snapshot.last_tick_request_event_id =
                    Some(next_event_id("sa", KIND_TICK_REQUEST, &mut next_event_seq));
                snapshot.last_tick_result_event_id =
                    Some(next_event_id("sa", KIND_TICK_RESULT, &mut next_event_seq));
                snapshot.last_result = Some(format!("tick {} completed", snapshot.tick_count));
                snapshot.queue_depth = snapshot.queue_depth.saturating_sub(1);
                snapshot_changed = true;
            }
        }

        if snapshot_changed {
            let _ = update_tx.send(SaLaneUpdate::Snapshot(Box::new(snapshot.clone())));
        }
    }
}

fn handle_sa_command(
    snapshot: &mut SaLaneSnapshot,
    next_event_seq: &mut u64,
    envelope: SequencedSaCommand,
    snapshot_changed: &mut bool,
) -> RuntimeCommandResponse {
    match envelope.command {
        SaLifecycleCommand::SetRunnerOnline { online } => {
            let event_id = next_runtime_event_id("sa", envelope.command_seq);
            let now = Instant::now();
            if online {
                snapshot.mode = SaRunnerMode::Connecting;
                snapshot.mode_changed_at = now;
                snapshot.connect_until = Some(now + SA_CONNECT_DELAY);
                snapshot.online_since = None;
                snapshot.last_heartbeat_at = None;
                snapshot.last_error_detail = None;
                snapshot.degraded_reason_code = None;
                snapshot.last_result = Some("SA runner connecting".to_string());
            } else {
                snapshot.mode = SaRunnerMode::Offline;
                snapshot.mode_changed_at = now;
                snapshot.connect_until = None;
                snapshot.online_since = None;
                snapshot.last_heartbeat_at = None;
                snapshot.queue_depth = 0;
                snapshot.last_error_detail = None;
                snapshot.degraded_reason_code = None;
                snapshot.last_result = Some("SA runner offline".to_string());
            }
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SaLifecycle,
                RuntimeCommandKind::SetRunnerOnline,
                event_id,
            )
        }
        SaLifecycleCommand::PublishAgentProfile {
            display_name,
            about,
            version,
        } => {
            if display_name.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SaLifecycle,
                    RuntimeCommandKind::PublishAgentProfile,
                    RuntimeCommandErrorClass::Validation,
                    "display_name cannot be empty",
                );
            }
            if about.trim().is_empty() || version.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SaLifecycle,
                    RuntimeCommandKind::PublishAgentProfile,
                    RuntimeCommandErrorClass::Validation,
                    "about and version are required",
                );
            }

            let event_id = next_event_id("sa", KIND_AGENT_PROFILE, next_event_seq);
            snapshot.profile_event_id = Some(event_id.clone());
            snapshot.last_result = Some(format!("profile published for {display_name}"));
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SaLifecycle,
                RuntimeCommandKind::PublishAgentProfile,
                event_id,
            )
        }
        SaLifecycleCommand::PublishAgentState {
            encrypted_state_ref,
        } => {
            if encrypted_state_ref.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SaLifecycle,
                    RuntimeCommandKind::PublishAgentState,
                    RuntimeCommandErrorClass::Validation,
                    "encrypted_state_ref cannot be empty",
                );
            }

            let event_id = next_event_id("sa", KIND_AGENT_STATE, next_event_seq);
            snapshot.state_event_id = Some(event_id.clone());
            snapshot.last_result = Some("agent state published".to_string());
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SaLifecycle,
                RuntimeCommandKind::PublishAgentState,
                event_id,
            )
        }
        SaLifecycleCommand::ConfigureAgentSchedule { heartbeat_seconds } => {
            if heartbeat_seconds == 0 {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SaLifecycle,
                    RuntimeCommandKind::ConfigureAgentSchedule,
                    RuntimeCommandErrorClass::Validation,
                    "heartbeat_seconds must be greater than 0",
                );
            }

            let event_id = next_event_id("sa", KIND_AGENT_SCHEDULE, next_event_seq);
            snapshot.schedule_event_id = Some(event_id.clone());
            snapshot.heartbeat_seconds = heartbeat_seconds;
            snapshot.last_result = Some(format!("schedule set to {heartbeat_seconds}s heartbeat"));
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SaLifecycle,
                RuntimeCommandKind::ConfigureAgentSchedule,
                event_id,
            )
        }
        SaLifecycleCommand::PublishTickRequest { reason } => {
            if reason.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SaLifecycle,
                    RuntimeCommandKind::PublishTickRequest,
                    RuntimeCommandErrorClass::Validation,
                    "tick reason cannot be empty",
                );
            }
            if snapshot.mode != SaRunnerMode::Online {
                return RuntimeCommandResponse::retryable(
                    envelope.command_seq,
                    RuntimeLane::SaLifecycle,
                    RuntimeCommandKind::PublishTickRequest,
                    RuntimeCommandErrorClass::Dependency,
                    "runner must be online before tick requests can be published",
                );
            }

            let event_id = next_event_id("sa", KIND_TICK_REQUEST, next_event_seq);
            snapshot.queue_depth = snapshot.queue_depth.saturating_add(1);
            snapshot.last_tick_request_event_id = Some(event_id.clone());
            snapshot.last_result = Some("manual tick request published".to_string());
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SaLifecycle,
                RuntimeCommandKind::PublishTickRequest,
                event_id,
            )
        }
    }
}

fn run_skl_lane_loop(command_rx: Receiver<SequencedSklCommand>, update_tx: Sender<SklLaneUpdate>) {
    let mut snapshot = SklLaneSnapshot::default();
    let mut next_event_seq: u64 = 1;
    let mut provisional_since: Option<Instant> = None;

    loop {
        match command_rx.recv_timeout(SKL_LANE_POLL) {
            Ok(envelope) => {
                let mut snapshot_changed = false;
                let response = handle_skl_command(
                    &mut snapshot,
                    &mut next_event_seq,
                    &mut provisional_since,
                    envelope,
                    &mut snapshot_changed,
                );
                if snapshot_changed {
                    let _ = update_tx.send(SklLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                }
                let _ = update_tx.send(SklLaneUpdate::CommandResponse(response));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        if snapshot.trust_tier == SkillTrustTier::Provisional
            && provisional_since.is_some_and(|since| since.elapsed() >= SKL_TRUST_DELAY)
        {
            snapshot.trust_tier = SkillTrustTier::Trusted;
            snapshot.last_error = None;
            provisional_since = None;
            let _ = update_tx.send(SklLaneUpdate::Snapshot(Box::new(snapshot.clone())));
        }
    }
}

fn handle_skl_command(
    snapshot: &mut SklLaneSnapshot,
    next_event_seq: &mut u64,
    provisional_since: &mut Option<Instant>,
    envelope: SequencedSklCommand,
    snapshot_changed: &mut bool,
) -> RuntimeCommandResponse {
    match envelope.command {
        SklDiscoveryTrustCommand::PublishSkillManifest {
            skill_slug,
            version,
        } => {
            if skill_slug.trim().is_empty() || version.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SklDiscoveryTrust,
                    RuntimeCommandKind::PublishSkillManifest,
                    RuntimeCommandErrorClass::Validation,
                    "skill_slug and version are required",
                );
            }

            let event_id = next_event_id("skl", KIND_SKILL_MANIFEST, next_event_seq);
            snapshot.manifest_event_id = Some(event_id.clone());
            snapshot.manifest_a = Some(format!(
                "{}:npub1agent:{}:{}",
                KIND_SKILL_MANIFEST,
                skill_slug.trim(),
                version.trim()
            ));
            snapshot.trust_tier = SkillTrustTier::Provisional;
            snapshot.last_error = None;
            snapshot.kill_switch_active = false;
            *provisional_since = Some(Instant::now());
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SklDiscoveryTrust,
                RuntimeCommandKind::PublishSkillManifest,
                event_id,
            )
        }
        SklDiscoveryTrustCommand::PublishSkillVersionLog {
            skill_slug,
            version,
            summary,
        } => {
            if snapshot.manifest_event_id.is_none() {
                return RuntimeCommandResponse::retryable(
                    envelope.command_seq,
                    RuntimeLane::SklDiscoveryTrust,
                    RuntimeCommandKind::PublishSkillVersionLog,
                    RuntimeCommandErrorClass::Dependency,
                    "publish manifest before version log",
                );
            }
            if skill_slug.trim().is_empty()
                || version.trim().is_empty()
                || summary.trim().is_empty()
            {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SklDiscoveryTrust,
                    RuntimeCommandKind::PublishSkillVersionLog,
                    RuntimeCommandErrorClass::Validation,
                    "skill_slug, version, and summary are required",
                );
            }

            let event_id = next_event_id("skl", KIND_SKILL_VERSION_LOG, next_event_seq);
            snapshot.version_log_event_id = Some(event_id.clone());
            snapshot.last_error = None;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SklDiscoveryTrust,
                RuntimeCommandKind::PublishSkillVersionLog,
                event_id,
            )
        }
        SklDiscoveryTrustCommand::SubmitSkillSearch { query, limit } => {
            if query.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SklDiscoveryTrust,
                    RuntimeCommandKind::SubmitSkillSearch,
                    RuntimeCommandErrorClass::Validation,
                    "query cannot be empty",
                );
            }
            if limit == 0 {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::SklDiscoveryTrust,
                    RuntimeCommandKind::SubmitSkillSearch,
                    RuntimeCommandErrorClass::Validation,
                    "limit must be greater than 0",
                );
            }

            let request_event_id = next_event_id("skl", KIND_SKILL_SEARCH_REQUEST, next_event_seq);
            let result_event_id = next_event_id("skl", KIND_SKILL_SEARCH_RESULT, next_event_seq);
            snapshot.search_result_event_id = Some(result_event_id);
            snapshot.last_error = None;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::SklDiscoveryTrust,
                RuntimeCommandKind::SubmitSkillSearch,
                request_event_id,
            )
        }
    }
}

fn run_ac_lane_loop(command_rx: Receiver<SequencedAcCommand>, update_tx: Sender<AcLaneUpdate>) {
    let mut snapshot = AcLaneSnapshot::default();
    let mut next_event_seq: u64 = 1;

    loop {
        match command_rx.recv_timeout(AC_LANE_POLL) {
            Ok(envelope) => {
                let mut snapshot_changed = false;
                let response = handle_ac_command(
                    &mut snapshot,
                    &mut next_event_seq,
                    envelope,
                    &mut snapshot_changed,
                );
                if snapshot_changed {
                    let _ = update_tx.send(AcLaneUpdate::Snapshot(Box::new(snapshot.clone())));
                }
                let _ = update_tx.send(AcLaneUpdate::CommandResponse(response));
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn handle_ac_command(
    snapshot: &mut AcLaneSnapshot,
    next_event_seq: &mut u64,
    envelope: SequencedAcCommand,
    snapshot_changed: &mut bool,
) -> RuntimeCommandResponse {
    match envelope.command {
        AcCreditCommand::PublishCreditIntent {
            scope,
            request_type,
            payload,
            skill_scope_id,
            credit_envelope_ref,
            requested_sats,
            timeout_seconds,
        } => {
            if scope.trim().is_empty()
                || request_type.trim().is_empty()
                || payload.trim().is_empty()
                || requested_sats == 0
                || timeout_seconds == 0
            {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditIntent,
                    RuntimeCommandErrorClass::Validation,
                    "scope/request_type/payload are required and sats/timeout must be > 0",
                );
            }
            if scope.starts_with("skill:")
                && skill_scope_id
                    .as_deref()
                    .is_none_or(|skill_scope| skill_scope.trim().is_empty())
            {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditIntent,
                    RuntimeCommandErrorClass::Validation,
                    "skill scope requests require skill_scope_id",
                );
            }

            let event_id = next_event_id("ac", KIND_CREDIT_INTENT, next_event_seq);
            snapshot.intent_event_id = Some(event_id.clone());
            snapshot.available_credit_sats = requested_sats;
            snapshot.credit_available = true;
            if let Some(envelope_ref) = credit_envelope_ref {
                snapshot.envelope_event_id = Some(envelope_ref);
            }
            snapshot.last_error = None;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::AcCredit,
                RuntimeCommandKind::PublishCreditIntent,
                event_id,
            )
        }
        AcCreditCommand::PublishCreditOffer {
            intent_event_id,
            offered_sats,
        } => {
            if snapshot
                .intent_event_id
                .as_deref()
                .is_none_or(|intent| intent != intent_event_id.trim())
            {
                return RuntimeCommandResponse::retryable(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditOffer,
                    RuntimeCommandErrorClass::Dependency,
                    "intent_event_id does not match latest intent",
                );
            }
            if offered_sats == 0 {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditOffer,
                    RuntimeCommandErrorClass::Validation,
                    "offered_sats must be greater than 0",
                );
            }

            let event_id = next_event_id("ac", KIND_CREDIT_OFFER, next_event_seq);
            snapshot.offer_event_id = Some(event_id.clone());
            snapshot.available_credit_sats = offered_sats;
            snapshot.last_error = None;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::AcCredit,
                RuntimeCommandKind::PublishCreditOffer,
                event_id,
            )
        }
        AcCreditCommand::PublishCreditEnvelope {
            offer_event_id,
            cap_sats,
        } => {
            if snapshot
                .offer_event_id
                .as_deref()
                .is_none_or(|offer| offer != offer_event_id.trim())
            {
                return RuntimeCommandResponse::retryable(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditEnvelope,
                    RuntimeCommandErrorClass::Dependency,
                    "offer_event_id does not match latest offer",
                );
            }
            if cap_sats == 0 {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditEnvelope,
                    RuntimeCommandErrorClass::Validation,
                    "cap_sats must be greater than 0",
                );
            }

            let event_id = next_event_id("ac", KIND_CREDIT_ENVELOPE, next_event_seq);
            snapshot.envelope_event_id = Some(event_id.clone());
            snapshot.available_credit_sats = cap_sats;
            snapshot.credit_available = true;
            snapshot.last_error = None;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::AcCredit,
                RuntimeCommandKind::PublishCreditEnvelope,
                event_id,
            )
        }
        AcCreditCommand::PublishCreditSpendAuth {
            envelope_event_id,
            job_id,
            spend_sats,
        } => {
            if snapshot
                .envelope_event_id
                .as_deref()
                .is_none_or(|envelope_id| envelope_id != envelope_event_id.trim())
            {
                return RuntimeCommandResponse::retryable(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditSpendAuth,
                    RuntimeCommandErrorClass::Dependency,
                    "envelope_event_id does not match latest envelope",
                );
            }
            if job_id.trim().is_empty() || spend_sats == 0 {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditSpendAuth,
                    RuntimeCommandErrorClass::Validation,
                    "job_id and spend_sats are required",
                );
            }
            if spend_sats > snapshot.available_credit_sats {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditSpendAuth,
                    RuntimeCommandErrorClass::Validation,
                    "spend exceeds available credit",
                );
            }

            let event_id = next_event_id("ac", KIND_CREDIT_SPEND_AUTH, next_event_seq);
            snapshot.spend_auth_event_id = Some(event_id.clone());
            snapshot.available_credit_sats =
                snapshot.available_credit_sats.saturating_sub(spend_sats);
            snapshot.last_error = None;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::AcCredit,
                RuntimeCommandKind::PublishCreditSpendAuth,
                event_id,
            )
        }
        AcCreditCommand::PublishCreditSettlement {
            envelope_event_id,
            result_event_id,
            payment_pointer,
        } => {
            if snapshot
                .envelope_event_id
                .as_deref()
                .is_none_or(|envelope_id| envelope_id != envelope_event_id.trim())
            {
                return RuntimeCommandResponse::retryable(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditSettlement,
                    RuntimeCommandErrorClass::Dependency,
                    "envelope_event_id does not match latest envelope",
                );
            }
            if result_event_id.trim().is_empty() || payment_pointer.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditSettlement,
                    RuntimeCommandErrorClass::Validation,
                    "result_event_id and payment_pointer are required",
                );
            }

            let event_id = next_event_id("ac", KIND_CREDIT_SETTLEMENT, next_event_seq);
            snapshot.settlement_event_id = Some(event_id.clone());
            snapshot.credit_available = false;
            snapshot.last_error = None;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::AcCredit,
                RuntimeCommandKind::PublishCreditSettlement,
                event_id,
            )
        }
        AcCreditCommand::PublishCreditDefault {
            envelope_event_id,
            reason,
        } => {
            if snapshot
                .envelope_event_id
                .as_deref()
                .is_none_or(|envelope_id| envelope_id != envelope_event_id.trim())
            {
                return RuntimeCommandResponse::retryable(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditDefault,
                    RuntimeCommandErrorClass::Dependency,
                    "envelope_event_id does not match latest envelope",
                );
            }
            if reason.trim().is_empty() {
                return RuntimeCommandResponse::rejected(
                    envelope.command_seq,
                    RuntimeLane::AcCredit,
                    RuntimeCommandKind::PublishCreditDefault,
                    RuntimeCommandErrorClass::Validation,
                    "reason is required",
                );
            }

            let event_id = next_event_id("ac", KIND_CREDIT_DEFAULT_NOTICE, next_event_seq);
            snapshot.default_event_id = Some(event_id.clone());
            snapshot.last_error = Some(reason.trim().to_string());
            snapshot.credit_available = false;
            *snapshot_changed = true;
            RuntimeCommandResponse::accepted(
                envelope.command_seq,
                RuntimeLane::AcCredit,
                RuntimeCommandKind::PublishCreditDefault,
                event_id,
            )
        }
    }
}

fn next_event_id(prefix: &str, kind: u16, next_event_seq: &mut u64) -> String {
    let seq = *next_event_seq;
    *next_event_seq = next_event_seq.saturating_add(1);
    format!("{prefix}:{kind}:{seq:08x}")
}

fn next_runtime_event_id(prefix: &str, command_seq: u64) -> String {
    format!("{prefix}:runtime:{command_seq:08x}")
}

#[cfg(test)]
mod tests {
    use super::{
        AcCreditCommand, AcLaneSnapshot, RuntimeCommandErrorClass, RuntimeCommandKind,
        RuntimeCommandStatus, RuntimeLane, SaLaneSnapshot, SaLifecycleCommand, SaRunnerMode,
        SequencedAcCommand, SequencedSaCommand, SklDiscoveryTrustCommand, SklLaneSnapshot,
        handle_ac_command, handle_sa_command, handle_skl_command,
    };

    #[test]
    fn sa_profile_validation_errors_are_typed() {
        let mut snapshot = SaLaneSnapshot::default();
        let mut next_event_seq = 1;
        let mut snapshot_changed = false;
        let response = handle_sa_command(
            &mut snapshot,
            &mut next_event_seq,
            SequencedSaCommand {
                command_seq: 9,
                command: SaLifecycleCommand::PublishAgentProfile {
                    display_name: String::new(),
                    about: "about".to_string(),
                    version: "0.1.0".to_string(),
                },
            },
            &mut snapshot_changed,
        );

        assert_eq!(response.status, RuntimeCommandStatus::Rejected);
        assert_eq!(response.command, RuntimeCommandKind::PublishAgentProfile);
        assert_eq!(response.lane, RuntimeLane::SaLifecycle);
        assert_eq!(
            response.error.as_ref().map(|error| error.class),
            Some(RuntimeCommandErrorClass::Validation)
        );
        assert!(!snapshot_changed);
    }

    #[test]
    fn sa_runner_online_command_updates_snapshot() {
        let mut snapshot = SaLaneSnapshot::default();
        let mut next_event_seq = 1;
        let mut snapshot_changed = false;
        let response = handle_sa_command(
            &mut snapshot,
            &mut next_event_seq,
            SequencedSaCommand {
                command_seq: 11,
                command: SaLifecycleCommand::SetRunnerOnline { online: true },
            },
            &mut snapshot_changed,
        );

        assert_eq!(response.status, RuntimeCommandStatus::Accepted);
        assert_eq!(snapshot.mode, SaRunnerMode::Connecting);
        assert!(snapshot.connect_until.is_some());
        assert!(snapshot_changed);
    }

    #[test]
    fn skl_version_log_requires_manifest() {
        let mut snapshot = SklLaneSnapshot::default();
        let mut next_event_seq = 1;
        let mut provisional_since = None;
        let mut snapshot_changed = false;
        let response = handle_skl_command(
            &mut snapshot,
            &mut next_event_seq,
            &mut provisional_since,
            super::SequencedSklCommand {
                command_seq: 5,
                command: SklDiscoveryTrustCommand::PublishSkillVersionLog {
                    skill_slug: "summarize-text".to_string(),
                    version: "0.1.0".to_string(),
                    summary: "initial".to_string(),
                },
            },
            &mut snapshot_changed,
        );

        assert_eq!(response.status, RuntimeCommandStatus::Retryable);
        assert_eq!(
            response.error.as_ref().map(|error| error.class),
            Some(RuntimeCommandErrorClass::Dependency)
        );
        assert!(!snapshot_changed);
    }

    #[test]
    fn ac_spend_cannot_exceed_available_credit() {
        let mut snapshot = AcLaneSnapshot {
            credit_available: true,
            available_credit_sats: 100,
            envelope_event_id: Some("ac:39242:00000001".to_string()),
            ..AcLaneSnapshot::default()
        };
        let mut next_event_seq = 1;
        let mut snapshot_changed = false;
        let response = handle_ac_command(
            &mut snapshot,
            &mut next_event_seq,
            SequencedAcCommand {
                command_seq: 13,
                command: AcCreditCommand::PublishCreditSpendAuth {
                    envelope_event_id: "ac:39242:00000001".to_string(),
                    job_id: "job-1".to_string(),
                    spend_sats: 101,
                },
            },
            &mut snapshot_changed,
        );

        assert_eq!(response.status, RuntimeCommandStatus::Rejected);
        assert_eq!(response.command, RuntimeCommandKind::PublishCreditSpendAuth);
        assert_eq!(
            response.error.as_ref().map(|error| error.class),
            Some(RuntimeCommandErrorClass::Validation)
        );
        assert!(!snapshot_changed);
    }
}
