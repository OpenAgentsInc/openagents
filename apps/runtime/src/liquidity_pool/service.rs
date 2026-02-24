use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use openagents_l402::Bolt11;

use crate::artifacts::{sign_receipt_sha256, verify_receipt_signature};
use crate::bridge::{
    BridgeNostrPublisher, LiquidityReceiptPointerV1, PoolSnapshotBridgeV1, bridge_relays_from_env,
    build_liquidity_receipt_pointer_event, build_pool_snapshot_event,
};
use crate::lightning_node::{LightningNode, NoopLightningNode};
use crate::liquidity_pool::store::{
    DepositInsertInput, LiquidityPoolStore, LiquidityPoolStoreError, ReceiptInsertInput,
    SignerSetUpsertInput, SigningApprovalInsertInput, SigningRequestInsertInput,
    WithdrawalInsertInput,
};
use crate::liquidity_pool::types::{
    DEPOSIT_QUOTE_REQUEST_SCHEMA_V1, DEPOSIT_QUOTE_RESPONSE_SCHEMA_V1, DEPOSIT_RECEIPT_SCHEMA_V1,
    DepositQuoteRequestV1, DepositQuoteResponseV1, DepositRailV1, DepositReceiptV1, DepositRow,
    DepositStatusV1, POOL_CREATE_REQUEST_SCHEMA_V1, POOL_CREATE_RESPONSE_SCHEMA_V1,
    POOL_SIGNER_SET_RESPONSE_SCHEMA_V1, POOL_SIGNER_SET_SCHEMA_V1,
    POOL_SIGNER_SET_UPSERT_REQUEST_SCHEMA_V1, POOL_SIGNING_APPROVAL_SUBMIT_REQUEST_SCHEMA_V1,
    POOL_SIGNING_REQUEST_EXECUTE_RESPONSE_SCHEMA_V1, POOL_SIGNING_REQUEST_LIST_SCHEMA_V1,
    POOL_SIGNING_REQUEST_RESPONSE_SCHEMA_V1, POOL_SNAPSHOT_RECEIPT_SCHEMA_V1,
    POOL_SNAPSHOT_SCHEMA_V1, POOL_STATUS_SCHEMA_V1, POOL_TREASURY_ACTION_RECEIPT_SCHEMA_V1,
    POOL_TREASURY_CLOSE_CHANNEL_REQUEST_SCHEMA_V1, POOL_TREASURY_OPEN_CHANNEL_REQUEST_SCHEMA_V1,
    PoolCreateRequestV1, PoolCreateResponseV1, PoolKindV1, PoolPartitionKindV1,
    PoolPartitionStatusV1, PoolRow, PoolSignerPolicyV1, PoolSignerSetResponseV1, PoolSignerSetRow,
    PoolSignerSetUpsertRequestV1, PoolSigningApprovalRow, PoolSigningApprovalSubmitRequestV1,
    PoolSigningRequestExecuteResponseV1, PoolSigningRequestListResponseV1,
    PoolSigningRequestResponseV1, PoolSigningRequestRow, PoolSnapshotReceiptV1,
    PoolSnapshotResponseV1, PoolSnapshotRow, PoolStatusResponseV1, PoolStatusV1,
    PoolTreasuryActionReceiptV1, PoolTreasuryCloseChannelRequestV1,
    PoolTreasuryOpenChannelRequestV1, TreasuryActionClassV1, WITHDRAW_REQUEST_RECEIPT_SCHEMA_V1,
    WITHDRAW_REQUEST_SCHEMA_V1, WITHDRAW_RESPONSE_SCHEMA_V1, WITHDRAW_SETTLEMENT_RECEIPT_SCHEMA_V1,
    WithdrawRequestReceiptV1, WithdrawRequestV1, WithdrawResponseV1, WithdrawSettlementReceiptV1,
    WithdrawalRailPreferenceV1, WithdrawalRow, WithdrawalStatusV1,
};

const SIGNING_REQUEST_STATUS_PENDING: &str = "pending";
const SIGNING_REQUEST_STATUS_EXECUTED: &str = "executed";

const WITHDRAWAL_WALLET_HOST: &str = "l402.openagents.com";
const WITHDRAWAL_AUTOPAY_MAX_SATS: u64 = 100_000;
const WITHDRAWAL_EXECUTION_DEFAULT_LIMIT: i64 = 50;

