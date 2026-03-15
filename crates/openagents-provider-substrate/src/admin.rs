use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use rusqlite::{Connection, OptionalExtension, params};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::{
    ProviderAvailability, ProviderBackendHealth, ProviderBackendKind, ProviderFailureClass,
    ProviderInventoryRow, ProviderMode, ProviderSandboxAvailability, ProviderSandboxProfile,
    ProviderSandboxRuntimeHealth,
};

const PROVIDER_ADMIN_SCHEMA_VERSION: i64 = 1;
const PROVIDER_ADMIN_SNAPSHOT_ROW_ID: i64 = 1;
const PROVIDER_ADMIN_DEFAULT_LIST_LIMIT: usize = 32;
const PROVIDER_ADMIN_MAX_LIST_LIMIT: usize = 256;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderDesiredMode {
    #[default]
    Offline,
    Online,
    Paused,
}

impl ProviderDesiredMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Online => "online",
            Self::Paused => "paused",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderControlAction {
    Online,
    Offline,
    Pause,
    Resume,
}

impl ProviderControlAction {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Online => "online",
            Self::Offline => "offline",
            Self::Pause => "pause",
            Self::Resume => "resume",
        }
    }

    pub const fn target_desired_mode(self) -> ProviderDesiredMode {
        match self {
            Self::Online | Self::Resume => ProviderDesiredMode::Online,
            Self::Offline => ProviderDesiredMode::Offline,
            Self::Pause => ProviderDesiredMode::Paused,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderTransitionError {
    pub code: String,
    pub detail: String,
    pub action: ProviderControlAction,
    pub current_state: String,
    pub desired_mode: ProviderDesiredMode,
}

impl std::fmt::Display for ProviderTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for ProviderTransitionError {}

pub fn provider_runtime_state_label(status: &ProviderStatusResponse) -> String {
    status
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.runtime.authoritative_status.clone())
        .or_else(|| {
            status
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.runtime.mode.label().to_string())
        })
        .unwrap_or_else(|| "unconfigured".to_string())
}

