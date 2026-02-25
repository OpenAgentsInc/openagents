use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifacts::ReceiptSignatureV1;

pub const POOL_CREATE_REQUEST_SCHEMA_V1: &str = "openagents.liquidity.pool.create_request.v1";
pub const POOL_CREATE_RESPONSE_SCHEMA_V1: &str = "openagents.liquidity.pool.create_response.v1";

pub const DEPOSIT_QUOTE_REQUEST_SCHEMA_V1: &str =
    "openagents.liquidity.pool.deposit_quote_request.v1";
pub const DEPOSIT_QUOTE_RESPONSE_SCHEMA_V1: &str =
    "openagents.liquidity.pool.deposit_quote_response.v1";

pub const WITHDRAW_REQUEST_SCHEMA_V1: &str = "openagents.liquidity.pool.withdraw_request.v1";
pub const WITHDRAW_RESPONSE_SCHEMA_V1: &str = "openagents.liquidity.pool.withdraw_response.v1";

pub const POOL_STATUS_SCHEMA_V1: &str = "openagents.liquidity.pool.status.v1";
pub const POOL_SNAPSHOT_SCHEMA_V1: &str = "openagents.liquidity.pool.snapshot.v1";

pub const DEPOSIT_RECEIPT_SCHEMA_V1: &str = "openagents.liquidity.deposit_receipt.v1";
pub const WITHDRAW_REQUEST_RECEIPT_SCHEMA_V1: &str =
    "openagents.liquidity.withdraw_request_receipt.v1";
pub const WITHDRAW_SETTLEMENT_RECEIPT_SCHEMA_V1: &str =
    "openagents.liquidity.withdraw_settlement_receipt.v1";
pub const WITHDRAW_THROTTLE_RECEIPT_SCHEMA_V1: &str =
    "openagents.liquidity.withdraw_throttle_receipt.v1";
pub const POOL_SNAPSHOT_RECEIPT_SCHEMA_V1: &str = "openagents.liquidity.pool_snapshot_receipt.v1";
pub const POOL_WITHDRAW_THROTTLE_STATUS_SCHEMA_V1: &str =
    "openagents.liquidity.pool_withdraw_throttle_status.v1";

pub const POOL_SIGNER_SET_UPSERT_REQUEST_SCHEMA_V1: &str =
    "openagents.liquidity.pool_signer_set_upsert_request.v1";
pub const POOL_SIGNER_SET_RESPONSE_SCHEMA_V1: &str =
    "openagents.liquidity.pool_signer_set_response.v1";
pub const POOL_SIGNER_SET_SCHEMA_V1: &str = "openagents.liquidity.pool_signer_set.v1";

pub const POOL_TREASURY_OPEN_CHANNEL_REQUEST_SCHEMA_V1: &str =
    "openagents.liquidity.pool_treasury.open_channel_request.v1";
pub const POOL_TREASURY_CLOSE_CHANNEL_REQUEST_SCHEMA_V1: &str =
    "openagents.liquidity.pool_treasury.close_channel_request.v1";
pub const POOL_SIGNING_REQUEST_RESPONSE_SCHEMA_V1: &str =
    "openagents.liquidity.pool_signing_request_response.v1";
pub const POOL_SIGNING_REQUEST_LIST_SCHEMA_V1: &str =
    "openagents.liquidity.pool_signing_request_list.v1";
pub const POOL_SIGNING_APPROVAL_SUBMIT_REQUEST_SCHEMA_V1: &str =
    "openagents.liquidity.pool_signing_approval_submit_request.v1";
pub const POOL_SIGNING_REQUEST_EXECUTE_RESPONSE_SCHEMA_V1: &str =
    "openagents.liquidity.pool_signing_request_execute_response.v1";