#[derive(Debug, thiserror::Error)]
pub enum LiquidityPoolError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("not found")]
    NotFound,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("dependency unavailable: {0}")]
    DependencyUnavailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl LiquidityPoolError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::NotFound => "not_found",
            Self::Conflict(_) => "conflict",
            Self::DependencyUnavailable(_) => "dependency_unavailable",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn message(&self) -> String {
        match self {
            Self::InvalidRequest(message)
            | Self::Conflict(message)
            | Self::DependencyUnavailable(message)
            | Self::Internal(message) => message.clone(),
            Self::NotFound => "not found".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ExecuteDueWithdrawalsOutcome {
    pub attempted: usize,
    pub paid: usize,
    pub signing_requests_created: usize,
    pub failed: usize,
}

#[derive(Clone)]
pub struct LiquidityPoolService {
    store: Arc<dyn LiquidityPoolStore>,
    wallet: Arc<dyn WalletExecutorClient>,
    lightning_node: Arc<dyn LightningNode>,
    default_withdraw_delay_hours: i64,
    receipt_signing_key: Option<[u8; 32]>,
}

impl LiquidityPoolService {
    pub fn new(
        store: Arc<dyn LiquidityPoolStore>,
        wallet: Arc<dyn WalletExecutorClient>,
        receipt_signing_key: Option<[u8; 32]>,
    ) -> Self {
        Self::new_with_lightning_node(
            store,
            wallet,
            Arc::new(NoopLightningNode),
            receipt_signing_key,
        )
    }

    pub fn new_with_lightning_node(
        store: Arc<dyn LiquidityPoolStore>,
        wallet: Arc<dyn WalletExecutorClient>,
        lightning_node: Arc<dyn LightningNode>,
        receipt_signing_key: Option<[u8; 32]>,
    ) -> Self {
        Self {
            store,
            wallet,
            lightning_node,
            default_withdraw_delay_hours: 24,
            receipt_signing_key,
        }
    }

    #[must_use]
    pub fn with_withdraw_delay_hours(mut self, hours: i64) -> Self {
        self.default_withdraw_delay_hours = hours.clamp(0, 168);
        self
    }

    pub async fn create_pool(
        &self,
        pool_id: &str,
        body: PoolCreateRequestV1,
    ) -> Result<PoolCreateResponseV1, LiquidityPoolError> {
        if body.schema.trim() != POOL_CREATE_REQUEST_SCHEMA_V1 {
            return Err(LiquidityPoolError::InvalidRequest(format!(
                "schema must be {POOL_CREATE_REQUEST_SCHEMA_V1}"
            )));
        }

        let operator_id = body.operator_id.trim().to_string();
        if operator_id.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "operator_id is required".to_string(),
            ));
        }

        let pool_kind = body.pool_kind.unwrap_or(PoolKindV1::Llp);
        let status = body.status.unwrap_or(PoolStatusV1::Active);
        let created_at = Utc::now();

        let pool = PoolRow {
            pool_id: pool_id.trim().to_string(),
            pool_kind: pool_kind.as_str().to_string(),
            operator_id,
            status: status.as_str().to_string(),
            config: body.config,
            created_at,
        };

        let stored = self
            .store
            .create_or_get_pool(pool)
            .await
            .map_err(map_store_error)?;

        Ok(PoolCreateResponseV1 {
            schema: POOL_CREATE_RESPONSE_SCHEMA_V1.to_string(),
            pool: stored,
        })
    }

    pub async fn upsert_signer_set(
        &self,
        pool_id: &str,
        body: PoolSignerSetUpsertRequestV1,
    ) -> Result<PoolSignerSetResponseV1, LiquidityPoolError> {
        if body.schema.trim() != POOL_SIGNER_SET_UPSERT_REQUEST_SCHEMA_V1 {
            return Err(LiquidityPoolError::InvalidRequest(format!(
                "schema must be {POOL_SIGNER_SET_UPSERT_REQUEST_SCHEMA_V1}"
            )));
        }

        self.store
            .get_pool(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;

        if body.signers.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "signers is required".to_string(),
            ));
        }

        if body.threshold == 0 {
            return Err(LiquidityPoolError::InvalidRequest(
                "threshold must be > 0".to_string(),
            ));
        }
        if body.threshold as usize > body.signers.len() {
            return Err(LiquidityPoolError::InvalidRequest(
                "threshold cannot exceed signer count".to_string(),
            ));
        }

        let mut seen = HashSet::new();
        let mut signers = body.signers;
        for signer in signers.iter_mut() {
            signer.pubkey = normalize_xonly_pubkey_hex(signer.pubkey.as_str())?;
            if !seen.insert(signer.pubkey.clone()) {
                return Err(LiquidityPoolError::InvalidRequest(
                    "duplicate signer pubkey".to_string(),
                ));
            }
        }

        let policy = body
            .policy
            .unwrap_or_else(|| PoolSignerPolicyV1::default_for_threshold(body.threshold));

        for action in &policy.actions {
            if let Some(required) = action.required_signatures {
                if required == 0 {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "required_signatures must be > 0".to_string(),
                    ));
                }
                if required as usize > signers.len() {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "required_signatures cannot exceed signer count".to_string(),
                    ));
                }
            }
        }

        #[derive(Serialize)]
        struct SignerSetHashInput<'a> {
            schema: &'a str,
            pool_id: &'a str,
            threshold: u32,
            signers: &'a [crate::liquidity_pool::types::PoolSignerV1],
            policy: &'a PoolSignerPolicyV1,
        }

        let canonical_json_sha256 = canonical_sha256(&SignerSetHashInput {
            schema: POOL_SIGNER_SET_SCHEMA_V1,
            pool_id,
            threshold: body.threshold,
            signers: &signers,
            policy: &policy,
        })
        .map_err(LiquidityPoolError::Internal)?;

        let now = Utc::now();
        let signer_set = self
            .store
            .upsert_signer_set(SignerSetUpsertInput {
                pool_id: pool_id.to_string(),
                schema: POOL_SIGNER_SET_SCHEMA_V1.to_string(),
                threshold: i64::from(body.threshold),
                signers_json: serde_json::to_value(&signers)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                policy_json: serde_json::to_value(&policy)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                canonical_json_sha256,
                updated_at: now,
            })
            .await
            .map_err(map_store_error)?;

        Ok(PoolSignerSetResponseV1 {
            schema: POOL_SIGNER_SET_RESPONSE_SCHEMA_V1.to_string(),
            signer_set,
        })
    }

    pub async fn get_signer_set(
        &self,
        pool_id: &str,
    ) -> Result<Option<PoolSignerSetRow>, LiquidityPoolError> {
        self.store
            .get_signer_set(pool_id)
            .await
            .map_err(map_store_error)
    }

    pub async fn list_signing_requests(
        &self,
        pool_id: &str,
        status: Option<String>,
        limit: Option<i64>,
    ) -> Result<PoolSigningRequestListResponseV1, LiquidityPoolError> {
        let limit = limit.unwrap_or(50).max(1).min(500);
        let requests = self
            .store
            .list_signing_requests(
                pool_id,
                status.as_deref().map(str::trim).filter(|v| !v.is_empty()),
                limit,
            )
            .await
            .map_err(map_store_error)?;

        Ok(PoolSigningRequestListResponseV1 {
            schema: POOL_SIGNING_REQUEST_LIST_SCHEMA_V1.to_string(),
            requests,
        })
    }

    pub async fn treasury_open_channel_request(
        &self,
        pool_id: &str,
        body: PoolTreasuryOpenChannelRequestV1,
    ) -> Result<PoolSigningRequestResponseV1, LiquidityPoolError> {
        if body.schema.trim() != POOL_TREASURY_OPEN_CHANNEL_REQUEST_SCHEMA_V1 {
            return Err(LiquidityPoolError::InvalidRequest(format!(
                "schema must be {POOL_TREASURY_OPEN_CHANNEL_REQUEST_SCHEMA_V1}"
            )));
        }

        let idempotency_key = body.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "idempotency_key is required".to_string(),
            ));
        }
        let peer_id = body.peer_id.trim().to_string();
        if peer_id.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "peer_id is required".to_string(),
            ));
        }
        if body.amount_sats == 0 {
            return Err(LiquidityPoolError::InvalidRequest(
                "amount_sats must be > 0".to_string(),
            ));
        }

        let signer_set = self
            .store
            .get_signer_set(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or_else(|| {
                LiquidityPoolError::Conflict("pool signer set not configured".to_string())
            })?;

        let required_signatures = required_signatures_for_action(
            &signer_set,
            TreasuryActionClassV1::OpenChannel,
            Some(body.amount_sats),
        )?;

        #[derive(Serialize)]
        struct Payload<'a> {
            schema: &'a str,
            pool_id: &'a str,
            action_class: &'a str,
            idempotency_key: &'a str,
            peer_id: &'a str,
            amount_sats: u64,
        }

        let payload = Payload {
            schema: POOL_TREASURY_OPEN_CHANNEL_REQUEST_SCHEMA_V1,
            pool_id,
            action_class: TreasuryActionClassV1::OpenChannel.as_str(),
            idempotency_key: idempotency_key.as_str(),
            peer_id: peer_id.as_str(),
            amount_sats: body.amount_sats,
        };
        let payload_sha256 = canonical_sha256(&payload).map_err(LiquidityPoolError::Internal)?;
        let payload_json = serde_json::to_value(&payload)
            .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?;

        let now = Utc::now();
        let request = self
            .store
            .create_or_get_signing_request(SigningRequestInsertInput {
                request_id: format!("sigreq_{}", Uuid::now_v7()),
                pool_id: pool_id.to_string(),
                action_class: TreasuryActionClassV1::OpenChannel.as_str().to_string(),
                idempotency_key: idempotency_key.clone(),
                payload_json,
                payload_sha256,
                required_signatures: i64::from(required_signatures),
                status: SIGNING_REQUEST_STATUS_PENDING.to_string(),
                created_at: now,
            })
            .await
            .map_err(map_store_error)?;

        let approvals = self
            .store
            .list_signing_approvals(request.request_id.as_str())
            .await
            .map_err(map_store_error)?;

        Ok(PoolSigningRequestResponseV1 {
            schema: POOL_SIGNING_REQUEST_RESPONSE_SCHEMA_V1.to_string(),
            request,
            approvals,
        })
    }

    pub async fn treasury_close_channel_request(
        &self,
        pool_id: &str,
        body: PoolTreasuryCloseChannelRequestV1,
    ) -> Result<PoolSigningRequestResponseV1, LiquidityPoolError> {
        if body.schema.trim() != POOL_TREASURY_CLOSE_CHANNEL_REQUEST_SCHEMA_V1 {
            return Err(LiquidityPoolError::InvalidRequest(format!(
                "schema must be {POOL_TREASURY_CLOSE_CHANNEL_REQUEST_SCHEMA_V1}"
            )));
        }

        let idempotency_key = body.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "idempotency_key is required".to_string(),
            ));
        }
        let channel_id = body.channel_id.trim().to_string();
        if channel_id.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "channel_id is required".to_string(),
            ));
        }

        let signer_set = self
            .store
            .get_signer_set(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or_else(|| {
                LiquidityPoolError::Conflict("pool signer set not configured".to_string())
            })?;

        let required_signatures =
            required_signatures_for_action(&signer_set, TreasuryActionClassV1::CloseChannel, None)?;

        #[derive(Serialize)]
        struct Payload<'a> {
            schema: &'a str,
            pool_id: &'a str,
            action_class: &'a str,
            idempotency_key: &'a str,
            channel_id: &'a str,
        }

        let payload = Payload {
            schema: POOL_TREASURY_CLOSE_CHANNEL_REQUEST_SCHEMA_V1,
            pool_id,
            action_class: TreasuryActionClassV1::CloseChannel.as_str(),
            idempotency_key: idempotency_key.as_str(),
            channel_id: channel_id.as_str(),
        };
        let payload_sha256 = canonical_sha256(&payload).map_err(LiquidityPoolError::Internal)?;
        let payload_json = serde_json::to_value(&payload)
            .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?;

        let now = Utc::now();
        let request = self
            .store
            .create_or_get_signing_request(SigningRequestInsertInput {
                request_id: format!("sigreq_{}", Uuid::now_v7()),
                pool_id: pool_id.to_string(),
                action_class: TreasuryActionClassV1::CloseChannel.as_str().to_string(),
                idempotency_key: idempotency_key.clone(),
                payload_json,
                payload_sha256,
                required_signatures: i64::from(required_signatures),
                status: SIGNING_REQUEST_STATUS_PENDING.to_string(),
                created_at: now,
            })
            .await
            .map_err(map_store_error)?;

        let approvals = self
            .store
            .list_signing_approvals(request.request_id.as_str())
            .await
            .map_err(map_store_error)?;

        Ok(PoolSigningRequestResponseV1 {
            schema: POOL_SIGNING_REQUEST_RESPONSE_SCHEMA_V1.to_string(),
            request,
            approvals,
        })
    }

    pub async fn submit_signing_approval(
        &self,
        pool_id: &str,
        request_id: &str,
        body: PoolSigningApprovalSubmitRequestV1,
    ) -> Result<PoolSigningRequestResponseV1, LiquidityPoolError> {
        if body.schema.trim() != POOL_SIGNING_APPROVAL_SUBMIT_REQUEST_SCHEMA_V1 {
            return Err(LiquidityPoolError::InvalidRequest(format!(
                "schema must be {POOL_SIGNING_APPROVAL_SUBMIT_REQUEST_SCHEMA_V1}"
            )));
        }

        let request = self
            .store
            .get_signing_request(pool_id, request_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;

        let signer_set = self
            .store
            .get_signer_set(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or_else(|| {
                LiquidityPoolError::Conflict("pool signer set not configured".to_string())
            })?;

        let signer_pubkey = normalize_xonly_pubkey_hex(body.signature.signer_pubkey.as_str())?;
        if !signer_set
            .signers
            .iter()
            .any(|signer| signer.pubkey.eq_ignore_ascii_case(signer_pubkey.as_str()))
        {
            return Err(LiquidityPoolError::InvalidRequest(
                "signer is not in signer set".to_string(),
            ));
        }

        if body.signature.signed_sha256.trim() != request.payload_sha256 {
            return Err(LiquidityPoolError::InvalidRequest(
                "signed_sha256 must match signing request payload sha".to_string(),
            ));
        }

        let mut signature = body.signature;
        signature.signer_pubkey = signer_pubkey.clone();
        signature.signed_sha256 = request.payload_sha256.clone();

        let is_valid = verify_receipt_signature(&signature)
            .map_err(|error| LiquidityPoolError::InvalidRequest(error.to_string()))?;
        if !is_valid {
            return Err(LiquidityPoolError::InvalidRequest(
                "signature verification failed".to_string(),
            ));
        }

        let now = Utc::now();
        let _stored = self
            .store
            .create_or_get_signing_approval(SigningApprovalInsertInput {
                approval_id: format!("sigapp_{}", Uuid::now_v7()),
                request_id: request.request_id.clone(),
                signer_pubkey: signer_pubkey.clone(),
                signature_json: serde_json::to_value(&signature)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                created_at: now,
            })
            .await
            .map_err(map_store_error)?;

        let approvals = self
            .store
            .list_signing_approvals(request.request_id.as_str())
            .await
            .map_err(map_store_error)?;

        Ok(PoolSigningRequestResponseV1 {
            schema: POOL_SIGNING_REQUEST_RESPONSE_SCHEMA_V1.to_string(),
            request,
            approvals,
        })
    }

    pub async fn execute_signing_request(
        &self,
        pool_id: &str,
        request_id: &str,
    ) -> Result<PoolSigningRequestExecuteResponseV1, LiquidityPoolError> {
        let request = self
            .store
            .get_signing_request(pool_id, request_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;

        if request.status != SIGNING_REQUEST_STATUS_PENDING {
            return Err(LiquidityPoolError::Conflict(
                "signing request is not pending".to_string(),
            ));
        }

        let signer_set = self
            .store
            .get_signer_set(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or_else(|| {
                LiquidityPoolError::Conflict("pool signer set not configured".to_string())
            })?;

        let mut approvals = self
            .store
            .list_signing_approvals(request.request_id.as_str())
            .await
            .map_err(map_store_error)?;
        approvals.sort_by(|a, b| a.signer_pubkey.cmp(&b.signer_pubkey));

        let allowed_pubkeys = signer_set
            .signers
            .iter()
            .map(|signer| signer.pubkey.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        approvals.retain(|approval| {
            allowed_pubkeys.contains(&approval.signer_pubkey.to_ascii_lowercase())
        });

        if approvals.len() < request.required_signatures as usize {
            return Err(LiquidityPoolError::Conflict(
                "signing quorum not met".to_string(),
            ));
        }

        let action_class = request.action_class.clone();
        let execution_result_json = match action_class.as_str() {
            "invoice_pay_small" | "invoice_pay_large" => {
                let withdrawal_id = request
                    .payload_json
                    .get("withdrawal_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let amount_sats = request
                    .payload_json
                    .get("amount_sats")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let payout_invoice_hash = request
                    .payload_json
                    .get("payout_invoice_hash")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();

                if withdrawal_id.is_empty() || amount_sats == 0 || payout_invoice_hash.is_empty() {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "invalid invoice_pay payload".to_string(),
                    ));
                }

                let Some(withdrawal) = self
                    .store
                    .get_withdrawal(pool_id, withdrawal_id.as_str())
                    .await
                    .map_err(map_store_error)?
                else {
                    return Err(LiquidityPoolError::NotFound);
                };

                if withdrawal.status == WithdrawalStatusV1::Paid.as_str()
                    || withdrawal.paid_at.is_some()
                {
                    json!({
                        "schema": "openagents.liquidity.pool.withdrawal_execution_result.v1",
                        "status": "already_paid",
                        "withdrawal_id": withdrawal.withdrawal_id,
                        "wallet_receipt_sha256": withdrawal.wallet_receipt_sha256,
                        "paid_at": withdrawal.paid_at.map(|t| t.to_rfc3339()),
                    })
                } else {
                    if withdrawal.status != WithdrawalStatusV1::Queued.as_str()
                        && withdrawal.status != WithdrawalStatusV1::Approved.as_str()
                    {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal not in a payable state".to_string(),
                        ));
                    }

                    if withdrawal.earliest_settlement_at > Utc::now() {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal is not yet due".to_string(),
                        ));
                    }

                    if withdrawal.amount_sats_estimate != i64::try_from(amount_sats).unwrap_or(0) {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal amount does not match signing payload".to_string(),
                        ));
                    }

                    let expected_invoice_hash = withdrawal
                        .payout_invoice_hash
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_ascii_lowercase();
                    if expected_invoice_hash.is_empty()
                        || expected_invoice_hash != payout_invoice_hash.to_ascii_lowercase()
                    {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal invoice hash does not match signing payload".to_string(),
                        ));
                    }

                    let invoice = withdrawal
                        .payout_invoice_bolt11
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            LiquidityPoolError::Conflict(
                                "withdrawal payout invoice missing".to_string(),
                            )
                        })?;
                    let computed_hash = hex::encode(Sha256::digest(invoice.as_bytes()));
                    if !computed_hash.eq_ignore_ascii_case(expected_invoice_hash.as_str()) {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal payout invoice hash mismatch".to_string(),
                        ));
                    }

                    let expected_msats = amount_sats.saturating_mul(1_000);
                    let invoice_amount_msats = Bolt11::amount_msats(invoice).ok_or_else(|| {
                        LiquidityPoolError::Conflict(
                            "withdrawal payout invoice missing amount".to_string(),
                        )
                    })?;
                    if invoice_amount_msats != expected_msats {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal payout invoice amount mismatch".to_string(),
                        ));
                    }

                    let wallet_request_id = wallet_request_id(
                        "withdrawal_signing",
                        withdrawal.pool_id.as_str(),
                        withdrawal.lp_id.as_str(),
                        request.request_id.as_str(),
                    );
                    let paid = self
                        .wallet
                        .pay_bolt11(
                            wallet_request_id.as_str(),
                            invoice.to_string(),
                            invoice_amount_msats,
                            WITHDRAWAL_WALLET_HOST.to_string(),
                        )
                        .await?;

                    let paid_at = Utc::now();
                    let updated = self
                        .store
                        .mark_withdrawal_paid_and_burn_shares(
                            withdrawal.pool_id.as_str(),
                            withdrawal.withdrawal_id.as_str(),
                            paid.wallet_receipt_sha256.as_str(),
                            paid_at,
                        )
                        .await
                        .map_err(map_store_error)?;

                    let _receipt = self
                        .persist_withdraw_settlement_receipt(
                            &updated,
                            WithdrawalRailPreferenceV1::Lightning,
                            paid.wallet_receipt_sha256.as_str(),
                            Some(paid.payment_id.as_str()),
                            None,
                            paid_at,
                        )
                        .await?;

                    json!({
                        "schema": "openagents.liquidity.pool.withdrawal_execution_result.v1",
                        "withdrawal_id": updated.withdrawal_id,
                        "rail": "lightning",
                        "amount_sats": amount_sats,
                        "wallet_receipt_sha256": paid.wallet_receipt_sha256,
                        "payment_id": paid.payment_id,
                        "paid_at": paid_at.to_rfc3339(),
                    })
                }
            }
            "onchain_withdrawal_batch" => {
                let withdrawal_id = request
                    .payload_json
                    .get("withdrawal_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let amount_sats = request
                    .payload_json
                    .get("amount_sats")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let payout_address = request
                    .payload_json
                    .get("payout_address")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                let confirmation_speed = request
                    .payload_json
                    .get("confirmation_speed")
                    .and_then(Value::as_str)
                    .unwrap_or("normal")
                    .trim()
                    .to_string();

                if withdrawal_id.is_empty() || amount_sats == 0 || payout_address.is_empty() {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "invalid onchain_withdrawal_batch payload".to_string(),
                    ));
                }

                let Some(withdrawal) = self
                    .store
                    .get_withdrawal(pool_id, withdrawal_id.as_str())
                    .await
                    .map_err(map_store_error)?
                else {
                    return Err(LiquidityPoolError::NotFound);
                };

                if withdrawal.status == WithdrawalStatusV1::Paid.as_str()
                    || withdrawal.paid_at.is_some()
                {
                    json!({
                        "schema": "openagents.liquidity.pool.withdrawal_execution_result.v1",
                        "status": "already_paid",
                        "withdrawal_id": withdrawal.withdrawal_id,
                        "wallet_receipt_sha256": withdrawal.wallet_receipt_sha256,
                        "paid_at": withdrawal.paid_at.map(|t| t.to_rfc3339()),
                    })
                } else {
                    if withdrawal.status != WithdrawalStatusV1::Queued.as_str()
                        && withdrawal.status != WithdrawalStatusV1::Approved.as_str()
                    {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal not in a payable state".to_string(),
                        ));
                    }
                    if withdrawal.earliest_settlement_at > Utc::now() {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal is not yet due".to_string(),
                        ));
                    }
                    if withdrawal.amount_sats_estimate != i64::try_from(amount_sats).unwrap_or(0) {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal amount does not match signing payload".to_string(),
                        ));
                    }

                    let expected_address = withdrawal
                        .payout_address
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .to_string();
                    if expected_address.is_empty()
                        || expected_address.to_ascii_lowercase()
                            != payout_address.to_ascii_lowercase()
                    {
                        return Err(LiquidityPoolError::Conflict(
                            "withdrawal payout address does not match signing payload".to_string(),
                        ));
                    }

                    let wallet_request_id = wallet_request_id(
                        "withdrawal_onchain",
                        withdrawal.pool_id.as_str(),
                        withdrawal.lp_id.as_str(),
                        request.request_id.as_str(),
                    );
                    let sent = self
                        .wallet
                        .send_onchain(
                            wallet_request_id.as_str(),
                            payout_address,
                            amount_sats,
                            confirmation_speed,
                        )
                        .await?;

                    let paid_at = Utc::now();
                    let updated = self
                        .store
                        .mark_withdrawal_paid_and_burn_shares(
                            withdrawal.pool_id.as_str(),
                            withdrawal.withdrawal_id.as_str(),
                            sent.wallet_receipt_sha256.as_str(),
                            paid_at,
                        )
                        .await
                        .map_err(map_store_error)?;

                    let _receipt = self
                        .persist_withdraw_settlement_receipt(
                            &updated,
                            WithdrawalRailPreferenceV1::Onchain,
                            sent.wallet_receipt_sha256.as_str(),
                            None,
                            Some(sent.txid.as_str()),
                            paid_at,
                        )
                        .await?;

                    json!({
                        "schema": "openagents.liquidity.pool.withdrawal_execution_result.v1",
                        "withdrawal_id": updated.withdrawal_id,
                        "rail": "onchain",
                        "amount_sats": amount_sats,
                        "wallet_receipt_sha256": sent.wallet_receipt_sha256,
                        "txid": sent.txid,
                        "paid_at": paid_at.to_rfc3339(),
                    })
                }
            }
            "open_channel" => {
                let peer_id = request
                    .payload_json
                    .get("peer_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let amount_sats = request
                    .payload_json
                    .get("amount_sats")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                if peer_id.is_empty() || amount_sats == 0 {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "invalid open_channel payload".to_string(),
                    ));
                }
                let result = self
                    .lightning_node
                    .open_channel(peer_id.as_str(), amount_sats)
                    .await
                    .map_err(|error| LiquidityPoolError::DependencyUnavailable(error.message()))?;
                serde_json::to_value(&result)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?
            }
            "close_channel" => {
                let channel_id = request
                    .payload_json
                    .get("channel_id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                if channel_id.is_empty() {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "invalid close_channel payload".to_string(),
                    ));
                }
                let result = self
                    .lightning_node
                    .close_channel(channel_id.as_str())
                    .await
                    .map_err(|error| LiquidityPoolError::DependencyUnavailable(error.message()))?;
                serde_json::to_value(&result)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?
            }
            _ => {
                return Err(LiquidityPoolError::InvalidRequest(
                    "unsupported signing request action_class".to_string(),
                ));
            }
        };

        let executed_at = Utc::now();
        let request = self
            .store
            .mark_signing_request_executed(
                request.request_id.as_str(),
                SIGNING_REQUEST_STATUS_EXECUTED,
                execution_result_json.clone(),
                executed_at,
            )
            .await
            .map_err(map_store_error)?;

        let receipt = build_treasury_action_receipt(
            &request,
            &approvals,
            &execution_result_json,
            executed_at,
            self.receipt_signing_key.as_ref(),
        )?;

        let signature_json = match receipt.signature.as_ref() {
            Some(sig) => Some(
                serde_json::to_value(sig)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
            )
            .filter(|value| !value.is_null()),
            None => None,
        };

        self.store
            .put_receipt(ReceiptInsertInput {
                receipt_id: receipt.receipt_id.clone(),
                entity_kind: "treasury_action".to_string(),
                entity_id: request.request_id.clone(),
                schema: receipt.schema.clone(),
                canonical_json_sha256: receipt.canonical_json_sha256.clone(),
                signature_json,
                receipt_json: serde_json::to_value(&receipt)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                created_at: executed_at,
            })
            .await
            .map_err(map_store_error)?;

        Ok(PoolSigningRequestExecuteResponseV1 {
            schema: POOL_SIGNING_REQUEST_EXECUTE_RESPONSE_SCHEMA_V1.to_string(),
            request,
            receipt,
        })
    }

    pub async fn deposit_quote(
        &self,
        pool_id: &str,
        body: DepositQuoteRequestV1,
    ) -> Result<DepositQuoteResponseV1, LiquidityPoolError> {
        if body.schema.trim() != DEPOSIT_QUOTE_REQUEST_SCHEMA_V1 {
            return Err(LiquidityPoolError::InvalidRequest(format!(
                "schema must be {DEPOSIT_QUOTE_REQUEST_SCHEMA_V1}"
            )));
        }

        let pool = self
            .store
            .get_pool(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;
        if pool.status != PoolStatusV1::Active.as_str() {
            return Err(LiquidityPoolError::Conflict(
                "pool is not active".to_string(),
            ));
        }

        let lp_id = body.lp_id.trim().to_string();
        if lp_id.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "lp_id is required".to_string(),
            ));
        }
        let idempotency_key = body.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "idempotency_key is required".to_string(),
            ));
        }
        if body.amount_sats == 0 {
            return Err(LiquidityPoolError::InvalidRequest(
                "amount_sats must be > 0".to_string(),
            ));
        }

        #[derive(Serialize)]
        struct DepositFingerprint<'a> {
            schema: &'a str,
            pool_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            partition_kind: Option<&'a str>,
            lp_id: &'a str,
            rail: &'a str,
            amount_sats: u64,
            description: Option<&'a str>,
            expiry_secs: Option<u64>,
        }

        let partition_kind = body.partition_kind.unwrap_or(PoolPartitionKindV1::Llp);
        let partition_kind_str = partition_kind.as_str();

        let rail = body.rail.as_str().to_string();
        let request_fingerprint_sha256 = canonical_sha256(&DepositFingerprint {
            schema: DEPOSIT_QUOTE_REQUEST_SCHEMA_V1,
            pool_id,
            partition_kind: (partition_kind != PoolPartitionKindV1::Llp)
                .then_some(partition_kind_str),
            lp_id: lp_id.as_str(),
            rail: rail.as_str(),
            amount_sats: body.amount_sats,
            description: body
                .description
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty()),
            expiry_secs: body.expiry_secs,
        })
        .map_err(LiquidityPoolError::Internal)?;

        let created_at = Utc::now();
        let share_price_sats = self
            .store
            .get_latest_snapshot(pool_id, partition_kind_str)
            .await
            .map_err(map_store_error)?
            .map(|row| row.share_price_sats)
            .unwrap_or(1);

        if share_price_sats <= 0 {
            return Err(LiquidityPoolError::Internal(
                "share_price_sats must be > 0".to_string(),
            ));
        }

        let amount_sats_i64 = i64::try_from(body.amount_sats)
            .map_err(|_| LiquidityPoolError::InvalidRequest("amount_sats too large".to_string()))?;
        let share_price_u64 = u64::try_from(share_price_sats)
            .map_err(|_| LiquidityPoolError::Internal("share_price_sats invalid".to_string()))?;
        let shares_minted_u64 = body.amount_sats / share_price_u64;
        let shares_minted = i64::try_from(shares_minted_u64)
            .map_err(|_| LiquidityPoolError::Internal("shares_minted too large".to_string()))?;

        let deposit_id = format!("liqdep_{}", Uuid::now_v7());

        let (invoice_bolt11, invoice_hash, deposit_address) = match body.rail {
            DepositRailV1::LightningInvoice => {
                let scoped_idempotency_key = if partition_kind == PoolPartitionKindV1::Llp {
                    idempotency_key.clone()
                } else {
                    format!("{partition_kind_str}:{idempotency_key}")
                };
                let wallet_request_id =
                    wallet_request_id("deposit", pool_id, lp_id.as_str(), &scoped_idempotency_key);
                let created = self
                    .wallet
                    .create_invoice(
                        wallet_request_id.as_str(),
                        body.amount_sats,
                        body.description.clone(),
                        body.expiry_secs,
                    )
                    .await?;

                (Some(created.invoice), Some(created.invoice_hash), None)
            }
            DepositRailV1::OnchainAddress => {
                let addresses = self.wallet.get_receive_addresses().await?;
                (None, None, Some(addresses.bitcoin_address))
            }
        };

        let stored = self
            .store
            .create_or_get_deposit(DepositInsertInput {
                deposit_id: deposit_id.clone(),
                pool_id: pool_id.to_string(),
                partition_kind: partition_kind_str.to_string(),
                lp_id: lp_id.clone(),
                rail: rail.clone(),
                amount_sats: amount_sats_i64,
                share_price_sats,
                shares_minted,
                status: DepositStatusV1::Quoted.as_str().to_string(),
                request_fingerprint_sha256,
                idempotency_key: idempotency_key.clone(),
                invoice_bolt11: invoice_bolt11.clone(),
                invoice_hash: invoice_hash.clone(),
                deposit_address: deposit_address.clone(),
                created_at,
            })
            .await
            .map_err(map_store_error)?;

        let stored_amount_sats = u64::try_from(stored.amount_sats).map_err(|_| {
            LiquidityPoolError::Internal("stored deposit amount invalid".to_string())
        })?;
        let receipt = build_deposit_receipt(
            &stored,
            body.rail,
            stored_amount_sats,
            stored.created_at,
            self.receipt_signing_key.as_ref(),
        )?;

        let signature_json = match receipt.signature.as_ref() {
            Some(sig) => Some(
                serde_json::to_value(sig)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
            )
            .filter(|value| !value.is_null()),
            None => None,
        };

        self.store
            .put_receipt(ReceiptInsertInput {
                receipt_id: receipt.receipt_id.clone(),
                entity_kind: "deposit".to_string(),
                entity_id: stored.deposit_id.clone(),
                schema: receipt.schema.clone(),
                canonical_json_sha256: receipt.canonical_json_sha256.clone(),
                signature_json,
                receipt_json: serde_json::to_value(&receipt)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                created_at: stored.created_at,
            })
            .await
            .map_err(map_store_error)?;

        self.maybe_spawn_nostr_liquidity_receipt_pointer_mirror(LiquidityReceiptPointerV1 {
            receipt_id: receipt.receipt_id.clone(),
            pool_id: Some(receipt.pool_id.clone()),
            lp_id: Some(receipt.lp_id.clone()),
            deposit_id: Some(receipt.deposit_id.clone()),
            withdrawal_id: None,
            quote_id: None,
            receipt_sha256: receipt.canonical_json_sha256.clone(),
            receipt_url: format!("openagents://receipt/{}", receipt.receipt_id),
        });

        Ok(DepositQuoteResponseV1 {
            schema: DEPOSIT_QUOTE_RESPONSE_SCHEMA_V1.to_string(),
            deposit: stored,
            receipt,
        })
    }

    pub async fn confirm_deposit(
        &self,
        pool_id: &str,
        deposit_id: &str,
    ) -> Result<(DepositRow, bool), LiquidityPoolError> {
        let confirmed_at = Utc::now();
        self.store
            .confirm_deposit_and_mint_shares(pool_id, deposit_id, confirmed_at)
            .await
            .map_err(map_store_error)
    }

    pub async fn withdraw_request(
        &self,
        pool_id: &str,
        body: WithdrawRequestV1,
    ) -> Result<WithdrawResponseV1, LiquidityPoolError> {
        if body.schema.trim() != WITHDRAW_REQUEST_SCHEMA_V1 {
            return Err(LiquidityPoolError::InvalidRequest(format!(
                "schema must be {WITHDRAW_REQUEST_SCHEMA_V1}"
            )));
        }

        let pool = self
            .store
            .get_pool(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;
        if pool.status != PoolStatusV1::Active.as_str() {
            return Err(LiquidityPoolError::Conflict(
                "pool is not active".to_string(),
            ));
        }

        let lp_id = body.lp_id.trim().to_string();
        if lp_id.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "lp_id is required".to_string(),
            ));
        }
        let idempotency_key = body.idempotency_key.trim().to_string();
        if idempotency_key.is_empty() {
            return Err(LiquidityPoolError::InvalidRequest(
                "idempotency_key is required".to_string(),
            ));
        }
        if body.shares_burned == 0 {
            return Err(LiquidityPoolError::InvalidRequest(
                "shares_burned must be > 0".to_string(),
            ));
        }

        #[derive(Serialize)]
        struct WithdrawFingerprint<'a> {
            schema: &'a str,
            pool_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            partition_kind: Option<&'a str>,
            lp_id: &'a str,
            shares_burned: u64,
            rail_preference: &'a str,
            payout_invoice_hash: Option<&'a str>,
            payout_address: Option<&'a str>,
        }

        let partition_kind = body.partition_kind.unwrap_or(PoolPartitionKindV1::Llp);
        let partition_kind_str = partition_kind.as_str();

        let rail_preference = body.rail_preference.as_str().to_string();
        let latest_share_price_sats = self
            .store
            .get_latest_snapshot(pool_id, partition_kind_str)
            .await
            .map_err(map_store_error)?
            .map(|row| row.share_price_sats)
            .unwrap_or(1);

        let amount_sats_estimate =
            estimate_withdraw_amount_sats(body.shares_burned, latest_share_price_sats)?;

        let payout_invoice_bolt11 = body
            .payout_invoice_bolt11
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        let payout_address = body
            .payout_address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());

        let payout_invoice_hash = match body.rail_preference {
            WithdrawalRailPreferenceV1::Lightning => {
                let invoice = payout_invoice_bolt11.as_deref().ok_or_else(|| {
                    LiquidityPoolError::InvalidRequest(
                        "payout_invoice_bolt11 is required for lightning withdrawals".to_string(),
                    )
                })?;

                let amount_msats = Bolt11::amount_msats(invoice).ok_or_else(|| {
                    LiquidityPoolError::InvalidRequest(
                        "payout_invoice_bolt11 must include an amount".to_string(),
                    )
                })?;
                if amount_msats % 1_000 != 0 {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "payout_invoice_bolt11 amount must be a whole number of sats".to_string(),
                    ));
                }
                let amount_sats = amount_msats / 1_000;
                if amount_sats == 0 {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "payout_invoice_bolt11 amount must be > 0".to_string(),
                    ));
                }
                let expected_sats = u64::try_from(amount_sats_estimate).map_err(|_| {
                    LiquidityPoolError::Internal("withdraw estimate invalid".to_string())
                })?;
                if amount_sats != expected_sats {
                    return Err(LiquidityPoolError::InvalidRequest(format!(
                        "payout invoice amount {} sats does not match withdrawal estimate {} sats",
                        amount_sats, expected_sats
                    )));
                }

                Some(hex::encode(Sha256::digest(invoice.as_bytes())))
            }
            WithdrawalRailPreferenceV1::Onchain => {
                if payout_address.is_none() {
                    return Err(LiquidityPoolError::InvalidRequest(
                        "payout_address is required for onchain withdrawals".to_string(),
                    ));
                }
                None
            }
        };

        let request_fingerprint_sha256 = canonical_sha256(&WithdrawFingerprint {
            schema: WITHDRAW_REQUEST_SCHEMA_V1,
            pool_id,
            partition_kind: (partition_kind != PoolPartitionKindV1::Llp)
                .then_some(partition_kind_str),
            lp_id: lp_id.as_str(),
            shares_burned: body.shares_burned,
            rail_preference: rail_preference.as_str(),
            payout_invoice_hash: payout_invoice_hash.as_deref(),
            payout_address: payout_address.as_deref(),
        })
        .map_err(LiquidityPoolError::Internal)?;
        let earliest_settlement_at =
            Utc::now() + Duration::hours(self.default_withdraw_delay_hours);

        let withdrawal_id = format!("liqwd_{}", Uuid::now_v7());
        let created_at = Utc::now();

        let stored = self
            .store
            .create_or_get_withdrawal(WithdrawalInsertInput {
                withdrawal_id: withdrawal_id.clone(),
                pool_id: pool_id.to_string(),
                partition_kind: partition_kind_str.to_string(),
                lp_id: lp_id.clone(),
                shares_burned: i64::try_from(body.shares_burned).map_err(|_| {
                    LiquidityPoolError::InvalidRequest("shares_burned too large".to_string())
                })?,
                amount_sats_estimate,
                rail_preference: rail_preference.clone(),
                status: WithdrawalStatusV1::Queued.as_str().to_string(),
                request_fingerprint_sha256,
                idempotency_key: idempotency_key.clone(),
                earliest_settlement_at,
                payout_invoice_bolt11,
                payout_invoice_hash,
                payout_address,
                created_at,
            })
            .await
            .map_err(map_store_error)?;

        let receipt = build_withdraw_request_receipt(
            &stored,
            body.rail_preference,
            stored.created_at,
            self.receipt_signing_key.as_ref(),
        )?;

        let signature_json = match receipt.signature.as_ref() {
            Some(sig) => Some(
                serde_json::to_value(sig)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
            )
            .filter(|value| !value.is_null()),
            None => None,
        };

        self.store
            .put_receipt(ReceiptInsertInput {
                receipt_id: receipt.receipt_id.clone(),
                entity_kind: "withdraw_request".to_string(),
                entity_id: stored.withdrawal_id.clone(),
                schema: receipt.schema.clone(),
                canonical_json_sha256: receipt.canonical_json_sha256.clone(),
                signature_json,
                receipt_json: serde_json::to_value(&receipt)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                created_at: stored.created_at,
            })
            .await
            .map_err(map_store_error)?;

        self.maybe_spawn_nostr_liquidity_receipt_pointer_mirror(LiquidityReceiptPointerV1 {
            receipt_id: receipt.receipt_id.clone(),
            pool_id: Some(receipt.pool_id.clone()),
            lp_id: Some(receipt.lp_id.clone()),
            deposit_id: None,
            withdrawal_id: Some(receipt.withdrawal_id.clone()),
            quote_id: None,
            receipt_sha256: receipt.canonical_json_sha256.clone(),
            receipt_url: format!("openagents://receipt/{}", receipt.receipt_id),
        });

        Ok(WithdrawResponseV1 {
            schema: WITHDRAW_RESPONSE_SCHEMA_V1.to_string(),
            withdrawal: stored,
            receipt,
        })
    }

    pub async fn execute_due_withdrawals(
        &self,
        now: DateTime<Utc>,
        limit: Option<i64>,
    ) -> Result<ExecuteDueWithdrawalsOutcome, LiquidityPoolError> {
        let limit = limit
            .unwrap_or(WITHDRAWAL_EXECUTION_DEFAULT_LIMIT)
            .max(1)
            .min(5000);

        let due = self
            .store
            .list_due_withdrawals(now, limit)
            .await
            .map_err(map_store_error)?;

        let mut outcome = ExecuteDueWithdrawalsOutcome {
            attempted: 0,
            paid: 0,
            signing_requests_created: 0,
            failed: 0,
        };

        for withdrawal in due {
            outcome.attempted = outcome.attempted.saturating_add(1);

            if withdrawal.status == WithdrawalStatusV1::Paid.as_str()
                || withdrawal.paid_at.is_some()
            {
                continue;
            }

            let amount_sats = match u64::try_from(withdrawal.amount_sats_estimate) {
                Ok(value) if value > 0 => value,
                _ => {
                    outcome.failed = outcome.failed.saturating_add(1);
                    tracing::warn!(
                        pool_id = withdrawal.pool_id,
                        withdrawal_id = withdrawal.withdrawal_id,
                        amount_sats_estimate = withdrawal.amount_sats_estimate,
                        "liquidity pool withdrawal executor skipped invalid amount_sats_estimate"
                    );
                    continue;
                }
            };

            match withdrawal.rail_preference.as_str() {
                rail if rail == WithdrawalRailPreferenceV1::Lightning.as_str() => {
                    if amount_sats <= WITHDRAWAL_AUTOPAY_MAX_SATS {
                        match self
                            .execute_lightning_withdrawal_direct(&withdrawal, amount_sats, now)
                            .await
                        {
                            Ok(_) => outcome.paid = outcome.paid.saturating_add(1),
                            Err(error) => {
                                outcome.failed = outcome.failed.saturating_add(1);
                                tracing::warn!(
                                    pool_id = withdrawal.pool_id,
                                    withdrawal_id = withdrawal.withdrawal_id,
                                    reason = %error,
                                    "liquidity pool withdrawal executor failed lightning payout"
                                );
                            }
                        }
                        continue;
                    }

                    let signer_set = match self
                        .store
                        .get_signer_set(withdrawal.pool_id.as_str())
                        .await
                        .map_err(map_store_error)?
                    {
                        Some(value) => value,
                        None => {
                            outcome.failed = outcome.failed.saturating_add(1);
                            tracing::warn!(
                                pool_id = withdrawal.pool_id,
                                withdrawal_id = withdrawal.withdrawal_id,
                                "liquidity pool withdrawal executor missing signer set for large lightning withdrawal"
                            );
                            continue;
                        }
                    };

                    match self
                        .ensure_invoice_pay_signing_request(
                            &withdrawal,
                            &signer_set,
                            TreasuryActionClassV1::InvoicePayLarge,
                            amount_sats,
                            now,
                        )
                        .await
                    {
                        Ok((request, created)) => {
                            if created {
                                outcome.signing_requests_created =
                                    outcome.signing_requests_created.saturating_add(1);
                            }

                            tracing::info!(
                                pool_id = request.pool_id,
                                withdrawal_id = withdrawal.withdrawal_id,
                                signing_request_id = request.request_id,
                                "liquidity pool withdrawal executor queued signing request for large lightning withdrawal"
                            );
                        }
                        Err(error) => {
                            outcome.failed = outcome.failed.saturating_add(1);
                            tracing::warn!(
                                pool_id = withdrawal.pool_id,
                                withdrawal_id = withdrawal.withdrawal_id,
                                reason = %error,
                                "liquidity pool withdrawal executor failed to queue signing request for large lightning withdrawal"
                            );
                        }
                    }
                }
                rail if rail == WithdrawalRailPreferenceV1::Onchain.as_str() => {
                    let signer_set = match self
                        .store
                        .get_signer_set(withdrawal.pool_id.as_str())
                        .await
                        .map_err(map_store_error)?
                    {
                        Some(value) => value,
                        None => {
                            outcome.failed = outcome.failed.saturating_add(1);
                            tracing::warn!(
                                pool_id = withdrawal.pool_id,
                                withdrawal_id = withdrawal.withdrawal_id,
                                "liquidity pool withdrawal executor missing signer set for onchain withdrawal"
                            );
                            continue;
                        }
                    };

                    match self
                        .ensure_onchain_withdrawal_signing_request(
                            &withdrawal,
                            &signer_set,
                            amount_sats,
                            now,
                        )
                        .await
                    {
                        Ok((request, created)) => {
                            if created {
                                outcome.signing_requests_created =
                                    outcome.signing_requests_created.saturating_add(1);
                            }

                            tracing::info!(
                                pool_id = request.pool_id,
                                withdrawal_id = withdrawal.withdrawal_id,
                                signing_request_id = request.request_id,
                                "liquidity pool withdrawal executor queued signing request for onchain withdrawal"
                            );
                        }
                        Err(error) => {
                            outcome.failed = outcome.failed.saturating_add(1);
                            tracing::warn!(
                                pool_id = withdrawal.pool_id,
                                withdrawal_id = withdrawal.withdrawal_id,
                                reason = %error,
                                "liquidity pool withdrawal executor failed to queue signing request for onchain withdrawal"
                            );
                        }
                    }
                }
                other => {
                    outcome.failed = outcome.failed.saturating_add(1);
                    tracing::warn!(
                        pool_id = withdrawal.pool_id,
                        withdrawal_id = withdrawal.withdrawal_id,
                        rail_preference = other,
                        "liquidity pool withdrawal executor skipped unsupported rail_preference"
                    );
                }
            }
        }

        Ok(outcome)
    }

    pub async fn status(&self, pool_id: &str) -> Result<PoolStatusResponseV1, LiquidityPoolError> {
        let pool = self
            .store
            .get_pool(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;

        let now = Utc::now();
        let partition_kinds = [
            PoolPartitionKindV1::Llp,
            PoolPartitionKindV1::Cep,
            PoolPartitionKindV1::Rrp,
        ];
        let mut partitions = Vec::with_capacity(partition_kinds.len());

        for partition_kind in partition_kinds {
            let partition_kind_str = partition_kind.as_str();
            let total_shares = self
                .store
                .get_total_shares(pool_id, partition_kind_str)
                .await
                .map_err(map_store_error)?;
            let assets_sats_estimate = self
                .store
                .get_confirmed_deposits_total_sats(pool_id, partition_kind_str)
                .await
                .map_err(map_store_error)?;
            let pending_withdrawals_sats_estimate = self
                .store
                .get_pending_withdrawals_estimate_sats(pool_id, partition_kind_str)
                .await
                .map_err(map_store_error)?;

            let mut liabilities_sats_estimate = pending_withdrawals_sats_estimate;
            if partition_kind == PoolPartitionKindV1::Cep {
                let reserved = self
                    .store
                    .get_credit_reserved_commitments_sats(pool_id, now)
                    .await
                    .map_err(map_store_error)?;
                liabilities_sats_estimate = liabilities_sats_estimate.saturating_add(reserved);
            }

            let share_price_sats = self
                .store
                .get_latest_snapshot(pool_id, partition_kind_str)
                .await
                .map_err(map_store_error)?
                .map(|row| row.share_price_sats)
                .unwrap_or_else(|| {
                    if total_shares > 0 {
                        let per_share = assets_sats_estimate / total_shares;
                        per_share.max(1)
                    } else {
                        1
                    }
                });

            partitions.push(PoolPartitionStatusV1 {
                partition_kind,
                assets_sats_estimate,
                liabilities_sats_estimate,
                share_price_sats,
                total_shares,
                pending_withdrawals_sats_estimate,
            });
        }

        let llp = partitions
            .iter()
            .find(|entry| entry.partition_kind == PoolPartitionKindV1::Llp)
            .cloned()
            .unwrap_or(PoolPartitionStatusV1 {
                partition_kind: PoolPartitionKindV1::Llp,
                assets_sats_estimate: 0,
                liabilities_sats_estimate: 0,
                share_price_sats: 1,
                total_shares: 0,
                pending_withdrawals_sats_estimate: 0,
            });

        Ok(PoolStatusResponseV1 {
            schema: POOL_STATUS_SCHEMA_V1.to_string(),
            pool,
            share_price_sats: llp.share_price_sats,
            total_shares: llp.total_shares,
            pending_withdrawals_sats_estimate: llp.pending_withdrawals_sats_estimate,
            partitions,
            updated_at: now,
        })
    }

    pub async fn latest_snapshot(
        &self,
        pool_id: &str,
        partition_kind: PoolPartitionKindV1,
    ) -> Result<Option<PoolSnapshotResponseV1>, LiquidityPoolError> {
        let partition_kind_str = partition_kind.as_str();
        let Some(snapshot) = self
            .store
            .get_latest_snapshot(pool_id, partition_kind_str)
            .await
            .map_err(map_store_error)?
        else {
            return Ok(None);
        };

        let receipt =
            build_snapshot_receipt(&snapshot, partition_kind, self.receipt_signing_key.as_ref())?;
        Ok(Some(PoolSnapshotResponseV1 {
            schema: POOL_SNAPSHOT_SCHEMA_V1.to_string(),
            snapshot,
            receipt,
        }))
    }

    pub async fn generate_snapshot(
        &self,
        pool_id: &str,
        partition_kind: PoolPartitionKindV1,
    ) -> Result<PoolSnapshotResponseV1, LiquidityPoolError> {
        let pool = self
            .store
            .get_pool(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;
        if pool.status == PoolStatusV1::Disabled.as_str() {
            return Err(LiquidityPoolError::Conflict("pool is disabled".to_string()));
        }

        let as_of = Utc::now();
        let created_at = Utc::now();

        let wallet_status = self.wallet.get_status().await?;
        let wallet_balance_sats = wallet_status.balance_sats.unwrap_or(0);

        let lightning_backend = self.lightning_node.backend().to_string();
        let (
            mut onchain_sats,
            mut channel_total_sats,
            mut channel_outbound_sats,
            mut channel_inbound_sats,
        ) = (0u64, 0u64, 0u64, 0u64);
        let (mut channel_count, mut connected_channel_count) = (0u64, 0u64);
        let mut lightning_last_error: Option<String> = None;

        match self.lightning_node.get_balances().await {
            Ok(balances) => {
                onchain_sats = balances.onchain_sats;
                channel_total_sats = balances.channel_total_sats;
                channel_outbound_sats = balances.channel_outbound_sats;
                channel_inbound_sats = balances.channel_inbound_sats;
            }
            Err(error) => {
                lightning_last_error = Some(format!(
                    "get_balances failed ({}): {}",
                    error.code(),
                    error.message()
                ));
            }
        }

        match self.lightning_node.channel_health_snapshot().await {
            Ok(health) => {
                channel_count = health.channel_count;
                connected_channel_count = health.connected_channel_count;
            }
            Err(error) => {
                lightning_last_error = lightning_last_error.or_else(|| {
                    Some(format!(
                        "channel_health_snapshot failed ({}): {}",
                        error.code(),
                        error.message()
                    ))
                });
            }
        }

        let partition_kind_str = partition_kind.as_str();
        let total_shares = self
            .store
            .get_total_shares(pool_id, partition_kind_str)
            .await
            .map_err(map_store_error)?;
        let book_assets_sats = self
            .store
            .get_confirmed_deposits_total_sats(pool_id, partition_kind_str)
            .await
            .map_err(map_store_error)?;
        let pending_withdrawals_sats_estimate = self
            .store
            .get_pending_withdrawals_estimate_sats(pool_id, partition_kind_str)
            .await
            .map_err(map_store_error)?;

        let credit_reserved_commitments_sats = if partition_kind == PoolPartitionKindV1::Cep {
            self.store
                .get_credit_reserved_commitments_sats(pool_id, as_of)
                .await
                .map_err(map_store_error)?
        } else {
            0
        };

        let lightning_json = if partition_kind == PoolPartitionKindV1::Llp {
            json!({
                "schema": "openagents.liquidity.llp_lightning_snapshot.v1",
                "backend": lightning_backend,
                "onchainSats": onchain_sats,
                "channelTotalSats": channel_total_sats,
                "channelOutboundSats": channel_outbound_sats,
                "channelInboundSats": channel_inbound_sats,
                "channelCount": channel_count,
                "connectedChannelCount": connected_channel_count,
                "lastError": lightning_last_error,
            })
        } else {
            Value::Null
        };

        let assets_json = json!({
            "schema": "openagents.liquidity.pool_assets.v1",
            "partitionKind": partition_kind_str,
            "walletBalanceSats": wallet_balance_sats,
            "bookAssetsSats": book_assets_sats,
            "lightning": lightning_json
        });

        let liabilities_json = json!({
            "schema": "openagents.liquidity.pool_liabilities.v1",
            "partitionKind": partition_kind_str,
            "sharesOutstanding": total_shares,
            "pendingWithdrawalsSatsEstimate": pending_withdrawals_sats_estimate,
            "creditReservedCommitmentsSats": credit_reserved_commitments_sats,
        });

        let share_price_sats = if total_shares > 0 {
            let denom = u64::try_from(total_shares)
                .map_err(|_| LiquidityPoolError::Internal("total_shares invalid".to_string()))?;
            let assets_sats = u64::try_from(book_assets_sats.max(0)).map_err(|_| {
                LiquidityPoolError::Internal("book_assets_sats invalid".to_string())
            })?;
            let per_share = assets_sats / denom;
            i64::try_from(per_share.max(1)).map_err(|_| {
                LiquidityPoolError::Internal("share_price_sats too large".to_string())
            })?
        } else {
            1
        };

        let assets_json_sha256 =
            canonical_sha256(&assets_json).map_err(LiquidityPoolError::Internal)?;
        let liabilities_json_sha256 =
            canonical_sha256(&liabilities_json).map_err(LiquidityPoolError::Internal)?;

        #[derive(Serialize)]
        struct SnapshotHashInput<'a> {
            schema: &'a str,
            pool_id: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            partition_kind: Option<&'a str>,
            as_of: &'a DateTime<Utc>,
            assets_json_sha256: &'a str,
            liabilities_json_sha256: &'a str,
            share_price_sats: i64,
            created_at: &'a DateTime<Utc>,
        }

        let canonical_json_sha256 = canonical_sha256(&SnapshotHashInput {
            schema: POOL_SNAPSHOT_RECEIPT_SCHEMA_V1,
            pool_id,
            partition_kind: (partition_kind != PoolPartitionKindV1::Llp)
                .then_some(partition_kind_str),
            as_of: &as_of,
            assets_json_sha256: assets_json_sha256.as_str(),
            liabilities_json_sha256: liabilities_json_sha256.as_str(),
            share_price_sats,
            created_at: &created_at,
        })
        .map_err(LiquidityPoolError::Internal)?;

        let snapshot_id = format!("lips_{}", &canonical_json_sha256[..24]);

        let signature = match self.receipt_signing_key.as_ref() {
            Some(secret_key) => Some(
                sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
            ),
            None => None,
        };

        let receipt = PoolSnapshotReceiptV1 {
            schema: POOL_SNAPSHOT_RECEIPT_SCHEMA_V1.to_string(),
            receipt_id: format!("lpsr_{}", &canonical_json_sha256[..24]),
            pool_id: pool_id.to_string(),
            snapshot_id: snapshot_id.clone(),
            partition_kind: (partition_kind != PoolPartitionKindV1::Llp).then_some(partition_kind),
            as_of,
            assets_json_sha256: assets_json_sha256.clone(),
            liabilities_json_sha256: liabilities_json_sha256.clone(),
            share_price_sats,
            created_at,
            canonical_json_sha256: canonical_json_sha256.clone(),
            signature: signature.clone(),
        };

        let signature_json = match signature.as_ref() {
            Some(sig) => Some(
                serde_json::to_value(sig)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
            )
            .filter(|value| !value.is_null()),
            None => None,
        };

        let snapshot_row = PoolSnapshotRow {
            snapshot_id: snapshot_id.clone(),
            pool_id: pool_id.to_string(),
            partition_kind: partition_kind_str.to_string(),
            as_of,
            assets_json,
            liabilities_json,
            share_price_sats,
            canonical_json_sha256: canonical_json_sha256.clone(),
            signature_json: signature_json.clone(),
            created_at,
        };

        let stored = self
            .store
            .create_or_get_snapshot(snapshot_row)
            .await
            .map_err(map_store_error)?;

        self.store
            .put_receipt(ReceiptInsertInput {
                receipt_id: receipt.receipt_id.clone(),
                entity_kind: "snapshot".to_string(),
                entity_id: stored.snapshot_id.clone(),
                schema: receipt.schema.clone(),
                canonical_json_sha256: receipt.canonical_json_sha256.clone(),
                signature_json,
                receipt_json: serde_json::to_value(&receipt)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                created_at: stored.created_at,
            })
            .await
            .map_err(map_store_error)?;

        self.maybe_spawn_nostr_pool_snapshot_mirror(&stored, &receipt);

        Ok(PoolSnapshotResponseV1 {
            schema: POOL_SNAPSHOT_SCHEMA_V1.to_string(),
            snapshot: stored,
            receipt,
        })
    }

    pub async fn prune_snapshots_keep_latest(
        &self,
        pool_id: &str,
        partition_kind: PoolPartitionKindV1,
        keep_latest: i64,
    ) -> Result<u64, LiquidityPoolError> {
        let keep_latest = keep_latest.max(1);
        self.store
            .prune_snapshots_keep_latest(pool_id, partition_kind.as_str(), keep_latest)
            .await
            .map_err(map_store_error)
    }

    async fn execute_lightning_withdrawal_direct(
        &self,
        withdrawal: &WithdrawalRow,
        amount_sats: u64,
        now: DateTime<Utc>,
    ) -> Result<WithdrawalRow, LiquidityPoolError> {
        let invoice = withdrawal
            .payout_invoice_bolt11
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                LiquidityPoolError::Conflict("withdrawal payout invoice missing".to_string())
            })?;
        let expected_hash = withdrawal
            .payout_invoice_hash
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                LiquidityPoolError::Internal("withdrawal invoice hash missing".to_string())
            })?;
        let computed_hash = hex::encode(Sha256::digest(invoice.as_bytes()));
        if !computed_hash.eq_ignore_ascii_case(expected_hash) {
            return Err(LiquidityPoolError::Conflict(
                "withdrawal payout invoice hash mismatch".to_string(),
            ));
        }

        let amount_msats = Bolt11::amount_msats(invoice).ok_or_else(|| {
            LiquidityPoolError::Conflict("withdrawal payout invoice missing amount".to_string())
        })?;
        let expected_msats = amount_sats.saturating_mul(1_000);
        if amount_msats != expected_msats {
            return Err(LiquidityPoolError::Conflict(
                "withdrawal payout invoice amount mismatch".to_string(),
            ));
        }

        let wallet_request_id = wallet_request_id(
            "withdrawal_payout",
            withdrawal.pool_id.as_str(),
            withdrawal.lp_id.as_str(),
            withdrawal.withdrawal_id.as_str(),
        );

        let paid = self
            .wallet
            .pay_bolt11(
                wallet_request_id.as_str(),
                invoice.to_string(),
                amount_msats,
                WITHDRAWAL_WALLET_HOST.to_string(),
            )
            .await?;

        let paid_at = now;
        let updated = self
            .store
            .mark_withdrawal_paid_and_burn_shares(
                withdrawal.pool_id.as_str(),
                withdrawal.withdrawal_id.as_str(),
                paid.wallet_receipt_sha256.as_str(),
                paid_at,
            )
            .await
            .map_err(map_store_error)?;

        let _receipt = self
            .persist_withdraw_settlement_receipt(
                &updated,
                WithdrawalRailPreferenceV1::Lightning,
                paid.wallet_receipt_sha256.as_str(),
                Some(paid.payment_id.as_str()),
                None,
                paid_at,
            )
            .await?;

        Ok(updated)
    }

    async fn ensure_invoice_pay_signing_request(
        &self,
        withdrawal: &WithdrawalRow,
        signer_set: &PoolSignerSetRow,
        action_class: TreasuryActionClassV1,
        amount_sats: u64,
        now: DateTime<Utc>,
    ) -> Result<(PoolSigningRequestRow, bool), LiquidityPoolError> {
        let payout_invoice_hash = withdrawal
            .payout_invoice_hash
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                LiquidityPoolError::InvalidRequest(
                    "withdrawal missing payout invoice hash".to_string(),
                )
            })?;

        let required_signatures =
            required_signatures_for_action(signer_set, action_class, Some(amount_sats))?;

        #[derive(Serialize)]
        struct Payload<'a> {
            schema: &'a str,
            pool_id: &'a str,
            action_class: &'a str,
            withdrawal_id: &'a str,
            amount_sats: u64,
            payout_invoice_hash: &'a str,
        }

        let payload = Payload {
            schema: "openagents.liquidity.pool.withdrawal_invoice_pay_action.v1",
            pool_id: withdrawal.pool_id.as_str(),
            action_class: action_class.as_str(),
            withdrawal_id: withdrawal.withdrawal_id.as_str(),
            amount_sats,
            payout_invoice_hash,
        };
        let payload_sha256 = canonical_sha256(&payload).map_err(LiquidityPoolError::Internal)?;
        let payload_json = serde_json::to_value(&payload)
            .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?;

        let request_id = format!("sigreq_{}", Uuid::now_v7());
        let request = self
            .store
            .create_or_get_signing_request(SigningRequestInsertInput {
                request_id: request_id.clone(),
                pool_id: withdrawal.pool_id.clone(),
                action_class: action_class.as_str().to_string(),
                idempotency_key: format!("withdrawal:{}", withdrawal.withdrawal_id),
                payload_json,
                payload_sha256,
                required_signatures: i64::from(required_signatures),
                status: SIGNING_REQUEST_STATUS_PENDING.to_string(),
                created_at: now,
            })
            .await
            .map_err(map_store_error)?;

        Ok((request.clone(), request.request_id == request_id))
    }

    async fn ensure_onchain_withdrawal_signing_request(
        &self,
        withdrawal: &WithdrawalRow,
        signer_set: &PoolSignerSetRow,
        amount_sats: u64,
        now: DateTime<Utc>,
    ) -> Result<(PoolSigningRequestRow, bool), LiquidityPoolError> {
        let payout_address = withdrawal
            .payout_address
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                LiquidityPoolError::InvalidRequest("withdrawal missing payout address".to_string())
            })?;

        let required_signatures = required_signatures_for_action(
            signer_set,
            TreasuryActionClassV1::OnchainWithdrawalBatch,
            Some(amount_sats),
        )?;

        #[derive(Serialize)]
        struct Payload<'a> {
            schema: &'a str,
            pool_id: &'a str,
            action_class: &'a str,
            withdrawal_id: &'a str,
            amount_sats: u64,
            payout_address: &'a str,
            confirmation_speed: &'a str,
        }

        let payload = Payload {
            schema: "openagents.liquidity.pool.withdrawal_onchain_action.v1",
            pool_id: withdrawal.pool_id.as_str(),
            action_class: TreasuryActionClassV1::OnchainWithdrawalBatch.as_str(),
            withdrawal_id: withdrawal.withdrawal_id.as_str(),
            amount_sats,
            payout_address,
            confirmation_speed: "normal",
        };
        let payload_sha256 = canonical_sha256(&payload).map_err(LiquidityPoolError::Internal)?;
        let payload_json = serde_json::to_value(&payload)
            .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?;

        let request_id = format!("sigreq_{}", Uuid::now_v7());
        let request = self
            .store
            .create_or_get_signing_request(SigningRequestInsertInput {
                request_id: request_id.clone(),
                pool_id: withdrawal.pool_id.clone(),
                action_class: TreasuryActionClassV1::OnchainWithdrawalBatch
                    .as_str()
                    .to_string(),
                idempotency_key: format!("withdrawal:{}", withdrawal.withdrawal_id),
                payload_json,
                payload_sha256,
                required_signatures: i64::from(required_signatures),
                status: SIGNING_REQUEST_STATUS_PENDING.to_string(),
                created_at: now,
            })
            .await
            .map_err(map_store_error)?;

        Ok((request.clone(), request.request_id == request_id))
    }

    async fn persist_withdraw_settlement_receipt(
        &self,
        withdrawal: &WithdrawalRow,
        rail: WithdrawalRailPreferenceV1,
        wallet_receipt_sha256: &str,
        payout_payment_id: Option<&str>,
        payout_txid: Option<&str>,
        paid_at: DateTime<Utc>,
    ) -> Result<WithdrawSettlementReceiptV1, LiquidityPoolError> {
        let receipt = build_withdraw_settlement_receipt(
            withdrawal,
            rail,
            wallet_receipt_sha256,
            payout_payment_id,
            payout_txid,
            paid_at,
            self.receipt_signing_key.as_ref(),
        )?;

        let signature_json = match receipt.signature.as_ref() {
            Some(sig) => Some(
                serde_json::to_value(sig)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
            )
            .filter(|value| !value.is_null()),
            None => None,
        };

        self.store
            .put_receipt(ReceiptInsertInput {
                receipt_id: receipt.receipt_id.clone(),
                entity_kind: "withdraw_settlement".to_string(),
                entity_id: withdrawal.withdrawal_id.clone(),
                schema: receipt.schema.clone(),
                canonical_json_sha256: receipt.canonical_json_sha256.clone(),
                signature_json,
                receipt_json: serde_json::to_value(&receipt)
                    .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
                created_at: paid_at,
            })
            .await
            .map_err(map_store_error)?;

        self.maybe_spawn_nostr_liquidity_receipt_pointer_mirror(LiquidityReceiptPointerV1 {
            receipt_id: receipt.receipt_id.clone(),
            pool_id: Some(receipt.pool_id.clone()),
            lp_id: Some(receipt.lp_id.clone()),
            deposit_id: None,
            withdrawal_id: Some(receipt.withdrawal_id.clone()),
            quote_id: None,
            receipt_sha256: receipt.canonical_json_sha256.clone(),
            receipt_url: format!("openagents://receipt/{}", receipt.receipt_id),
        });

        Ok(receipt)
    }

    fn maybe_spawn_nostr_liquidity_receipt_pointer_mirror(
        &self,
        payload: LiquidityReceiptPointerV1,
    ) {
        let relays = bridge_relays_from_env();
        if relays.is_empty() {
            return;
        }
        let Some(secret_key) = self.receipt_signing_key else {
            return;
        };

        tokio::spawn(async move {
            let event = match build_liquidity_receipt_pointer_event(&secret_key, None, &payload) {
                Ok(event) => event,
                Err(error) => {
                    tracing::warn!(
                        reason = %error,
                        "bridge nostr mirror failed to build liquidity receipt pointer"
                    );
                    return;
                }
            };
            let publisher = BridgeNostrPublisher::new(relays);
            if let Err(error) = publisher.connect().await {
                tracing::warn!(
                    reason = %error,
                    "bridge nostr mirror failed to connect to relays"
                );
                return;
            }
            if let Err(error) = publisher.publish(&event).await {
                tracing::warn!(
                    reason = %error,
                    "bridge nostr mirror failed to publish liquidity receipt pointer"
                );
            }
        });
    }

    fn maybe_spawn_nostr_pool_snapshot_mirror(
        &self,
        snapshot: &PoolSnapshotRow,
        receipt: &PoolSnapshotReceiptV1,
    ) {
        let relays = bridge_relays_from_env();
        if relays.is_empty() {
            return;
        }
        let Some(secret_key) = self.receipt_signing_key else {
            return;
        };

        let wallet_balance_sats = snapshot
            .assets_json
            .get("walletBalanceSats")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let shares_outstanding = snapshot
            .liabilities_json
            .get("sharesOutstanding")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let pending_withdrawals_sats_estimate = snapshot
            .liabilities_json
            .get("pendingWithdrawalsSatsEstimate")
            .and_then(Value::as_i64)
            .unwrap_or(0);

        let payload = PoolSnapshotBridgeV1 {
            pool_id: snapshot.pool_id.clone(),
            snapshot_id: snapshot.snapshot_id.clone(),
            snapshot_sha256: snapshot.canonical_json_sha256.clone(),
            as_of_unix: snapshot.as_of.timestamp().max(0) as u64,
            wallet_balance_sats,
            shares_outstanding,
            pending_withdrawals_sats_estimate,
            share_price_sats: snapshot.share_price_sats,
            receipt_id: Some(receipt.receipt_id.clone()),
            receipt_sha256: Some(receipt.canonical_json_sha256.clone()),
        };

        tokio::spawn(async move {
            let event = match build_pool_snapshot_event(&secret_key, None, &payload) {
                Ok(event) => event,
                Err(error) => {
                    tracing::warn!(
                        pool_id = %payload.pool_id,
                        reason = %error,
                        "bridge nostr mirror failed to build pool snapshot event"
                    );
                    return;
                }
            };
            let publisher = BridgeNostrPublisher::new(relays);
            if let Err(error) = publisher.connect().await {
                tracing::warn!(
                    pool_id = %payload.pool_id,
                    reason = %error,
                    "bridge nostr mirror failed to connect to relays"
                );
                return;
            }
            if let Err(error) = publisher.publish(&event).await {
                tracing::warn!(
                    pool_id = %payload.pool_id,
                    reason = %error,
                    "bridge nostr mirror failed to publish pool snapshot event"
                );
            }
        });
    }
}

