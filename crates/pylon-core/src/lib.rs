#![forbid(unsafe_code)]

use openagents_provider_substrate::{
    ProviderDesiredMode, ProviderInventoryRow, ProviderPersistedSnapshot, ProviderReceiptSummary,
    ProviderStatusResponse, provider_runtime_state_label,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub const PYLON_CORE_SCHEMA_VERSION: &str = "openagents.pylon_core.v1";
pub const DEFAULT_PYLON_HEARTBEAT_ROUTE: &str = "/api/provider-presence/heartbeat";
pub const DEFAULT_PYLON_HEARTBEAT_INTERVAL_MS: u64 = 30_000;

pub const CONTRIBUTOR_OPEN_SOURCE_SURFACES: &[&str] = &[
    "installable_app",
    "tui",
    "contributor_wallet_ux",
    "provider_inventory_truth",
    "payout_behavior",
    "public_receipts",
];

pub const PRIVATE_CLOUD_DEPENDENCY_MARKERS: &[&str] = &[
    "OpenAgentsInc/cloud",
    "openagentsinc/cloud",
    "../cloud",
    "../../cloud",
    "openagents-cloud-contract",
    "oa-node",
    "oa-workroomd",
];

pub const PUBLIC_BOUNDARY_FORBIDDEN_KEYS: &[&str] = &[
    "wallet_seed",
    "node_entropy",
    "private_key",
    "preimage",
    "bearer_token",
    "api_key",
    "private_topology",
    "capacity_pool_secret",
    "internal_accounting_credential",
];

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum PylonCoreBoundaryError {
    #[error("{0}")]
    Invalid(String),
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreProjectionOptions {
    pub client_name: String,
    pub client_version: String,
    pub heartbeat_route: String,
    pub heartbeat_interval_ms: u64,
}

impl Default for PylonCoreProjectionOptions {
    fn default() -> Self {
        Self {
            client_name: "pylon".to_string(),
            client_version: "unknown".to_string(),
            heartbeat_route: DEFAULT_PYLON_HEARTBEAT_ROUTE.to_string(),
            heartbeat_interval_ms: DEFAULT_PYLON_HEARTBEAT_INTERVAL_MS,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreSnapshot {
    pub schema_version: String,
    pub identity: PylonCoreIdentity,
    pub admin: PylonCoreAdminState,
    pub availability: PylonCoreAvailability,
    pub inventory: PylonCoreInventory,
    pub lifecycle: PylonCoreLifecycle,
    pub heartbeat: PylonCoreHeartbeat,
    pub receipts: Vec<PylonCoreReceipt>,
}

impl PylonCoreSnapshot {
    pub fn from_provider_status(
        status: &ProviderStatusResponse,
        options: PylonCoreProjectionOptions,
    ) -> Self {
        let snapshot = status.snapshot.as_ref();
        let identity = PylonCoreIdentity::from_provider_snapshot(snapshot, &options);
        let availability = snapshot
            .map(PylonCoreAvailability::from_provider_snapshot)
            .unwrap_or_else(PylonCoreAvailability::unavailable);
        let inventory = snapshot
            .map(|snapshot| PylonCoreInventory::from_rows(&snapshot.inventory_rows))
            .unwrap_or_default();
        let lifecycle = PylonCoreLifecycle::from_provider_status(status);
        let heartbeat = PylonCoreHeartbeat::from_provider_status(status, &options);
        let receipts = snapshot
            .map(|snapshot| {
                snapshot
                    .receipts
                    .iter()
                    .map(PylonCoreReceipt::from)
                    .collect()
            })
            .unwrap_or_default();

        Self {
            schema_version: PYLON_CORE_SCHEMA_VERSION.to_string(),
            identity,
            admin: PylonCoreAdminState {
                desired_mode: status.desired_mode.into(),
                local_admin_enabled: status.listen_addr.is_some(),
                tui_required: false,
            },
            availability,
            inventory,
            lifecycle,
            heartbeat,
            receipts,
        }
    }

    pub fn validate_public_boundary(&self) -> Result<(), PylonCoreBoundaryError> {
        if self.schema_version != PYLON_CORE_SCHEMA_VERSION {
            return Err(PylonCoreBoundaryError::Invalid(format!(
                "unexpected pylon-core schema version '{}'",
                self.schema_version
            )));
        }
        if self.identity.node_id.trim().is_empty() {
            return Err(PylonCoreBoundaryError::Invalid(
                "pylon-core identity requires a public node id".to_string(),
            ));
        }
        if self.heartbeat.interval_ms == 0 {
            return Err(PylonCoreBoundaryError::Invalid(
                "pylon-core heartbeat interval must be non-zero".to_string(),
            ));
        }
        for item in &self.inventory.items {
            if item.product_id.trim().is_empty() {
                return Err(PylonCoreBoundaryError::Invalid(
                    "pylon-core inventory product id must not be empty".to_string(),
                ));
            }
        }
        for receipt in &self.receipts {
            if receipt.receipt_id.trim().is_empty() || receipt.receipt_type.trim().is_empty() {
                return Err(PylonCoreBoundaryError::Invalid(
                    "pylon-core receipts require id and type".to_string(),
                ));
            }
        }

        let value = serde_json::to_value(self).map_err(|error| {
            PylonCoreBoundaryError::Invalid(format!(
                "failed to encode pylon-core boundary for validation: {error}"
            ))
        })?;
        assert_public_json_boundary(&value)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreIdentity {
    pub node_id: String,
    pub node_label: Option<String>,
    pub npub: Option<String>,
    pub public_key_hex: Option<String>,
    pub client_name: String,
    pub client_version: String,
}

impl PylonCoreIdentity {
    fn from_provider_snapshot(
        snapshot: Option<&ProviderPersistedSnapshot>,
        options: &PylonCoreProjectionOptions,
    ) -> Self {
        let metadata = snapshot.and_then(|snapshot| snapshot.identity.as_ref());
        let npub = metadata.and_then(|identity| public_string(identity.npub.as_deref()));
        let public_key_hex =
            metadata.and_then(|identity| public_string(identity.public_key_hex.as_deref()));
        let node_label =
            metadata.and_then(|identity| public_string(identity.node_label.as_deref()));
        let node_id = public_key_hex
            .clone()
            .or_else(|| npub.clone())
            .or_else(|| node_label.clone())
            .unwrap_or_default();

        Self {
            node_id,
            node_label,
            npub,
            public_key_hex,
            client_name: options.client_name.clone(),
            client_version: options.client_version.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PylonDesiredMode {
    #[default]
    Offline,
    Online,
    Paused,
}

impl From<ProviderDesiredMode> for PylonDesiredMode {
    fn from(value: ProviderDesiredMode) -> Self {
        match value {
            ProviderDesiredMode::Offline => Self::Offline,
            ProviderDesiredMode::Online => Self::Online,
            ProviderDesiredMode::Paused => Self::Paused,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreAdminState {
    pub desired_mode: PylonDesiredMode,
    pub local_admin_enabled: bool,
    pub tui_required: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreAvailability {
    pub provider_available: bool,
    pub observed_state: String,
    pub active_inference_backend: Option<String>,
    pub execution_backend_label: String,
    pub eligible_inventory_count: usize,
    pub blocker_codes: Vec<String>,
    pub degraded_reason_code: Option<String>,
}

impl PylonCoreAvailability {
    fn unavailable() -> Self {
        Self {
            provider_available: false,
            observed_state: "unconfigured".to_string(),
            active_inference_backend: None,
            execution_backend_label: "no active inference backend".to_string(),
            eligible_inventory_count: 0,
            blocker_codes: Vec::new(),
            degraded_reason_code: None,
        }
    }

    fn from_provider_snapshot(snapshot: &ProviderPersistedSnapshot) -> Self {
        let observed_state = snapshot
            .runtime
            .authoritative_status
            .clone()
            .unwrap_or_else(|| snapshot.runtime.mode.label().to_string());
        let provider_available = matches!(observed_state.as_str(), "online" | "ready" | "degraded");
        let active_inference_backend = snapshot
            .availability
            .active_inference_backend()
            .map(|backend| backend.label().to_string());
        let eligible_inventory_count = snapshot
            .inventory_rows
            .iter()
            .filter(|row| row.eligible)
            .count();

        Self {
            provider_available,
            observed_state,
            active_inference_backend,
            execution_backend_label: snapshot.availability.execution_backend_label().to_string(),
            eligible_inventory_count,
            blocker_codes: snapshot.runtime.provider_blocker_codes.clone(),
            degraded_reason_code: snapshot.runtime.degraded_reason_code.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreInventory {
    pub items: Vec<PylonCoreInventoryItem>,
}

impl PylonCoreInventory {
    fn from_rows(rows: &[ProviderInventoryRow]) -> Self {
        Self {
            items: rows.iter().map(PylonCoreInventoryItem::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreInventoryItem {
    pub product_id: String,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub available_quantity: u64,
    pub capability_summary: String,
    pub market_receipt_class: String,
}

impl From<&ProviderInventoryRow> for PylonCoreInventoryItem {
    fn from(row: &ProviderInventoryRow) -> Self {
        Self {
            product_id: row.target.product_id().to_string(),
            enabled: row.enabled,
            backend_ready: row.backend_ready,
            eligible: row.eligible,
            available_quantity: row.available_quantity,
            capability_summary: row.capability_summary.clone(),
            market_receipt_class: row.market_receipt_class.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreLifecycle {
    pub desired_mode: PylonDesiredMode,
    pub observed_state: String,
    pub captured_at_ms: Option<i64>,
    pub online_uptime_seconds: u64,
    pub last_completed_job_at_epoch_ms: Option<i64>,
}

impl PylonCoreLifecycle {
    fn from_provider_status(status: &ProviderStatusResponse) -> Self {
        let snapshot = status.snapshot.as_ref();
        Self {
            desired_mode: status.desired_mode.into(),
            observed_state: provider_runtime_state_label(status),
            captured_at_ms: snapshot.map(|snapshot| snapshot.captured_at_ms),
            online_uptime_seconds: snapshot
                .map(|snapshot| snapshot.runtime.online_uptime_seconds)
                .unwrap_or_default(),
            last_completed_job_at_epoch_ms: snapshot
                .and_then(|snapshot| snapshot.runtime.last_completed_job_at_epoch_ms),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreHeartbeat {
    pub route: String,
    pub interval_ms: u64,
    pub last_heartbeat_at_ms: Option<i64>,
    pub last_authoritative_event_id: Option<String>,
}

impl PylonCoreHeartbeat {
    fn from_provider_status(
        status: &ProviderStatusResponse,
        options: &PylonCoreProjectionOptions,
    ) -> Self {
        let snapshot = status.snapshot.as_ref();
        Self {
            route: options.heartbeat_route.clone(),
            interval_ms: options.heartbeat_interval_ms,
            last_heartbeat_at_ms: None,
            last_authoritative_event_id: snapshot
                .and_then(|snapshot| snapshot.runtime.last_authoritative_event_id.clone()),
        }
    }

    pub fn next_due_at_ms(&self) -> Option<i64> {
        let interval = i64::try_from(self.interval_ms).ok()?;
        self.last_heartbeat_at_ms
            .and_then(|last| last.checked_add(interval))
    }

    pub fn is_due_at_ms(&self, now_ms: i64) -> bool {
        self.next_due_at_ms().is_none_or(|due_at| now_ms >= due_at)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct PylonCoreReceipt {
    pub receipt_id: String,
    pub receipt_type: String,
    pub created_at_ms: i64,
    pub canonical_hash: String,
    pub work_unit_id: Option<String>,
    pub reason_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artanis_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artanis_assignment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_intent_id: Option<String>,
}

impl From<&ProviderReceiptSummary> for PylonCoreReceipt {
    fn from(receipt: &ProviderReceiptSummary) -> Self {
        Self {
            receipt_id: receipt.receipt_id.clone(),
            receipt_type: receipt.receipt_type.clone(),
            created_at_ms: receipt.created_at_ms,
            canonical_hash: receipt.canonical_hash.clone(),
            work_unit_id: receipt.work_unit_id.clone(),
            reason_code: receipt.reason_code.clone(),
            artanis_run_id: receipt.artanis_run_id.clone(),
            artanis_assignment_id: receipt.artanis_assignment_id.clone(),
            settlement_intent_id: receipt.settlement_intent_id.clone(),
        }
    }
}

pub fn assert_public_json_boundary(value: &Value) -> Result<(), PylonCoreBoundaryError> {
    let mut violations = Vec::new();
    collect_forbidden_keys(value, "$", &mut violations);
    if violations.is_empty() {
        return Ok(());
    }

    Err(PylonCoreBoundaryError::Invalid(format!(
        "public Pylon boundary contains forbidden private key(s): {}",
        violations.join(", ")
    )))
}

pub fn assert_no_private_cloud_dependency_text(
    label: &str,
    text: &str,
) -> Result<(), PylonCoreBoundaryError> {
    for marker in PRIVATE_CLOUD_DEPENDENCY_MARKERS {
        if text.contains(marker) {
            return Err(PylonCoreBoundaryError::Invalid(format!(
                "{label} must not depend on private cloud marker '{marker}'"
            )));
        }
    }
    Ok(())
}

fn collect_forbidden_keys(value: &Value, path: &str, violations: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let child_path = format!("{path}.{key}");
                if key_contains_forbidden_marker(key) {
                    violations.push(child_path.clone());
                }
                collect_forbidden_keys(child, &child_path, violations);
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                collect_forbidden_keys(child, &format!("{path}[{index}]"), violations);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

fn key_contains_forbidden_marker(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase();
    let compact = normalized
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect::<String>();
    PUBLIC_BOUNDARY_FORBIDDEN_KEYS.iter().any(|forbidden| {
        let forbidden_compact = forbidden.replace('_', "");
        normalized.contains(forbidden) || compact.contains(forbidden_compact.as_str())
    })
}

fn public_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openagents_provider_substrate::{
        ProviderAvailability, ProviderBackendHealth, ProviderComputeProduct,
        ProviderIdentityMetadata, ProviderInventoryRow, ProviderMode,
        ProviderRuntimeStatusSnapshot, ProviderSnapshotParts, assemble_provider_persisted_snapshot,
    };

    const CONTRIBUTOR_CLOUD_NODE_FIXTURE: &str =
        include_str!("../../../docs/pylon/fixtures/cloud_node_v1/contributor-pylon.json");
    const ROOT_CARGO: &str = include_str!("../../../Cargo.toml");
    const PYLON_CARGO: &str = include_str!("../../../apps/pylon/Cargo.toml");
    const PYLON_TUI_CARGO: &str = include_str!("../../../apps/pylon-tui/Cargo.toml");
    const PYLON_CORE_CARGO: &str = include_str!("../Cargo.toml");
    const OPEN_SOURCE_BOUNDARY_DOC: &str =
        include_str!("../../../docs/pylon/OPEN_SOURCE_CONTRIBUTOR_BOUNDARY.md");

    #[test]
    fn provider_status_projects_public_pylon_core_without_tui() {
        let status = sample_provider_status();
        let snapshot = PylonCoreSnapshot::from_provider_status(
            &status,
            PylonCoreProjectionOptions {
                client_name: "pylon".to_string(),
                client_version: "0.1.23".to_string(),
                ..PylonCoreProjectionOptions::default()
            },
        );

        snapshot
            .validate_public_boundary()
            .unwrap_or_else(|error| panic!("public pylon-core boundary should validate: {error}"));
        assert_eq!(snapshot.identity.node_id, "abcdef");
        assert_eq!(snapshot.admin.desired_mode, PylonDesiredMode::Online);
        assert!(!snapshot.admin.tui_required);
        assert!(snapshot.availability.provider_available);
        assert_eq!(
            snapshot.availability.active_inference_backend.as_deref(),
            Some("local Gemma runtime")
        );
        assert_eq!(snapshot.inventory.items.len(), 1);
        assert_eq!(
            snapshot.inventory.items[0].product_id,
            "psionic.local.inference.gemma.single_node"
        );
        assert_eq!(snapshot.lifecycle.observed_state, "ready");
        assert_eq!(
            snapshot.heartbeat.last_authoritative_event_id.as_deref(),
            Some("event.heartbeat.1")
        );
        assert_eq!(snapshot.receipts[0].receipt_type, "accepted_work");
    }

    #[test]
    fn heartbeat_due_helper_is_pure_core_logic() {
        let heartbeat = PylonCoreHeartbeat {
            route: DEFAULT_PYLON_HEARTBEAT_ROUTE.to_string(),
            interval_ms: 30_000,
            last_heartbeat_at_ms: Some(100_000),
            last_authoritative_event_id: None,
        };

        assert_eq!(heartbeat.next_due_at_ms(), Some(130_000));
        assert!(!heartbeat.is_due_at_ms(129_999));
        assert!(heartbeat.is_due_at_ms(130_000));
    }

    #[test]
    fn contributor_cloud_node_fixture_is_public_safe_core_input() {
        let fixture: Value = serde_json::from_str(CONTRIBUTOR_CLOUD_NODE_FIXTURE)
            .unwrap_or_else(|error| panic!("contributor Cloud Node fixture should parse: {error}"));
        assert_public_json_boundary(&fixture)
            .unwrap_or_else(|error| panic!("fixture should be public safe: {error}"));
    }

    #[test]
    fn contributor_pylon_manifests_do_not_depend_on_private_cloud_repo() {
        for (label, text) in [
            ("root Cargo.toml", ROOT_CARGO),
            ("apps/pylon/Cargo.toml", PYLON_CARGO),
            ("apps/pylon-tui/Cargo.toml", PYLON_TUI_CARGO),
            ("crates/pylon-core/Cargo.toml", PYLON_CORE_CARGO),
        ] {
            assert_no_private_cloud_dependency_text(label, text)
                .unwrap_or_else(|error| panic!("{label} should stay public-only: {error}"));
        }
    }

    #[test]
    fn open_source_boundary_doc_names_required_public_surfaces() {
        for surface in CONTRIBUTOR_OPEN_SOURCE_SURFACES {
            assert!(
                OPEN_SOURCE_BOUNDARY_DOC.contains(surface),
                "boundary doc must name public surface {surface}"
            );
        }
    }

    #[test]
    fn public_boundary_rejects_private_key_material_names() {
        let fixture = serde_json::json!({
            "contract_version": "openagents.cloud_node.v1",
            "walletSeed": "redacted"
        });
        let error = assert_public_json_boundary(&fixture)
            .err()
            .unwrap_or_else(|| panic!("fixture should fail public boundary validation"));
        assert!(
            error.to_string().contains("walletSeed"),
            "error should name the forbidden key"
        );
    }

    fn sample_provider_status() -> ProviderStatusResponse {
        let snapshot = assemble_provider_persisted_snapshot(ProviderSnapshotParts {
            captured_at_ms: 1_700_000_000_000,
            identity: Some(ProviderIdentityMetadata {
                public_key_hex: Some("abcdef".to_string()),
                npub: Some("npub1example".to_string()),
                display_name: Some("Contributor Pylon".to_string()),
                node_label: Some("macbook-pro".to_string()),
            }),
            runtime: ProviderRuntimeStatusSnapshot {
                mode: ProviderMode::Online,
                authoritative_status: Some("ready".to_string()),
                queue_depth: 2,
                online_uptime_seconds: 42,
                inventory_session_started_at_ms: Some(1_700_000_000_000),
                last_authoritative_event_id: Some("event.heartbeat.1".to_string()),
                execution_backend_label: "local Gemma runtime".to_string(),
                ..ProviderRuntimeStatusSnapshot::default()
            },
            availability: ProviderAvailability {
                local_gemma: ProviderBackendHealth {
                    reachable: true,
                    ready: true,
                    configured_model: Some("gemma".to_string()),
                    ready_model: Some("gemma".to_string()),
                    ..ProviderBackendHealth::default()
                },
                ..ProviderAvailability::default()
            },
            inventory_rows: vec![ProviderInventoryRow {
                target: ProviderComputeProduct::GptOssInference,
                enabled: true,
                backend_ready: true,
                eligible: true,
                capability_summary: "local inference".to_string(),
                market_receipt_class: "accepted_work".to_string(),
                earnings_summary: "paid per accepted job".to_string(),
                source_badge: "local".to_string(),
                capacity_lot_id: None,
                total_quantity: 1,
                reserved_quantity: 0,
                available_quantity: 1,
                delivery_state: "ready".to_string(),
                price_floor_sats: 1,
                terms_label: "best effort".to_string(),
                forward_capacity_lot_id: None,
                forward_delivery_window_label: None,
                forward_total_quantity: 0,
                forward_reserved_quantity: 0,
                forward_available_quantity: 0,
                forward_terms_label: None,
            }],
            receipts: vec![ProviderReceiptSummary {
                receipt_id: "receipt.accepted_work.1".to_string(),
                receipt_type: "accepted_work".to_string(),
                created_at_ms: 1_700_000_001_000,
                canonical_hash: "sha256:accepted-work".to_string(),
                work_unit_id: Some("work.1".to_string()),
                ..ProviderReceiptSummary::default()
            }],
            ..ProviderSnapshotParts::default()
        });

        ProviderStatusResponse {
            listen_addr: Some("127.0.0.1:7777".to_string()),
            desired_mode: ProviderDesiredMode::Online,
            snapshot: Some(snapshot),
        }
    }
}
