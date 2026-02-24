use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::Config;

#[derive(Clone)]
pub struct DomainStore {
    state: Arc<RwLock<DomainStoreState>>,
    path: Option<PathBuf>,
    integration_secret_cipher: Option<IntegrationSecretCipher>,
}

#[derive(Clone)]
struct IntegrationSecretCipher {
    key_id: String,
    key: [u8; 32],
}

const INTEGRATION_SECRET_ENCRYPTION_KEY_ENV: &str = "OA_INTEGRATION_SECRET_ENCRYPTION_KEY";
const INTEGRATION_SECRET_KEY_ID_ENV: &str = "OA_INTEGRATION_SECRET_KEY_ID";
const INTEGRATION_SECRET_ENVELOPE_PREFIX: &str = "enc:v1:";

#[derive(Debug, thiserror::Error)]
pub enum DomainStoreError {
    #[error("record not found")]
    NotFound,
    #[error("forbidden")]
    Forbidden,
    #[error("{field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
    #[error("{message}")]
    Conflict { message: String },
    #[error("{message}")]
    Persistence { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotRecord {
    pub id: String,
    pub owner_user_id: String,
    pub handle: String,
    pub display_name: String,
    pub avatar: Option<String>,
    pub status: String,
    pub visibility: String,
    pub tagline: Option<String>,
    pub config_version: u32,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotProfileRecord {
    pub autopilot_id: String,
    pub owner_display_name: String,
    pub persona_summary: Option<String>,
    pub autopilot_voice: Option<String>,
    pub principles: Option<Value>,
    pub preferences: Option<Value>,
    pub onboarding_answers: Value,
    pub schema_version: u16,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotPolicyRecord {
    pub autopilot_id: String,
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub tool_allowlist: Vec<String>,
    pub tool_denylist: Vec<String>,
    pub l402_require_approval: bool,
    pub l402_max_spend_msats_per_call: Option<u64>,
    pub l402_max_spend_msats_per_day: Option<u64>,
    pub l402_allowed_hosts: Vec<String>,
    pub data_policy: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotRuntimeBindingRecord {
    pub id: String,
    pub autopilot_id: String,
    pub runtime_type: String,
    pub runtime_ref: Option<String>,
    pub is_primary: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub meta: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeDriverOverrideRecord {
    pub id: String,
    pub scope_type: String,
    pub scope_id: String,
    pub driver: String,
    pub is_active: bool,
    pub reason: Option<String>,
    pub meta: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertRuntimeDriverOverrideInput {
    pub scope_type: String,
    pub scope_id: String,
    pub driver: String,
    pub is_active: bool,
    pub reason: Option<String>,
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutopilotAggregate {
    pub autopilot: AutopilotRecord,
    pub profile: AutopilotProfileRecord,
    pub policy: AutopilotPolicyRecord,
    pub runtime_bindings: Vec<AutopilotRuntimeBindingRecord>,
}

#[derive(Debug, Clone)]
pub struct CreateAutopilotInput {
    pub owner_user_id: String,
    pub owner_display_name: String,
    pub display_name: String,
    pub handle_seed: Option<String>,
    pub avatar: Option<String>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub tagline: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateAutopilotInput {
    pub display_name: Option<String>,
    pub avatar: Option<String>,
    pub status: Option<String>,
    pub visibility: Option<String>,
    pub tagline: Option<String>,
    pub profile: Option<UpsertAutopilotProfileInput>,
    pub policy: Option<UpsertAutopilotPolicyInput>,
}

#[derive(Debug, Clone, Default)]
pub struct UpsertAutopilotProfileInput {
    pub owner_display_name: Option<String>,
    pub persona_summary: Option<String>,
    pub autopilot_voice: Option<String>,
    pub principles: Option<Value>,
    pub preferences: Option<Value>,
    pub onboarding_answers: Option<Value>,
    pub schema_version: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub struct UpsertAutopilotPolicyInput {
    pub model_provider: Option<String>,
    pub model: Option<String>,
    pub tool_allowlist: Option<Vec<String>>,
    pub tool_denylist: Option<Vec<String>>,
    pub l402_require_approval: Option<bool>,
    pub l402_max_spend_msats_per_call: Option<u64>,
    pub l402_max_spend_msats_per_day: Option<u64>,
    pub l402_allowed_hosts: Option<Vec<String>>,
    pub data_policy: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct UpsertAutopilotRuntimeBindingInput {
    pub autopilot_id: String,
    pub runtime_type: String,
    pub runtime_ref: Option<String>,
    pub is_primary: bool,
    pub last_seen_at: Option<DateTime<Utc>>,
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L402CredentialRecord {
    pub host: String,
    pub scope: String,
    pub macaroon: String,
    pub preimage: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertL402CredentialInput {
    pub host: String,
    pub scope: String,
    pub macaroon: String,
    pub preimage: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L402ReceiptRecord {
    pub id: u64,
    pub user_id: String,
    pub thread_id: String,
    pub run_id: String,
    pub autopilot_id: Option<String>,
    pub thread_title: Option<String>,
    pub run_status: Option<String>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub run_completed_at: Option<DateTime<Utc>>,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RecordL402ReceiptInput {
    pub user_id: String,
    pub thread_id: String,
    pub run_id: String,
    pub autopilot_id: Option<String>,
    pub thread_title: Option<String>,
    pub run_status: Option<String>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub run_completed_at: Option<DateTime<Utc>>,
    pub payload: Value,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L402GatewayEventRecord {
    pub id: u64,
    pub user_id: String,
    pub autopilot_id: Option<String>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RecordL402GatewayEventInput {
    pub user_id: String,
    pub autopilot_id: Option<String>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct L402PaywallRecord {
    pub id: String,
    pub owner_user_id: String,
    pub name: String,
    pub host_regexp: String,
    pub path_regexp: String,
    pub price_msats: u64,
    pub upstream: String,
    pub enabled: bool,
    pub meta: Option<Value>,
    pub last_reconcile_status: Option<String>,
    pub last_reconcile_error: Option<String>,
    pub last_reconciled_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateL402PaywallInput {
    pub owner_user_id: String,
    pub name: String,
    pub host_regexp: String,
    pub path_regexp: String,
    pub price_msats: u64,
    pub upstream: String,
    pub enabled: Option<bool>,
    pub meta: Option<Value>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateL402PaywallInput {
    pub name: Option<String>,
    pub host_regexp: Option<String>,
    pub path_regexp: Option<String>,
    pub price_msats: Option<u64>,
    pub upstream: Option<String>,
    pub enabled: Option<bool>,
    pub meta: Option<Value>,
    pub last_reconcile_status: Option<String>,
    pub last_reconcile_error: Option<String>,
    pub last_reconciled_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSparkWalletRecord {
    pub user_id: String,
    pub wallet_id: String,
    pub mnemonic: String,
    pub spark_address: Option<String>,
    pub lightning_address: Option<String>,
    pub identity_pubkey: Option<String>,
    pub last_balance_sats: Option<u64>,
    pub status: String,
    pub provider: String,
    pub last_error: Option<String>,
    pub meta: Option<Value>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertUserSparkWalletInput {
    pub user_id: String,
    pub wallet_id: String,
    pub mnemonic: String,
    pub spark_address: Option<String>,
    pub lightning_address: Option<String>,
    pub identity_pubkey: Option<String>,
    pub last_balance_sats: Option<u64>,
    pub status: Option<String>,
    pub provider: Option<String>,
    pub last_error: Option<String>,
    pub meta: Option<Value>,
    pub last_synced_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIntegrationRecord {
    pub id: u64,
    pub user_id: String,
    pub provider: String,
    pub status: String,
    pub encrypted_secret: Option<String>,
    pub secret_fingerprint: Option<String>,
    pub secret_last4: Option<String>,
    pub metadata: Option<Value>,
    pub connected_at: Option<DateTime<Utc>>,
    pub disconnected_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserIntegrationAuditRecord {
    pub id: u64,
    pub user_id: String,
    pub user_integration_id: Option<u64>,
    pub provider: String,
    pub action: String,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxThreadStateRecord {
    pub id: u64,
    pub user_id: String,
    pub thread_id: String,
    pub pending_approval: bool,
    pub decision: Option<String>,
    pub draft_preview: Option<String>,
    pub source: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxAuditRecord {
    pub id: u64,
    pub user_id: String,
    pub thread_id: String,
    pub action: String,
    pub detail: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertInboxThreadStateInput {
    pub user_id: String,
    pub thread_id: String,
    pub pending_approval: bool,
    pub decision: Option<String>,
    pub draft_preview: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecordInboxAuditInput {
    pub user_id: String,
    pub thread_id: String,
    pub action: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct IntegrationUpsertResult {
    pub integration: UserIntegrationRecord,
    pub action: String,
}

#[derive(Debug, Clone)]
pub struct UpsertResendIntegrationInput {
    pub user_id: String,
    pub api_key: String,
    pub sender_email: Option<String>,
    pub sender_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpsertGoogleIntegrationInput {
    pub user_id: String,
    pub refresh_token: Option<String>,
    pub access_token: Option<String>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommsWebhookEventRecord {
    pub id: u64,
    pub provider: String,
    pub idempotency_key: String,
    pub external_event_id: Option<String>,
    pub event_type: Option<String>,
    pub delivery_state: Option<String>,
    pub message_id: Option<String>,
    pub integration_id: Option<String>,
    pub user_id: Option<String>,
    pub recipient: Option<String>,
    pub signature_valid: bool,
    pub status: String,
    pub normalized_hash: Option<String>,
    pub runtime_attempts: u32,
    pub runtime_status_code: Option<u16>,
    pub runtime_response: Option<Value>,
    pub normalized_payload: Option<Value>,
    pub raw_payload: Option<Value>,
    pub last_error: Option<String>,
    pub forwarded_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RecordWebhookEventInput {
    pub provider: String,
    pub idempotency_key: String,
    pub external_event_id: Option<String>,
    pub event_type: Option<String>,
    pub delivery_state: Option<String>,
    pub message_id: Option<String>,
    pub integration_id: Option<String>,
    pub user_id: Option<String>,
    pub recipient: Option<String>,
    pub signature_valid: bool,
    pub status: Option<String>,
    pub normalized_payload: Option<Value>,
    pub raw_payload: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordWebhookEventResult {
    pub event: CommsWebhookEventRecord,
    pub inserted: bool,
}

#[derive(Debug, Clone)]
pub struct MarkWebhookEventVerifiedInput {
    pub webhook_event_id: u64,
    pub status: String,
    pub event_type: Option<String>,
    pub delivery_state: Option<String>,
    pub message_id: Option<String>,
    pub integration_id: Option<String>,
    pub user_id: Option<String>,
    pub recipient: Option<String>,
    pub normalized_hash: Option<String>,
    pub normalized_payload: Option<Value>,
    pub raw_payload: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommsDeliveryProjectionRecord {
    pub id: u64,
    pub user_id: String,
    pub provider: String,
    pub integration_id: String,
    pub last_state: Option<String>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub last_message_id: Option<String>,
    pub last_recipient: Option<String>,
    pub runtime_event_id: Option<String>,
    pub source: String,
    pub last_webhook_event_id: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct UpsertDeliveryProjectionInput {
    pub user_id: String,
    pub provider: String,
    pub integration_id: Option<String>,
    pub last_state: Option<String>,
    pub last_event_at: Option<DateTime<Utc>>,
    pub last_message_id: Option<String>,
    pub last_recipient: Option<String>,
    pub runtime_event_id: Option<String>,
    pub source: Option<String>,
    pub last_webhook_event_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShoutRecord {
    pub id: u64,
    pub user_id: String,
    pub zone: Option<String>,
    pub body: String,
    pub visibility: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CreateShoutInput {
    pub user_id: String,
    pub zone: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperRecord {
    pub id: u64,
    pub sender_id: String,
    pub recipient_id: String,
    pub body: String,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SendWhisperInput {
    pub sender_id: String,
    pub recipient_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ZoneCount {
    pub zone: String,
    pub count24h: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
struct DomainStoreState {
    autopilots: HashMap<String, AutopilotRecord>,
    autopilot_profiles: HashMap<String, AutopilotProfileRecord>,
    autopilot_policies: HashMap<String, AutopilotPolicyRecord>,
    autopilot_runtime_bindings: HashMap<String, AutopilotRuntimeBindingRecord>,
    runtime_driver_overrides: HashMap<String, RuntimeDriverOverrideRecord>,
    l402_credentials: HashMap<String, L402CredentialRecord>,
    l402_receipts: Vec<L402ReceiptRecord>,
    l402_gateway_events: Vec<L402GatewayEventRecord>,
    l402_paywalls: HashMap<String, L402PaywallRecord>,
    user_spark_wallets: HashMap<String, UserSparkWalletRecord>,
    user_integrations: HashMap<String, UserIntegrationRecord>,
    user_integration_audits: Vec<UserIntegrationAuditRecord>,
    inbox_thread_states: HashMap<String, InboxThreadStateRecord>,
    inbox_audits: Vec<InboxAuditRecord>,
    comms_webhook_events: HashMap<String, CommsWebhookEventRecord>,
    comms_delivery_projections: HashMap<String, CommsDeliveryProjectionRecord>,
    shouts: Vec<ShoutRecord>,
    whispers: Vec<WhisperRecord>,
    next_user_integration_id: u64,
    next_user_integration_audit_id: u64,
    next_inbox_thread_state_id: u64,
    next_inbox_audit_id: u64,
    next_comms_webhook_event_id: u64,
    next_comms_delivery_projection_id: u64,
    next_l402_receipt_id: u64,
    next_l402_gateway_event_id: u64,
    next_shout_id: u64,
    next_whisper_id: u64,
}

impl DomainStoreState {
    fn normalize_counters(&mut self) {
        if self.next_user_integration_id == 0 {
            self.next_user_integration_id = self
                .user_integrations
                .values()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_user_integration_audit_id == 0 {
            self.next_user_integration_audit_id = self
                .user_integration_audits
                .iter()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_inbox_thread_state_id == 0 {
            self.next_inbox_thread_state_id = self
                .inbox_thread_states
                .values()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_inbox_audit_id == 0 {
            self.next_inbox_audit_id = self
                .inbox_audits
                .iter()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_comms_webhook_event_id == 0 {
            self.next_comms_webhook_event_id = self
                .comms_webhook_events
                .values()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_comms_delivery_projection_id == 0 {
            self.next_comms_delivery_projection_id = self
                .comms_delivery_projections
                .values()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_l402_receipt_id == 0 {
            self.next_l402_receipt_id = self
                .l402_receipts
                .iter()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_l402_gateway_event_id == 0 {
            self.next_l402_gateway_event_id = self
                .l402_gateway_events
                .iter()
                .map(|row| row.id)
                .max()
                .unwrap_or(0)
                + 1;
        }
        if self.next_shout_id == 0 {
            self.next_shout_id = self.shouts.iter().map(|row| row.id).max().unwrap_or(0) + 1;
        }
        if self.next_whisper_id == 0 {
            self.next_whisper_id = self.whispers.iter().map(|row| row.id).max().unwrap_or(0) + 1;
        }
    }
}

impl DomainStore {
    pub fn from_config(config: &Config) -> Self {
        let path = config.domain_store_path.clone();
        let mut state = Self::load_state(path.as_ref());
        state.normalize_counters();
        let integration_secret_cipher = integration_secret_cipher_from_env();

        Self {
            state: Arc::new(RwLock::new(state)),
            path,
            integration_secret_cipher,
        }
    }

    pub async fn create_autopilot(
        &self,
        input: CreateAutopilotInput,
    ) -> Result<AutopilotAggregate, DomainStoreError> {
        let owner_user_id = normalize_non_empty(&input.owner_user_id, "owner_user_id")?;
        let owner_display_name = normalize_owner_display_name(&input.owner_display_name);
        let display_name = normalize_display_name(&input.display_name);

        let result = self
            .mutate(|state| {
                let now = Utc::now();
                let seed = input.handle_seed.as_deref().unwrap_or(&display_name);
                let handle = generate_unique_autopilot_handle(state, seed, None);
                let id = format!("ap_{}", Uuid::new_v4().simple());

                let autopilot = AutopilotRecord {
                    id: id.clone(),
                    owner_user_id: owner_user_id.clone(),
                    handle,
                    display_name: display_name.clone(),
                    avatar: normalize_optional_string(input.avatar.as_deref()),
                    status: normalize_status_or_default(input.status.as_deref(), "active"),
                    visibility: normalize_status_or_default(input.visibility.as_deref(), "private"),
                    tagline: normalize_optional_string(input.tagline.as_deref()),
                    config_version: 1,
                    deleted_at: None,
                    created_at: now,
                    updated_at: now,
                };

                let profile = AutopilotProfileRecord {
                    autopilot_id: id.clone(),
                    owner_display_name,
                    persona_summary: None,
                    autopilot_voice: None,
                    principles: None,
                    preferences: None,
                    onboarding_answers: Value::Array(Vec::new()),
                    schema_version: 1,
                    created_at: now,
                    updated_at: now,
                };

                let policy = AutopilotPolicyRecord {
                    autopilot_id: id.clone(),
                    model_provider: None,
                    model: None,
                    tool_allowlist: Vec::new(),
                    tool_denylist: Vec::new(),
                    l402_require_approval: true,
                    l402_max_spend_msats_per_call: None,
                    l402_max_spend_msats_per_day: None,
                    l402_allowed_hosts: Vec::new(),
                    data_policy: None,
                    created_at: now,
                    updated_at: now,
                };

                state.autopilots.insert(id.clone(), autopilot.clone());
                state.autopilot_profiles.insert(id.clone(), profile.clone());
                state.autopilot_policies.insert(id.clone(), policy.clone());

                Ok(AutopilotAggregate {
                    autopilot,
                    profile,
                    policy,
                    runtime_bindings: Vec::new(),
                })
            })
            .await?;

        Ok(result)
    }

    pub async fn list_autopilots_for_owner(
        &self,
        owner_user_id: &str,
        limit: usize,
    ) -> Result<Vec<AutopilotAggregate>, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;
        let safe_limit = limit.clamp(1, 500);

        let state = self.state.read().await;
        let mut autopilots: Vec<AutopilotRecord> = state
            .autopilots
            .values()
            .filter(|record| record.owner_user_id == owner_user_id)
            .filter(|record| record.deleted_at.is_none())
            .cloned()
            .collect();
        autopilots.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

        let rows = autopilots
            .into_iter()
            .take(safe_limit)
            .map(|autopilot| build_autopilot_aggregate(&state, autopilot))
            .collect();
        Ok(rows)
    }

    pub async fn resolve_owned_autopilot(
        &self,
        owner_user_id: &str,
        reference: &str,
    ) -> Result<AutopilotAggregate, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;
        let reference = normalize_non_empty(reference, "reference")?;

        let state = self.state.read().await;
        let autopilot = resolve_owned_autopilot_record(&state, &owner_user_id, &reference)?;
        Ok(build_autopilot_aggregate(&state, autopilot))
    }

    pub async fn resolve_autopilot_filter_for_owner(
        &self,
        owner_user_id: &str,
        reference: &str,
    ) -> Result<AutopilotRecord, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;
        let reference = normalize_non_empty(reference, "reference")?;
        let reference_normalized = reference.to_lowercase();

        let state = self.state.read().await;
        let autopilot = state
            .autopilots
            .values()
            .find(|record| {
                record.deleted_at.is_none()
                    && (record.id == reference
                        || record.handle.eq_ignore_ascii_case(&reference_normalized))
            })
            .cloned()
            .ok_or(DomainStoreError::NotFound)?;

        if autopilot.owner_user_id != owner_user_id {
            return Err(DomainStoreError::Forbidden);
        }

        Ok(autopilot)
    }

    pub async fn update_owned_autopilot(
        &self,
        owner_user_id: &str,
        reference: &str,
        update: UpdateAutopilotInput,
    ) -> Result<AutopilotAggregate, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;
        let reference = normalize_non_empty(reference, "reference")?;

        self.mutate(|state| {
            let autopilot_id = resolve_owned_autopilot_id(state, &owner_user_id, &reference)?;
            let now = Utc::now();

            let autopilot = state
                .autopilots
                .get_mut(&autopilot_id)
                .ok_or(DomainStoreError::NotFound)?;

            if let Some(display_name) = update.display_name.as_deref() {
                autopilot.display_name = normalize_display_name(display_name);
            }
            if update.avatar.is_some() {
                autopilot.avatar = normalize_optional_string(update.avatar.as_deref());
            }
            if let Some(status) = update.status.as_deref() {
                autopilot.status = normalize_status_or_default(Some(status), "active");
            }
            if let Some(visibility) = update.visibility.as_deref() {
                autopilot.visibility = normalize_status_or_default(Some(visibility), "private");
            }
            if update.tagline.is_some() {
                autopilot.tagline = normalize_optional_string(update.tagline.as_deref());
            }

            let mut increment_config = false;

            if let Some(profile_update) = update.profile {
                let profile = state
                    .autopilot_profiles
                    .entry(autopilot_id.clone())
                    .or_insert_with(|| AutopilotProfileRecord {
                        autopilot_id: autopilot_id.clone(),
                        owner_display_name: autopilot.display_name.clone(),
                        persona_summary: None,
                        autopilot_voice: None,
                        principles: None,
                        preferences: None,
                        onboarding_answers: Value::Array(Vec::new()),
                        schema_version: 1,
                        created_at: now,
                        updated_at: now,
                    });

                if let Some(value) = profile_update.owner_display_name {
                    profile.owner_display_name = normalize_owner_display_name(&value);
                }
                if profile_update.persona_summary.is_some() {
                    profile.persona_summary =
                        normalize_optional_string(profile_update.persona_summary.as_deref());
                }
                if profile_update.autopilot_voice.is_some() {
                    profile.autopilot_voice =
                        normalize_optional_string(profile_update.autopilot_voice.as_deref());
                }
                if profile_update.principles.is_some() {
                    profile.principles = profile_update.principles;
                }
                if profile_update.preferences.is_some() {
                    profile.preferences = profile_update.preferences;
                }
                if let Some(onboarding_answers) = profile_update.onboarding_answers {
                    profile.onboarding_answers = ensure_json_array(onboarding_answers);
                }
                if let Some(schema_version) = profile_update.schema_version {
                    profile.schema_version = schema_version.max(1);
                }
                profile.updated_at = now;
                increment_config = true;
            }

            if let Some(policy_update) = update.policy {
                let policy = state
                    .autopilot_policies
                    .entry(autopilot_id.clone())
                    .or_insert_with(|| AutopilotPolicyRecord {
                        autopilot_id: autopilot_id.clone(),
                        model_provider: None,
                        model: None,
                        tool_allowlist: Vec::new(),
                        tool_denylist: Vec::new(),
                        l402_require_approval: true,
                        l402_max_spend_msats_per_call: None,
                        l402_max_spend_msats_per_day: None,
                        l402_allowed_hosts: Vec::new(),
                        data_policy: None,
                        created_at: now,
                        updated_at: now,
                    });

                if policy_update.model_provider.is_some() {
                    policy.model_provider =
                        normalize_optional_string(policy_update.model_provider.as_deref());
                }
                if policy_update.model.is_some() {
                    policy.model = normalize_optional_string(policy_update.model.as_deref());
                }
                if let Some(tool_allowlist) = policy_update.tool_allowlist {
                    policy.tool_allowlist = normalize_string_vec(tool_allowlist);
                }
                if let Some(tool_denylist) = policy_update.tool_denylist {
                    policy.tool_denylist = normalize_string_vec(tool_denylist);
                }
                if let Some(require_approval) = policy_update.l402_require_approval {
                    policy.l402_require_approval = require_approval;
                }
                if policy_update.l402_max_spend_msats_per_call.is_some() {
                    policy.l402_max_spend_msats_per_call =
                        policy_update.l402_max_spend_msats_per_call;
                }
                if policy_update.l402_max_spend_msats_per_day.is_some() {
                    policy.l402_max_spend_msats_per_day =
                        policy_update.l402_max_spend_msats_per_day;
                }
                if let Some(allowed_hosts) = policy_update.l402_allowed_hosts {
                    policy.l402_allowed_hosts = normalize_string_vec(allowed_hosts);
                }
                if policy_update.data_policy.is_some() {
                    policy.data_policy = policy_update.data_policy;
                }
                policy.updated_at = now;
                increment_config = true;
            }

            if increment_config {
                autopilot.config_version = autopilot.config_version.saturating_add(1);
            }
            autopilot.updated_at = now;

            let snapshot = autopilot.clone();
            Ok(build_autopilot_aggregate(state, snapshot))
        })
        .await
    }

    pub async fn upsert_autopilot_runtime_binding(
        &self,
        owner_user_id: &str,
        input: UpsertAutopilotRuntimeBindingInput,
    ) -> Result<AutopilotRuntimeBindingRecord, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;
        let autopilot_id = normalize_non_empty(&input.autopilot_id, "autopilot_id")?;
        let runtime_type = normalize_non_empty(&input.runtime_type, "runtime_type")?;

        self.mutate(|state| {
            let autopilot = state
                .autopilots
                .get(&autopilot_id)
                .ok_or(DomainStoreError::NotFound)?;
            if autopilot.owner_user_id != owner_user_id {
                return Err(DomainStoreError::Forbidden);
            }

            let now = Utc::now();
            let existing_id = state
                .autopilot_runtime_bindings
                .iter()
                .find_map(|(id, binding)| {
                    if binding.autopilot_id == autopilot_id && binding.runtime_type == runtime_type
                    {
                        Some(id.clone())
                    } else {
                        None
                    }
                });

            if input.is_primary {
                for binding in state.autopilot_runtime_bindings.values_mut() {
                    if binding.autopilot_id == autopilot_id && binding.runtime_type == runtime_type
                    {
                        binding.is_primary = false;
                        binding.updated_at = now;
                    }
                }
            }

            let binding = if let Some(binding_id) = existing_id {
                let existing = state
                    .autopilot_runtime_bindings
                    .get_mut(&binding_id)
                    .ok_or(DomainStoreError::NotFound)?;
                existing.runtime_ref = normalize_optional_string(input.runtime_ref.as_deref());
                existing.is_primary = input.is_primary;
                existing.last_seen_at = input.last_seen_at.or(existing.last_seen_at);
                if input.meta.is_some() {
                    existing.meta = input.meta;
                }
                existing.updated_at = now;
                existing.clone()
            } else {
                let record = AutopilotRuntimeBindingRecord {
                    id: format!("arb_{}", Uuid::new_v4().simple()),
                    autopilot_id: autopilot_id.clone(),
                    runtime_type,
                    runtime_ref: normalize_optional_string(input.runtime_ref.as_deref()),
                    is_primary: input.is_primary,
                    last_seen_at: input.last_seen_at,
                    meta: input.meta,
                    created_at: now,
                    updated_at: now,
                };
                state
                    .autopilot_runtime_bindings
                    .insert(record.id.clone(), record.clone());
                record
            };

            Ok(binding)
        })
        .await
    }

    pub async fn find_primary_autopilot_binding_driver(
        &self,
        autopilot_id: &str,
    ) -> Result<Option<String>, DomainStoreError> {
        let autopilot_id = normalize_non_empty(autopilot_id, "autopilot_id")?;
        let state = self.state.read().await;
        let runtime_type = state
            .autopilot_runtime_bindings
            .values()
            .find(|binding| binding.autopilot_id == autopilot_id && binding.is_primary)
            .map(|binding| binding.runtime_type.trim().to_ascii_lowercase());

        let Some(runtime_type) = runtime_type else {
            return Ok(None);
        };

        let driver = match runtime_type.as_str() {
            "elixir" | "runtime" => Some("elixir".to_string()),
            "legacy" | "laravel" | "openagents.com" => Some("legacy".to_string()),
            _ => None,
        };
        Ok(driver)
    }

    pub async fn upsert_runtime_driver_override(
        &self,
        input: UpsertRuntimeDriverOverrideInput,
    ) -> Result<RuntimeDriverOverrideRecord, DomainStoreError> {
        let scope_type = normalize_runtime_scope_type(&input.scope_type)?;
        let scope_id = normalize_non_empty(&input.scope_id, "scope_id")?;
        let driver = normalize_runtime_driver(&input.driver)?;
        let reason = normalize_optional_string(input.reason.as_deref());
        let key = runtime_override_key(&scope_type, &scope_id);

        self.mutate(|state| {
            let now = Utc::now();
            let row = state
                .runtime_driver_overrides
                .entry(key.clone())
                .or_insert_with(|| RuntimeDriverOverrideRecord {
                    id: format!("rdo_{}", Uuid::new_v4().simple()),
                    scope_type: scope_type.clone(),
                    scope_id: scope_id.clone(),
                    driver: driver.clone(),
                    is_active: input.is_active,
                    reason: reason.clone(),
                    meta: input.meta.clone(),
                    created_at: now,
                    updated_at: now,
                });

            row.driver = driver;
            row.is_active = input.is_active;
            row.reason = reason;
            if input.meta.is_some() {
                row.meta = input.meta;
            }
            row.updated_at = now;

            Ok(row.clone())
        })
        .await
    }

    pub async fn find_active_runtime_driver_override(
        &self,
        scope_type: &str,
        scope_id: &str,
    ) -> Result<Option<RuntimeDriverOverrideRecord>, DomainStoreError> {
        let scope_type = normalize_runtime_scope_type(scope_type)?;
        let scope_id = normalize_non_empty(scope_id, "scope_id")?;
        let key = runtime_override_key(&scope_type, &scope_id);
        let state = self.state.read().await;
        let row = state.runtime_driver_overrides.get(&key).cloned();
        let Some(row) = row else {
            return Ok(None);
        };

        if !row.is_active {
            return Ok(None);
        }
        if normalize_runtime_driver(&row.driver).is_err() {
            return Ok(None);
        }

        Ok(Some(row))
    }

    pub async fn list_runtime_driver_overrides(
        &self,
    ) -> Result<Vec<RuntimeDriverOverrideRecord>, DomainStoreError> {
        let state = self.state.read().await;
        let mut rows: Vec<RuntimeDriverOverrideRecord> =
            state.runtime_driver_overrides.values().cloned().collect();
        rows.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(rows)
    }

    pub async fn upsert_l402_credential(
        &self,
        input: UpsertL402CredentialInput,
    ) -> Result<L402CredentialRecord, DomainStoreError> {
        let host = normalize_non_empty(&input.host, "host")?.to_lowercase();
        let scope = normalize_non_empty(&input.scope, "scope")?;
        let macaroon = normalize_non_empty(&input.macaroon, "macaroon")?;
        let preimage = normalize_non_empty(&input.preimage, "preimage")?;

        self.mutate(|state| {
            let now = Utc::now();
            let key = credential_key(&host, &scope);

            let row = state
                .l402_credentials
                .entry(key)
                .or_insert_with(|| L402CredentialRecord {
                    host: host.clone(),
                    scope: scope.clone(),
                    macaroon: macaroon.clone(),
                    preimage: preimage.clone(),
                    expires_at: input.expires_at,
                    created_at: now,
                    updated_at: now,
                });

            row.macaroon = macaroon;
            row.preimage = preimage;
            row.expires_at = input.expires_at;
            row.updated_at = now;

            Ok(row.clone())
        })
        .await
    }

    pub async fn list_active_l402_credentials(
        &self,
    ) -> Result<Vec<L402CredentialRecord>, DomainStoreError> {
        let now = Utc::now();
        let state = self.state.read().await;
        let mut rows: Vec<L402CredentialRecord> = state
            .l402_credentials
            .values()
            .filter(|row| row.expires_at > now)
            .cloned()
            .collect();
        rows.sort_by(|left, right| {
            left.host
                .cmp(&right.host)
                .then(left.scope.cmp(&right.scope))
        });
        Ok(rows)
    }

    pub async fn record_l402_receipt(
        &self,
        input: RecordL402ReceiptInput,
    ) -> Result<L402ReceiptRecord, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let thread_id = normalize_non_empty(&input.thread_id, "thread_id")?;
        let run_id = normalize_non_empty(&input.run_id, "run_id")?;
        let autopilot_id = normalize_optional_string(input.autopilot_id.as_deref());
        let thread_title = normalize_optional_string(input.thread_title.as_deref());
        let run_status = normalize_optional_string(input.run_status.as_deref());

        self.mutate(|state| {
            let now = Utc::now();
            let created_at = input.created_at.unwrap_or(now);
            let id = state.next_l402_receipt_id.max(1);
            state.next_l402_receipt_id = id.saturating_add(1);

            let row = L402ReceiptRecord {
                id,
                user_id,
                thread_id,
                run_id,
                autopilot_id,
                thread_title,
                run_status,
                run_started_at: input.run_started_at,
                run_completed_at: input.run_completed_at,
                payload: input.payload,
                created_at,
                updated_at: now,
            };

            state.l402_receipts.push(row.clone());
            Ok(row)
        })
        .await
    }

    pub async fn list_l402_receipts_for_user(
        &self,
        user_id: &str,
        autopilot_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<L402ReceiptRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let autopilot_id = autopilot_id
            .and_then(|value| normalize_optional_string(Some(value)))
            .map(|value| value.to_lowercase());
        let safe_limit = limit.clamp(1, 1000);

        let state = self.state.read().await;
        let mut rows: Vec<L402ReceiptRecord> = state
            .l402_receipts
            .iter()
            .filter(|row| row.user_id == user_id)
            .filter(|row| {
                autopilot_id
                    .as_deref()
                    .map(|needle| {
                        row.autopilot_id
                            .as_deref()
                            .map(|value| value.eq_ignore_ascii_case(needle))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        rows.sort_by(|left, right| right.id.cmp(&left.id));

        let start = offset.min(rows.len());
        let end = start.saturating_add(safe_limit).min(rows.len());
        Ok(rows[start..end].to_vec())
    }

    pub async fn count_l402_receipts_for_user(
        &self,
        user_id: &str,
        autopilot_id: Option<&str>,
    ) -> Result<u64, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let autopilot_id = autopilot_id
            .and_then(|value| normalize_optional_string(Some(value)))
            .map(|value| value.to_lowercase());

        let state = self.state.read().await;
        let count = state
            .l402_receipts
            .iter()
            .filter(|row| row.user_id == user_id)
            .filter(|row| {
                autopilot_id
                    .as_deref()
                    .map(|needle| {
                        row.autopilot_id
                            .as_deref()
                            .map(|value| value.eq_ignore_ascii_case(needle))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .count() as u64;
        Ok(count)
    }

    pub async fn find_l402_receipt_for_user(
        &self,
        user_id: &str,
        event_id: u64,
    ) -> Result<Option<L402ReceiptRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let state = self.state.read().await;
        Ok(state
            .l402_receipts
            .iter()
            .find(|row| row.user_id == user_id && row.id == event_id)
            .cloned())
    }

    pub async fn record_l402_gateway_event(
        &self,
        input: RecordL402GatewayEventInput,
    ) -> Result<L402GatewayEventRecord, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let autopilot_id = normalize_optional_string(input.autopilot_id.as_deref());
        let event_type = normalize_non_empty(&input.event_type, "event_type")?;

        self.mutate(|state| {
            let now = Utc::now();
            let created_at = input.created_at.unwrap_or(now);
            let id = state.next_l402_gateway_event_id.max(1);
            state.next_l402_gateway_event_id = id.saturating_add(1);

            let row = L402GatewayEventRecord {
                id,
                user_id,
                autopilot_id,
                event_type,
                payload: input.payload,
                created_at,
                updated_at: now,
            };

            state.l402_gateway_events.push(row.clone());
            Ok(row)
        })
        .await
    }

    pub async fn list_l402_gateway_events_for_user(
        &self,
        user_id: &str,
        autopilot_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<L402GatewayEventRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let autopilot_id = autopilot_id
            .and_then(|value| normalize_optional_string(Some(value)))
            .map(|value| value.to_lowercase());
        let safe_limit = limit.clamp(1, 1000);

        let state = self.state.read().await;
        let mut rows: Vec<L402GatewayEventRecord> = state
            .l402_gateway_events
            .iter()
            .filter(|row| row.user_id == user_id)
            .filter(|row| {
                autopilot_id
                    .as_deref()
                    .map(|needle| {
                        row.autopilot_id
                            .as_deref()
                            .map(|value| value.eq_ignore_ascii_case(needle))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        rows.sort_by(|left, right| right.id.cmp(&left.id));
        rows.truncate(safe_limit);
        Ok(rows)
    }

    pub async fn create_l402_paywall(
        &self,
        input: CreateL402PaywallInput,
    ) -> Result<L402PaywallRecord, DomainStoreError> {
        let owner_user_id = normalize_non_empty(&input.owner_user_id, "owner_user_id")?;
        let name = normalize_non_empty(&input.name, "name")?;
        let host_regexp = normalize_non_empty(&input.host_regexp, "host_regexp")?;
        let path_regexp = normalize_non_empty(&input.path_regexp, "path_regexp")?;
        let upstream = normalize_non_empty(&input.upstream, "upstream")?;

        self.mutate(|state| {
            let now = Utc::now();
            let row = L402PaywallRecord {
                id: format!("pw_{}", Uuid::new_v4().simple()),
                owner_user_id,
                name,
                host_regexp,
                path_regexp,
                price_msats: input.price_msats,
                upstream,
                enabled: input.enabled.unwrap_or(true),
                meta: input.meta,
                last_reconcile_status: None,
                last_reconcile_error: None,
                last_reconciled_at: None,
                deleted_at: None,
                created_at: now,
                updated_at: now,
            };

            state.l402_paywalls.insert(row.id.clone(), row.clone());
            Ok(row)
        })
        .await
    }

    pub async fn update_owned_l402_paywall(
        &self,
        owner_user_id: &str,
        paywall_id: &str,
        update: UpdateL402PaywallInput,
    ) -> Result<L402PaywallRecord, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;
        let paywall_id = normalize_non_empty(paywall_id, "paywall_id")?;

        self.mutate(|state| {
            let now = Utc::now();
            let paywall = state
                .l402_paywalls
                .get_mut(&paywall_id)
                .ok_or(DomainStoreError::NotFound)?;

            if paywall.owner_user_id != owner_user_id {
                return Err(DomainStoreError::Forbidden);
            }
            if paywall.deleted_at.is_some() {
                return Err(DomainStoreError::Conflict {
                    message: "cannot update a deleted paywall".to_string(),
                });
            }

            if let Some(value) = update.name {
                paywall.name = normalize_non_empty(&value, "name")?;
            }
            if let Some(value) = update.host_regexp {
                paywall.host_regexp = normalize_non_empty(&value, "host_regexp")?;
            }
            if let Some(value) = update.path_regexp {
                paywall.path_regexp = normalize_non_empty(&value, "path_regexp")?;
            }
            if let Some(value) = update.price_msats {
                paywall.price_msats = value;
            }
            if let Some(value) = update.upstream {
                paywall.upstream = normalize_non_empty(&value, "upstream")?;
            }
            if let Some(value) = update.enabled {
                paywall.enabled = value;
            }
            if update.meta.is_some() {
                paywall.meta = update.meta;
            }
            if update.last_reconcile_status.is_some() {
                paywall.last_reconcile_status =
                    normalize_optional_string(update.last_reconcile_status.as_deref());
            }
            if update.last_reconcile_error.is_some() {
                paywall.last_reconcile_error =
                    normalize_optional_string(update.last_reconcile_error.as_deref());
            }
            if update.last_reconciled_at.is_some() {
                paywall.last_reconciled_at = update.last_reconciled_at;
            }
            paywall.updated_at = now;

            Ok(paywall.clone())
        })
        .await
    }

    pub async fn soft_delete_owned_l402_paywall(
        &self,
        owner_user_id: &str,
        paywall_id: &str,
    ) -> Result<L402PaywallRecord, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;
        let paywall_id = normalize_non_empty(paywall_id, "paywall_id")?;

        self.mutate(|state| {
            let now = Utc::now();
            let paywall = state
                .l402_paywalls
                .get_mut(&paywall_id)
                .ok_or(DomainStoreError::NotFound)?;

            if paywall.owner_user_id != owner_user_id {
                return Err(DomainStoreError::Forbidden);
            }

            if paywall.deleted_at.is_none() {
                paywall.deleted_at = Some(now);
            }
            paywall.updated_at = now;
            Ok(paywall.clone())
        })
        .await
    }

    pub async fn list_l402_paywalls_for_owner(
        &self,
        owner_user_id: &str,
        include_deleted: bool,
    ) -> Result<Vec<L402PaywallRecord>, DomainStoreError> {
        let owner_user_id = normalize_non_empty(owner_user_id, "owner_user_id")?;

        let state = self.state.read().await;
        let mut rows: Vec<L402PaywallRecord> = state
            .l402_paywalls
            .values()
            .filter(|row| row.owner_user_id == owner_user_id)
            .filter(|row| include_deleted || row.deleted_at.is_none())
            .cloned()
            .collect();
        rows.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(rows)
    }

    pub async fn list_all_l402_paywalls(
        &self,
        include_deleted: bool,
    ) -> Result<Vec<L402PaywallRecord>, DomainStoreError> {
        let state = self.state.read().await;
        let mut rows: Vec<L402PaywallRecord> = state
            .l402_paywalls
            .values()
            .filter(|row| include_deleted || row.deleted_at.is_none())
            .cloned()
            .collect();
        rows.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(rows)
    }

    pub async fn upsert_user_spark_wallet(
        &self,
        input: UpsertUserSparkWalletInput,
    ) -> Result<UserSparkWalletRecord, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let wallet_id = normalize_non_empty(&input.wallet_id, "wallet_id")?;
        let mnemonic = normalize_non_empty(&input.mnemonic, "mnemonic")?;

        self.mutate(|state| {
            let now = Utc::now();
            let row = state
                .user_spark_wallets
                .entry(user_id.clone())
                .or_insert_with(|| UserSparkWalletRecord {
                    user_id: user_id.clone(),
                    wallet_id: wallet_id.clone(),
                    mnemonic: mnemonic.clone(),
                    spark_address: normalize_optional_string(input.spark_address.as_deref()),
                    lightning_address: normalize_optional_string(
                        input.lightning_address.as_deref(),
                    ),
                    identity_pubkey: normalize_optional_string(input.identity_pubkey.as_deref()),
                    last_balance_sats: input.last_balance_sats,
                    status: normalize_status_or_default(input.status.as_deref(), "active"),
                    provider: normalize_status_or_default(
                        input.provider.as_deref(),
                        "spark_executor",
                    ),
                    last_error: normalize_optional_string(input.last_error.as_deref()),
                    meta: input.meta.clone(),
                    last_synced_at: input.last_synced_at,
                    created_at: now,
                    updated_at: now,
                });

            row.wallet_id = wallet_id;
            row.mnemonic = mnemonic;
            row.spark_address = normalize_optional_string(input.spark_address.as_deref());
            row.lightning_address = normalize_optional_string(input.lightning_address.as_deref());
            row.identity_pubkey = normalize_optional_string(input.identity_pubkey.as_deref());
            row.last_balance_sats = input.last_balance_sats;
            if let Some(status) = input.status.as_deref() {
                row.status = normalize_status_or_default(Some(status), "active");
            }
            if let Some(provider) = input.provider.as_deref() {
                row.provider = normalize_status_or_default(Some(provider), "spark_executor");
            }
            if input.last_error.is_some() {
                row.last_error = normalize_optional_string(input.last_error.as_deref());
            }
            if input.meta.is_some() {
                row.meta = input.meta;
            }
            if input.last_synced_at.is_some() {
                row.last_synced_at = input.last_synced_at;
            }
            row.updated_at = now;

            Ok(row.clone())
        })
        .await
    }

    pub async fn find_user_spark_wallet(
        &self,
        user_id: &str,
    ) -> Result<Option<UserSparkWalletRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let state = self.state.read().await;
        Ok(state.user_spark_wallets.get(&user_id).cloned())
    }

    pub async fn upsert_resend_integration(
        &self,
        input: UpsertResendIntegrationInput,
    ) -> Result<IntegrationUpsertResult, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let api_key = normalize_non_empty(&input.api_key, "api_key")?;
        let sender_email = normalize_optional_string(input.sender_email.as_deref());
        let sender_name = normalize_optional_string(input.sender_name.as_deref());
        let fingerprint = sha256_hex(&api_key);
        let secret_last4 = Some(last4(&api_key));
        let integration_secret_cipher = self.integration_secret_cipher.clone();

        self.mutate(move |state| {
            let mut metadata = serde_json::Map::new();
            metadata.insert(
                "sender_email".to_string(),
                sender_email
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );
            metadata.insert(
                "sender_name".to_string(),
                sender_name
                    .clone()
                    .map(Value::String)
                    .unwrap_or(Value::Null),
            );

            let secret = encrypt_integration_secret(&api_key, integration_secret_cipher.as_ref())?;
            let result = upsert_integration_secret(
                state,
                &user_id,
                "resend",
                secret,
                fingerprint,
                secret_last4,
                Some(Value::Object(metadata)),
            );

            append_integration_audit(
                state,
                &user_id,
                Some(result.integration.id),
                "resend",
                &result.action,
                result.integration.metadata.clone(),
            );

            Ok(result)
        })
        .await
    }

    pub async fn upsert_google_integration(
        &self,
        input: UpsertGoogleIntegrationInput,
    ) -> Result<IntegrationUpsertResult, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let integration_secret_cipher = self.integration_secret_cipher.clone();

        self.mutate(move |state| {
            let key = integration_key(&user_id, "google");
            let existing = state.user_integrations.get(&key).cloned();
            let existing_payload = existing
                .as_ref()
                .and_then(|row| row.encrypted_secret.as_ref())
                .and_then(|value| {
                    decrypt_integration_secret(value, integration_secret_cipher.as_ref()).ok()
                })
                .and_then(|value| serde_json::from_str::<Value>(value.as_str()).ok())
                .unwrap_or(Value::Null);

            let refresh_token = input
                .refresh_token
                .clone()
                .or_else(|| {
                    existing_payload
                        .get("refresh_token")
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string)
                })
                .ok_or(DomainStoreError::Validation {
                    field: "refresh_token",
                    message: "Google token response did not include refresh token.".to_string(),
                })?;

            let access_token = input.access_token.clone().or_else(|| {
                existing_payload
                    .get("access_token")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string)
            });
            let scope = input.scope.clone().or_else(|| {
                existing_payload
                    .get("scope")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string)
            });
            let token_type = input.token_type.clone().or_else(|| {
                existing_payload
                    .get("token_type")
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string)
            });
            let expires_at = input.expires_at.or_else(|| {
                existing_payload
                    .get("expires_at")
                    .and_then(|value| value.as_str())
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| value.with_timezone(&Utc))
            });

            let secret_payload = serde_json::json!({
                "provider": "google",
                "integration_id": "gmail.primary",
                "refresh_token": refresh_token,
                "access_token": access_token,
                "scope": scope,
                "token_type": token_type,
                "expires_at": expires_at,
                "obtained_at": Utc::now().to_rfc3339(),
            });
            let secret_plain = serde_json::to_string(&secret_payload).map_err(|error| {
                DomainStoreError::Persistence {
                    message: format!("failed to encode google secret payload: {error}"),
                }
            })?;
            let secret = encrypt_integration_secret(
                secret_plain.as_str(),
                integration_secret_cipher.as_ref(),
            )?;

            let refresh_token = secret_payload
                .get("refresh_token")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let fingerprint = sha256_hex(&refresh_token);
            let secret_last4 = Some(last4(&refresh_token));

            let mut metadata = serde_json::Map::new();
            metadata.insert(
                "integration_id".to_string(),
                Value::String("gmail.primary".to_string()),
            );
            metadata.insert(
                "scope".to_string(),
                secret_payload.get("scope").cloned().unwrap_or(Value::Null),
            );
            metadata.insert(
                "token_type".to_string(),
                secret_payload
                    .get("token_type")
                    .cloned()
                    .unwrap_or(Value::Null),
            );
            metadata.insert(
                "expires_at".to_string(),
                secret_payload
                    .get("expires_at")
                    .cloned()
                    .unwrap_or(Value::Null),
            );

            let result = upsert_integration_secret(
                state,
                &user_id,
                "google",
                secret,
                fingerprint,
                secret_last4,
                Some(Value::Object(metadata)),
            );

            append_integration_audit(
                state,
                &user_id,
                Some(result.integration.id),
                "google",
                &result.action,
                result.integration.metadata.clone(),
            );

            Ok(result)
        })
        .await
    }

    pub async fn revoke_integration(
        &self,
        user_id: &str,
        provider: &str,
    ) -> Result<Option<UserIntegrationRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let provider = normalize_non_empty(provider, "provider")?.to_lowercase();

        self.mutate(|state| {
            let key = integration_key(&user_id, &provider);
            let Some(updated) = ({
                let Some(integration) = state.user_integrations.get_mut(&key) else {
                    return Ok(None);
                };

                integration.status = "inactive".to_string();
                integration.encrypted_secret = None;
                integration.secret_fingerprint = None;
                integration.secret_last4 = None;
                integration.disconnected_at = Some(Utc::now());
                integration.updated_at = Utc::now();
                Some(integration.clone())
            }) else {
                return Ok(None);
            };

            append_integration_audit(
                state,
                &user_id,
                Some(updated.id),
                &provider,
                "secret_revoked",
                Some(serde_json::json!({ "status": "inactive" })),
            );

            Ok(Some(updated))
        })
        .await
    }

    pub async fn audit_integration_test_request(
        &self,
        user_id: &str,
        provider: &str,
    ) -> Result<Option<UserIntegrationRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let provider = normalize_non_empty(provider, "provider")?.to_lowercase();

        self.mutate(|state| {
            let key = integration_key(&user_id, &provider);
            let Some(integration) = state.user_integrations.get(&key).cloned() else {
                return Ok(None);
            };

            append_integration_audit(
                state,
                &user_id,
                Some(integration.id),
                &provider,
                "test_requested",
                Some(serde_json::json!({
                    "status": integration.status,
                    "secret_last4": integration.secret_last4,
                })),
            );
            Ok(Some(integration))
        })
        .await
    }

    pub async fn find_active_integration_secret(
        &self,
        user_id: &str,
        provider: &str,
    ) -> Result<Option<UserIntegrationRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let provider = normalize_non_empty(provider, "provider")?.to_lowercase();
        let key = integration_key(&user_id, &provider);

        let state = self.state.read().await;
        let row = state.user_integrations.get(&key).cloned();
        let Some(row) = row else {
            return Ok(None);
        };

        if row.status != "active" {
            return Ok(None);
        }
        if row
            .encrypted_secret
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_none()
        {
            return Ok(None);
        }

        let mut row = row;
        let mut needs_migration = false;
        let mut migrated_secret: Option<String> = None;
        if let Some(stored_secret) = row.encrypted_secret.clone() {
            let decrypted = decrypt_integration_secret(
                stored_secret.as_str(),
                self.integration_secret_cipher.as_ref(),
            )?;
            if self.integration_secret_cipher.is_some()
                && !is_encrypted_integration_secret(stored_secret.as_str())
            {
                needs_migration = true;
                migrated_secret = Some(encrypt_integration_secret(
                    decrypted.as_str(),
                    self.integration_secret_cipher.as_ref(),
                )?);
            }
            row.encrypted_secret = Some(decrypted);
        }
        drop(state);

        if needs_migration {
            let key_for_update = key.clone();
            let new_secret = migrated_secret.unwrap_or_default();
            self.mutate(move |state| {
                if let Some(integration) = state.user_integrations.get_mut(&key_for_update) {
                    integration.encrypted_secret = Some(new_secret.clone());
                    integration.updated_at = Utc::now();
                }
                Ok(())
            })
            .await?;
        }

        Ok(Some(row))
    }

    pub async fn upsert_inbox_thread_state(
        &self,
        input: UpsertInboxThreadStateInput,
    ) -> Result<InboxThreadStateRecord, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let thread_id = normalize_non_empty(&input.thread_id, "thread_id")?;
        let decision = normalize_optional_string(input.decision.as_deref());
        let draft_preview = normalize_optional_string(input.draft_preview.as_deref());
        let source = normalize_optional_string(input.source.as_deref())
            .unwrap_or_else(|| "gmail_adapter".to_string());

        self.mutate(|state| {
            let now = Utc::now();
            let key = inbox_thread_state_key(&user_id, &thread_id);

            let row = state.inbox_thread_states.entry(key).or_insert_with(|| {
                let id = state.next_inbox_thread_state_id;
                state.next_inbox_thread_state_id =
                    state.next_inbox_thread_state_id.saturating_add(1);
                InboxThreadStateRecord {
                    id,
                    user_id: user_id.clone(),
                    thread_id: thread_id.clone(),
                    pending_approval: input.pending_approval,
                    decision: None,
                    draft_preview: None,
                    source: source.clone(),
                    created_at: now,
                    updated_at: now,
                }
            });

            row.pending_approval = input.pending_approval;
            row.decision = decision;
            row.draft_preview = draft_preview;
            row.source = source;
            row.updated_at = now;

            Ok(row.clone())
        })
        .await
    }

    pub async fn inbox_thread_state(
        &self,
        user_id: &str,
        thread_id: &str,
    ) -> Result<Option<InboxThreadStateRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let thread_id = normalize_non_empty(thread_id, "thread_id")?;
        let key = inbox_thread_state_key(&user_id, &thread_id);
        let state = self.state.read().await;
        Ok(state.inbox_thread_states.get(&key).cloned())
    }

    pub async fn list_inbox_thread_states_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<InboxThreadStateRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let state = self.state.read().await;
        let mut rows: Vec<InboxThreadStateRecord> = state
            .inbox_thread_states
            .values()
            .filter(|row| row.user_id == user_id)
            .cloned()
            .collect();
        rows.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(rows)
    }

    pub async fn record_inbox_audit(
        &self,
        input: RecordInboxAuditInput,
    ) -> Result<InboxAuditRecord, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let thread_id = normalize_non_empty(&input.thread_id, "thread_id")?;
        let action = normalize_non_empty(&input.action, "action")?;
        let detail = normalize_non_empty(&input.detail, "detail")?;

        self.mutate(|state| {
            let now = Utc::now();
            let row = InboxAuditRecord {
                id: state.next_inbox_audit_id,
                user_id,
                thread_id,
                action,
                detail,
                created_at: now,
                updated_at: now,
            };
            state.next_inbox_audit_id = state.next_inbox_audit_id.saturating_add(1);
            state.inbox_audits.push(row.clone());
            Ok(row)
        })
        .await
    }

    pub async fn list_inbox_audits_for_user(
        &self,
        user_id: &str,
        thread_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<InboxAuditRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let thread_filter = normalize_optional_string(thread_id);
        let safe_limit = limit.clamp(1, 200);

        let state = self.state.read().await;
        let mut rows: Vec<InboxAuditRecord> = state
            .inbox_audits
            .iter()
            .filter(|row| row.user_id == user_id)
            .filter(|row| {
                thread_filter
                    .as_ref()
                    .map(|thread| row.thread_id == *thread)
                    .unwrap_or(true)
            })
            .cloned()
            .collect();
        rows.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        rows.truncate(safe_limit);
        Ok(rows)
    }

    pub async fn list_integrations_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserIntegrationRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;

        let state = self.state.read().await;
        let mut rows: Vec<UserIntegrationRecord> = state
            .user_integrations
            .values()
            .filter(|row| row.user_id == user_id)
            .cloned()
            .collect();
        rows.sort_by(|left, right| left.provider.cmp(&right.provider));
        Ok(rows)
    }

    pub async fn list_integration_audits_for_user(
        &self,
        user_id: &str,
    ) -> Result<Vec<UserIntegrationAuditRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;

        let state = self.state.read().await;
        let mut rows: Vec<UserIntegrationAuditRecord> = state
            .user_integration_audits
            .iter()
            .filter(|row| row.user_id == user_id)
            .cloned()
            .collect();
        rows.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(rows)
    }

    pub async fn record_webhook_event(
        &self,
        input: RecordWebhookEventInput,
    ) -> Result<RecordWebhookEventResult, DomainStoreError> {
        let provider = normalize_non_empty(&input.provider, "provider")?.to_lowercase();
        let idempotency_key = normalize_non_empty(&input.idempotency_key, "idempotency_key")?;

        self.mutate(|state| {
            if let Some(existing) = state.comms_webhook_events.get(&idempotency_key) {
                return Ok(RecordWebhookEventResult {
                    event: existing.clone(),
                    inserted: false,
                });
            }

            let now = Utc::now();
            let id = state.next_comms_webhook_event_id;
            state.next_comms_webhook_event_id = state.next_comms_webhook_event_id.saturating_add(1);

            let event = CommsWebhookEventRecord {
                id,
                provider,
                idempotency_key: idempotency_key.clone(),
                external_event_id: normalize_optional_string(input.external_event_id.as_deref()),
                event_type: normalize_optional_string(input.event_type.as_deref()),
                delivery_state: normalize_optional_string(input.delivery_state.as_deref()),
                message_id: normalize_optional_string(input.message_id.as_deref()),
                integration_id: normalize_optional_string(input.integration_id.as_deref()),
                user_id: normalize_optional_string(input.user_id.as_deref()),
                recipient: normalize_optional_string(input.recipient.as_deref()),
                signature_valid: input.signature_valid,
                status: normalize_status_or_default(input.status.as_deref(), "received"),
                normalized_hash: input
                    .normalized_payload
                    .as_ref()
                    .map(|payload| sha256_hex(&payload.to_string())),
                runtime_attempts: 0,
                runtime_status_code: None,
                runtime_response: None,
                normalized_payload: input.normalized_payload,
                raw_payload: input.raw_payload,
                last_error: None,
                forwarded_at: None,
                created_at: now,
                updated_at: now,
            };

            state
                .comms_webhook_events
                .insert(idempotency_key, event.clone());

            Ok(RecordWebhookEventResult {
                event,
                inserted: true,
            })
        })
        .await
    }

    pub async fn webhook_event_by_idempotency_key(
        &self,
        idempotency_key: &str,
    ) -> Result<Option<CommsWebhookEventRecord>, DomainStoreError> {
        let idempotency_key = normalize_non_empty(idempotency_key, "idempotency_key")?;
        let state = self.state.read().await;
        Ok(state.comms_webhook_events.get(&idempotency_key).cloned())
    }

    pub async fn webhook_event_by_id(
        &self,
        webhook_event_id: u64,
    ) -> Result<Option<CommsWebhookEventRecord>, DomainStoreError> {
        let state = self.state.read().await;
        Ok(state
            .comms_webhook_events
            .values()
            .find(|row| row.id == webhook_event_id)
            .cloned())
    }

    pub async fn upsert_invalid_webhook_event(
        &self,
        input: RecordWebhookEventInput,
    ) -> Result<CommsWebhookEventRecord, DomainStoreError> {
        let provider = normalize_non_empty(&input.provider, "provider")?.to_lowercase();
        let idempotency_key = normalize_non_empty(&input.idempotency_key, "idempotency_key")?;
        let external_event_id = normalize_optional_string(input.external_event_id.as_deref());
        let raw_payload = input.raw_payload.clone();

        self.mutate(|state| {
            let now = Utc::now();
            if let Some(existing) = state.comms_webhook_events.get_mut(&idempotency_key) {
                existing.signature_valid = false;
                existing.status = "invalid_signature".to_string();
                existing.raw_payload = raw_payload.clone();
                existing.last_error = Some("invalid_signature".to_string());
                existing.updated_at = now;
                return Ok(existing.clone());
            }

            let id = state.next_comms_webhook_event_id;
            state.next_comms_webhook_event_id = state.next_comms_webhook_event_id.saturating_add(1);

            let event = CommsWebhookEventRecord {
                id,
                provider,
                idempotency_key: idempotency_key.clone(),
                external_event_id,
                event_type: None,
                delivery_state: None,
                message_id: None,
                integration_id: None,
                user_id: None,
                recipient: None,
                signature_valid: false,
                status: "invalid_signature".to_string(),
                normalized_hash: None,
                runtime_attempts: 0,
                runtime_status_code: None,
                runtime_response: None,
                normalized_payload: None,
                raw_payload: raw_payload.clone(),
                last_error: Some("invalid_signature".to_string()),
                forwarded_at: None,
                created_at: now,
                updated_at: now,
            };

            state
                .comms_webhook_events
                .insert(idempotency_key, event.clone());

            Ok(event)
        })
        .await
    }

    pub async fn mark_webhook_event_verified(
        &self,
        input: MarkWebhookEventVerifiedInput,
    ) -> Result<Option<CommsWebhookEventRecord>, DomainStoreError> {
        let status = normalize_status_or_default(Some(input.status.as_str()), "received");

        self.mutate(|state| {
            let now = Utc::now();
            let Some(row) = state
                .comms_webhook_events
                .values_mut()
                .find(|row| row.id == input.webhook_event_id)
            else {
                return Ok(None);
            };

            row.signature_valid = true;
            row.status = status.clone();
            row.event_type = normalize_optional_string(input.event_type.as_deref());
            row.delivery_state = normalize_optional_string(input.delivery_state.as_deref());
            row.message_id = normalize_optional_string(input.message_id.as_deref());
            row.integration_id = normalize_optional_string(input.integration_id.as_deref());
            row.user_id = normalize_optional_string(input.user_id.as_deref());
            row.recipient = normalize_optional_string(input.recipient.as_deref());
            row.normalized_hash = normalize_optional_string(input.normalized_hash.as_deref());
            row.normalized_payload = input.normalized_payload.clone();
            row.raw_payload = input.raw_payload.clone();
            row.last_error = None;
            row.updated_at = now;

            Ok(Some(row.clone()))
        })
        .await
    }

    pub async fn mark_webhook_event_forwarding(
        &self,
        webhook_event_id: u64,
    ) -> Result<Option<CommsWebhookEventRecord>, DomainStoreError> {
        self.mutate(|state| {
            let now = Utc::now();
            let Some(row) = state
                .comms_webhook_events
                .values_mut()
                .find(|row| row.id == webhook_event_id)
            else {
                return Ok(None);
            };

            row.runtime_attempts = row.runtime_attempts.saturating_add(1);
            row.status = "forwarding".to_string();
            row.updated_at = now;
            Ok(Some(row.clone()))
        })
        .await
    }

    pub async fn mark_webhook_event_retrying(
        &self,
        webhook_event_id: u64,
        runtime_attempts: u32,
        runtime_status_code: Option<u16>,
        runtime_response: Option<Value>,
        last_error: Option<String>,
    ) -> Result<Option<CommsWebhookEventRecord>, DomainStoreError> {
        self.mutate(|state| {
            let now = Utc::now();
            let Some(row) = state
                .comms_webhook_events
                .values_mut()
                .find(|row| row.id == webhook_event_id)
            else {
                return Ok(None);
            };

            row.status = "forward_retrying".to_string();
            row.runtime_attempts = row.runtime_attempts.max(runtime_attempts.max(1));
            row.runtime_status_code = runtime_status_code;
            row.runtime_response = runtime_response.clone();
            row.last_error = normalize_optional_string(last_error.as_deref());
            row.updated_at = now;
            Ok(Some(row.clone()))
        })
        .await
    }

    pub async fn mark_webhook_event_forward_failed(
        &self,
        webhook_event_id: u64,
        runtime_attempts: Option<u32>,
        runtime_status_code: Option<u16>,
        runtime_response: Option<Value>,
        last_error: Option<String>,
    ) -> Result<Option<CommsWebhookEventRecord>, DomainStoreError> {
        self.mutate(|state| {
            let now = Utc::now();
            let Some(row) = state
                .comms_webhook_events
                .values_mut()
                .find(|row| row.id == webhook_event_id)
            else {
                return Ok(None);
            };

            row.status = "failed".to_string();
            if let Some(runtime_attempts) = runtime_attempts {
                row.runtime_attempts = row.runtime_attempts.max(runtime_attempts.max(1));
            }
            row.runtime_status_code = runtime_status_code;
            row.runtime_response = runtime_response.clone();
            row.last_error = normalize_optional_string(last_error.as_deref());
            row.updated_at = now;
            Ok(Some(row.clone()))
        })
        .await
    }

    pub async fn mark_webhook_event_forwarded(
        &self,
        webhook_event_id: u64,
        runtime_attempts: Option<u32>,
        runtime_status_code: Option<u16>,
        runtime_response: Option<Value>,
    ) -> Result<Option<CommsWebhookEventRecord>, DomainStoreError> {
        self.mutate(|state| {
            let now = Utc::now();
            let Some(row) = state
                .comms_webhook_events
                .values_mut()
                .find(|row| row.id == webhook_event_id)
            else {
                return Ok(None);
            };

            row.status = "forwarded".to_string();
            if let Some(runtime_attempts) = runtime_attempts {
                row.runtime_attempts = row.runtime_attempts.max(runtime_attempts.max(1));
            }
            row.runtime_status_code = runtime_status_code;
            row.runtime_response = runtime_response.clone();
            row.forwarded_at = Some(now);
            row.last_error = None;
            row.updated_at = now;
            Ok(Some(row.clone()))
        })
        .await
    }

    pub async fn upsert_delivery_projection(
        &self,
        input: UpsertDeliveryProjectionInput,
    ) -> Result<CommsDeliveryProjectionRecord, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let provider = normalize_non_empty(&input.provider, "provider")?.to_lowercase();
        let integration_id = normalize_optional_string(input.integration_id.as_deref())
            .unwrap_or_else(|| "unknown".to_string());

        self.mutate(|state| {
            let now = Utc::now();
            let key = delivery_projection_key(&user_id, &provider, &integration_id);

            let row = state
                .comms_delivery_projections
                .entry(key)
                .or_insert_with(|| {
                    let id = state.next_comms_delivery_projection_id;
                    state.next_comms_delivery_projection_id =
                        state.next_comms_delivery_projection_id.saturating_add(1);
                    CommsDeliveryProjectionRecord {
                        id,
                        user_id: user_id.clone(),
                        provider: provider.clone(),
                        integration_id: integration_id.clone(),
                        last_state: None,
                        last_event_at: None,
                        last_message_id: None,
                        last_recipient: None,
                        runtime_event_id: None,
                        source: "runtime_forwarder".to_string(),
                        last_webhook_event_id: None,
                        created_at: now,
                        updated_at: now,
                    }
                });

            if input.last_state.is_some() {
                row.last_state = normalize_optional_string(input.last_state.as_deref());
            }
            if input.last_event_at.is_some() {
                row.last_event_at = input.last_event_at;
            }
            if input.last_message_id.is_some() {
                row.last_message_id = normalize_optional_string(input.last_message_id.as_deref());
            }
            if input.last_recipient.is_some() {
                row.last_recipient = normalize_optional_string(input.last_recipient.as_deref());
            }
            if input.runtime_event_id.is_some() {
                row.runtime_event_id = normalize_optional_string(input.runtime_event_id.as_deref());
            }
            if let Some(source) = input.source.as_deref() {
                row.source = normalize_status_or_default(Some(source), "runtime_forwarder");
            }
            if input.last_webhook_event_id.is_some() {
                row.last_webhook_event_id = input.last_webhook_event_id;
            }
            row.updated_at = now;

            Ok(row.clone())
        })
        .await
    }

    pub async fn delivery_projection(
        &self,
        user_id: &str,
        provider: &str,
        integration_id: Option<&str>,
    ) -> Result<Option<CommsDeliveryProjectionRecord>, DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let provider = normalize_non_empty(provider, "provider")?.to_lowercase();
        let integration_id =
            normalize_optional_string(integration_id).unwrap_or_else(|| "unknown".to_string());
        let key = delivery_projection_key(&user_id, &provider, &integration_id);

        let state = self.state.read().await;
        Ok(state.comms_delivery_projections.get(&key).cloned())
    }

    pub async fn audit_delivery_projection_updated(
        &self,
        user_id: &str,
        provider: &str,
        projection: &CommsDeliveryProjectionRecord,
    ) -> Result<(), DomainStoreError> {
        let user_id = normalize_non_empty(user_id, "user_id")?;
        let provider = normalize_non_empty(provider, "provider")?.to_lowercase();

        self.mutate(|state| {
            let key = integration_key(&user_id, &provider);
            let Some(integration) = state.user_integrations.get(&key).cloned() else {
                return Ok(());
            };

            append_integration_audit(
                state,
                &user_id,
                Some(integration.id),
                &provider,
                "delivery_projection_updated",
                Some(serde_json::json!({
                    "projection_id": projection.id,
                    "delivery_state": projection.last_state,
                    "message_id": projection.last_message_id,
                    "source": projection.source,
                })),
            );
            Ok(())
        })
        .await
    }

    pub async fn create_shout(
        &self,
        input: CreateShoutInput,
    ) -> Result<ShoutRecord, DomainStoreError> {
        let user_id = normalize_non_empty(&input.user_id, "user_id")?;
        let body = normalize_non_empty(&input.body, "body")?;
        let zone = normalize_zone(input.zone.as_deref());

        self.mutate(|state| {
            let now = Utc::now();
            let row = ShoutRecord {
                id: state.next_shout_id,
                user_id,
                zone,
                body,
                visibility: "public".to_string(),
                created_at: now,
                updated_at: now,
            };
            state.next_shout_id = state.next_shout_id.saturating_add(1);
            state.shouts.push(row.clone());
            Ok(row)
        })
        .await
    }

    pub async fn list_shouts(
        &self,
        zone: Option<&str>,
        limit: usize,
        before_id: Option<u64>,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<ShoutRecord>, DomainStoreError> {
        let normalized_zone = normalize_zone(zone);
        let safe_limit = limit.clamp(1, 200);
        let state = self.state.read().await;

        let mut rows: Vec<ShoutRecord> = state
            .shouts
            .iter()
            .filter(|row| {
                normalized_zone
                    .as_ref()
                    .map(|zone| row.zone.as_ref() == Some(zone))
                    .unwrap_or(true)
            })
            .filter(|row| before_id.map(|id| row.id < id).unwrap_or(true))
            .filter(|row| since.map(|value| row.created_at >= value).unwrap_or(true))
            .cloned()
            .collect();

        rows.sort_by(|left, right| right.id.cmp(&left.id));
        rows.truncate(safe_limit);
        Ok(rows)
    }

    pub async fn top_shout_zones(&self, limit: usize) -> Result<Vec<ZoneCount>, DomainStoreError> {
        let safe_limit = limit.clamp(1, 100);
        let cutoff = Utc::now() - Duration::hours(24);
        let state = self.state.read().await;

        let mut counters: HashMap<String, u64> = HashMap::new();
        for shout in &state.shouts {
            if shout.created_at < cutoff {
                continue;
            }
            let Some(zone) = shout.zone.as_ref() else {
                continue;
            };
            *counters.entry(zone.clone()).or_insert(0) += 1;
        }

        let mut rows: Vec<ZoneCount> = counters
            .into_iter()
            .map(|(zone, count24h)| ZoneCount { zone, count24h })
            .collect();
        rows.sort_by(|left, right| {
            right
                .count24h
                .cmp(&left.count24h)
                .then(left.zone.cmp(&right.zone))
        });
        rows.truncate(safe_limit);
        Ok(rows)
    }

    pub async fn send_whisper(
        &self,
        input: SendWhisperInput,
    ) -> Result<WhisperRecord, DomainStoreError> {
        let sender_id = normalize_non_empty(&input.sender_id, "sender_id")?;
        let recipient_id = normalize_non_empty(&input.recipient_id, "recipient_id")?;
        let body = normalize_non_empty(&input.body, "body")?;

        self.mutate(|state| {
            let now = Utc::now();
            let whisper = WhisperRecord {
                id: state.next_whisper_id,
                sender_id,
                recipient_id,
                body,
                read_at: None,
                created_at: now,
                updated_at: now,
            };
            state.next_whisper_id = state.next_whisper_id.saturating_add(1);
            state.whispers.push(whisper.clone());
            Ok(whisper)
        })
        .await
    }

    pub async fn list_whispers_for(
        &self,
        actor_id: &str,
        with_user_id: Option<&str>,
        limit: usize,
        before_id: Option<u64>,
    ) -> Result<Vec<WhisperRecord>, DomainStoreError> {
        let actor_id = normalize_non_empty(actor_id, "actor_id")?;
        let with_user_id = with_user_id.map(|value| value.trim().to_string());
        let safe_limit = limit.clamp(1, 200);

        let state = self.state.read().await;
        let mut rows: Vec<WhisperRecord> = state
            .whispers
            .iter()
            .filter(|row| {
                if let Some(with_user_id) = with_user_id.as_ref() {
                    (row.sender_id == actor_id && row.recipient_id == *with_user_id)
                        || (row.sender_id == *with_user_id && row.recipient_id == actor_id)
                } else {
                    row.sender_id == actor_id || row.recipient_id == actor_id
                }
            })
            .filter(|row| before_id.map(|id| row.id < id).unwrap_or(true))
            .cloned()
            .collect();

        rows.sort_by(|left, right| right.id.cmp(&left.id));
        rows.truncate(safe_limit);
        Ok(rows)
    }

    pub async fn whisper_by_id(
        &self,
        whisper_id: u64,
    ) -> Result<Option<WhisperRecord>, DomainStoreError> {
        let state = self.state.read().await;
        Ok(state
            .whispers
            .iter()
            .find(|row| row.id == whisper_id)
            .cloned())
    }

    pub async fn mark_whisper_read(
        &self,
        whisper_id: u64,
        recipient_id: &str,
    ) -> Result<WhisperRecord, DomainStoreError> {
        let recipient_id = normalize_non_empty(recipient_id, "recipient_id")?;

        self.mutate(|state| {
            let whisper = state
                .whispers
                .iter_mut()
                .find(|row| row.id == whisper_id)
                .ok_or(DomainStoreError::NotFound)?;

            if whisper.recipient_id != recipient_id {
                return Ok(whisper.clone());
            }

            if whisper.read_at.is_none() {
                whisper.read_at = Some(Utc::now());
                whisper.updated_at = Utc::now();
            }
            Ok(whisper.clone())
        })
        .await
    }

    fn load_state(path: Option<&PathBuf>) -> DomainStoreState {
        let Some(path) = path else {
            return DomainStoreState::default();
        };

        let raw = match std::fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return DomainStoreState::default();
            }
            Err(error) => {
                tracing::warn!(
                    target: "openagents.domain_store",
                    path = %path.display(),
                    error = %error,
                    "failed to read domain store; booting with empty state",
                );
                return DomainStoreState::default();
            }
        };

        match serde_json::from_str::<DomainStoreState>(&raw) {
            Ok(state) => state,
            Err(error) => {
                tracing::warn!(
                    target: "openagents.domain_store",
                    path = %path.display(),
                    error = %error,
                    "failed to parse domain store; booting with empty state",
                );
                DomainStoreState::default()
            }
        }
    }

    async fn persist_state(&self, snapshot: &DomainStoreState) -> Result<(), DomainStoreError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                DomainStoreError::Persistence {
                    message: format!("failed to prepare domain store directory: {error}"),
                }
            })?;
        }

        let payload =
            serde_json::to_vec(snapshot).map_err(|error| DomainStoreError::Persistence {
                message: format!("failed to encode domain store payload: {error}"),
            })?;

        let temp_path = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
        tokio::fs::write(&temp_path, payload)
            .await
            .map_err(|error| DomainStoreError::Persistence {
                message: format!("failed to write domain store payload: {error}"),
            })?;

        tokio::fs::rename(&temp_path, path).await.map_err(|error| {
            DomainStoreError::Persistence {
                message: format!("failed to finalize domain store payload: {error}"),
            }
        })?;

        Ok(())
    }

    async fn mutate<T, F>(&self, operation: F) -> Result<T, DomainStoreError>
    where
        F: FnOnce(&mut DomainStoreState) -> Result<T, DomainStoreError>,
    {
        let (result, snapshot) = {
            let mut state = self.state.write().await;
            let result = operation(&mut state)?;
            (result, state.clone())
        };

        self.persist_state(&snapshot).await?;
        Ok(result)
    }
}

fn resolve_owned_autopilot_record(
    state: &DomainStoreState,
    owner_user_id: &str,
    reference: &str,
) -> Result<AutopilotRecord, DomainStoreError> {
    let reference_normalized = reference.trim().to_lowercase();
    state
        .autopilots
        .values()
        .find(|record| {
            record.owner_user_id == owner_user_id
                && record.deleted_at.is_none()
                && (record.id == reference || record.handle == reference_normalized)
        })
        .cloned()
        .ok_or(DomainStoreError::NotFound)
}

fn resolve_owned_autopilot_id(
    state: &DomainStoreState,
    owner_user_id: &str,
    reference: &str,
) -> Result<String, DomainStoreError> {
    resolve_owned_autopilot_record(state, owner_user_id, reference).map(|record| record.id)
}

fn build_autopilot_aggregate(
    state: &DomainStoreState,
    autopilot: AutopilotRecord,
) -> AutopilotAggregate {
    let default_timestamp = autopilot.updated_at;
    let profile = state
        .autopilot_profiles
        .get(&autopilot.id)
        .cloned()
        .unwrap_or(AutopilotProfileRecord {
            autopilot_id: autopilot.id.clone(),
            owner_display_name: autopilot.display_name.clone(),
            persona_summary: None,
            autopilot_voice: None,
            principles: None,
            preferences: None,
            onboarding_answers: Value::Array(Vec::new()),
            schema_version: 1,
            created_at: default_timestamp,
            updated_at: default_timestamp,
        });
    let policy = state
        .autopilot_policies
        .get(&autopilot.id)
        .cloned()
        .unwrap_or(AutopilotPolicyRecord {
            autopilot_id: autopilot.id.clone(),
            model_provider: None,
            model: None,
            tool_allowlist: Vec::new(),
            tool_denylist: Vec::new(),
            l402_require_approval: true,
            l402_max_spend_msats_per_call: None,
            l402_max_spend_msats_per_day: None,
            l402_allowed_hosts: Vec::new(),
            data_policy: None,
            created_at: default_timestamp,
            updated_at: default_timestamp,
        });

    let mut runtime_bindings: Vec<AutopilotRuntimeBindingRecord> = state
        .autopilot_runtime_bindings
        .values()
        .filter(|binding| binding.autopilot_id == autopilot.id)
        .cloned()
        .collect();
    runtime_bindings.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));

    AutopilotAggregate {
        autopilot,
        profile,
        policy,
        runtime_bindings,
    }
}

fn upsert_integration_secret(
    state: &mut DomainStoreState,
    user_id: &str,
    provider: &str,
    encrypted_secret: String,
    fingerprint: String,
    secret_last4: Option<String>,
    metadata: Option<Value>,
) -> IntegrationUpsertResult {
    let now = Utc::now();
    let key = integration_key(user_id, provider);

    let integration = state
        .user_integrations
        .entry(key)
        .or_insert_with(|| UserIntegrationRecord {
            id: state.next_user_integration_id,
            user_id: user_id.to_string(),
            provider: provider.to_string(),
            status: "inactive".to_string(),
            encrypted_secret: None,
            secret_fingerprint: None,
            secret_last4: None,
            metadata: None,
            connected_at: None,
            disconnected_at: None,
            created_at: now,
            updated_at: now,
        });

    if integration.id >= state.next_user_integration_id {
        state.next_user_integration_id = integration.id.saturating_add(1);
    }

    let action = if integration.encrypted_secret.is_none() {
        "secret_created".to_string()
    } else if integration.secret_fingerprint.as_deref() != Some(fingerprint.as_str()) {
        "secret_rotated".to_string()
    } else {
        "secret_updated".to_string()
    };

    integration.status = "active".to_string();
    integration.encrypted_secret = Some(encrypted_secret);
    integration.secret_fingerprint = Some(fingerprint);
    integration.secret_last4 = secret_last4;
    integration.metadata = metadata;
    integration.connected_at = Some(now);
    integration.disconnected_at = None;
    integration.updated_at = now;

    IntegrationUpsertResult {
        integration: integration.clone(),
        action,
    }
}

fn append_integration_audit(
    state: &mut DomainStoreState,
    user_id: &str,
    user_integration_id: Option<u64>,
    provider: &str,
    action: &str,
    metadata: Option<Value>,
) {
    let now = Utc::now();
    let id = state.next_user_integration_audit_id;
    state.next_user_integration_audit_id = state.next_user_integration_audit_id.saturating_add(1);
    state
        .user_integration_audits
        .push(UserIntegrationAuditRecord {
            id,
            user_id: user_id.to_string(),
            user_integration_id,
            provider: provider.to_string(),
            action: action.to_string(),
            metadata,
            created_at: now,
            updated_at: now,
        });
}

fn integration_secret_cipher_from_env() -> Option<IntegrationSecretCipher> {
    let encoded_key = std::env::var(INTEGRATION_SECRET_ENCRYPTION_KEY_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())?;
    let key_id = std::env::var(INTEGRATION_SECRET_KEY_ID_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "v1".to_string());

    let decoded = URL_SAFE_NO_PAD
        .decode(encoded_key.as_bytes())
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(encoded_key.as_bytes()));
    let key = match decoded {
        Ok(bytes) if bytes.len() == 32 => {
            let mut material = [0u8; 32];
            material.copy_from_slice(bytes.as_slice());
            material
        }
        Ok(bytes) => {
            tracing::warn!(
                target: "openagents.domain_store",
                env = INTEGRATION_SECRET_ENCRYPTION_KEY_ENV,
                key_bytes = bytes.len(),
                "integration secret encryption key ignored: expected 32-byte base64 value",
            );
            return None;
        }
        Err(error) => {
            tracing::warn!(
                target: "openagents.domain_store",
                env = INTEGRATION_SECRET_ENCRYPTION_KEY_ENV,
                error = %error,
                "integration secret encryption key ignored: invalid base64 payload",
            );
            return None;
        }
    };

    Some(IntegrationSecretCipher { key_id, key })
}

fn is_encrypted_integration_secret(value: &str) -> bool {
    value.starts_with(INTEGRATION_SECRET_ENVELOPE_PREFIX)
}

fn encrypt_integration_secret(
    plaintext: &str,
    cipher: Option<&IntegrationSecretCipher>,
) -> Result<String, DomainStoreError> {
    let Some(cipher) = cipher else {
        return Ok(plaintext.to_string());
    };
    if is_encrypted_integration_secret(plaintext) {
        return Ok(plaintext.to_string());
    }

    let nonce_source = Uuid::new_v4().as_bytes().to_owned();
    let nonce = Nonce::from_slice(&nonce_source[..12]);
    let aead = ChaCha20Poly1305::new_from_slice(&cipher.key).map_err(|error| {
        DomainStoreError::Persistence {
            message: format!("failed to initialize integration secret cipher: {error}"),
        }
    })?;
    let ciphertext = aead.encrypt(nonce, plaintext.as_bytes()).map_err(|error| {
        DomainStoreError::Persistence {
            message: format!("failed to encrypt integration secret: {error}"),
        }
    })?;

    let nonce_b64 = URL_SAFE_NO_PAD.encode(nonce_source[..12].as_ref());
    let ciphertext_b64 = URL_SAFE_NO_PAD.encode(ciphertext);
    Ok(format!(
        "{INTEGRATION_SECRET_ENVELOPE_PREFIX}{}:{nonce_b64}:{ciphertext_b64}",
        cipher.key_id
    ))
}

fn decrypt_integration_secret(
    stored: &str,
    cipher: Option<&IntegrationSecretCipher>,
) -> Result<String, DomainStoreError> {
    if !is_encrypted_integration_secret(stored) {
        return Ok(stored.to_string());
    }

    let Some(cipher) = cipher else {
        return Err(DomainStoreError::Persistence {
            message: "integration secret is encrypted but no decryption key is configured"
                .to_string(),
        });
    };

    let mut parts = stored.split(':');
    let version = parts.next().unwrap_or_default();
    let version_suffix = parts.next().unwrap_or_default();
    let key_id = parts.next().unwrap_or_default();
    let nonce_b64 = parts.next().unwrap_or_default();
    let ciphertext_b64 = parts.next().unwrap_or_default();
    let has_extra_parts = parts.next().is_some();

    if version != "enc" || version_suffix != "v1" || has_extra_parts {
        return Err(DomainStoreError::Persistence {
            message: "integration secret envelope is invalid".to_string(),
        });
    }
    if key_id != cipher.key_id {
        return Err(DomainStoreError::Persistence {
            message: format!("integration secret key id {key_id} is not configured"),
        });
    }

    let nonce_raw = URL_SAFE_NO_PAD
        .decode(nonce_b64.as_bytes())
        .map_err(|error| DomainStoreError::Persistence {
            message: format!("failed to decode integration secret nonce: {error}"),
        })?;
    if nonce_raw.len() != 12 {
        return Err(DomainStoreError::Persistence {
            message: "integration secret nonce length is invalid".to_string(),
        });
    }
    let ciphertext = URL_SAFE_NO_PAD
        .decode(ciphertext_b64.as_bytes())
        .map_err(|error| DomainStoreError::Persistence {
            message: format!("failed to decode integration secret payload: {error}"),
        })?;

    let nonce = Nonce::from_slice(nonce_raw.as_slice());
    let aead = ChaCha20Poly1305::new_from_slice(&cipher.key).map_err(|error| {
        DomainStoreError::Persistence {
            message: format!("failed to initialize integration secret cipher: {error}"),
        }
    })?;
    let decrypted = aead.decrypt(nonce, ciphertext.as_ref()).map_err(|error| {
        DomainStoreError::Persistence {
            message: format!("failed to decrypt integration secret: {error}"),
        }
    })?;
    String::from_utf8(decrypted).map_err(|error| DomainStoreError::Persistence {
        message: format!("integration secret plaintext is invalid utf8: {error}"),
    })
}

fn normalize_display_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "Autopilot".to_string();
    }

    trimmed.chars().take(120).collect()
}

fn normalize_owner_display_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "Autopilot".to_string();
    }
    trimmed.chars().take(120).collect()
}

fn normalize_non_empty(value: &str, field: &'static str) -> Result<String, DomainStoreError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(DomainStoreError::Validation {
            field,
            message: "value is required".to_string(),
        });
    }

    Ok(trimmed.to_string())
}

fn normalize_optional_string(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_status_or_default(value: Option<&str>, fallback: &str) -> String {
    normalize_optional_string(value)
        .map(|value| value.to_lowercase())
        .unwrap_or_else(|| fallback.to_string())
}

fn normalize_string_vec(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect()
}

fn ensure_json_array(value: Value) -> Value {
    if value.is_array() {
        value
    } else {
        Value::Array(Vec::new())
    }
}

fn normalize_zone(value: Option<&str>) -> Option<String> {
    normalize_optional_string(value).map(|zone| zone.to_lowercase())
}

fn generate_unique_autopilot_handle(
    state: &DomainStoreState,
    seed: &str,
    ignore_id: Option<&str>,
) -> String {
    let base = normalize_handle_base(seed);
    let base = if base.is_empty() {
        "autopilot".to_string()
    } else {
        base
    };

    let mut candidate = base.clone();
    let mut suffix = 1usize;

    while state.autopilots.values().any(|row| {
        row.handle == candidate && ignore_id.map(|value| value != row.id).unwrap_or(true)
    }) {
        let suffix_text = format!("-{suffix}");
        let max_base_len = 64usize.saturating_sub(suffix_text.len()).max(1);
        let trimmed: String = base.chars().take(max_base_len).collect();
        candidate = format!("{}{}", trimmed, suffix_text);
        suffix = suffix.saturating_add(1);
    }

    candidate
}

fn normalize_handle_base(seed: &str) -> String {
    let mut output = String::with_capacity(seed.len().min(64));
    let mut previous_dash = false;

    for ch in seed.chars() {
        let normalized = ch.to_ascii_lowercase();
        if normalized.is_ascii_alphanumeric() {
            output.push(normalized);
            previous_dash = false;
            continue;
        }

        if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }

    let trimmed = output.trim_matches('-').to_string();
    trimmed.chars().take(64).collect()
}

fn credential_key(host: &str, scope: &str) -> String {
    format!(
        "{}::{}",
        host.trim().to_lowercase(),
        scope.trim().to_lowercase()
    )
}

fn normalize_runtime_scope_type(value: &str) -> Result<String, DomainStoreError> {
    let normalized = normalize_non_empty(value, "scope_type")?.to_ascii_lowercase();
    if !matches!(normalized.as_str(), "user" | "autopilot") {
        return Err(DomainStoreError::Validation {
            field: "scope_type",
            message: "value must be one of: user, autopilot".to_string(),
        });
    }
    Ok(normalized)
}

fn normalize_runtime_driver(value: &str) -> Result<String, DomainStoreError> {
    let normalized = normalize_non_empty(value, "driver")?.to_ascii_lowercase();
    if !matches!(normalized.as_str(), "legacy" | "elixir") {
        return Err(DomainStoreError::Validation {
            field: "driver",
            message: "value must be one of: legacy, elixir".to_string(),
        });
    }
    Ok(normalized)
}

fn runtime_override_key(scope_type: &str, scope_id: &str) -> String {
    format!("{}::{}", scope_type.trim(), scope_id.trim())
}

fn integration_key(user_id: &str, provider: &str) -> String {
    format!("{}::{}", user_id.trim(), provider.trim().to_lowercase())
}

fn inbox_thread_state_key(user_id: &str, thread_id: &str) -> String {
    format!("{}::{}", user_id.trim(), thread_id.trim())
}

fn delivery_projection_key(user_id: &str, provider: &str, integration_id: &str) -> String {
    format!(
        "{}::{}::{}",
        user_id.trim(),
        provider.trim().to_lowercase(),
        integration_id.trim().to_lowercase()
    )
}

fn last4(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    let start = len.saturating_sub(4);
    chars[start..].iter().collect()
}

fn sha256_hex(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    let digest = hasher.finalize();

    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push(hex_char(byte >> 4));
        output.push(hex_char(byte & 0x0f));
    }
    output
}

fn hex_char(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        10..=15 => (b'a' + (value - 10)) as char,
        _ => '0',
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::config::Config;

    fn test_config(store_path: Option<PathBuf>) -> Config {
        let mut config = Config::for_tests(std::env::temp_dir());
        config.route_split_enabled = false;
        config.route_split_mode = "legacy".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        config.route_split_cohort_percentage = 0;
        config.auth_store_path = None;
        config.domain_store_path = store_path;
        config.smoke_stream_secret = None;
        config
    }

    #[test]
    fn integration_secret_cipher_roundtrip_and_rotation_guard() {
        let cipher = IntegrationSecretCipher {
            key_id: "v1".to_string(),
            key: [7u8; 32],
        };
        let rotated = IntegrationSecretCipher {
            key_id: "v2".to_string(),
            key: [7u8; 32],
        };

        let plaintext = "super-secret-token-value";
        let encrypted =
            encrypt_integration_secret(plaintext, Some(&cipher)).expect("encrypt integration");
        assert!(is_encrypted_integration_secret(&encrypted));
        assert_ne!(encrypted, plaintext);

        let decrypted =
            decrypt_integration_secret(encrypted.as_str(), Some(&cipher)).expect("decrypt");
        assert_eq!(decrypted, plaintext);

        let wrong_key = decrypt_integration_secret(encrypted.as_str(), Some(&rotated))
            .expect_err("key id mismatch must fail");
        assert!(matches!(wrong_key, DomainStoreError::Persistence { .. }));

        let missing_key = decrypt_integration_secret(encrypted.as_str(), None)
            .expect_err("missing key must fail");
        assert!(matches!(missing_key, DomainStoreError::Persistence { .. }));

        let passthrough =
            decrypt_integration_secret("legacy-plaintext", None).expect("plaintext passthrough");
        assert_eq!(passthrough, "legacy-plaintext");
    }

    #[tokio::test]
    async fn integration_secret_migrates_plaintext_after_encryption_key_is_enabled() {
        let store = DomainStore::from_config(&test_config(None));
        store
            .upsert_resend_integration(UpsertResendIntegrationInput {
                user_id: "usr_encrypt".to_string(),
                api_key: "re_plaintext_1234567890".to_string(),
                sender_email: Some("bot@openagents.com".to_string()),
                sender_name: Some("OpenAgents Bot".to_string()),
            })
            .await
            .expect("seed resend integration");

        {
            let state = store.state.read().await;
            let key = integration_key("usr_encrypt", "resend");
            let row = state
                .user_integrations
                .get(&key)
                .expect("seeded integration row");
            assert_eq!(
                row.encrypted_secret.as_deref(),
                Some("re_plaintext_1234567890")
            );
        }

        let mut store = store;
        store.integration_secret_cipher = Some(IntegrationSecretCipher {
            key_id: "v1".to_string(),
            key: [11u8; 32],
        });

        let fetched = store
            .find_active_integration_secret("usr_encrypt", "resend")
            .await
            .expect("find secret")
            .expect("integration exists");
        assert_eq!(
            fetched.encrypted_secret.as_deref(),
            Some("re_plaintext_1234567890")
        );

        let state = store.state.read().await;
        let key = integration_key("usr_encrypt", "resend");
        let row = state
            .user_integrations
            .get(&key)
            .expect("integration row after migration");
        let stored = row.encrypted_secret.clone().unwrap_or_default();
        assert!(is_encrypted_integration_secret(stored.as_str()));
    }

    #[tokio::test]
    async fn inbox_thread_state_and_audit_roundtrip() {
        let store = DomainStore::from_config(&test_config(None));

        let upserted = store
            .upsert_inbox_thread_state(UpsertInboxThreadStateInput {
                user_id: "usr_inbox".to_string(),
                thread_id: "thread_1".to_string(),
                pending_approval: true,
                decision: Some("draft_only".to_string()),
                draft_preview: Some("Draft reply".to_string()),
                source: Some("gmail_adapter".to_string()),
            })
            .await
            .expect("upsert inbox state");
        assert_eq!(upserted.thread_id, "thread_1");
        assert!(upserted.pending_approval);

        let updated = store
            .upsert_inbox_thread_state(UpsertInboxThreadStateInput {
                user_id: "usr_inbox".to_string(),
                thread_id: "thread_1".to_string(),
                pending_approval: false,
                decision: Some("approved".to_string()),
                draft_preview: Some("Approved draft".to_string()),
                source: Some("inbox_api".to_string()),
            })
            .await
            .expect("update inbox state");
        assert_eq!(updated.id, upserted.id);
        assert!(!updated.pending_approval);
        assert_eq!(updated.decision.as_deref(), Some("approved"));

        let loaded = store
            .inbox_thread_state("usr_inbox", "thread_1")
            .await
            .expect("load inbox state")
            .expect("state exists");
        assert_eq!(loaded.id, upserted.id);

        store
            .record_inbox_audit(RecordInboxAuditInput {
                user_id: "usr_inbox".to_string(),
                thread_id: "thread_1".to_string(),
                action: "approve_draft".to_string(),
                detail: "draft approved and queued".to_string(),
            })
            .await
            .expect("record inbox audit");

        let audits = store
            .list_inbox_audits_for_user("usr_inbox", Some("thread_1"), 20)
            .await
            .expect("list inbox audits");
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].action, "approve_draft");

        let rows = store
            .list_inbox_thread_states_for_user("usr_inbox")
            .await
            .expect("list inbox states");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].thread_id, "thread_1");
    }

    #[tokio::test]
    async fn autopilot_round_trip_and_persistence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("domain-store.json");
        let store = DomainStore::from_config(&test_config(Some(path.clone())));

        let created = store
            .create_autopilot(CreateAutopilotInput {
                owner_user_id: "usr_1".to_string(),
                owner_display_name: "Owner".to_string(),
                display_name: "My Pilot".to_string(),
                handle_seed: None,
                avatar: None,
                status: None,
                visibility: None,
                tagline: Some("hello".to_string()),
            })
            .await
            .expect("create autopilot");

        assert_eq!(created.autopilot.handle, "my-pilot");
        assert_eq!(created.autopilot.config_version, 1);

        let updated = store
            .update_owned_autopilot(
                "usr_1",
                &created.autopilot.id,
                UpdateAutopilotInput {
                    profile: Some(UpsertAutopilotProfileInput {
                        owner_display_name: Some("Owner 2".to_string()),
                        ..Default::default()
                    }),
                    policy: Some(UpsertAutopilotPolicyInput {
                        tool_allowlist: Some(vec!["calendar.read".to_string()]),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .expect("update autopilot");

        assert_eq!(updated.autopilot.config_version, 2);
        assert_eq!(updated.profile.owner_display_name, "Owner 2");
        assert_eq!(updated.policy.tool_allowlist, vec!["calendar.read"]);

        let restored = DomainStore::from_config(&test_config(Some(path)));
        let listed = restored
            .list_autopilots_for_owner("usr_1", 10)
            .await
            .expect("list autopilots");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].autopilot.handle, "my-pilot");
        assert_eq!(listed[0].autopilot.config_version, 2);
    }

    #[tokio::test]
    async fn integration_lifecycle_tracks_actions_and_audits() {
        let store = DomainStore::from_config(&test_config(None));

        let first = store
            .upsert_resend_integration(UpsertResendIntegrationInput {
                user_id: "usr_2".to_string(),
                api_key: "resend_live_1234".to_string(),
                sender_email: Some("hello@example.com".to_string()),
                sender_name: None,
            })
            .await
            .expect("create integration");
        assert_eq!(first.action, "secret_created");

        let second = store
            .upsert_resend_integration(UpsertResendIntegrationInput {
                user_id: "usr_2".to_string(),
                api_key: "resend_live_1234".to_string(),
                sender_email: Some("hello@example.com".to_string()),
                sender_name: None,
            })
            .await
            .expect("update integration");
        assert_eq!(second.action, "secret_updated");

        let third = store
            .upsert_resend_integration(UpsertResendIntegrationInput {
                user_id: "usr_2".to_string(),
                api_key: "resend_live_9999".to_string(),
                sender_email: Some("hi@example.com".to_string()),
                sender_name: Some("Ops".to_string()),
            })
            .await
            .expect("rotate integration");
        assert_eq!(third.action, "secret_rotated");

        let revoked = store
            .revoke_integration("usr_2", "resend")
            .await
            .expect("revoke integration")
            .expect("integration exists");
        assert_eq!(revoked.status, "inactive");

        let audits = store
            .list_integration_audits_for_user("usr_2")
            .await
            .expect("list audits");
        assert_eq!(audits.len(), 4);
        assert_eq!(audits[0].action, "secret_revoked");
    }

    #[tokio::test]
    async fn webhook_idempotency_and_delivery_projection_upsert() {
        let store = DomainStore::from_config(&test_config(None));

        let first = store
            .record_webhook_event(RecordWebhookEventInput {
                provider: "resend".to_string(),
                idempotency_key: "evt:123".to_string(),
                external_event_id: Some("re_1".to_string()),
                event_type: Some("delivered".to_string()),
                delivery_state: Some("delivered".to_string()),
                message_id: Some("msg_1".to_string()),
                integration_id: Some("gmail.primary".to_string()),
                user_id: Some("usr_1".to_string()),
                recipient: Some("a@example.com".to_string()),
                signature_valid: true,
                status: Some("processed".to_string()),
                normalized_payload: Some(serde_json::json!({ "state": "delivered" })),
                raw_payload: Some(serde_json::json!({ "raw": true })),
            })
            .await
            .expect("insert webhook event");
        assert!(first.inserted);

        let second = store
            .record_webhook_event(RecordWebhookEventInput {
                provider: "resend".to_string(),
                idempotency_key: "evt:123".to_string(),
                external_event_id: Some("re_1".to_string()),
                event_type: Some("delivered".to_string()),
                delivery_state: Some("delivered".to_string()),
                message_id: Some("msg_1".to_string()),
                integration_id: Some("gmail.primary".to_string()),
                user_id: Some("usr_1".to_string()),
                recipient: Some("a@example.com".to_string()),
                signature_valid: true,
                status: Some("processed".to_string()),
                normalized_payload: Some(serde_json::json!({ "state": "delivered" })),
                raw_payload: Some(serde_json::json!({ "raw": true })),
            })
            .await
            .expect("idempotent event");
        assert!(!second.inserted);
        assert_eq!(first.event.id, second.event.id);

        let projection = store
            .upsert_delivery_projection(UpsertDeliveryProjectionInput {
                user_id: "usr_1".to_string(),
                provider: "resend".to_string(),
                integration_id: Some("gmail.primary".to_string()),
                last_state: Some("delivered".to_string()),
                last_event_at: Some(Utc::now()),
                last_message_id: Some("msg_1".to_string()),
                last_recipient: Some("a@example.com".to_string()),
                runtime_event_id: Some("runtime_evt_1".to_string()),
                source: Some("runtime_forwarder".to_string()),
                last_webhook_event_id: Some(first.event.id),
            })
            .await
            .expect("upsert projection");

        assert_eq!(projection.provider, "resend");
        assert_eq!(projection.integration_id, "gmail.primary");
        assert_eq!(projection.last_state.as_deref(), Some("delivered"));
    }

    #[tokio::test]
    async fn social_surfaces_keep_zone_and_read_semantics() {
        let store = DomainStore::from_config(&test_config(None));

        let _ = store
            .create_shout(CreateShoutInput {
                user_id: "usr_a".to_string(),
                zone: Some("General".to_string()),
                body: "hello world".to_string(),
            })
            .await
            .expect("create shout");

        let _ = store
            .create_shout(CreateShoutInput {
                user_id: "usr_b".to_string(),
                zone: Some("general".to_string()),
                body: "hello again".to_string(),
            })
            .await
            .expect("create shout");

        let zone_rows = store
            .list_shouts(Some("general"), 10, None, None)
            .await
            .expect("list by zone");
        assert_eq!(zone_rows.len(), 2);

        let zones = store.top_shout_zones(10).await.expect("top zones");
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0].zone, "general");

        let whisper = store
            .send_whisper(SendWhisperInput {
                sender_id: "usr_a".to_string(),
                recipient_id: "usr_b".to_string(),
                body: "private message".to_string(),
            })
            .await
            .expect("send whisper");

        let for_b = store
            .list_whispers_for("usr_b", Some("usr_a"), 10, None)
            .await
            .expect("list whispers");
        assert_eq!(for_b.len(), 1);

        let read = store
            .mark_whisper_read(whisper.id, "usr_b")
            .await
            .expect("mark read");
        assert!(read.read_at.is_some());
    }

    #[tokio::test]
    async fn l402_paywall_round_trip_and_soft_delete() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("domain-store.json");
        let store = DomainStore::from_config(&test_config(Some(path.clone())));

        let created = store
            .create_l402_paywall(CreateL402PaywallInput {
                owner_user_id: "usr_5".to_string(),
                name: "Default".to_string(),
                host_regexp: "example.com".to_string(),
                path_regexp: "/api/.*".to_string(),
                price_msats: 1000,
                upstream: "https://upstream.example.com".to_string(),
                enabled: Some(true),
                meta: None,
            })
            .await
            .expect("create paywall");

        let updated = store
            .update_owned_l402_paywall(
                "usr_5",
                &created.id,
                UpdateL402PaywallInput {
                    enabled: Some(false),
                    ..Default::default()
                },
            )
            .await
            .expect("update paywall");
        assert!(!updated.enabled);

        let deleted = store
            .soft_delete_owned_l402_paywall("usr_5", &created.id)
            .await
            .expect("delete paywall");
        assert!(deleted.deleted_at.is_some());

        let restored_store = DomainStore::from_config(&test_config(Some(path)));
        let rows = restored_store
            .list_l402_paywalls_for_owner("usr_5", true)
            .await
            .expect("list paywalls");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].deleted_at.is_some());
    }

    #[tokio::test]
    async fn l402_receipts_gateway_events_and_autopilot_filter_parity() {
        let store = DomainStore::from_config(&test_config(None));
        let now = Utc::now();

        let owned_autopilot = store
            .create_autopilot(CreateAutopilotInput {
                owner_user_id: "usr_7".to_string(),
                owner_display_name: "Owner".to_string(),
                display_name: "Payments Bot".to_string(),
                handle_seed: None,
                avatar: None,
                status: None,
                visibility: None,
                tagline: None,
            })
            .await
            .expect("create owned autopilot");

        let foreign_autopilot = store
            .create_autopilot(CreateAutopilotInput {
                owner_user_id: "usr_9".to_string(),
                owner_display_name: "Other".to_string(),
                display_name: "Other Bot".to_string(),
                handle_seed: None,
                avatar: None,
                status: None,
                visibility: None,
                tagline: None,
            })
            .await
            .expect("create foreign autopilot");

        let resolved = store
            .resolve_autopilot_filter_for_owner("usr_7", "PAYMENTS-BOT")
            .await
            .expect("resolve owned filter");
        assert_eq!(resolved.id, owned_autopilot.autopilot.id);

        let forbidden = store
            .resolve_autopilot_filter_for_owner("usr_7", &foreign_autopilot.autopilot.id)
            .await
            .expect_err("foreign autopilot should be forbidden");
        assert!(matches!(forbidden, DomainStoreError::Forbidden));

        let first_receipt = store
            .record_l402_receipt(RecordL402ReceiptInput {
                user_id: "usr_7".to_string(),
                thread_id: "thread_1".to_string(),
                run_id: "run_1".to_string(),
                autopilot_id: Some(owned_autopilot.autopilot.id.clone()),
                thread_title: Some("Conversation 1".to_string()),
                run_status: Some("completed".to_string()),
                run_started_at: Some(now - Duration::minutes(5)),
                run_completed_at: Some(now - Duration::minutes(4)),
                payload: serde_json::json!({
                    "status": "paid",
                    "paid": true,
                    "amountMsats": 2100
                }),
                created_at: Some(now - Duration::minutes(4)),
            })
            .await
            .expect("record first receipt");

        let second_receipt = store
            .record_l402_receipt(RecordL402ReceiptInput {
                user_id: "usr_7".to_string(),
                thread_id: "thread_2".to_string(),
                run_id: "run_2".to_string(),
                autopilot_id: Some(owned_autopilot.autopilot.id.clone()),
                thread_title: Some("Conversation 2".to_string()),
                run_status: Some("completed".to_string()),
                run_started_at: Some(now - Duration::minutes(3)),
                run_completed_at: Some(now - Duration::minutes(2)),
                payload: serde_json::json!({
                    "status": "cached",
                    "cacheStatus": "hit",
                    "paid": false
                }),
                created_at: Some(now - Duration::minutes(2)),
            })
            .await
            .expect("record second receipt");

        let _ = store
            .record_l402_receipt(RecordL402ReceiptInput {
                user_id: "usr_7".to_string(),
                thread_id: "thread_3".to_string(),
                run_id: "run_3".to_string(),
                autopilot_id: None,
                thread_title: None,
                run_status: None,
                run_started_at: None,
                run_completed_at: None,
                payload: serde_json::json!({
                    "status": "blocked",
                    "denyCode": "policy_denied"
                }),
                created_at: Some(now - Duration::minutes(1)),
            })
            .await
            .expect("record third receipt");

        let _ = store
            .record_l402_receipt(RecordL402ReceiptInput {
                user_id: "usr_8".to_string(),
                thread_id: "thread_4".to_string(),
                run_id: "run_4".to_string(),
                autopilot_id: Some(owned_autopilot.autopilot.id.clone()),
                thread_title: None,
                run_status: None,
                run_started_at: None,
                run_completed_at: None,
                payload: serde_json::json!({"status":"paid","paid":true,"amountMsats":500}),
                created_at: Some(now),
            })
            .await
            .expect("record receipt for another user");

        let all_receipts = store
            .list_l402_receipts_for_user("usr_7", None, 10, 0)
            .await
            .expect("list all receipts");
        assert_eq!(all_receipts.len(), 3);
        assert!(all_receipts[0].id > all_receipts[1].id);

        let filtered_receipts = store
            .list_l402_receipts_for_user("usr_7", Some(&owned_autopilot.autopilot.id), 10, 0)
            .await
            .expect("list filtered receipts");
        assert_eq!(filtered_receipts.len(), 2);
        assert_eq!(
            store
                .count_l402_receipts_for_user("usr_7", Some(&owned_autopilot.autopilot.id))
                .await
                .expect("count filtered receipts"),
            2
        );

        let paged = store
            .list_l402_receipts_for_user("usr_7", Some(&owned_autopilot.autopilot.id), 1, 1)
            .await
            .expect("page receipts");
        assert_eq!(paged.len(), 1);
        assert_eq!(paged[0].id, first_receipt.id);

        let found = store
            .find_l402_receipt_for_user("usr_7", second_receipt.id)
            .await
            .expect("find receipt")
            .expect("receipt exists");
        assert_eq!(found.run_id, "run_2");

        let wallet = store
            .upsert_user_spark_wallet(UpsertUserSparkWalletInput {
                user_id: "usr_7".to_string(),
                wallet_id: "wallet_123".to_string(),
                mnemonic: "mnemonic words".to_string(),
                spark_address: Some("spark:abc".to_string()),
                lightning_address: Some("ln@openagents.com".to_string()),
                identity_pubkey: Some("pubkey_1".to_string()),
                last_balance_sats: Some(4200),
                status: Some("active".to_string()),
                provider: Some("spark_executor".to_string()),
                last_error: None,
                meta: None,
                last_synced_at: Some(now),
            })
            .await
            .expect("upsert wallet");
        assert_eq!(wallet.wallet_id, "wallet_123");

        let wallet_lookup = store
            .find_user_spark_wallet("usr_7")
            .await
            .expect("find wallet")
            .expect("wallet exists");
        assert_eq!(wallet_lookup.spark_address.as_deref(), Some("spark:abc"));

        let first_event = store
            .record_l402_gateway_event(RecordL402GatewayEventInput {
                user_id: "usr_7".to_string(),
                autopilot_id: Some(owned_autopilot.autopilot.id.clone()),
                event_type: "l402_gateway_event".to_string(),
                payload: serde_json::json!({"status":"ok"}),
                created_at: Some(now - Duration::minutes(2)),
            })
            .await
            .expect("record gateway event");

        let second_event = store
            .record_l402_gateway_event(RecordL402GatewayEventInput {
                user_id: "usr_7".to_string(),
                autopilot_id: None,
                event_type: "l402_executor_heartbeat".to_string(),
                payload: serde_json::json!({"healthy":true}),
                created_at: Some(now - Duration::minutes(1)),
            })
            .await
            .expect("record second gateway event");

        let _ = store
            .record_l402_gateway_event(RecordL402GatewayEventInput {
                user_id: "usr_8".to_string(),
                autopilot_id: Some(owned_autopilot.autopilot.id.clone()),
                event_type: "l402_gateway_event".to_string(),
                payload: serde_json::json!({"status":"ignored"}),
                created_at: Some(now),
            })
            .await
            .expect("record foreign gateway event");

        let filtered_events = store
            .list_l402_gateway_events_for_user("usr_7", Some(&owned_autopilot.autopilot.id), 10)
            .await
            .expect("list filtered gateway events");
        assert_eq!(filtered_events.len(), 1);
        assert_eq!(filtered_events[0].id, first_event.id);

        let limited_events = store
            .list_l402_gateway_events_for_user("usr_7", None, 1)
            .await
            .expect("list limited gateway events");
        assert_eq!(limited_events.len(), 1);
        assert_eq!(limited_events[0].id, second_event.id);
    }
}