fn map_store_error(error: LiquidityPoolStoreError) -> LiquidityPoolError {
    match error {
        LiquidityPoolStoreError::Conflict(message) => LiquidityPoolError::Conflict(message),
        LiquidityPoolStoreError::NotFound(_) => LiquidityPoolError::NotFound,
        LiquidityPoolStoreError::Db(message) => LiquidityPoolError::Internal(message),
    }
}

fn normalize_xonly_pubkey_hex(value: &str) -> Result<String, LiquidityPoolError> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(LiquidityPoolError::InvalidRequest(
            "pubkey is required".to_string(),
        ));
    }
    let bytes = hex::decode(normalized.as_str()).map_err(|error| {
        LiquidityPoolError::InvalidRequest(format!("pubkey must be hex: {error}"))
    })?;
    if bytes.len() != 32 {
        return Err(LiquidityPoolError::InvalidRequest(
            "pubkey must be 32-byte xonly hex".to_string(),
        ));
    }
    Ok(normalized)
}

fn required_signatures_for_action(
    signer_set: &PoolSignerSetRow,
    action_class: TreasuryActionClassV1,
    amount_sats: Option<u64>,
) -> Result<u32, LiquidityPoolError> {
    let signer_count = signer_set.signers.len() as u32;
    if signer_count == 0 {
        return Err(LiquidityPoolError::Conflict(
            "pool signer set has no signers".to_string(),
        ));
    }

    let mut required = signer_set.threshold;
    if let Some(policy) = signer_set.policy.policy_for_action(action_class) {
        if let (Some(amount_sats), Some(single_signer_max_sats)) =
            (amount_sats, policy.single_signer_max_sats)
        {
            if amount_sats <= single_signer_max_sats {
                required = 1;
            }
        }

        if let Some(policy_required) = policy.required_signatures {
            required = policy_required;
        }
    }

    if required == 0 {
        return Err(LiquidityPoolError::Conflict(
            "pool signer set required signatures is zero".to_string(),
        ));
    }
    if required > signer_count {
        return Err(LiquidityPoolError::Conflict(
            "pool signer set threshold exceeds signer count".to_string(),
        ));
    }

    Ok(required)
}