pub const POOL_TREASURY_ACTION_RECEIPT_SCHEMA_V1: &str =
    "openagents.liquidity.pool_treasury_action_receipt.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreasuryActionClassV1 {
    InvoicePaySmall,
    InvoicePayLarge,
    OpenChannel,
    CloseChannel,
    OnchainWithdrawalBatch,
}

impl TreasuryActionClassV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvoicePaySmall => "invoice_pay_small",
            Self::InvoicePayLarge => "invoice_pay_large",
            Self::OpenChannel => "open_channel",
            Self::CloseChannel => "close_channel",
            Self::OnchainWithdrawalBatch => "onchain_withdrawal_batch",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSignerV1 {
    /// X-only pubkey hex (32 bytes) used for schnorr signing.
    pub pubkey: String,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSignerActionPolicyV1 {
    pub action_class: TreasuryActionClassV1,
    /// If set, actions at/below this amount can be executed with a single signer approval.
    #[serde(default)]
    pub single_signer_max_sats: Option<u64>,
    /// If set, overrides the signer-set default threshold for this action.
    #[serde(default)]
    pub required_signatures: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSignerPolicyV1 {
    pub schema: String,
    pub actions: Vec<PoolSignerActionPolicyV1>,
}

impl PoolSignerPolicyV1 {
    pub fn default_for_threshold(threshold: u32) -> Self {
        Self {
            schema: "openagents.liquidity.pool_signer_policy.v1".to_string(),
            actions: vec![
                PoolSignerActionPolicyV1 {
                    action_class: TreasuryActionClassV1::InvoicePaySmall,
                    single_signer_max_sats: Some(100_000),
                    required_signatures: Some(1),
                },
                PoolSignerActionPolicyV1 {
                    action_class: TreasuryActionClassV1::InvoicePayLarge,
                    single_signer_max_sats: None,
                    required_signatures: Some(threshold),
                },
                PoolSignerActionPolicyV1 {
                    action_class: TreasuryActionClassV1::OpenChannel,
                    single_signer_max_sats: None,
                    required_signatures: Some(threshold),
                },
                PoolSignerActionPolicyV1 {
                    action_class: TreasuryActionClassV1::CloseChannel,
                    single_signer_max_sats: None,
                    required_signatures: Some(threshold),
                },
                PoolSignerActionPolicyV1 {
                    action_class: TreasuryActionClassV1::OnchainWithdrawalBatch,
                    single_signer_max_sats: None,
                    required_signatures: Some(threshold),
                },
            ],
        }
    }

    pub fn policy_for_action(
        &self,
        action: TreasuryActionClassV1,
    ) -> Option<&PoolSignerActionPolicyV1> {
        self.actions
            .iter()
            .find(|entry| entry.action_class == action)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSignerSetUpsertRequestV1 {
    pub schema: String,
    pub threshold: u32,
    pub signers: Vec<PoolSignerV1>,
    #[serde(default)]
    pub policy: Option<PoolSignerPolicyV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSignerSetResponseV1 {
    pub schema: String,
    pub signer_set: PoolSignerSetRow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSignerSetRow {
    pub pool_id: String,
    pub schema: String,
    pub threshold: u32,
    pub signers: Vec<PoolSignerV1>,
    pub policy: PoolSignerPolicyV1,
    pub canonical_json_sha256: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolTreasuryOpenChannelRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub peer_id: String,
    pub amount_sats: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolTreasuryCloseChannelRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub channel_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSigningRequestRow {
    pub request_id: String,
    pub pool_id: String,
    pub action_class: String,
    pub idempotency_key: String,
    pub payload_json: Value,
    pub payload_sha256: String,
    pub required_signatures: u32,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_result_json: Option<Value>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSigningApprovalRow {
    pub approval_id: String,
    pub request_id: String,
    pub signer_pubkey: String,
    pub signature: ReceiptSignatureV1,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSigningRequestResponseV1 {
    pub schema: String,
    pub request: PoolSigningRequestRow,
    pub approvals: Vec<PoolSigningApprovalRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSigningRequestListResponseV1 {
    pub schema: String,
    pub requests: Vec<PoolSigningRequestRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSigningApprovalSubmitRequestV1 {
    pub schema: String,
    pub signature: ReceiptSignatureV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolTreasuryActionReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub pool_id: String,
    pub signing_request_id: String,
    pub action_class: String,
    pub payload_sha256: String,
    pub approvals: Vec<PoolSigningApprovalRow>,
    pub execution_result_json: Value,
    pub executed_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSigningRequestExecuteResponseV1 {
    pub schema: String,
    pub request: PoolSigningRequestRow,
    pub receipt: PoolTreasuryActionReceiptV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolKindV1 {
    Llp,
}

impl PoolKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llp => "llp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolPartitionKindV1 {
    Llp,
    Cep,
    Rrp,
}

impl PoolPartitionKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llp => "llp",
            Self::Cep => "cep",
            Self::Rrp => "rrp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PoolStatusV1 {
    Active,
    Paused,
    Disabled,
}

impl PoolStatusV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepositRailV1 {
    LightningInvoice,
    OnchainAddress,
}

impl DepositRailV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LightningInvoice => "lightning_invoice",
            Self::OnchainAddress => "onchain_address",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DepositStatusV1 {
    Quoted,
    Pending,
    Confirmed,
    Expired,
    Failed,
}

impl DepositStatusV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quoted => "quoted",
            Self::Pending => "pending",
            Self::Confirmed => "confirmed",
            Self::Expired => "expired",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalRailPreferenceV1 {
    Lightning,
    Onchain,
}

impl WithdrawalRailPreferenceV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lightning => "lightning",
            Self::Onchain => "onchain",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawalStatusV1 {
    Requested,
    Queued,
    Approved,
    Paid,
    Failed,
    Canceled,
}

impl WithdrawalStatusV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Queued => "queued",
            Self::Approved => "approved",
            Self::Paid => "paid",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCreateRequestV1 {
    pub schema: String,
    pub operator_id: String,
    #[serde(default)]
    pub pool_kind: Option<PoolKindV1>,
    #[serde(default)]
    pub status: Option<PoolStatusV1>,
    #[serde(default)]
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolCreateResponseV1 {
    pub schema: String,
    pub pool: PoolRow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositQuoteRequestV1 {
    pub schema: String,
    pub lp_id: String,
    pub idempotency_key: String,
    #[serde(default)]
    pub partition_kind: Option<PoolPartitionKindV1>,
    pub rail: DepositRailV1,
    pub amount_sats: u64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub expiry_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositQuoteResponseV1 {
    pub schema: String,
    pub deposit: DepositRow,
    pub receipt: DepositReceiptV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawRequestV1 {
    pub schema: String,
    pub lp_id: String,
    pub idempotency_key: String,
    #[serde(default)]
    pub partition_kind: Option<PoolPartitionKindV1>,
    pub shares_burned: u64,
    pub rail_preference: WithdrawalRailPreferenceV1,
    #[serde(default)]
    pub payout_invoice_bolt11: Option<String>,
    #[serde(default)]
    pub payout_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawResponseV1 {
    pub schema: String,
    pub withdrawal: WithdrawalRow,
    pub receipt: WithdrawRequestReceiptV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw_throttle: Option<WithdrawThrottleStatusV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatusResponseV1 {
    pub schema: String,
    pub pool: PoolRow,
    pub share_price_sats: i64,
    pub total_shares: i64,
    pub pending_withdrawals_sats_estimate: i64,
    #[serde(default)]
    pub partitions: Vec<PoolPartitionStatusV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withdraw_throttle: Option<WithdrawThrottleStatusV1>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithdrawThrottleModeV1 {
    Normal,
    Stressed,
    Halted,
}

impl WithdrawThrottleModeV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Stressed => "stressed",
            Self::Halted => "halted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawThrottleStatusV1 {
    pub schema: String,
    pub pool_id: String,
    pub lp_mode_enabled: bool,
    pub mode: WithdrawThrottleModeV1,
    #[serde(default)]
    pub reasons: Vec<String>,
    pub liabilities_pressure_bps: u32,
    pub pending_withdrawals_sats_estimate: i64,
    pub cep_reserved_commitments_sats: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_connected_ratio_bps: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_outbound_coverage_bps: Option<u32>,
    pub extra_delay_hours: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_cap_per_tick: Option<u32>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolPartitionStatusV1 {
    pub partition_kind: PoolPartitionKindV1,
    pub assets_sats_estimate: i64,
    pub liabilities_sats_estimate: i64,
    pub share_price_sats: i64,
    pub total_shares: i64,
    pub pending_withdrawals_sats_estimate: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSnapshotResponseV1 {
    pub schema: String,
    pub snapshot: PoolSnapshotRow,
    pub receipt: PoolSnapshotReceiptV1,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub pool_id: String,
    pub lp_id: String,
    pub deposit_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_kind: Option<PoolPartitionKindV1>,
    pub rail: DepositRailV1,
    pub amount_sats: u64,
    pub share_price_sats: i64,
    pub shares_minted: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawRequestReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub pool_id: String,
    pub lp_id: String,
    pub withdrawal_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_kind: Option<PoolPartitionKindV1>,
    pub shares_burned: i64,
    pub amount_sats_estimate: i64,
    pub rail_preference: WithdrawalRailPreferenceV1,
    pub earliest_settlement_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawSettlementReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub pool_id: String,
    pub lp_id: String,
    pub withdrawal_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_kind: Option<PoolPartitionKindV1>,
    pub amount_sats: i64,
    pub rail: WithdrawalRailPreferenceV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_invoice_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_receipt_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_payment_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_txid: Option<String>,
    pub paid_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawThrottleReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub pool_id: String,
    pub withdrawal_id: String,
    pub action: String,
    pub mode: WithdrawThrottleModeV1,
    pub extra_delay_hours: i64,
    #[serde(default)]
    pub reasons: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution_cap_per_tick: Option<u32>,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSnapshotReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub pool_id: String,
    pub snapshot_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_kind: Option<PoolPartitionKindV1>,
    pub as_of: DateTime<Utc>,
    pub assets_json_sha256: String,
    pub liabilities_json_sha256: String,
    pub share_price_sats: i64,
    pub created_at: DateTime<Utc>,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<ReceiptSignatureV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolRow {
    pub pool_id: String,
    pub pool_kind: String,
    pub operator_id: String,
    pub status: String,
    pub config: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LpAccountRow {
    pub pool_id: String,
    pub partition_kind: String,
    pub lp_id: String,
    pub shares_total: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositRow {
    pub deposit_id: String,
    pub pool_id: String,
    pub partition_kind: String,
    pub lp_id: String,
    pub rail: String,
    pub amount_sats: i64,
    pub share_price_sats: i64,
    pub shares_minted: i64,
    pub status: String,
    pub idempotency_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_bolt11: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invoice_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_address: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawalRow {
    pub withdrawal_id: String,
    pub pool_id: String,
    pub partition_kind: String,
    pub lp_id: String,
    pub shares_burned: i64,
    pub amount_sats_estimate: i64,
    pub rail_preference: String,
    pub status: String,
    pub idempotency_key: String,
    pub earliest_settlement_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_invoice_bolt11: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_invoice_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payout_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallet_receipt_sha256: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paid_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolSnapshotRow {
    pub snapshot_id: String,
    pub pool_id: String,
    pub partition_kind: String,
    pub as_of: DateTime<Utc>,
    pub assets_json: Value,
    pub liabilities_json: Value,
    pub share_price_sats: i64,
    pub canonical_json_sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature_json: Option<Value>,
    pub created_at: DateTime<Utc>,
}
