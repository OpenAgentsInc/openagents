use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::artifacts::sign_receipt_sha256;
use crate::bridge::{
    BridgeNostrPublisher, LiquidityReceiptPointerV1, PoolSnapshotBridgeV1, bridge_relays_from_env,
    build_liquidity_receipt_pointer_event, build_pool_snapshot_event,
};
use crate::lightning_node::{LightningNode, NoopLightningNode};
use crate::liquidity_pool::store::{
    DepositInsertInput, LiquidityPoolStore, LiquidityPoolStoreError, ReceiptInsertInput,
    WithdrawalInsertInput,
};
use crate::liquidity_pool::types::{
    DEPOSIT_QUOTE_REQUEST_SCHEMA_V1, DEPOSIT_QUOTE_RESPONSE_SCHEMA_V1, DEPOSIT_RECEIPT_SCHEMA_V1,
    DepositQuoteRequestV1, DepositQuoteResponseV1, DepositRailV1, DepositReceiptV1, DepositRow,
    DepositStatusV1, POOL_CREATE_REQUEST_SCHEMA_V1, POOL_CREATE_RESPONSE_SCHEMA_V1,
    POOL_SNAPSHOT_RECEIPT_SCHEMA_V1, POOL_SNAPSHOT_SCHEMA_V1, POOL_STATUS_SCHEMA_V1,
    PoolCreateRequestV1, PoolCreateResponseV1, PoolKindV1, PoolRow, PoolSnapshotReceiptV1,
    PoolSnapshotResponseV1, PoolSnapshotRow, PoolStatusResponseV1, PoolStatusV1,
    WITHDRAW_REQUEST_RECEIPT_SCHEMA_V1, WITHDRAW_REQUEST_SCHEMA_V1, WITHDRAW_RESPONSE_SCHEMA_V1,
    WithdrawRequestReceiptV1, WithdrawRequestV1, WithdrawResponseV1, WithdrawalRailPreferenceV1,
    WithdrawalRow, WithdrawalStatusV1,
};

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
            lp_id: &'a str,
            rail: &'a str,
            amount_sats: u64,
            description: Option<&'a str>,
            expiry_secs: Option<u64>,
        }

        let rail = body.rail.as_str().to_string();
        let request_fingerprint_sha256 = canonical_sha256(&DepositFingerprint {
            schema: DEPOSIT_QUOTE_REQUEST_SCHEMA_V1,
            pool_id,
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
            .get_latest_snapshot(pool_id)
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
                let wallet_request_id =
                    wallet_request_id("deposit", pool_id, lp_id.as_str(), &idempotency_key);
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

        self.maybe_spawn_nostr_liquidity_receipt_pointer_mirror(
            LiquidityReceiptPointerV1 {
                receipt_id: receipt.receipt_id.clone(),
                pool_id: Some(receipt.pool_id.clone()),
                lp_id: Some(receipt.lp_id.clone()),
                deposit_id: Some(receipt.deposit_id.clone()),
                withdrawal_id: None,
                quote_id: None,
                receipt_sha256: receipt.canonical_json_sha256.clone(),
                receipt_url: format!("openagents://receipt/{}", receipt.receipt_id),
            },
        );

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
            lp_id: &'a str,
            shares_burned: u64,
            rail_preference: &'a str,
        }

        let rail_preference = body.rail_preference.as_str().to_string();
        let request_fingerprint_sha256 = canonical_sha256(&WithdrawFingerprint {
            schema: WITHDRAW_REQUEST_SCHEMA_V1,
            pool_id,
            lp_id: lp_id.as_str(),
            shares_burned: body.shares_burned,
            rail_preference: rail_preference.as_str(),
        })
        .map_err(LiquidityPoolError::Internal)?;

        let latest_share_price_sats = self
            .store
            .get_latest_snapshot(pool_id)
            .await
            .map_err(map_store_error)?
            .map(|row| row.share_price_sats)
            .unwrap_or(1);

        let amount_sats_estimate =
            estimate_withdraw_amount_sats(body.shares_burned, latest_share_price_sats)?;
        let earliest_settlement_at =
            Utc::now() + Duration::hours(self.default_withdraw_delay_hours);

        let withdrawal_id = format!("liqwd_{}", Uuid::now_v7());
        let created_at = Utc::now();

        let stored = self
            .store
            .create_or_get_withdrawal(WithdrawalInsertInput {
                withdrawal_id: withdrawal_id.clone(),
                pool_id: pool_id.to_string(),
                lp_id: lp_id.clone(),
                shares_burned: i64::try_from(body.shares_burned).map_err(|_| {
                    LiquidityPoolError::InvalidRequest("shares_burned too large".to_string())
                })?,
                amount_sats_estimate,
                rail_preference: rail_preference.clone(),
                status: WithdrawalStatusV1::Requested.as_str().to_string(),
                request_fingerprint_sha256,
                idempotency_key: idempotency_key.clone(),
                earliest_settlement_at,
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

        self.maybe_spawn_nostr_liquidity_receipt_pointer_mirror(
            LiquidityReceiptPointerV1 {
                receipt_id: receipt.receipt_id.clone(),
                pool_id: Some(receipt.pool_id.clone()),
                lp_id: Some(receipt.lp_id.clone()),
                deposit_id: None,
                withdrawal_id: Some(receipt.withdrawal_id.clone()),
                quote_id: None,
                receipt_sha256: receipt.canonical_json_sha256.clone(),
                receipt_url: format!("openagents://receipt/{}", receipt.receipt_id),
            },
        );

        Ok(WithdrawResponseV1 {
            schema: WITHDRAW_RESPONSE_SCHEMA_V1.to_string(),
            withdrawal: stored,
            receipt,
        })
    }

    pub async fn status(&self, pool_id: &str) -> Result<PoolStatusResponseV1, LiquidityPoolError> {
        let pool = self
            .store
            .get_pool(pool_id)
            .await
            .map_err(map_store_error)?
            .ok_or(LiquidityPoolError::NotFound)?;

        let share_price_sats = self
            .store
            .get_latest_snapshot(pool_id)
            .await
            .map_err(map_store_error)?
            .map(|row| row.share_price_sats)
            .unwrap_or(1);
        let total_shares = self
            .store
            .get_total_shares(pool_id)
            .await
            .map_err(map_store_error)?;
        let pending_withdrawals_sats_estimate = self
            .store
            .get_pending_withdrawals_estimate_sats(pool_id)
            .await
            .map_err(map_store_error)?;

        Ok(PoolStatusResponseV1 {
            schema: POOL_STATUS_SCHEMA_V1.to_string(),
            pool,
            share_price_sats,
            total_shares,
            pending_withdrawals_sats_estimate,
            updated_at: Utc::now(),
        })
    }

    pub async fn latest_snapshot(
        &self,
        pool_id: &str,
    ) -> Result<Option<PoolSnapshotResponseV1>, LiquidityPoolError> {
        let Some(snapshot) = self
            .store
            .get_latest_snapshot(pool_id)
            .await
            .map_err(map_store_error)?
        else {
            return Ok(None);
        };

        let receipt = build_snapshot_receipt(&snapshot, self.receipt_signing_key.as_ref())?;
        Ok(Some(PoolSnapshotResponseV1 {
            schema: POOL_SNAPSHOT_SCHEMA_V1.to_string(),
            snapshot,
            receipt,
        }))
    }

    pub async fn generate_snapshot(
        &self,
        pool_id: &str,
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
        let (mut onchain_sats, mut channel_total_sats, mut channel_outbound_sats, mut channel_inbound_sats) =
            (0u64, 0u64, 0u64, 0u64);
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

        let total_shares = self
            .store
            .get_total_shares(pool_id)
            .await
            .map_err(map_store_error)?;
        let pending_withdrawals_sats_estimate = self
            .store
            .get_pending_withdrawals_estimate_sats(pool_id)
            .await
            .map_err(map_store_error)?;

        let assets_json = json!({
            "schema": "openagents.liquidity.pool_assets.v1",
            "walletBalanceSats": wallet_balance_sats,
            "lightning": {
                "schema": "openagents.liquidity.llp_lightning_snapshot.v1",
                "backend": lightning_backend,
                "onchainSats": onchain_sats,
                "channelTotalSats": channel_total_sats,
                "channelOutboundSats": channel_outbound_sats,
                "channelInboundSats": channel_inbound_sats,
                "channelCount": channel_count,
                "connectedChannelCount": connected_channel_count,
                "lastError": lightning_last_error,
            }
        });

        let liabilities_json = json!({
            "schema": "openagents.liquidity.pool_liabilities.v1",
            "sharesOutstanding": total_shares,
            "pendingWithdrawalsSatsEstimate": pending_withdrawals_sats_estimate,
        });

        let share_price_sats = if total_shares > 0 {
            let denom = u64::try_from(total_shares)
                .map_err(|_| LiquidityPoolError::Internal("total_shares invalid".to_string()))?;
            let per_share = wallet_balance_sats / denom;
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
            as_of: &'a DateTime<Utc>,
            assets_json_sha256: &'a str,
            liabilities_json_sha256: &'a str,
            share_price_sats: i64,
            created_at: &'a DateTime<Utc>,
        }

        let canonical_json_sha256 = canonical_sha256(&SnapshotHashInput {
            schema: POOL_SNAPSHOT_RECEIPT_SCHEMA_V1,
            pool_id,
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

    fn maybe_spawn_nostr_liquidity_receipt_pointer_mirror(&self, payload: LiquidityReceiptPointerV1) {
        let relays = bridge_relays_from_env();
        if relays.is_empty() {
            return;
        }
        let Some(secret_key) = self.receipt_signing_key else {
            return;
        };

        tokio::spawn(async move {
            let event =
                match build_liquidity_receipt_pointer_event(&secret_key, None, &payload) {
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

fn canonical_sha256(value: &impl Serialize) -> Result<String, String> {
    let canonical_json =
        protocol::hash::canonical_json(value).map_err(|error| error.to_string())?;
    let digest = Sha256::digest(canonical_json.as_bytes());
    Ok(hex::encode(digest))
}

fn build_deposit_receipt(
    deposit: &DepositRow,
    rail: DepositRailV1,
    amount_sats: u64,
    created_at: DateTime<Utc>,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<DepositReceiptV1, LiquidityPoolError> {
    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        pool_id: &'a str,
        lp_id: &'a str,
        deposit_id: &'a str,
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
    #[derive(Serialize)]
    struct ReceiptHashInput<'a> {
        schema: &'a str,
        pool_id: &'a str,
        lp_id: &'a str,
        withdrawal_id: &'a str,
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
        shares_burned: withdrawal.shares_burned,
        amount_sats_estimate: withdrawal.amount_sats_estimate,
        rail_preference: rail,
        earliest_settlement_at: withdrawal.earliest_settlement_at,
        created_at,
        canonical_json_sha256,
        signature,
    })
}

fn build_snapshot_receipt(
    snapshot: &PoolSnapshotRow,
    receipt_signing_key: Option<&[u8; 32]>,
) -> Result<PoolSnapshotReceiptV1, LiquidityPoolError> {
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
}