fn canonical_sha256(value: &impl Serialize) -> Result<String, String> {
    let canonical_json =
        protocol::hash::canonical_json(value).map_err(|error| error.to_string())?;
    let digest = Sha256::digest(canonical_json.as_bytes());
    Ok(hex::encode(digest))
}

fn build_treasury_action_receipt(
    request: &PoolSigningRequestRow,
    approvals: &[PoolSigningApprovalRow],
    execution_result_json: &Value,
    executed_at: DateTime<Utc>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<PoolTreasuryActionReceiptV1, LiquidityPoolError> {
    #[derive(Serialize)]
    struct ApprovalHashInput<'a> {
        signer_pubkey: &'a str,
        scheme: &'a str,
        signed_sha256: &'a str,
        signature_hex: &'a str,
    }

    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        pool_id: &'a str,
        signing_request_id: &'a str,
        action_class: &'a str,
        payload_sha256: &'a str,
        approvals: &'a [ApprovalHashInput<'a>],
        execution_result_json: &'a Value,
        executed_at: &'a DateTime<Utc>,
    }

    let approval_inputs = approvals
        .iter()
        .map(|approval| ApprovalHashInput {
            signer_pubkey: approval.signer_pubkey.as_str(),
            scheme: approval.signature.scheme.as_str(),
            signed_sha256: approval.signature.signed_sha256.as_str(),
            signature_hex: approval.signature.signature_hex.as_str(),
        })
        .collect::<Vec<_>>();

    let canonical_json_sha256 = canonical_sha256(&ReceiptHashInput {
        schema: POOL_TREASURY_ACTION_RECEIPT_SCHEMA_V1,
        pool_id: request.pool_id.as_str(),
        signing_request_id: request.request_id.as_str(),
        action_class: request.action_class.as_str(),
        payload_sha256: request.payload_sha256.as_str(),
        approvals: &approval_inputs,
        execution_result_json,
        executed_at: &executed_at,
    })
    .map_err(LiquidityPoolError::Internal)?;

    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(PoolTreasuryActionReceiptV1 {
        schema: POOL_TREASURY_ACTION_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id: format!("lpta_{}", &canonical_json_sha256[..24]),
        pool_id: request.pool_id.clone(),
        signing_request_id: request.request_id.clone(),
        action_class: request.action_class.clone(),
        payload_sha256: request.payload_sha256.clone(),
        approvals: approvals.to_vec(),
        execution_result_json: execution_result_json.clone(),
        executed_at,
        canonical_json_sha256,
        signature,
    })
}