pub fn validate_provider_control_action(
    status: &ProviderStatusResponse,
    action: ProviderControlAction,
) -> Result<ProviderDesiredMode, ProviderTransitionError> {
    let current_state = provider_runtime_state_label(status);
    match action {
        ProviderControlAction::Online | ProviderControlAction::Offline => {
            Ok(action.target_desired_mode())
        }
        ProviderControlAction::Pause => {
            if status.desired_mode == ProviderDesiredMode::Online
                || matches!(current_state.as_str(), "online" | "degraded" | "draining")
            {
                Ok(ProviderDesiredMode::Paused)
            } else {
                Err(ProviderTransitionError {
                    code: "provider_not_online".to_string(),
                    detail: "pause requires an online or degraded provider".to_string(),
                    action,
                    current_state,
                    desired_mode: action.target_desired_mode(),
                })
            }
        }
        ProviderControlAction::Resume => {
            if status.desired_mode == ProviderDesiredMode::Paused
                || current_state.as_str() == "paused"
            {
                Ok(ProviderDesiredMode::Online)
            } else {
                Err(ProviderTransitionError {
                    code: "provider_not_paused".to_string(),
                    detail: "resume requires a paused provider".to_string(),
                    action,
                    current_state,
                    desired_mode: action.target_desired_mode(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderJsonEntry {
    pub key: String,
    pub value: Value,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderIdentityMetadata {
    pub npub: Option<String>,
    pub public_key_hex: Option<String>,
    pub display_name: Option<String>,
    pub node_label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderRuntimeStatusSnapshot {
    pub mode: ProviderMode,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub degraded_reason_code: Option<String>,
    pub authoritative_status: Option<String>,
    pub authoritative_error_class: Option<ProviderFailureClass>,
    pub queue_depth: u32,
    pub online_uptime_seconds: u64,
    pub inventory_session_started_at_ms: Option<i64>,
    pub last_completed_job_at_epoch_ms: Option<i64>,
    pub last_authoritative_event_id: Option<String>,
    pub execution_backend_label: String,
    pub provider_blocker_codes: Vec<String>,
}

impl Default for ProviderRuntimeStatusSnapshot {
    fn default() -> Self {
        Self {
            mode: ProviderMode::Offline,
            last_action: None,
            last_error: None,
            degraded_reason_code: None,
            authoritative_status: None,
            authoritative_error_class: None,
            queue_depth: 0,
            online_uptime_seconds: 0,
            inventory_session_started_at_ms: None,
            last_completed_job_at_epoch_ms: None,
            last_authoritative_event_id: None,
            execution_backend_label: "no active inference backend".to_string(),
            provider_blocker_codes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderEarningsSummary {
    pub sats_today: u64,
    pub lifetime_sats: u64,
    pub jobs_today: u64,
    pub online_uptime_seconds: u64,
    pub last_job_result: String,
    pub first_job_latency_seconds: Option<u64>,
    pub completion_ratio_bps: Option<u16>,
    pub payout_success_ratio_bps: Option<u16>,
    pub avg_wallet_confirmation_latency_seconds: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderRecentJob {
    pub job_id: String,
    pub request_id: Option<String>,
    pub status: String,
    pub demand_source: String,
    pub product_id: Option<String>,
    pub compute_family: Option<String>,
    pub backend_family: Option<String>,
    pub sandbox_execution_class: Option<String>,
    pub sandbox_profile_id: Option<String>,
    pub sandbox_profile_digest: Option<String>,
    pub sandbox_termination_reason: Option<String>,
    pub completed_at_epoch_seconds: u64,
    pub payout_sats: u64,
    pub payment_pointer: String,
    pub failure_reason: Option<String>,
    pub delivery_proof_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderReceiptSummary {
    pub receipt_id: String,
    pub receipt_type: String,
    pub created_at_ms: i64,
    pub canonical_hash: String,
    pub compute_family: Option<String>,
    pub backend_family: Option<String>,
    pub sandbox_execution_class: Option<String>,
    pub sandbox_profile_id: Option<String>,
    pub sandbox_profile_digest: Option<String>,
    pub sandbox_termination_reason: Option<String>,
    pub reason_code: Option<String>,
    pub failure_reason: Option<String>,
    pub severity: Option<String>,
    pub notional_sats: Option<u64>,
    pub liability_premium_sats: Option<u64>,
    pub work_unit_id: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderPayoutSummary {
    pub payout_id: String,
    pub amount_sats: u64,
    pub direction: String,
    pub status: String,
    pub created_at_epoch_seconds: u64,
    pub payment_pointer: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderHealthEvent {
    pub event_id: String,
    pub occurred_at_ms: i64,
    pub severity: String,
    pub code: String,
    pub detail: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderPersistedSnapshot {
    pub captured_at_ms: i64,
    pub config_metadata: Vec<ProviderJsonEntry>,
    pub identity: Option<ProviderIdentityMetadata>,
    pub runtime: ProviderRuntimeStatusSnapshot,
    pub availability: ProviderAvailability,
    pub inventory_rows: Vec<ProviderInventoryRow>,
    pub recent_jobs: Vec<ProviderRecentJob>,
    pub receipts: Vec<ProviderReceiptSummary>,
    pub payouts: Vec<ProviderPayoutSummary>,
    pub health_events: Vec<ProviderHealthEvent>,
    pub earnings: Option<ProviderEarningsSummary>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProviderSnapshotParts {
    pub captured_at_ms: i64,
    pub config_metadata: Vec<ProviderJsonEntry>,
    pub identity: Option<ProviderIdentityMetadata>,
    pub runtime: ProviderRuntimeStatusSnapshot,
    pub availability: ProviderAvailability,
    pub inventory_rows: Vec<ProviderInventoryRow>,
    pub recent_jobs: Vec<ProviderRecentJob>,
    pub receipts: Vec<ProviderReceiptSummary>,
    pub payouts: Vec<ProviderPayoutSummary>,
    pub health_events: Vec<ProviderHealthEvent>,
    pub earnings: Option<ProviderEarningsSummary>,
}

pub fn assemble_provider_persisted_snapshot(
    parts: ProviderSnapshotParts,
) -> ProviderPersistedSnapshot {
    ProviderPersistedSnapshot {
        captured_at_ms: parts.captured_at_ms,
        config_metadata: parts.config_metadata,
        identity: parts.identity,
        runtime: parts.runtime,
        availability: parts.availability,
        inventory_rows: parts.inventory_rows,
        recent_jobs: parts.recent_jobs,
        receipts: parts.receipts,
        payouts: parts.payouts,
        health_events: parts.health_events,
        earnings: parts.earnings,
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderStatusResponse {
    pub listen_addr: Option<String>,
    pub desired_mode: ProviderDesiredMode,
    pub snapshot: Option<ProviderPersistedSnapshot>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderAdminRuntimeConfig {
    pub listen_addr: SocketAddr,
    pub list_limit: usize,
}

impl ProviderAdminRuntimeConfig {
    pub fn with_list_limit(self, list_limit: usize) -> Self {
        Self { list_limit, ..self }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAdminStoreConfig {
    pub db_path: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderAdminConfig {
    pub store: ProviderAdminStoreConfig,
    pub runtime: ProviderAdminRuntimeConfig,
}

impl ProviderAdminConfig {
    pub fn new(db_path: PathBuf, listen_addr: SocketAddr) -> Self {
        Self {
            store: ProviderAdminStoreConfig { db_path },
            runtime: ProviderAdminRuntimeConfig {
                listen_addr,
                list_limit: PROVIDER_ADMIN_DEFAULT_LIST_LIMIT,
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderControlEvent {
    pub action: ProviderControlAction,
    pub desired_mode: ProviderDesiredMode,
    pub issued_at_ms: i64,
    pub source: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderAdminUpdate {
    ControlEvent(ProviderControlEvent),
    WorkerError(String),
}

enum ProviderAdminCommand {
    SyncSnapshot(Box<ProviderPersistedSnapshot>),
    SetDesiredMode(ProviderDesiredMode),
    Shutdown,
}

pub struct ProviderAdminRuntime {
    command_tx: Sender<ProviderAdminCommand>,
    update_rx: Receiver<ProviderAdminUpdate>,
    listen_addr: SocketAddr,
    db_path: PathBuf,
    join_handle: Option<JoinHandle<()>>,
}

impl ProviderAdminRuntime {
    pub fn spawn(config: ProviderAdminConfig) -> Result<Self, String> {
        let (command_tx, command_rx) = mpsc::channel::<ProviderAdminCommand>();
        let (update_tx, update_rx) = mpsc::channel::<ProviderAdminUpdate>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<SocketAddr, String>>();
        let db_path = config.store.db_path.clone();
        let join_handle = std::thread::spawn(move || {
            run_provider_admin_loop(command_rx, update_tx, ready_tx, config);
        });
        let listen_addr = ready_rx.recv().map_err(|error| {
            format!("Provider admin runtime failed to report readiness: {error}")
        })??;
        Ok(Self {
            command_tx,
            update_rx,
            listen_addr,
            db_path,
            join_handle: Some(join_handle),
        })
    }

    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
    }

    pub fn sync_snapshot(&self, snapshot: ProviderPersistedSnapshot) -> Result<(), String> {
        self.command_tx
            .send(ProviderAdminCommand::SyncSnapshot(Box::new(snapshot)))
            .map_err(|error| format!("Provider admin runtime offline: {error}"))
    }

    pub fn set_desired_mode(&self, desired_mode: ProviderDesiredMode) -> Result<(), String> {
        self.command_tx
            .send(ProviderAdminCommand::SetDesiredMode(desired_mode))
            .map_err(|error| format!("Provider admin runtime offline: {error}"))
    }

    pub fn drain_updates(&mut self) -> Vec<ProviderAdminUpdate> {
        let mut updates = Vec::new();
        while let Ok(update) = self.update_rx.try_recv() {
            updates.push(update);
        }
        updates
    }

    pub fn shutdown_async(&mut self) {
        let _ = self.command_tx.send(ProviderAdminCommand::Shutdown);
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ProviderAdminRuntime {
    fn drop(&mut self) {
        self.shutdown_async();
    }
}

pub struct ProviderPersistenceStore {
    connection: Connection,
    list_limit: usize,
}

impl ProviderPersistenceStore {
    pub fn open(config: &ProviderAdminConfig) -> Result<Self, String> {
        let db_path = config.store.db_path.as_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create provider admin db dir: {error}"))?;
        }
        let connection = Connection::open(db_path)
            .map_err(|error| format!("Failed to open provider admin sqlite db: {error}"))?;
        let mut store = Self {
            connection,
            list_limit: config
                .runtime
                .list_limit
                .clamp(1, PROVIDER_ADMIN_MAX_LIST_LIMIT),
        };
        store.apply_migrations()?;
        Ok(store)
    }

    pub fn set_listen_addr(&mut self, listen_addr: &str) -> Result<(), String> {
        self.upsert_metadata_json("listen_addr", &Value::String(listen_addr.to_string()))
    }

    pub fn set_desired_mode(&mut self, desired_mode: ProviderDesiredMode) -> Result<(), String> {
        self.upsert_metadata_json("desired_mode", &json!(desired_mode))
    }

    pub fn desired_mode(&self) -> Result<ProviderDesiredMode, String> {
        self.metadata_json::<ProviderDesiredMode>("desired_mode")
            .map(|mode| mode.unwrap_or_default())
    }

    pub fn persist_snapshot(&mut self, snapshot: &ProviderPersistedSnapshot) -> Result<(), String> {
        let tx = self.connection.transaction().map_err(|error| {
            format!("Failed to start provider admin sqlite transaction: {error}")
        })?;

        tx.execute(
            "INSERT OR REPLACE INTO provider_runtime_snapshot (row_id, captured_at_ms, value_json) VALUES (?1, ?2, ?3)",
            params![
                PROVIDER_ADMIN_SNAPSHOT_ROW_ID,
                snapshot.captured_at_ms,
                encode_json(snapshot)?
            ],
        )
        .map_err(|error| format!("Failed to upsert provider runtime snapshot: {error}"))?;

        replace_config_rows(&tx, snapshot.config_metadata.as_slice())?;
        replace_singleton_json(
            &tx,
            "provider_identity_snapshot",
            snapshot.identity.as_ref(),
        )?;
        replace_singleton_json(&tx, "provider_runtime_status", Some(&snapshot.runtime))?;
        replace_backend_rows(&tx, &snapshot.availability, snapshot.captured_at_ms)?;
        replace_sandbox_rows(&tx, &snapshot.availability.sandbox, snapshot.captured_at_ms)?;
        replace_inventory_rows(
            &tx,
            snapshot.inventory_rows.as_slice(),
            snapshot.captured_at_ms,
        )?;
        replace_recent_jobs(&tx, snapshot.recent_jobs.as_slice())?;
        replace_receipts(&tx, snapshot.receipts.as_slice())?;
        replace_payouts(&tx, snapshot.payouts.as_slice())?;
        replace_health_events(&tx, snapshot.health_events.as_slice())?;
        replace_singleton_json(&tx, "provider_earnings_summary", snapshot.earnings.as_ref())?;

        tx.commit().map_err(|error| {
            format!("Failed to commit provider admin sqlite transaction: {error}")
        })?;
        Ok(())
    }

    pub fn load_status(&self) -> Result<ProviderStatusResponse, String> {
        let listen_addr = self.metadata_json::<String>("listen_addr")?;
        let desired_mode = self.desired_mode()?;
        let snapshot = self
            .connection
            .query_row(
                "SELECT value_json FROM provider_runtime_snapshot WHERE row_id = ?1",
                params![PROVIDER_ADMIN_SNAPSHOT_ROW_ID],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("Failed to load provider runtime snapshot: {error}"))?
            .map(|payload| decode_json::<ProviderPersistedSnapshot>(&payload))
            .transpose()?;
        Ok(ProviderStatusResponse {
            listen_addr,
            desired_mode,
            snapshot,
        })
    }

    pub fn load_recent_jobs(&self, limit: Option<usize>) -> Result<Vec<ProviderRecentJob>, String> {
        let limit = bounded_limit(limit, self.list_limit);
        load_json_rows(
            &self.connection,
            "SELECT value_json FROM provider_recent_jobs ORDER BY completed_at_epoch_seconds DESC, job_id ASC LIMIT ?1",
            limit,
        )
    }

    pub fn load_receipts(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ProviderReceiptSummary>, String> {
        let limit = bounded_limit(limit, self.list_limit);
        load_json_rows(
            &self.connection,
            "SELECT value_json FROM provider_receipts ORDER BY created_at_ms DESC, receipt_id ASC LIMIT ?1",
            limit,
        )
    }

    pub fn load_payouts(&self, limit: Option<usize>) -> Result<Vec<ProviderPayoutSummary>, String> {
        let limit = bounded_limit(limit, self.list_limit);
        load_json_rows(
            &self.connection,
            "SELECT value_json FROM provider_payouts ORDER BY created_at_epoch_seconds DESC, payout_id ASC LIMIT ?1",
            limit,
        )
    }

    pub fn load_health_events(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ProviderHealthEvent>, String> {
        let limit = bounded_limit(limit, self.list_limit);
        load_json_rows(
            &self.connection,
            "SELECT value_json FROM provider_health_events ORDER BY occurred_at_ms DESC, event_id ASC LIMIT ?1",
            limit,
        )
    }

    pub fn load_availability(&self) -> Result<ProviderAvailability, String> {
        let mut availability = ProviderAvailability::default();
        let mut statement = self
            .connection
            .prepare("SELECT backend_kind, value_json FROM provider_backend_health")
            .map_err(|error| format!("Failed to prepare backend health query: {error}"))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| format!("Failed to query backend health rows: {error}"))?;
        for row in rows {
            let (backend_kind, value_json) =
                row.map_err(|error| format!("Failed to decode backend health row: {error}"))?;
            let health = decode_json::<ProviderBackendHealth>(&value_json)?;
            match backend_kind.as_str() {
                "gpt_oss" => availability.gpt_oss = health,
                "apple_foundation_models" => availability.apple_foundation_models = health,
                _ => {}
            }
        }
        availability.sandbox = ProviderSandboxAvailability {
            runtimes: self.load_sandbox_runtimes(None)?,
            profiles: self.load_sandbox_profiles(None)?,
            last_scan_error: None,
        };
        Ok(availability)
    }

    pub fn load_inventory_rows(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ProviderInventoryRow>, String> {
        let limit = bounded_limit(limit, self.list_limit);
        load_json_rows(
            &self.connection,
            "SELECT value_json FROM provider_inventory_rows ORDER BY product_id ASC LIMIT ?1",
            limit,
        )
    }

    pub fn load_sandbox_runtimes(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ProviderSandboxRuntimeHealth>, String> {
        let limit = bounded_limit(limit, self.list_limit);
        load_json_rows(
            &self.connection,
            "SELECT value_json FROM provider_sandbox_runtime_health ORDER BY runtime_kind ASC LIMIT ?1",
            limit,
        )
    }

    pub fn load_sandbox_profiles(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<ProviderSandboxProfile>, String> {
        let limit = bounded_limit(limit, self.list_limit);
        load_json_rows(
            &self.connection,
            "SELECT value_json FROM provider_sandbox_profiles ORDER BY execution_class ASC, profile_id ASC LIMIT ?1",
            limit,
        )
    }

    fn apply_migrations(&mut self) -> Result<(), String> {
        self.connection
            .execute_batch(
                "
                PRAGMA journal_mode = WAL;
                PRAGMA foreign_keys = ON;
                CREATE TABLE IF NOT EXISTS provider_admin_metadata (
                    key TEXT PRIMARY KEY,
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_runtime_snapshot (
                    row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
                    captured_at_ms INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_config_metadata (
                    key TEXT PRIMARY KEY,
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_identity_snapshot (
                    row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_runtime_status (
                    row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_backend_health (
                    backend_kind TEXT PRIMARY KEY,
                    captured_at_ms INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_sandbox_runtime_health (
                    runtime_kind TEXT PRIMARY KEY,
                    captured_at_ms INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_sandbox_profiles (
                    profile_id TEXT PRIMARY KEY,
                    execution_class TEXT NOT NULL,
                    profile_digest TEXT NOT NULL,
                    captured_at_ms INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS provider_sandbox_profiles_class_idx
                    ON provider_sandbox_profiles (execution_class ASC, profile_id ASC);
                CREATE TABLE IF NOT EXISTS provider_inventory_rows (
                    product_id TEXT PRIMARY KEY,
                    captured_at_ms INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS provider_recent_jobs (
                    job_id TEXT PRIMARY KEY,
                    completed_at_epoch_seconds INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS provider_recent_jobs_completed_idx
                    ON provider_recent_jobs (completed_at_epoch_seconds DESC, job_id ASC);
                CREATE TABLE IF NOT EXISTS provider_receipts (
                    receipt_id TEXT PRIMARY KEY,
                    created_at_ms INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS provider_receipts_created_idx
                    ON provider_receipts (created_at_ms DESC, receipt_id ASC);
                CREATE TABLE IF NOT EXISTS provider_payouts (
                    payout_id TEXT PRIMARY KEY,
                    created_at_epoch_seconds INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS provider_payouts_created_idx
                    ON provider_payouts (created_at_epoch_seconds DESC, payout_id ASC);
                CREATE TABLE IF NOT EXISTS provider_health_events (
                    event_id TEXT PRIMARY KEY,
                    occurred_at_ms INTEGER NOT NULL,
                    value_json TEXT NOT NULL
                );
                CREATE INDEX IF NOT EXISTS provider_health_events_occurred_idx
                    ON provider_health_events (occurred_at_ms DESC, event_id ASC);
                CREATE TABLE IF NOT EXISTS provider_earnings_summary (
                    row_id INTEGER PRIMARY KEY CHECK (row_id = 1),
                    value_json TEXT NOT NULL
                );
                ",
            )
            .map_err(|error| {
                format!("Failed to apply provider admin sqlite migrations: {error}")
            })?;
        self.connection
            .pragma_update(None, "user_version", PROVIDER_ADMIN_SCHEMA_VERSION)
            .map_err(|error| {
                format!("Failed to set provider admin sqlite schema version: {error}")
            })?;
        Ok(())
    }

    fn upsert_metadata_json(&mut self, key: &str, value: &Value) -> Result<(), String> {
        self.connection
            .execute(
                "INSERT OR REPLACE INTO provider_admin_metadata (key, value_json) VALUES (?1, ?2)",
                params![key, encode_json(value)?],
            )
            .map_err(|error| format!("Failed to upsert provider admin metadata {key}: {error}"))?;
        Ok(())
    }

    fn metadata_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, String> {
        let payload = self
            .connection
            .query_row(
                "SELECT value_json FROM provider_admin_metadata WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|error| format!("Failed to query provider admin metadata {key}: {error}"))?;
        payload.map(|raw| decode_json::<T>(&raw)).transpose()
    }
}

struct ProviderAdminSharedState {
    store: Mutex<ProviderPersistenceStore>,
    update_tx: Sender<ProviderAdminUpdate>,
}

fn run_provider_admin_loop(
    command_rx: Receiver<ProviderAdminCommand>,
    update_tx: Sender<ProviderAdminUpdate>,
    ready_tx: Sender<Result<SocketAddr, String>>,
    config: ProviderAdminConfig,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            let message = format!("Failed to initialize provider admin runtime: {error}");
            let _ = ready_tx.send(Err(message.clone()));
            let _ = update_tx.send(ProviderAdminUpdate::WorkerError(message));
            return;
        }
    };

    let store = match ProviderPersistenceStore::open(&config) {
        Ok(store) => store,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            let _ = update_tx.send(ProviderAdminUpdate::WorkerError(error));
            return;
        }
    };

    let shared = Arc::new(ProviderAdminSharedState {
        store: Mutex::new(store),
        update_tx: update_tx.clone(),
    });

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let listen_addr_result = runtime.block_on(start_provider_admin_server(
        Arc::clone(&shared),
        config.runtime.listen_addr,
        shutdown_rx,
    ));
    let listen_addr = match listen_addr_result {
        Ok(listen_addr) => listen_addr,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            let _ = update_tx.send(ProviderAdminUpdate::WorkerError(error));
            return;
        }
    };

    match shared.store.lock() {
        Ok(mut locked_store) => {
            if let Err(error) = locked_store.set_listen_addr(listen_addr.to_string().as_str()) {
                let _ = update_tx.send(ProviderAdminUpdate::WorkerError(error));
            }
        }
        Err(error) => {
            let _ = update_tx.send(ProviderAdminUpdate::WorkerError(format!(
                "Provider admin sqlite store lock poisoned during server bootstrap: {error}"
            )));
        }
    }
    let _ = ready_tx.send(Ok(listen_addr));

    while let Ok(command) = command_rx.recv() {
        match command {
            ProviderAdminCommand::SyncSnapshot(snapshot) => {
                if let Err(error) =
                    with_locked_store(&shared, |store| store.persist_snapshot(&snapshot))
                {
                    let _ = update_tx.send(ProviderAdminUpdate::WorkerError(error));
                }
            }
            ProviderAdminCommand::SetDesiredMode(desired_mode) => {
                if let Err(error) =
                    with_locked_store(&shared, |store| store.set_desired_mode(desired_mode))
                {
                    let _ = update_tx.send(ProviderAdminUpdate::WorkerError(error));
                }
            }
            ProviderAdminCommand::Shutdown => {
                let _ = shutdown_tx.send(true);
                break;
            }
        }
    }
}

async fn start_provider_admin_server(
    shared: Arc<ProviderAdminSharedState>,
    listen_addr: SocketAddr,
    shutdown_rx: watch::Receiver<bool>,
) -> Result<SocketAddr, String> {
    let listener = tokio::net::TcpListener::bind(listen_addr)
        .await
        .map_err(|error| format!("Failed to bind provider admin HTTP listener: {error}"))?;
    let actual_addr = listener
        .local_addr()
        .map_err(|error| format!("Failed to read provider admin listener address: {error}"))?;
    let router = build_provider_admin_router(shared);
    tokio::spawn(async move {
        let _ = axum::serve(listener, router)
            .with_graceful_shutdown(wait_for_shutdown(shutdown_rx))
            .await;
    });
    Ok(actual_addr)
}

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    while !*shutdown_rx.borrow_and_update() {
        if shutdown_rx.changed().await.is_err() {
            break;
        }
    }
}

fn build_provider_admin_router(shared: Arc<ProviderAdminSharedState>) -> Router {
    Router::new()
        .route("/v1/status", get(get_status))
        .route("/v1/backend-health", get(get_backend_health))
        .route("/v1/sandbox/runtimes", get(get_sandbox_runtimes))
        .route("/v1/sandbox/profiles", get(get_sandbox_profiles))
        .route("/v1/inventory", get(get_inventory))
        .route("/v1/jobs", get(get_jobs))
        .route("/v1/earnings", get(get_earnings))
        .route("/v1/receipts", get(get_receipts))
        .route("/v1/payouts", get(get_payouts))
        .route("/v1/health", get(get_health_events))
        .route("/v1/online", post(post_online))
        .route("/v1/offline", post(post_offline))
        .route("/v1/pause", post(post_pause))
        .route("/v1/resume", post(post_resume))
        .with_state(shared)
}

#[derive(Clone, Debug, Default, Deserialize)]
struct LimitQuery {
    limit: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
struct ProviderApiError {
    code: String,
    error: String,
    current_state: Option<String>,
    desired_mode: Option<ProviderDesiredMode>,
}

type ProviderApiResult<T> = Result<Json<T>, (StatusCode, Json<ProviderApiError>)>;

async fn get_status(
    State(shared): State<Arc<ProviderAdminSharedState>>,
) -> ProviderApiResult<ProviderStatusResponse> {
    load_from_store(&shared, ProviderPersistenceStore::load_status).map(Json)
}

async fn get_backend_health(
    State(shared): State<Arc<ProviderAdminSharedState>>,
) -> ProviderApiResult<ProviderAvailability> {
    load_from_store(&shared, ProviderPersistenceStore::load_availability).map(Json)
}

async fn get_sandbox_runtimes(
    State(shared): State<Arc<ProviderAdminSharedState>>,
    Query(query): Query<LimitQuery>,
) -> ProviderApiResult<Vec<ProviderSandboxRuntimeHealth>> {
    load_from_store(&shared, |store| store.load_sandbox_runtimes(query.limit)).map(Json)
}

async fn get_sandbox_profiles(
    State(shared): State<Arc<ProviderAdminSharedState>>,
    Query(query): Query<LimitQuery>,
) -> ProviderApiResult<Vec<ProviderSandboxProfile>> {
    load_from_store(&shared, |store| store.load_sandbox_profiles(query.limit)).map(Json)
}

async fn get_inventory(
    State(shared): State<Arc<ProviderAdminSharedState>>,
    Query(query): Query<LimitQuery>,
) -> ProviderApiResult<Vec<ProviderInventoryRow>> {
    load_from_store(&shared, |store| store.load_inventory_rows(query.limit)).map(Json)
}

async fn get_jobs(
    State(shared): State<Arc<ProviderAdminSharedState>>,
    Query(query): Query<LimitQuery>,
) -> ProviderApiResult<Vec<ProviderRecentJob>> {
    load_from_store(&shared, |store| store.load_recent_jobs(query.limit)).map(Json)
}

async fn get_earnings(
    State(shared): State<Arc<ProviderAdminSharedState>>,
) -> ProviderApiResult<Option<ProviderEarningsSummary>> {
    load_from_store(&shared, |store| {
        Ok(store
            .load_status()?
            .snapshot
            .and_then(|snapshot| snapshot.earnings))
    })
    .map(Json)
}

async fn get_receipts(
    State(shared): State<Arc<ProviderAdminSharedState>>,
    Query(query): Query<LimitQuery>,
) -> ProviderApiResult<Vec<ProviderReceiptSummary>> {
    load_from_store(&shared, |store| store.load_receipts(query.limit)).map(Json)
}

async fn get_payouts(
    State(shared): State<Arc<ProviderAdminSharedState>>,
    Query(query): Query<LimitQuery>,
) -> ProviderApiResult<Vec<ProviderPayoutSummary>> {
    load_from_store(&shared, |store| store.load_payouts(query.limit)).map(Json)
}

async fn get_health_events(
    State(shared): State<Arc<ProviderAdminSharedState>>,
    Query(query): Query<LimitQuery>,
) -> ProviderApiResult<Vec<ProviderHealthEvent>> {
    load_from_store(&shared, |store| store.load_health_events(query.limit)).map(Json)
}

async fn post_online(
    State(shared): State<Arc<ProviderAdminSharedState>>,
) -> ProviderApiResult<ProviderStatusResponse> {
    apply_control_action(&shared, ProviderControlAction::Online)
}

async fn post_offline(
    State(shared): State<Arc<ProviderAdminSharedState>>,
) -> ProviderApiResult<ProviderStatusResponse> {
    apply_control_action(&shared, ProviderControlAction::Offline)
}

async fn post_pause(
    State(shared): State<Arc<ProviderAdminSharedState>>,
) -> ProviderApiResult<ProviderStatusResponse> {
    apply_control_action(&shared, ProviderControlAction::Pause)
}

async fn post_resume(
    State(shared): State<Arc<ProviderAdminSharedState>>,
) -> ProviderApiResult<ProviderStatusResponse> {
    apply_control_action(&shared, ProviderControlAction::Resume)
}

fn apply_control_action(
    shared: &Arc<ProviderAdminSharedState>,
    action: ProviderControlAction,
) -> ProviderApiResult<ProviderStatusResponse> {
    let status = with_locked_store(shared, |store| store.load_status()).map_err(api_error)?;
    let desired_mode =
        validate_provider_control_action(&status, action).map_err(transition_api_error)?;
    with_locked_store(shared, |store| store.set_desired_mode(desired_mode)).map_err(api_error)?;
    shared
        .update_tx
        .send(ProviderAdminUpdate::ControlEvent(ProviderControlEvent {
            action,
            desired_mode,
            issued_at_ms: now_epoch_ms(),
            source: "localhost_http".to_string(),
        }))
        .map_err(|error| {
            api_error(format!(
                "Failed to enqueue provider admin control event {}: {error}",
                action.label()
            ))
        })?;
    load_from_store(shared, ProviderPersistenceStore::load_status).map(Json)
}

fn with_locked_store<T>(
    shared: &ProviderAdminSharedState,
    operation: impl FnOnce(&mut ProviderPersistenceStore) -> Result<T, String>,
) -> Result<T, String> {
    let mut store = shared
        .store
        .lock()
        .map_err(|error| format!("Provider admin sqlite store lock poisoned: {error}"))?;
    operation(&mut store)
}

fn load_from_store<T>(
    shared: &ProviderAdminSharedState,
    operation: impl FnOnce(&ProviderPersistenceStore) -> Result<T, String>,
) -> Result<T, (StatusCode, Json<ProviderApiError>)> {
    let store = shared.store.lock().map_err(|error| {
        api_error(format!(
            "Provider admin sqlite store lock poisoned: {error}"
        ))
    })?;
    operation(&store).map_err(api_error)
}

fn replace_config_rows(
    tx: &rusqlite::Transaction<'_>,
    entries: &[ProviderJsonEntry],
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_config_metadata", [])
        .map_err(|error| format!("Failed to clear provider config metadata rows: {error}"))?;
    for entry in entries {
        tx.execute(
            "INSERT INTO provider_config_metadata (key, value_json) VALUES (?1, ?2)",
            params![entry.key, encode_json(&entry.value)?],
        )
        .map_err(|error| {
            format!(
                "Failed to insert provider config metadata row {}: {error}",
                entry.key
            )
        })?;
    }
    Ok(())
}

fn replace_singleton_json<T: Serialize>(
    tx: &rusqlite::Transaction<'_>,
    table_name: &str,
    value: Option<&T>,
) -> Result<(), String> {
    let delete_sql = format!("DELETE FROM {table_name}");
    tx.execute(delete_sql.as_str(), [])
        .map_err(|error| format!("Failed to clear singleton table {table_name}: {error}"))?;
    if let Some(value) = value {
        let insert_sql = format!("INSERT INTO {table_name} (row_id, value_json) VALUES (?1, ?2)");
        tx.execute(
            insert_sql.as_str(),
            params![PROVIDER_ADMIN_SNAPSHOT_ROW_ID, encode_json(value)?],
        )
        .map_err(|error| format!("Failed to insert singleton table {table_name}: {error}"))?;
    }
    Ok(())
}

fn replace_backend_rows(
    tx: &rusqlite::Transaction<'_>,
    availability: &ProviderAvailability,
    captured_at_ms: i64,
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_backend_health", [])
        .map_err(|error| format!("Failed to clear provider backend health rows: {error}"))?;
    insert_backend_row(
        tx,
        ProviderBackendKind::GptOss,
        &availability.gpt_oss,
        captured_at_ms,
    )?;
    insert_backend_row(
        tx,
        ProviderBackendKind::AppleFoundationModels,
        &availability.apple_foundation_models,
        captured_at_ms,
    )?;
    Ok(())
}

fn replace_sandbox_rows(
    tx: &rusqlite::Transaction<'_>,
    sandbox: &ProviderSandboxAvailability,
    captured_at_ms: i64,
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_sandbox_runtime_health", [])
        .map_err(|error| format!("Failed to clear provider sandbox runtime rows: {error}"))?;
    tx.execute("DELETE FROM provider_sandbox_profiles", [])
        .map_err(|error| format!("Failed to clear provider sandbox profile rows: {error}"))?;

    for runtime in &sandbox.runtimes {
        tx.execute(
            "INSERT INTO provider_sandbox_runtime_health (runtime_kind, captured_at_ms, value_json) VALUES (?1, ?2, ?3)",
            params![
                format!("{:?}", runtime.runtime_kind).to_lowercase(),
                captured_at_ms,
                encode_json(runtime)?
            ],
        )
        .map_err(|error| {
            format!(
                "Failed to insert provider sandbox runtime row {:?}: {error}",
                runtime.runtime_kind
            )
        })?;
    }

    for profile in &sandbox.profiles {
        tx.execute(
            "INSERT INTO provider_sandbox_profiles (profile_id, execution_class, profile_digest, captured_at_ms, value_json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                profile.profile_id,
                format!("{:?}", profile.execution_class).to_lowercase(),
                profile.profile_digest,
                captured_at_ms,
                encode_json(profile)?
            ],
        )
        .map_err(|error| {
            format!(
                "Failed to insert provider sandbox profile row {}: {error}",
                profile.profile_id
            )
        })?;
    }
    Ok(())
}

fn insert_backend_row(
    tx: &rusqlite::Transaction<'_>,
    backend_kind: ProviderBackendKind,
    health: &ProviderBackendHealth,
    captured_at_ms: i64,
) -> Result<(), String> {
    tx.execute(
        "INSERT INTO provider_backend_health (backend_kind, captured_at_ms, value_json) VALUES (?1, ?2, ?3)",
        params![backend_storage_key(backend_kind), captured_at_ms, encode_json(health)?],
    )
    .map_err(|error| {
        format!(
            "Failed to insert provider backend health row {}: {error}",
            backend_storage_key(backend_kind)
        )
    })?;
    Ok(())
}

fn replace_inventory_rows(
    tx: &rusqlite::Transaction<'_>,
    rows: &[ProviderInventoryRow],
    captured_at_ms: i64,
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_inventory_rows", [])
        .map_err(|error| format!("Failed to clear provider inventory rows: {error}"))?;
    for row in rows {
        tx.execute(
            "INSERT INTO provider_inventory_rows (product_id, captured_at_ms, value_json) VALUES (?1, ?2, ?3)",
            params![row.target.product_id(), captured_at_ms, encode_json(row)?],
        )
        .map_err(|error| {
            format!(
                "Failed to insert provider inventory row {}: {error}",
                row.target.product_id()
            )
        })?;
    }
    Ok(())
}

fn replace_recent_jobs(
    tx: &rusqlite::Transaction<'_>,
    jobs: &[ProviderRecentJob],
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_recent_jobs", [])
        .map_err(|error| format!("Failed to clear provider recent jobs: {error}"))?;
    for job in jobs {
        tx.execute(
            "INSERT INTO provider_recent_jobs (job_id, completed_at_epoch_seconds, value_json) VALUES (?1, ?2, ?3)",
            params![job.job_id, i64::try_from(job.completed_at_epoch_seconds).unwrap_or(i64::MAX), encode_json(job)?],
        )
        .map_err(|error| format!("Failed to insert provider recent job {}: {error}", job.job_id))?;
    }
    Ok(())
}

fn replace_receipts(
    tx: &rusqlite::Transaction<'_>,
    receipts: &[ProviderReceiptSummary],
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_receipts", [])
        .map_err(|error| format!("Failed to clear provider receipt rows: {error}"))?;
    for receipt in receipts {
        tx.execute(
            "INSERT INTO provider_receipts (receipt_id, created_at_ms, value_json) VALUES (?1, ?2, ?3)",
            params![receipt.receipt_id, receipt.created_at_ms, encode_json(receipt)?],
        )
        .map_err(|error| {
            format!(
                "Failed to insert provider receipt row {}: {error}",
                receipt.receipt_id
            )
        })?;
    }
    Ok(())
}

fn replace_payouts(
    tx: &rusqlite::Transaction<'_>,
    payouts: &[ProviderPayoutSummary],
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_payouts", [])
        .map_err(|error| format!("Failed to clear provider payout rows: {error}"))?;
    for payout in payouts {
        tx.execute(
            "INSERT INTO provider_payouts (payout_id, created_at_epoch_seconds, value_json) VALUES (?1, ?2, ?3)",
            params![
                payout.payout_id,
                i64::try_from(payout.created_at_epoch_seconds).unwrap_or(i64::MAX),
                encode_json(payout)?
            ],
        )
        .map_err(|error| {
            format!(
                "Failed to insert provider payout row {}: {error}",
                payout.payout_id
            )
        })?;
    }
    Ok(())
}

fn replace_health_events(
    tx: &rusqlite::Transaction<'_>,
    events: &[ProviderHealthEvent],
) -> Result<(), String> {
    tx.execute("DELETE FROM provider_health_events", [])
        .map_err(|error| format!("Failed to clear provider health event rows: {error}"))?;
    for event in events {
        tx.execute(
            "INSERT INTO provider_health_events (event_id, occurred_at_ms, value_json) VALUES (?1, ?2, ?3)",
            params![event.event_id, event.occurred_at_ms, encode_json(event)?],
        )
        .map_err(|error| {
            format!(
                "Failed to insert provider health event row {}: {error}",
                event.event_id
            )
        })?;
    }
    Ok(())
}

fn load_json_rows<T: DeserializeOwned>(
    connection: &Connection,
    sql: &str,
    limit: usize,
) -> Result<Vec<T>, String> {
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| format!("Failed to prepare provider admin list query: {error}"))?;
    let rows = statement
        .query_map(params![i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
            row.get::<_, String>(0)
        })
        .map_err(|error| format!("Failed to query provider admin list rows: {error}"))?;
    let mut values = Vec::new();
    for row in rows {
        let payload =
            row.map_err(|error| format!("Failed to decode provider admin list row: {error}"))?;
        values.push(decode_json::<T>(&payload)?);
    }
    Ok(values)
}

fn backend_storage_key(backend_kind: ProviderBackendKind) -> &'static str {
    match backend_kind {
        ProviderBackendKind::GptOss => "gpt_oss",
        ProviderBackendKind::AppleFoundationModels => "apple_foundation_models",
        ProviderBackendKind::PsionicTrain => "psionic_train",
    }
}

fn bounded_limit(limit: Option<usize>, default_limit: usize) -> usize {
    limit
        .unwrap_or(default_limit)
        .clamp(1, PROVIDER_ADMIN_MAX_LIST_LIMIT)
}

fn encode_json<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value)
        .map_err(|error| format!("Failed to encode provider admin json payload: {error}"))
}

fn decode_json<T: DeserializeOwned>(value: &str) -> Result<T, String> {
    serde_json::from_str(value)
        .map_err(|error| format!("Failed to decode provider admin json payload: {error}"))
}

fn now_epoch_ms() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => i64::try_from(duration.as_millis()).unwrap_or(i64::MAX),
        Err(_) => 0,
    }
}

fn api_error(error: impl Into<String>) -> (StatusCode, Json<ProviderApiError>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ProviderApiError {
            code: "internal_error".to_string(),
            error: error.into(),
            current_state: None,
            desired_mode: None,
        }),
    )
}

fn transition_api_error(error: ProviderTransitionError) -> (StatusCode, Json<ProviderApiError>) {
    (
        StatusCode::CONFLICT,
        Json(ProviderApiError {
            code: error.code,
            error: error.detail,
            current_state: Some(error.current_state),
            desired_mode: Some(error.desired_mode),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderAdminConfig, ProviderAdminRuntime, ProviderControlAction, ProviderDesiredMode,
        ProviderEarningsSummary, ProviderHealthEvent, ProviderIdentityMetadata, ProviderJsonEntry,
        ProviderPayoutSummary, ProviderPersistedSnapshot, ProviderPersistenceStore,
        ProviderReceiptSummary, ProviderRecentJob, ProviderRuntimeStatusSnapshot,
        ProviderSnapshotParts, ProviderStatusResponse, assemble_provider_persisted_snapshot,
    };
    use crate::{
        ProviderAvailability, ProviderBackendHealth, ProviderComputeProduct, ProviderInventoryRow,
        ProviderMode, ProviderSandboxAvailability, ProviderSandboxExecutionClass,
        ProviderSandboxProfile, ProviderSandboxRuntimeHealth, ProviderSandboxRuntimeKind,
    };
    use axum::http::StatusCode;
    use serde_json::json;
    use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
    use std::path::PathBuf;
    use std::time::Duration;

    fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other(message.to_string()).into())
        }
    }

    fn sample_config(db_path: PathBuf) -> ProviderAdminConfig {
        ProviderAdminConfig::new(
            db_path,
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, 0)),
        )
    }

    fn sample_snapshot() -> ProviderPersistedSnapshot {
        assemble_provider_persisted_snapshot(ProviderSnapshotParts {
            captured_at_ms: 1_762_300_000_000,
            config_metadata: vec![ProviderJsonEntry {
                key: "relay_urls".to_string(),
                value: json!(["wss://relay.example.com"]),
            }],
            identity: Some(ProviderIdentityMetadata {
                npub: Some("npub1example".to_string()),
                public_key_hex: Some("abcdef".to_string()),
                display_name: Some("Autopilot Desktop".to_string()),
                node_label: Some("desktop-01".to_string()),
            }),
            runtime: ProviderRuntimeStatusSnapshot {
                mode: ProviderMode::Online,
                last_action: Some("serving".to_string()),
                last_error: None,
                degraded_reason_code: None,
                authoritative_status: Some("ready".to_string()),
                authoritative_error_class: None,
                queue_depth: 1,
                online_uptime_seconds: 45,
                inventory_session_started_at_ms: Some(1_762_300_000_000),
                last_completed_job_at_epoch_ms: Some(1_762_300_030_000),
                last_authoritative_event_id: Some("evt-1".to_string()),
                execution_backend_label: "local GPT-OSS runtime".to_string(),
                provider_blocker_codes: Vec::new(),
            },
            availability: ProviderAvailability {
                gpt_oss: ProviderBackendHealth {
                    reachable: true,
                    ready: true,
                    configured_model: Some("llama3.2:latest".to_string()),
                    ready_model: Some("llama3.2:latest".to_string()),
                    available_models: vec!["llama3.2:latest".to_string()],
                    last_error: None,
                    last_action: Some("ready".to_string()),
                    availability_message: None,
                    latency_ms_p50: Some(110),
                },
                apple_foundation_models: ProviderBackendHealth::default(),
                apple_adapter_hosting: Default::default(),
                adapter_training_contributor: Default::default(),
                sandbox: ProviderSandboxAvailability {
                    runtimes: vec![ProviderSandboxRuntimeHealth {
                        runtime_kind: ProviderSandboxRuntimeKind::Python,
                        detected: true,
                        ready: true,
                        binary_name: Some("python3".to_string()),
                        binary_path: Some("/usr/bin/python3".to_string()),
                        runtime_version: Some("Python 3.11.8".to_string()),
                        supported_execution_classes: vec![
                            ProviderSandboxExecutionClass::PythonExec,
                        ],
                        last_error: None,
                    }],
                    profiles: vec![ProviderSandboxProfile {
                        profile_id: "python-batch".to_string(),
                        profile_digest: "sha256:python-profile".to_string(),
                        execution_class: ProviderSandboxExecutionClass::PythonExec,
                        runtime_family: "python3".to_string(),
                        runtime_version: "Python 3.11.8".to_string(),
                        sandbox_engine: "local_subprocess".to_string(),
                        os_family: "linux".to_string(),
                        arch: "x86_64".to_string(),
                        cpu_limit: 2,
                        memory_limit_mb: 2048,
                        disk_limit_mb: 4096,
                        timeout_limit_s: 120,
                        network_mode: "none".to_string(),
                        filesystem_mode: "workspace_only".to_string(),
                        workspace_mode: "ephemeral".to_string(),
                        artifact_output_mode: "declared_paths_only".to_string(),
                        secrets_mode: "none".to_string(),
                        allowed_binaries: vec!["python3".to_string()],
                        toolchain_inventory: vec!["python3".to_string()],
                        container_image: None,
                        runtime_image_digest: None,
                        accelerator_policy: None,
                        runtime_kind: ProviderSandboxRuntimeKind::Python,
                        runtime_ready: true,
                        runtime_binary_path: Some("/usr/bin/python3".to_string()),
                        capability_summary: "backend=sandbox execution=sandbox.python.exec family=sandbox_execution profile_id=python-batch".to_string(),
                    }],
                    last_scan_error: None,
                },
            },
            inventory_rows: vec![ProviderInventoryRow {
                target: ProviderComputeProduct::GptOssInference,
                enabled: true,
                backend_ready: true,
                eligible: true,
                capability_summary:
                    "backend=gpt_oss execution=local_inference family=inference".to_string(),
                source_badge: "local".to_string(),
                capacity_lot_id: Some("lot-1".to_string()),
                total_quantity: 1,
                reserved_quantity: 0,
                available_quantity: 1,
                delivery_state: "ready".to_string(),
                price_floor_sats: 21,
                terms_label: "spot session / local best effort".to_string(),
                forward_capacity_lot_id: None,
                forward_delivery_window_label: None,
                forward_total_quantity: 0,
                forward_reserved_quantity: 0,
                forward_available_quantity: 0,
                forward_terms_label: None,
            }],
            recent_jobs: vec![ProviderRecentJob {
                job_id: "job-1".to_string(),
                request_id: Some("req-1".to_string()),
                status: "succeeded".to_string(),
                demand_source: "open_network".to_string(),
                product_id: Some("gpt_oss.text_generation".to_string()),
                compute_family: Some("inference".to_string()),
                backend_family: Some("gpt_oss".to_string()),
                sandbox_execution_class: None,
                sandbox_profile_id: None,
                sandbox_profile_digest: None,
                sandbox_termination_reason: None,
                completed_at_epoch_seconds: 1_762_300_030,
                payout_sats: 42,
                payment_pointer: "payment-1".to_string(),
                failure_reason: None,
                delivery_proof_id: Some("proof-1".to_string()),
            }],
            receipts: vec![ProviderReceiptSummary {
                receipt_id: "receipt-1".to_string(),
                receipt_type: "earn.job.settled.v1".to_string(),
                created_at_ms: 1_762_300_030_500,
                canonical_hash: "sha256:receipt-1".to_string(),
                compute_family: Some("inference".to_string()),
                backend_family: Some("gpt_oss".to_string()),
                sandbox_execution_class: None,
                sandbox_profile_id: None,
                sandbox_profile_digest: None,
                sandbox_termination_reason: None,
                reason_code: Some("SETTLED".to_string()),
                failure_reason: None,
                severity: Some("low".to_string()),
                notional_sats: Some(42),
                liability_premium_sats: Some(0),
                work_unit_id: Some("work-unit-1".to_string()),
            }],
            payouts: vec![ProviderPayoutSummary {
                payout_id: "payment-1".to_string(),
                amount_sats: 42,
                direction: "receive".to_string(),
                status: "settled".to_string(),
                created_at_epoch_seconds: 1_762_300_031,
                payment_pointer: Some("payment-1".to_string()),
            }],
            health_events: vec![ProviderHealthEvent {
                event_id: "evt-health-1".to_string(),
                occurred_at_ms: 1_762_300_005_000,
                severity: "info".to_string(),
                code: "READY".to_string(),
                detail: "provider ready".to_string(),
                source: "provider_runtime".to_string(),
            }],
            earnings: Some(ProviderEarningsSummary {
                sats_today: 42,
                lifetime_sats: 420,
                jobs_today: 1,
                online_uptime_seconds: 45,
                last_job_result: "succeeded".to_string(),
                first_job_latency_seconds: Some(12),
                completion_ratio_bps: Some(10_000),
                payout_success_ratio_bps: Some(10_000),
                avg_wallet_confirmation_latency_seconds: Some(3),
            }),
        })
    }

    #[test]
    fn assembled_snapshot_preserves_linked_recent_jobs_and_receipts() {
        let snapshot = sample_snapshot();

        assert_eq!(
            snapshot
                .recent_jobs
                .first()
                .and_then(|job| job.delivery_proof_id.as_deref()),
            Some("proof-1")
        );
        assert_eq!(
            snapshot
                .recent_jobs
                .first()
                .and_then(|job| job.compute_family.as_deref()),
            Some("inference")
        );
        assert_eq!(
            snapshot
                .receipts
                .first()
                .and_then(|receipt| receipt.work_unit_id.as_deref()),
            Some("work-unit-1")
        );
        assert_eq!(
            snapshot
                .receipts
                .first()
                .and_then(|receipt| receipt.backend_family.as_deref()),
            Some("gpt_oss")
        );
        assert_eq!(
            snapshot
                .earnings
                .as_ref()
                .map(|earnings| earnings.lifetime_sats),
            Some(420)
        );
    }

    #[test]
    fn sqlite_store_round_trips_runtime_snapshot_and_lists()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("provider-admin.sqlite");
        let config = sample_config(db_path.clone());
        let mut store = ProviderPersistenceStore::open(&config)?;
        store.set_listen_addr("127.0.0.1:7777")?;
        store.set_desired_mode(ProviderDesiredMode::Online)?;
        let snapshot = sample_snapshot();
        store.persist_snapshot(&snapshot)?;

        let reopened = ProviderPersistenceStore::open(&config)?;
        let status = reopened.load_status()?;

        ensure(
            status.listen_addr.as_deref() == Some("127.0.0.1:7777"),
            "provider admin store did not persist listen addr",
        )?;
        ensure(
            status.desired_mode == ProviderDesiredMode::Online,
            "provider admin store did not persist desired mode",
        )?;
        ensure(
            status.snapshot == Some(snapshot.clone()),
            "provider admin store did not round-trip snapshot payload",
        )?;
        ensure(
            reopened.load_recent_jobs(Some(1))? == snapshot.recent_jobs,
            "provider admin store did not round-trip recent jobs",
        )?;
        ensure(
            reopened.load_receipts(Some(1))? == snapshot.receipts,
            "provider admin store did not round-trip receipts",
        )?;
        ensure(
            reopened.load_payouts(Some(1))? == snapshot.payouts,
            "provider admin store did not round-trip payouts",
        )?;
        ensure(
            reopened.load_health_events(Some(1))? == snapshot.health_events,
            "provider admin store did not round-trip health events",
        )?;
        ensure(
            reopened.load_sandbox_runtimes(Some(4))? == snapshot.availability.sandbox.runtimes,
            "provider admin store did not round-trip sandbox runtimes",
        )?;
        ensure(
            reopened.load_sandbox_profiles(Some(4))? == snapshot.availability.sandbox.profiles,
            "provider admin store did not round-trip sandbox profiles",
        )?;
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn local_admin_http_api_persists_control_events_and_status()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = tempfile::tempdir()?;
        let db_path = temp_dir.path().join("provider-admin-http.sqlite");
        let config = sample_config(db_path);
        let mut runtime = ProviderAdminRuntime::spawn(config)?;
        runtime.sync_snapshot(sample_snapshot())?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()?;
        let status_url = format!("http://{}/v1/status", runtime.listen_addr());
        let jobs_url = format!("http://{}/v1/jobs?limit=1", runtime.listen_addr());
        let sandbox_runtimes_url = format!(
            "http://{}/v1/sandbox/runtimes?limit=4",
            runtime.listen_addr()
        );
        let sandbox_profiles_url = format!(
            "http://{}/v1/sandbox/profiles?limit=4",
            runtime.listen_addr()
        );
        let online_url = format!("http://{}/v1/online", runtime.listen_addr());
        let pause_url = format!("http://{}/v1/pause", runtime.listen_addr());
        let resume_url = format!("http://{}/v1/resume", runtime.listen_addr());

        let mut status = None::<ProviderStatusResponse>;
        for _ in 0..20 {
            let response = client.get(status_url.as_str()).send().await?;
            let decoded = response.json::<ProviderStatusResponse>().await?;
            if decoded.snapshot.is_some() {
                status = Some(decoded);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let initial_status =
            status.ok_or("provider admin status never observed persisted snapshot")?;
        ensure(
            initial_status.desired_mode == ProviderDesiredMode::Offline,
            "provider admin api should default desired mode to offline",
        )?;
        ensure(
            initial_status
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.identity.as_ref())
                .and_then(|identity| identity.npub.as_deref())
                == Some("npub1example"),
            "provider admin api did not expose persisted identity metadata",
        )?;

        let jobs = client
            .get(jobs_url.as_str())
            .send()
            .await?
            .json::<Vec<ProviderRecentJob>>()
            .await?;
        ensure(
            jobs.len() == 1,
            "provider admin api did not return recent jobs",
        )?;
        ensure(
            jobs.first().map(|job| job.job_id.as_str()) == Some("job-1"),
            "provider admin api returned the wrong recent job id",
        )?;

        let sandbox_runtimes = client
            .get(sandbox_runtimes_url.as_str())
            .send()
            .await?
            .json::<Vec<ProviderSandboxRuntimeHealth>>()
            .await?;
        ensure(
            sandbox_runtimes.len() == 1,
            "provider admin api did not return sandbox runtimes",
        )?;

        let sandbox_profiles = client
            .get(sandbox_profiles_url.as_str())
            .send()
            .await?
            .json::<Vec<ProviderSandboxProfile>>()
            .await?;
        ensure(
            sandbox_profiles.len() == 1,
            "provider admin api did not return sandbox profiles",
        )?;

        let invalid_pause = client.post(pause_url.as_str()).send().await?;
        ensure(
            invalid_pause.status() == StatusCode::CONFLICT,
            "provider admin api should reject pause while offline",
        )?;
        let invalid_pause_payload = invalid_pause.json::<serde_json::Value>().await?;
        ensure(
            invalid_pause_payload
                .get("code")
                .and_then(serde_json::Value::as_str)
                == Some("provider_not_online"),
            "provider admin api did not expose a machine-readable pause rejection code",
        )?;

        let online_status = client
            .post(online_url.as_str())
            .send()
            .await?
            .json::<ProviderStatusResponse>()
            .await?;
        ensure(
            online_status.desired_mode == ProviderDesiredMode::Online,
            "provider admin api did not persist online desired mode",
        )?;

        let repeated_online_status = client
            .post(online_url.as_str())
            .send()
            .await?
            .json::<ProviderStatusResponse>()
            .await?;
        ensure(
            repeated_online_status.desired_mode == ProviderDesiredMode::Online,
            "provider admin api should allow idempotent online retries",
        )?;

        let paused_status = client
            .post(pause_url.as_str())
            .send()
            .await?
            .json::<ProviderStatusResponse>()
            .await?;
        ensure(
            paused_status.desired_mode == ProviderDesiredMode::Paused,
            "provider admin api did not persist paused desired mode",
        )?;

        let resumed_status = client
            .post(resume_url.as_str())
            .send()
            .await?
            .json::<ProviderStatusResponse>()
            .await?;
        ensure(
            resumed_status.desired_mode == ProviderDesiredMode::Online,
            "provider admin api did not persist resumed online desired mode",
        )?;

        let updates = tokio::task::spawn_blocking(move || {
            let start = std::time::Instant::now();
            loop {
                let updates = runtime.drain_updates();
                if !updates.is_empty() {
                    return Ok::<_, String>(updates);
                }
                if start.elapsed() > Duration::from_secs(2) {
                    return Err("timed out waiting for provider admin control event".to_string());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        })
        .await??;

        ensure(
            updates.iter().any(|update| {
                matches!(
                    update,
                    super::ProviderAdminUpdate::ControlEvent(event)
                        if event.action == ProviderControlAction::Online
                            && event.desired_mode == ProviderDesiredMode::Online
                )
            }),
            "provider admin runtime did not emit an online control event",
        )?;
        ensure(
            updates.iter().any(|update| {
                matches!(
                    update,
                    super::ProviderAdminUpdate::ControlEvent(event)
                        if event.action == ProviderControlAction::Pause
                            && event.desired_mode == ProviderDesiredMode::Paused
                )
            }),
            "provider admin runtime did not emit a pause control event",
        )?;
        ensure(
            updates.iter().any(|update| {
                matches!(
                    update,
                    super::ProviderAdminUpdate::ControlEvent(event)
                        if event.action == ProviderControlAction::Resume
                            && event.desired_mode == ProviderDesiredMode::Online
                )
            }),
            "provider admin runtime did not emit a resume control event",
        )?;
        Ok(())
    }
}