fn build_deposit_receipt(
    deposit: &DepositRow,
    rail: DepositRailV1,
    amount_sats: u64,
    created_at: DateTime<Utc>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<DepositReceiptV1, LiquidityPoolError> {
    let partition_kind = match deposit.partition_kind.as_str() {
        "llp" => None,
        "cep" => Some(PoolPartitionKindV1::Cep),
        "rrp" => Some(PoolPartitionKindV1::Rrp),
        other => {
            return Err(LiquidityPoolError::Internal(format!(
                "invalid deposit partition_kind: {other}"
            )));
        }
    };

    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        pool_id: &'a str,
        lp_id: &'a str,
        deposit_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        partition_kind: Option<&'a str>,
        rail: &'a str,
        amount_sats: u64,
        share_price_sats: i64,
        shares_minted: i64,
        invoice_hash: Option<&'a str>,
        deposit_address: Option<&'a str>,
        created_at: &'a DateTime<Utc>,
    }

    let canonical_json_sha256 = canonical_sha256(&ReceiptHashInput {
        schema: DEPOSIT_RECEIPT_SCHEMA_V1,
        pool_id: deposit.pool_id.as_str(),
        lp_id: deposit.lp_id.as_str(),
        deposit_id: deposit.deposit_id.as_str(),
        partition_kind: partition_kind.map(PoolPartitionKindV1::as_str),
        rail: rail.as_str(),
        amount_sats,
        share_price_sats: deposit.share_price_sats,
        shares_minted: deposit.shares_minted,
        invoice_hash: deposit.invoice_hash.as_deref(),
        deposit_address: deposit.deposit_address.as_deref(),
        created_at: &created_at,
    })
    .map_err(LiquidityPoolError::Internal)?;

    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(DepositReceiptV1 {
        schema: DEPOSIT_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id: format!("lpd_{}", &canonical_json_sha256[..24]),
        pool_id: deposit.pool_id.clone(),
        lp_id: deposit.lp_id.clone(),
        deposit_id: deposit.deposit_id.clone(),
        partition_kind,
        rail,
        amount_sats,
        share_price_sats: deposit.share_price_sats,
        shares_minted: deposit.shares_minted,
        invoice_hash: deposit.invoice_hash.clone(),
        deposit_address: deposit.deposit_address.clone(),
        created_at,
        canonical_json_sha256,
        signature,
    })
}

fn build_withdraw_request_receipt(
    withdrawal: &WithdrawalRow,
    rail: WithdrawalRailPreferenceV1,
    created_at: DateTime<Utc>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<WithdrawRequestReceiptV1, LiquidityPoolError> {
    let partition_kind = match withdrawal.partition_kind.as_str() {
        "llp" => None,
        "cep" => Some(PoolPartitionKindV1::Cep),
        "rrp" => Some(PoolPartitionKindV1::Rrp),
        other => {
            return Err(LiquidityPoolError::Internal(format!(
                "invalid withdrawal partition_kind: {other}"
            )));
        }
    };

    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        pool_id: &'a str,
        lp_id: &'a str,
        withdrawal_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        partition_kind: Option<&'a str>,
        shares_burned: i64,
        amount_sats_estimate: i64,
        rail_preference: &'a str,
        earliest_settlement_at: &'a DateTime<Utc>,
        created_at: &'a DateTime<Utc>,
    }

    let canonical_json_sha256 = canonical_sha256(&ReceiptHashInput {
        schema: WITHDRAW_REQUEST_RECEIPT_SCHEMA_V1,
        pool_id: withdrawal.pool_id.as_str(),
        lp_id: withdrawal.lp_id.as_str(),
        withdrawal_id: withdrawal.withdrawal_id.as_str(),
        partition_kind: partition_kind.map(PoolPartitionKindV1::as_str),
        shares_burned: withdrawal.shares_burned,
        amount_sats_estimate: withdrawal.amount_sats_estimate,
        rail_preference: rail.as_str(),
        earliest_settlement_at: &withdrawal.earliest_settlement_at,
        created_at: &created_at,
    })
    .map_err(LiquidityPoolError::Internal)?;

    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(WithdrawRequestReceiptV1 {
        schema: WITHDRAW_REQUEST_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id: format!("lpwdr_{}", &canonical_json_sha256[..24]),
        pool_id: withdrawal.pool_id.clone(),
        lp_id: withdrawal.lp_id.clone(),
        withdrawal_id: withdrawal.withdrawal_id.clone(),
        partition_kind,
        shares_burned: withdrawal.shares_burned,
        amount_sats_estimate: withdrawal.amount_sats_estimate,
        rail_preference: rail,
        earliest_settlement_at: withdrawal.earliest_settlement_at,
        created_at,
        canonical_json_sha256,
        signature,
    })
}

fn build_withdraw_settlement_receipt(
    withdrawal: &WithdrawalRow,
    rail: WithdrawalRailPreferenceV1,
    wallet_receipt_sha256: &str,
    payout_payment_id: Option<&str>,
    payout_txid: Option<&str>,
    paid_at: DateTime<Utc>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<WithdrawSettlementReceiptV1, LiquidityPoolError> {
    let partition_kind = match withdrawal.partition_kind.as_str() {
        "llp" => None,
        "cep" => Some(PoolPartitionKindV1::Cep),
        "rrp" => Some(PoolPartitionKindV1::Rrp),
        other => {
            return Err(LiquidityPoolError::Internal(format!(
                "invalid withdrawal partition_kind: {other}"
            )));
        }
    };

    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        pool_id: &'a str,
        lp_id: &'a str,
        withdrawal_id: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        partition_kind: Option<&'a str>,
        amount_sats: i64,
        rail: &'a str,
        payout_invoice_hash: Option<&'a str>,
        payout_address: Option<&'a str>,
        wallet_receipt_sha256: &'a str,
        payout_payment_id: Option<&'a str>,
        payout_txid: Option<&'a str>,
        paid_at: &'a DateTime<Utc>,
    }

    let canonical_json_sha256 = canonical_sha256(&ReceiptHashInput {
        schema: WITHDRAW_SETTLEMENT_RECEIPT_SCHEMA_V1,
        pool_id: withdrawal.pool_id.as_str(),
        lp_id: withdrawal.lp_id.as_str(),
        withdrawal_id: withdrawal.withdrawal_id.as_str(),
        partition_kind: partition_kind.map(PoolPartitionKindV1::as_str),
        amount_sats: withdrawal.amount_sats_estimate,
        rail: rail.as_str(),
        payout_invoice_hash: withdrawal.payout_invoice_hash.as_deref(),
        payout_address: withdrawal.payout_address.as_deref(),
        wallet_receipt_sha256: wallet_receipt_sha256.trim(),
        payout_payment_id,
        payout_txid,
        paid_at: &paid_at,
    })
    .map_err(LiquidityPoolError::Internal)?;

    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
        ),
        None => None,
    };

    Ok(WithdrawSettlementReceiptV1 {
        schema: WITHDRAW_SETTLEMENT_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id: format!("lpwds_{}", &canonical_json_sha256[..24]),
        pool_id: withdrawal.pool_id.clone(),
        lp_id: withdrawal.lp_id.clone(),
        withdrawal_id: withdrawal.withdrawal_id.clone(),
        partition_kind,
        amount_sats: withdrawal.amount_sats_estimate,
        rail,
        payout_invoice_hash: withdrawal.payout_invoice_hash.clone(),
        payout_address: withdrawal.payout_address.clone(),
        wallet_receipt_sha256: Some(wallet_receipt_sha256.trim().to_string())
            .filter(|value| !value.is_empty()),
        payout_payment_id: payout_payment_id
            .map(|value| value.trim().to_string())
            .filter(|v| !v.is_empty()),
        payout_txid: payout_txid
            .map(|value| value.trim().to_string())
            .filter(|v| !v.is_empty()),
        paid_at,
        canonical_json_sha256,
        signature,
    })
}

fn build_snapshot_receipt(
    snapshot: &PoolSnapshotRow,
    partition_kind: PoolPartitionKindV1,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<PoolSnapshotReceiptV1, LiquidityPoolError> {
    if snapshot.partition_kind != partition_kind.as_str() {
        return Err(LiquidityPoolError::Internal(
            "snapshot partition_kind mismatch".to_string(),
        ));
    }

    let partition_kind = match snapshot.partition_kind.as_str() {
        "llp" => None,
        "cep" => Some(PoolPartitionKindV1::Cep),
        "rrp" => Some(PoolPartitionKindV1::Rrp),
        other => {
            return Err(LiquidityPoolError::Internal(format!(
                "invalid snapshot partition_kind: {other}"
            )));
        }
    };

    let assets_json_sha256 =
        canonical_sha256(&snapshot.assets_json).map_err(LiquidityPoolError::Internal)?;
    let liabilities_json_sha256 =
        canonical_sha256(&snapshot.liabilities_json).map_err(LiquidityPoolError::Internal)?;

    // For MVP-0, the snapshot row stores the canonical receipt digest that `snapshot_id` and
    // signature derive from. Never recompute the digest from the receipt fields, otherwise
    // signature verification will drift.
    let canonical_json_sha256 = snapshot.canonical_json_sha256.clone();
    let expected_snapshot_id = format!("lips_{}", &canonical_json_sha256[..24]);
    if snapshot.snapshot_id != expected_snapshot_id {
        return Err(LiquidityPoolError::Internal(
            "snapshot_id does not match canonical_json_sha256".to_string(),
        ));
    }

    let signature = match receipt_signing_key {
        Some(secret_key) => Some(
            sign_receipt_sha256(secret_key, canonical_json_sha256.as_str())
                .map_err(|error| LiquidityPoolError::Internal(error.to_string()))?,
        ),
        None => snapshot
            .signature_json
            .clone()
            .and_then(|value| serde_json::from_value(value).ok()),
    };

    Ok(PoolSnapshotReceiptV1 {
        schema: POOL_SNAPSHOT_RECEIPT_SCHEMA_V1.to_string(),
        receipt_id: format!("lpsr_{}", &canonical_json_sha256[..24]),
        pool_id: snapshot.pool_id.clone(),
        snapshot_id: snapshot.snapshot_id.clone(),
        partition_kind,
        as_of: snapshot.as_of,
        assets_json_sha256,
        liabilities_json_sha256,
        share_price_sats: snapshot.share_price_sats,
        created_at: snapshot.created_at,
        canonical_json_sha256,
        signature,
    })
}

fn estimate_withdraw_amount_sats(
    shares_burned: u64,
    share_price_sats: i64,
) -> Result<i64, LiquidityPoolError> {
    if share_price_sats <= 0 {
        return Err(LiquidityPoolError::Internal(
            "share_price_sats must be > 0".to_string(),
        ));
    }
    let share_price_u64 = u64::try_from(share_price_sats)
        .map_err(|_| LiquidityPoolError::Internal("share_price_sats invalid".to_string()))?;
    let amount_u64 = shares_burned
        .checked_mul(share_price_u64)
        .ok_or_else(|| LiquidityPoolError::Internal("withdraw estimate overflow".to_string()))?;
    i64::try_from(amount_u64)
        .map_err(|_| LiquidityPoolError::InvalidRequest("withdraw estimate too large".to_string()))
}

fn wallet_request_id(kind: &str, pool_id: &str, lp_id: &str, idempotency_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update(b"|");
    hasher.update(pool_id.as_bytes());
    hasher.update(b"|");
    hasher.update(lp_id.as_bytes());
    hasher.update(b"|");
    hasher.update(idempotency_key.as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("liqpool:{kind}:{}", &digest[..24])
}

#[derive(Debug, Clone)]
pub struct WalletInvoiceResult {
    pub invoice: String,
    pub invoice_hash: String,
}

#[derive(Debug, Clone)]
pub struct WalletReceiveAddresses {
    pub spark_address: String,
    pub bitcoin_address: String,
}

#[derive(Debug, Clone)]
pub struct WalletStatusSummary {
    pub balance_sats: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct WalletPayBolt11Result {
    pub payment_id: String,
    pub wallet_receipt_sha256: String,
    pub paid_at_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct WalletSendOnchainResult {
    pub txid: String,
    pub wallet_receipt_sha256: String,
    pub paid_at_ms: Option<i64>,
}

#[async_trait::async_trait]
pub trait WalletExecutorClient: Send + Sync {
    async fn create_invoice(
        &self,
        request_id: &str,
        amount_sats: u64,
        description: Option<String>,
        expiry_secs: Option<u64>,
    ) -> Result<WalletInvoiceResult, LiquidityPoolError>;

    async fn get_receive_addresses(&self) -> Result<WalletReceiveAddresses, LiquidityPoolError>;

    async fn get_status(&self) -> Result<WalletStatusSummary, LiquidityPoolError>;

    async fn pay_bolt11(
        &self,
        request_id: &str,
        invoice: String,
        max_amount_msats: u64,
        host: String,
    ) -> Result<WalletPayBolt11Result, LiquidityPoolError>;

    async fn send_onchain(
        &self,
        request_id: &str,
        address: String,
        amount_sats: u64,
        confirmation_speed: String,
    ) -> Result<WalletSendOnchainResult, LiquidityPoolError>;
}

pub struct HttpWalletExecutorClient {
    base_url: String,
    auth_token: String,
    timeout_ms: u64,
}

impl HttpWalletExecutorClient {
    pub fn new(
        base_url: Option<String>,
        auth_token: Option<String>,
        timeout_ms: u64,
    ) -> Result<Self, LiquidityPoolError> {
        let base_url = base_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                LiquidityPoolError::DependencyUnavailable(
                    "wallet executor base url missing".to_string(),
                )
            })?
            .to_string();
        let auth_token = auth_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                LiquidityPoolError::DependencyUnavailable(
                    "wallet executor auth token missing".to_string(),
                )
            })?
            .to_string();

        Ok(Self {
            base_url,
            auth_token,
            timeout_ms: timeout_ms.max(250).min(120_000),
        })
    }

    fn client(&self) -> reqwest::Client {
        reqwest::Client::new()
    }
}

#[async_trait::async_trait]
impl WalletExecutorClient for HttpWalletExecutorClient {
    async fn create_invoice(
        &self,
        request_id: &str,
        amount_sats: u64,
        description: Option<String>,
        expiry_secs: Option<u64>,
    ) -> Result<WalletInvoiceResult, LiquidityPoolError> {
        let url = format!("{}/create-invoice", self.base_url.trim_end_matches('/'));
        let resp = self
            .client()
            .post(url.as_str())
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .header("authorization", format!("Bearer {}", self.auth_token))
            .header("x-request-id", request_id)
            .json(&json!({
                "amountSats": amount_sats,
                "description": description,
                "expirySecs": expiry_secs,
            }))
            .send()
            .await
            .map_err(|error| {
                LiquidityPoolError::DependencyUnavailable(format!(
                    "wallet executor create-invoice transport error: {error}"
                ))
            })?;

        let status = resp.status();
        let json = resp.json::<Value>().await.unwrap_or(Value::Null);
        if status.is_success() {
            let invoice = json
                .pointer("/result/invoice")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let invoice_hash = json
                .pointer("/result/invoiceHash")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            if invoice.is_empty() || invoice_hash.is_empty() {
                return Err(LiquidityPoolError::DependencyUnavailable(
                    "wallet executor create-invoice returned empty invoice".to_string(),
                ));
            }
            Ok(WalletInvoiceResult {
                invoice,
                invoice_hash,
            })
        } else {
            let code = json
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or_else(|| status.as_str())
                .to_string();
            let message = json
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("wallet executor create-invoice failed")
                .to_string();
            Err(LiquidityPoolError::DependencyUnavailable(format!(
                "wallet executor create-invoice failed: {code}: {message}"
            )))
        }
    }

    async fn get_receive_addresses(&self) -> Result<WalletReceiveAddresses, LiquidityPoolError> {
        let url = format!("{}/receive-address", self.base_url.trim_end_matches('/'));
        let resp = self
            .client()
            .get(url.as_str())
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .header("authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await
            .map_err(|error| {
                LiquidityPoolError::DependencyUnavailable(format!(
                    "wallet executor receive-address transport error: {error}"
                ))
            })?;

        let status = resp.status();
        let json = resp.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(LiquidityPoolError::DependencyUnavailable(format!(
                "wallet executor receive-address failed: http_{}",
                status.as_u16()
            )));
        }

        let spark_address = json
            .pointer("/result/sparkAddress")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let bitcoin_address = json
            .pointer("/result/bitcoinAddress")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if spark_address.is_empty() || bitcoin_address.is_empty() {
            return Err(LiquidityPoolError::DependencyUnavailable(
                "wallet executor receive-address returned empty address".to_string(),
            ));
        }

        Ok(WalletReceiveAddresses {
            spark_address,
            bitcoin_address,
        })
    }

    async fn get_status(&self) -> Result<WalletStatusSummary, LiquidityPoolError> {
        let url = format!("{}/status", self.base_url.trim_end_matches('/'));
        let resp = self
            .client()
            .get(url.as_str())
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .header("authorization", format!("Bearer {}", self.auth_token))
            .send()
            .await
            .map_err(|error| {
                LiquidityPoolError::DependencyUnavailable(format!(
                    "wallet executor status transport error: {error}"
                ))
            })?;

        let status = resp.status();
        let json = resp.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            return Err(LiquidityPoolError::DependencyUnavailable(format!(
                "wallet executor status failed: http_{}",
                status.as_u16()
            )));
        }

        let balance_sats = json.pointer("/status/balanceSats").and_then(Value::as_u64);

        Ok(WalletStatusSummary { balance_sats })
    }

    async fn pay_bolt11(
        &self,
        request_id: &str,
        invoice: String,
        max_amount_msats: u64,
        host: String,
    ) -> Result<WalletPayBolt11Result, LiquidityPoolError> {
        let url = format!("{}/pay-bolt11", self.base_url.trim_end_matches('/'));
        let resp = self
            .client()
            .post(url.as_str())
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .header("authorization", format!("Bearer {}", self.auth_token))
            .header("x-request-id", request_id)
            .json(&json!({
                "requestId": request_id,
                "payment": {
                    "invoice": invoice,
                    "maxAmountMsats": max_amount_msats,
                    "host": host,
                }
            }))
            .send()
            .await
            .map_err(|error| {
                LiquidityPoolError::DependencyUnavailable(format!(
                    "wallet executor pay-bolt11 transport error: {error}"
                ))
            })?;

        let status = resp.status();
        let json = resp.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            let code = json
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or_else(|| status.as_str())
                .to_string();
            let message = json
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("wallet executor pay-bolt11 failed")
                .to_string();
            return Err(LiquidityPoolError::DependencyUnavailable(format!(
                "wallet executor pay-bolt11 failed: {code}: {message}"
            )));
        }

        let payment_id = json
            .pointer("/result/payment/paymentId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let wallet_receipt_sha256 = json
            .pointer("/result/receipt/canonicalJsonSha256")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if payment_id.is_empty() || wallet_receipt_sha256.is_empty() {
            return Err(LiquidityPoolError::DependencyUnavailable(
                "wallet executor pay-bolt11 returned incomplete result".to_string(),
            ));
        }
        let paid_at_ms = json
            .pointer("/result/receipt/paidAtMs")
            .and_then(Value::as_i64);

        Ok(WalletPayBolt11Result {
            payment_id,
            wallet_receipt_sha256,
            paid_at_ms,
        })
    }

    async fn send_onchain(
        &self,
        request_id: &str,
        address: String,
        amount_sats: u64,
        confirmation_speed: String,
    ) -> Result<WalletSendOnchainResult, LiquidityPoolError> {
        let quote_url = format!("{}/send-onchain/quote", self.base_url.trim_end_matches('/'));
        let quote_resp = self
            .client()
            .post(quote_url.as_str())
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .header("authorization", format!("Bearer {}", self.auth_token))
            .header("x-request-id", request_id)
            .json(&json!({
                "requestId": request_id,
                "payment": {
                    "address": address,
                    "amountSats": amount_sats,
                    "confirmationSpeed": confirmation_speed,
                }
            }))
            .send()
            .await
            .map_err(|error| {
                LiquidityPoolError::DependencyUnavailable(format!(
                    "wallet executor send-onchain quote transport error: {error}"
                ))
            })?;

        if !quote_resp.status().is_success() {
            let status = quote_resp.status();
            let json = quote_resp.json::<Value>().await.unwrap_or(Value::Null);
            let code = json
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or_else(|| status.as_str())
                .to_string();
            let message = json
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("wallet executor send-onchain quote failed")
                .to_string();
            return Err(LiquidityPoolError::DependencyUnavailable(format!(
                "wallet executor send-onchain quote failed: {code}: {message}"
            )));
        }

        let commit_url = format!(
            "{}/send-onchain/commit",
            self.base_url.trim_end_matches('/')
        );
        let commit_resp = self
            .client()
            .post(commit_url.as_str())
            .timeout(std::time::Duration::from_millis(self.timeout_ms))
            .header("authorization", format!("Bearer {}", self.auth_token))
            .header("x-request-id", request_id)
            .json(&json!({ "planId": request_id }))
            .send()
            .await
            .map_err(|error| {
                LiquidityPoolError::DependencyUnavailable(format!(
                    "wallet executor send-onchain commit transport error: {error}"
                ))
            })?;

        let status = commit_resp.status();
        let json = commit_resp.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            let code = json
                .pointer("/error/code")
                .and_then(Value::as_str)
                .unwrap_or_else(|| status.as_str())
                .to_string();
            let message = json
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("wallet executor send-onchain commit failed")
                .to_string();
            return Err(LiquidityPoolError::DependencyUnavailable(format!(
                "wallet executor send-onchain commit failed: {code}: {message}"
            )));
        }

        let txid = json
            .pointer("/result/txid")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let wallet_receipt_sha256 = json
            .pointer("/result/receipt/canonicalJsonSha256")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if txid.is_empty() || wallet_receipt_sha256.is_empty() {
            return Err(LiquidityPoolError::DependencyUnavailable(
                "wallet executor send-onchain returned incomplete result".to_string(),
            ));
        }
        let paid_at_ms = json.pointer("/result/paidAtMs").and_then(Value::as_i64);

        Ok(WalletSendOnchainResult {
            txid,
            wallet_receipt_sha256,
            paid_at_ms,
        })
    }
}

pub struct UnavailableWalletExecutorClient {
    reason: String,
}

impl UnavailableWalletExecutorClient {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }
}

#[async_trait::async_trait]
impl WalletExecutorClient for UnavailableWalletExecutorClient {
    async fn create_invoice(
        &self,
        _request_id: &str,
        _amount_sats: u64,
        _description: Option<String>,
        _expiry_secs: Option<u64>,
    ) -> Result<WalletInvoiceResult, LiquidityPoolError> {
        Err(LiquidityPoolError::DependencyUnavailable(
            self.reason.clone(),
        ))
    }

    async fn get_receive_addresses(&self) -> Result<WalletReceiveAddresses, LiquidityPoolError> {
        Err(LiquidityPoolError::DependencyUnavailable(
            self.reason.clone(),
        ))
    }

    async fn get_status(&self) -> Result<WalletStatusSummary, LiquidityPoolError> {
        Err(LiquidityPoolError::DependencyUnavailable(
            self.reason.clone(),
        ))
    }

    async fn pay_bolt11(
        &self,
        _request_id: &str,
        _invoice: String,
        _max_amount_msats: u64,
        _host: String,
    ) -> Result<WalletPayBolt11Result, LiquidityPoolError> {
        Err(LiquidityPoolError::DependencyUnavailable(
            self.reason.clone(),
        ))
    }

    async fn send_onchain(
        &self,
        _request_id: &str,
        _address: String,
        _amount_sats: u64,
        _confirmation_speed: String,
    ) -> Result<WalletSendOnchainResult, LiquidityPoolError> {
        Err(LiquidityPoolError::DependencyUnavailable(
            self.reason.clone(),
        ))
    }
}
