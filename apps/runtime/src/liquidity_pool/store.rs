use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::db::RuntimeDb;
use crate::liquidity_pool::types::{
    DepositRow, LpAccountRow, PoolRow, PoolSignerSetRow, PoolSigningApprovalRow,
    PoolSigningRequestRow, PoolSnapshotRow, WithdrawalRow,
};

#[derive(Debug, thiserror::Error)]
pub enum LiquidityPoolStoreError {
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("db error: {0}")]
    Db(String),
}

#[derive(Debug, Clone)]
pub struct DepositInsertInput {
    pub deposit_id: String,
    pub pool_id: String,
    pub partition_kind: String,
    pub lp_id: String,
    pub rail: String,
    pub amount_sats: i64,
    pub share_price_sats: i64,
    pub shares_minted: i64,
    pub status: String,
    pub request_fingerprint_sha256: String,
    pub idempotency_key: String,
    pub invoice_bolt11: Option<String>,
    pub invoice_hash: Option<String>,
    pub deposit_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct WithdrawalInsertInput {
    pub withdrawal_id: String,
    pub pool_id: String,
    pub partition_kind: String,
    pub lp_id: String,
    pub shares_burned: i64,
    pub amount_sats_estimate: i64,
    pub rail_preference: String,
    pub status: String,
    pub request_fingerprint_sha256: String,
    pub idempotency_key: String,
    pub earliest_settlement_at: DateTime<Utc>,
    pub payout_invoice_bolt11: Option<String>,
    pub payout_invoice_hash: Option<String>,
    pub payout_address: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ReceiptInsertInput {
    pub receipt_id: String,
    pub entity_kind: String,
    pub entity_id: String,
    pub schema: String,
    pub canonical_json_sha256: String,
    pub signature_json: Option<Value>,
    pub receipt_json: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SignerSetUpsertInput {
    pub pool_id: String,
    pub schema: String,
    pub threshold: i64,
    pub signers_json: Value,
    pub policy_json: Value,
    pub canonical_json_sha256: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SigningRequestInsertInput {
    pub request_id: String,
    pub pool_id: String,
    pub action_class: String,
    pub idempotency_key: String,
    pub payload_json: Value,
    pub payload_sha256: String,
    pub required_signatures: i64,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct SigningApprovalInsertInput {
    pub approval_id: String,
    pub request_id: String,
    pub signer_pubkey: String,
    pub signature_json: Value,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait LiquidityPoolStore: Send + Sync {
    async fn create_or_get_pool(&self, pool: PoolRow) -> Result<PoolRow, LiquidityPoolStoreError>;
    async fn get_pool(&self, pool_id: &str) -> Result<Option<PoolRow>, LiquidityPoolStoreError>;

    async fn create_or_get_deposit(
        &self,
        input: DepositInsertInput,
    ) -> Result<DepositRow, LiquidityPoolStoreError>;

    async fn confirm_deposit_and_mint_shares(
        &self,
        pool_id: &str,
        deposit_id: &str,
        confirmed_at: DateTime<Utc>,
    ) -> Result<(DepositRow, bool), LiquidityPoolStoreError>;

    async fn create_or_get_withdrawal(
        &self,
        input: WithdrawalInsertInput,
    ) -> Result<WithdrawalRow, LiquidityPoolStoreError>;

    async fn get_withdrawal(
        &self,
        pool_id: &str,
        withdrawal_id: &str,
    ) -> Result<Option<WithdrawalRow>, LiquidityPoolStoreError>;

    async fn list_due_withdrawals(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<WithdrawalRow>, LiquidityPoolStoreError>;

    async fn mark_withdrawal_paid_and_burn_shares(
        &self,
        pool_id: &str,
        withdrawal_id: &str,
        wallet_receipt_sha256: &str,
        paid_at: DateTime<Utc>,
    ) -> Result<WithdrawalRow, LiquidityPoolStoreError>;

    async fn get_lp_account(
        &self,
        pool_id: &str,
        partition_kind: &str,
        lp_id: &str,
    ) -> Result<Option<LpAccountRow>, LiquidityPoolStoreError>;

    async fn get_total_shares(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError>;

    async fn get_confirmed_deposits_total_sats(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError>;

    async fn get_pending_withdrawals_estimate_sats(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError>;

    async fn create_or_get_snapshot(
        &self,
        snapshot: PoolSnapshotRow,
    ) -> Result<PoolSnapshotRow, LiquidityPoolStoreError>;

    async fn get_latest_snapshot(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<Option<PoolSnapshotRow>, LiquidityPoolStoreError>;

    async fn prune_snapshots_keep_latest(
        &self,
        pool_id: &str,
        partition_kind: &str,
        keep_latest: i64,
    ) -> Result<u64, LiquidityPoolStoreError>;

    /// CEP-specific liabilities: reserved commitments from accepted credit envelopes.
    async fn get_credit_reserved_commitments_sats(
        &self,
        pool_id: &str,
        now: DateTime<Utc>,
    ) -> Result<i64, LiquidityPoolStoreError>;

    async fn put_receipt(&self, receipt: ReceiptInsertInput)
    -> Result<(), LiquidityPoolStoreError>;

    async fn upsert_signer_set(
        &self,
        input: SignerSetUpsertInput,
    ) -> Result<PoolSignerSetRow, LiquidityPoolStoreError>;

    async fn get_signer_set(
        &self,
        pool_id: &str,
    ) -> Result<Option<PoolSignerSetRow>, LiquidityPoolStoreError>;

    async fn create_or_get_signing_request(
        &self,
        input: SigningRequestInsertInput,
    ) -> Result<PoolSigningRequestRow, LiquidityPoolStoreError>;

    async fn get_signing_request(
        &self,
        pool_id: &str,
        request_id: &str,
    ) -> Result<Option<PoolSigningRequestRow>, LiquidityPoolStoreError>;

    async fn list_signing_requests(
        &self,
        pool_id: &str,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PoolSigningRequestRow>, LiquidityPoolStoreError>;

    async fn create_or_get_signing_approval(
        &self,
        input: SigningApprovalInsertInput,
    ) -> Result<PoolSigningApprovalRow, LiquidityPoolStoreError>;

    async fn list_signing_approvals(
        &self,
        request_id: &str,
    ) -> Result<Vec<PoolSigningApprovalRow>, LiquidityPoolStoreError>;

    async fn mark_signing_request_executed(
        &self,
        request_id: &str,
        status: &str,
        execution_result_json: Value,
        executed_at: DateTime<Utc>,
    ) -> Result<PoolSigningRequestRow, LiquidityPoolStoreError>;
}

pub fn memory() -> Arc<dyn LiquidityPoolStore> {
    Arc::new(MemoryLiquidityPoolStore::default())
}

pub fn postgres(db: Arc<RuntimeDb>) -> Arc<dyn LiquidityPoolStore> {
    Arc::new(PostgresLiquidityPoolStore { db })
}

#[derive(Default)]
struct MemoryLiquidityPoolStore {
    inner: Mutex<MemoryPoolInner>,
}

#[derive(Default)]
struct MemoryPoolInner {
    pools: HashMap<String, PoolRow>,
    lp_accounts: HashMap<(String, String, String), LpAccountRow>,
    deposits: HashMap<String, (DepositRow, String)>,
    deposit_by_idempotency: HashMap<(String, String, String, String), String>,
    withdrawals: HashMap<String, (WithdrawalRow, String)>,
    withdrawal_by_idempotency: HashMap<(String, String, String, String), String>,
    latest_snapshot_by_pool_partition: HashMap<(String, String), PoolSnapshotRow>,
    receipts_by_unique: HashMap<(String, String, String), String>,
    receipts_by_id: HashMap<String, ReceiptInsertInput>,
    signer_sets_by_pool: HashMap<String, PoolSignerSetRow>,
    signing_requests: HashMap<String, (PoolSigningRequestRow, String)>,
    signing_request_by_idempotency: HashMap<(String, String, String), String>,
    signing_approvals_by_unique: HashMap<(String, String), String>,
    signing_approvals_by_id: HashMap<String, PoolSigningApprovalRow>,
}

#[async_trait]
impl LiquidityPoolStore for MemoryLiquidityPoolStore {
    async fn create_or_get_pool(&self, pool: PoolRow) -> Result<PoolRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        if let Some(existing) = inner.pools.get(&pool.pool_id).cloned() {
            if existing.pool_kind != pool.pool_kind || existing.operator_id != pool.operator_id {
                return Err(LiquidityPoolStoreError::Conflict(
                    "pool_id reused with different pool metadata".to_string(),
                ));
            }
            return Ok(existing);
        }
        inner.pools.insert(pool.pool_id.clone(), pool.clone());
        Ok(pool)
    }

    async fn get_pool(&self, pool_id: &str) -> Result<Option<PoolRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner.pools.get(pool_id).cloned())
    }

    async fn create_or_get_deposit(
        &self,
        input: DepositInsertInput,
    ) -> Result<DepositRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let key = (
            input.pool_id.clone(),
            input.partition_kind.clone(),
            input.lp_id.clone(),
            input.idempotency_key.clone(),
        );
        if let Some(existing_id) = inner.deposit_by_idempotency.get(&key) {
            let (row, fingerprint) =
                inner.deposits.get(existing_id).cloned().ok_or_else(|| {
                    LiquidityPoolStoreError::Db("missing deposit row".to_string())
                })?;
            if fingerprint != input.request_fingerprint_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "idempotency_key reused with different deposit parameters".to_string(),
                ));
            }
            return Ok(row);
        }

        let row = DepositRow {
            deposit_id: input.deposit_id.clone(),
            pool_id: input.pool_id.clone(),
            partition_kind: input.partition_kind.clone(),
            lp_id: input.lp_id.clone(),
            rail: input.rail.clone(),
            amount_sats: input.amount_sats,
            share_price_sats: input.share_price_sats,
            shares_minted: input.shares_minted,
            status: input.status.clone(),
            idempotency_key: input.idempotency_key.clone(),
            invoice_bolt11: input.invoice_bolt11.clone(),
            invoice_hash: input.invoice_hash.clone(),
            deposit_address: input.deposit_address.clone(),
            created_at: input.created_at,
            confirmed_at: None,
        };

        inner
            .deposit_by_idempotency
            .insert(key, input.deposit_id.clone());
        inner.deposits.insert(
            input.deposit_id.clone(),
            (row.clone(), input.request_fingerprint_sha256),
        );
        Ok(row)
    }

    async fn confirm_deposit_and_mint_shares(
        &self,
        pool_id: &str,
        deposit_id: &str,
        confirmed_at: DateTime<Utc>,
    ) -> Result<(DepositRow, bool), LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let Some((row, fingerprint)) = inner.deposits.get(deposit_id).cloned() else {
            return Err(LiquidityPoolStoreError::NotFound("deposit".to_string()));
        };

        if row.pool_id != pool_id {
            return Err(LiquidityPoolStoreError::NotFound("deposit".to_string()));
        }

        if row.status == "confirmed" {
            return Ok((row, false));
        }
        if row.status != "quoted" && row.status != "pending" {
            return Err(LiquidityPoolStoreError::Conflict(
                "deposit not in a confirmable state".to_string(),
            ));
        }

        let mut updated = row.clone();
        updated.status = "confirmed".to_string();
        updated.confirmed_at = Some(confirmed_at);

        let lp_key = (
            updated.pool_id.clone(),
            updated.partition_kind.clone(),
            updated.lp_id.clone(),
        );
        let account = inner
            .lp_accounts
            .entry(lp_key.clone())
            .or_insert_with(|| LpAccountRow {
                pool_id: updated.pool_id.clone(),
                partition_kind: updated.partition_kind.clone(),
                lp_id: updated.lp_id.clone(),
                shares_total: 0,
                updated_at: confirmed_at,
            });
        account.shares_total = account
            .shares_total
            .checked_add(updated.shares_minted)
            .ok_or_else(|| LiquidityPoolStoreError::Db("share accounting overflow".to_string()))?;
        account.updated_at = confirmed_at;

        inner
            .deposits
            .insert(deposit_id.to_string(), (updated.clone(), fingerprint));
        Ok((updated, true))
    }

    async fn create_or_get_withdrawal(
        &self,
        input: WithdrawalInsertInput,
    ) -> Result<WithdrawalRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let key = (
            input.pool_id.clone(),
            input.partition_kind.clone(),
            input.lp_id.clone(),
            input.idempotency_key.clone(),
        );
        if let Some(existing_id) = inner.withdrawal_by_idempotency.get(&key) {
            let (row, fingerprint) =
                inner.withdrawals.get(existing_id).cloned().ok_or_else(|| {
                    LiquidityPoolStoreError::Db("missing withdrawal row".to_string())
                })?;
            if fingerprint != input.request_fingerprint_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "idempotency_key reused with different withdrawal parameters".to_string(),
                ));
            }
            return Ok(row);
        }

        let row = WithdrawalRow {
            withdrawal_id: input.withdrawal_id.clone(),
            pool_id: input.pool_id.clone(),
            partition_kind: input.partition_kind.clone(),
            lp_id: input.lp_id.clone(),
            shares_burned: input.shares_burned,
            amount_sats_estimate: input.amount_sats_estimate,
            rail_preference: input.rail_preference.clone(),
            status: input.status.clone(),
            idempotency_key: input.idempotency_key.clone(),
            earliest_settlement_at: input.earliest_settlement_at,
            payout_invoice_bolt11: input.payout_invoice_bolt11.clone(),
            payout_invoice_hash: input.payout_invoice_hash.clone(),
            payout_address: input.payout_address.clone(),
            wallet_receipt_sha256: None,
            created_at: input.created_at,
            paid_at: None,
        };

        inner
            .withdrawal_by_idempotency
            .insert(key, input.withdrawal_id.clone());
        inner.withdrawals.insert(
            input.withdrawal_id.clone(),
            (row.clone(), input.request_fingerprint_sha256),
        );
        Ok(row)
    }

    async fn get_withdrawal(
        &self,
        pool_id: &str,
        withdrawal_id: &str,
    ) -> Result<Option<WithdrawalRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let Some((row, _)) = inner.withdrawals.get(withdrawal_id) else {
            return Ok(None);
        };
        if row.pool_id != pool_id {
            return Ok(None);
        }
        Ok(Some(row.clone()))
    }

    async fn list_due_withdrawals(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<WithdrawalRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut rows = inner
            .withdrawals
            .values()
            .map(|(row, _)| row.clone())
            .filter(|row| {
                (row.status == "queued" || row.status == "approved")
                    && row.paid_at.is_none()
                    && row.earliest_settlement_at <= now
            })
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| a.earliest_settlement_at.cmp(&b.earliest_settlement_at));
        let limit = usize::try_from(limit.max(1).min(5_000)).unwrap_or(5_000);
        rows.truncate(limit);
        Ok(rows)
    }

    async fn mark_withdrawal_paid_and_burn_shares(
        &self,
        pool_id: &str,
        withdrawal_id: &str,
        wallet_receipt_sha256: &str,
        paid_at: DateTime<Utc>,
    ) -> Result<WithdrawalRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let Some((row, fingerprint)) = inner.withdrawals.get(withdrawal_id).cloned() else {
            return Err(LiquidityPoolStoreError::NotFound(
                "withdrawal not found".to_string(),
            ));
        };
        if row.pool_id != pool_id {
            return Err(LiquidityPoolStoreError::NotFound(
                "withdrawal not found".to_string(),
            ));
        }
        if row.paid_at.is_some() || row.status == "paid" {
            return Ok(row);
        }
        if row.status != "queued" && row.status != "approved" {
            return Err(LiquidityPoolStoreError::Conflict(
                "withdrawal not in a payable state".to_string(),
            ));
        }

        let lp_key = (
            row.pool_id.clone(),
            row.partition_kind.clone(),
            row.lp_id.clone(),
        );
        let Some(account) = inner.lp_accounts.get_mut(&lp_key) else {
            return Err(LiquidityPoolStoreError::Conflict(
                "lp account not found".to_string(),
            ));
        };
        let new_total = account
            .shares_total
            .checked_sub(row.shares_burned)
            .ok_or_else(|| LiquidityPoolStoreError::Conflict("insufficient shares".to_string()))?;
        if new_total < 0 {
            return Err(LiquidityPoolStoreError::Conflict(
                "insufficient shares".to_string(),
            ));
        }
        account.shares_total = new_total;
        account.updated_at = paid_at;

        let mut updated = row.clone();
        updated.status = "paid".to_string();
        updated.wallet_receipt_sha256 = Some(wallet_receipt_sha256.trim().to_string());
        updated.paid_at = Some(paid_at);

        inner
            .withdrawals
            .insert(withdrawal_id.to_string(), (updated.clone(), fingerprint));
        Ok(updated)
    }

    async fn get_lp_account(
        &self,
        pool_id: &str,
        partition_kind: &str,
        lp_id: &str,
    ) -> Result<Option<LpAccountRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .lp_accounts
            .get(&(
                pool_id.to_string(),
                partition_kind.to_string(),
                lp_id.to_string(),
            ))
            .cloned())
    }

    async fn get_total_shares(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut total = 0_i64;
        for ((pool, partition, _), row) in inner.lp_accounts.iter() {
            if pool == pool_id && partition == partition_kind {
                total = total.checked_add(row.shares_total).ok_or_else(|| {
                    LiquidityPoolStoreError::Db("share accounting overflow".to_string())
                })?;
            }
        }
        Ok(total)
    }

    async fn get_confirmed_deposits_total_sats(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut total = 0_i64;
        for (row, _) in inner.deposits.values() {
            if row.pool_id == pool_id
                && row.partition_kind == partition_kind
                && row.status == "confirmed"
            {
                total = total.saturating_add(row.amount_sats);
            }
        }
        Ok(total)
    }

    async fn get_pending_withdrawals_estimate_sats(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut total = 0_i64;
        for (row, _) in inner.withdrawals.values() {
            if row.pool_id == pool_id
                && row.partition_kind == partition_kind
                && (row.status == "requested" || row.status == "queued" || row.status == "approved")
            {
                total = total.saturating_add(row.amount_sats_estimate);
            }
        }
        Ok(total)
    }

    async fn create_or_get_snapshot(
        &self,
        snapshot: PoolSnapshotRow,
    ) -> Result<PoolSnapshotRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        if let Some(existing) = inner
            .latest_snapshot_by_pool_partition
            .get(&(snapshot.pool_id.clone(), snapshot.partition_kind.clone()))
            .filter(|existing| existing.snapshot_id == snapshot.snapshot_id)
            .cloned()
        {
            return Ok(existing);
        }
        inner.latest_snapshot_by_pool_partition.insert(
            (snapshot.pool_id.clone(), snapshot.partition_kind.clone()),
            snapshot.clone(),
        );
        Ok(snapshot)
    }

    async fn get_latest_snapshot(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<Option<PoolSnapshotRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .latest_snapshot_by_pool_partition
            .get(&(pool_id.to_string(), partition_kind.to_string()))
            .cloned())
    }

    async fn prune_snapshots_keep_latest(
        &self,
        _pool_id: &str,
        _partition_kind: &str,
        _keep_latest: i64,
    ) -> Result<u64, LiquidityPoolStoreError> {
        // Memory store keeps only the latest snapshot projection per pool/partition.
        Ok(0)
    }

    async fn get_credit_reserved_commitments_sats(
        &self,
        _pool_id: &str,
        _now: DateTime<Utc>,
    ) -> Result<i64, LiquidityPoolStoreError> {
        Ok(0)
    }

    async fn put_receipt(
        &self,
        receipt: ReceiptInsertInput,
    ) -> Result<(), LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let unique = (
            receipt.entity_kind.clone(),
            receipt.entity_id.clone(),
            receipt.schema.clone(),
        );
        if let Some(existing_id) = inner.receipts_by_unique.get(&unique) {
            let existing = inner
                .receipts_by_id
                .get(existing_id)
                .cloned()
                .ok_or_else(|| LiquidityPoolStoreError::Db("missing receipt row".to_string()))?;
            if existing.canonical_json_sha256 != receipt.canonical_json_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "receipt already exists for entity_kind/entity_id/schema with different digest"
                        .to_string(),
                ));
            }
            return Ok(());
        }

        inner
            .receipts_by_unique
            .insert(unique, receipt.receipt_id.clone());
        inner
            .receipts_by_id
            .insert(receipt.receipt_id.clone(), receipt);
        Ok(())
    }

    async fn upsert_signer_set(
        &self,
        input: SignerSetUpsertInput,
    ) -> Result<PoolSignerSetRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let created_at = inner
            .signer_sets_by_pool
            .get(&input.pool_id)
            .map(|row| row.created_at)
            .unwrap_or(input.updated_at);

        let signers: Vec<crate::liquidity_pool::types::PoolSignerV1> =
            serde_json::from_value(input.signers_json.clone())
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        let policy: crate::liquidity_pool::types::PoolSignerPolicyV1 =
            serde_json::from_value(input.policy_json.clone())
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let row = PoolSignerSetRow {
            pool_id: input.pool_id.clone(),
            schema: input.schema,
            threshold: u32::try_from(input.threshold)
                .map_err(|_| LiquidityPoolStoreError::Db("threshold overflow".to_string()))?,
            signers,
            policy,
            canonical_json_sha256: input.canonical_json_sha256,
            created_at,
            updated_at: input.updated_at,
        };

        inner.signer_sets_by_pool.insert(input.pool_id, row.clone());
        Ok(row)
    }

    async fn get_signer_set(
        &self,
        pool_id: &str,
    ) -> Result<Option<PoolSignerSetRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner.signer_sets_by_pool.get(pool_id).cloned())
    }

    async fn create_or_get_signing_request(
        &self,
        input: SigningRequestInsertInput,
    ) -> Result<PoolSigningRequestRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let key = (
            input.pool_id.clone(),
            input.action_class.clone(),
            input.idempotency_key.clone(),
        );

        if let Some(existing_id) = inner.signing_request_by_idempotency.get(&key) {
            let (row, fingerprint) = inner
                .signing_requests
                .get(existing_id)
                .cloned()
                .ok_or_else(|| {
                    LiquidityPoolStoreError::Db("missing signing request row".to_string())
                })?;
            if fingerprint != input.payload_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "idempotency_key reused with different signing payload".to_string(),
                ));
            }
            return Ok(row);
        }

        let row = PoolSigningRequestRow {
            request_id: input.request_id.clone(),
            pool_id: input.pool_id.clone(),
            action_class: input.action_class.clone(),
            idempotency_key: input.idempotency_key.clone(),
            payload_json: input.payload_json.clone(),
            payload_sha256: input.payload_sha256.clone(),
            required_signatures: u32::try_from(input.required_signatures).map_err(|_| {
                LiquidityPoolStoreError::Db("required_signatures overflow".to_string())
            })?,
            status: input.status.clone(),
            execution_result_json: None,
            created_at: input.created_at,
            executed_at: None,
        };

        inner
            .signing_request_by_idempotency
            .insert(key, input.request_id.clone());
        inner.signing_requests.insert(
            input.request_id.clone(),
            (row.clone(), input.payload_sha256),
        );
        Ok(row)
    }

    async fn get_signing_request(
        &self,
        pool_id: &str,
        request_id: &str,
    ) -> Result<Option<PoolSigningRequestRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .signing_requests
            .get(request_id)
            .filter(|(row, _)| row.pool_id == pool_id)
            .map(|(row, _)| row.clone()))
    }

    async fn list_signing_requests(
        &self,
        pool_id: &str,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PoolSigningRequestRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut out = inner
            .signing_requests
            .values()
            .map(|(row, _)| row.clone())
            .filter(|row| row.pool_id == pool_id)
            .filter(|row| status.map_or(true, |status| row.status == status))
            .collect::<Vec<_>>();
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        if limit > 0 && out.len() > limit as usize {
            out.truncate(limit as usize);
        }
        Ok(out)
    }

    async fn create_or_get_signing_approval(
        &self,
        input: SigningApprovalInsertInput,
    ) -> Result<PoolSigningApprovalRow, LiquidityPoolStoreError> {
        let signature: crate::artifacts::ReceiptSignatureV1 =
            serde_json::from_value(input.signature_json.clone())
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let mut inner = self.inner.lock().await;
        let key = (input.request_id.clone(), input.signer_pubkey.clone());
        if let Some(existing_id) = inner.signing_approvals_by_unique.get(&key) {
            let existing = inner
                .signing_approvals_by_id
                .get(existing_id)
                .cloned()
                .ok_or_else(|| LiquidityPoolStoreError::Db("missing approval row".to_string()))?;

            if existing.signature.signed_sha256 != signature.signed_sha256
                || existing.signature.signature_hex != signature.signature_hex
                || existing.signature.scheme != signature.scheme
            {
                return Err(LiquidityPoolStoreError::Conflict(
                    "approval already exists with different signature".to_string(),
                ));
            }
            return Ok(existing);
        }

        let row = PoolSigningApprovalRow {
            approval_id: input.approval_id.clone(),
            request_id: input.request_id.clone(),
            signer_pubkey: input.signer_pubkey.clone(),
            signature,
            created_at: input.created_at,
        };

        inner
            .signing_approvals_by_unique
            .insert(key, input.approval_id.clone());
        inner
            .signing_approvals_by_id
            .insert(input.approval_id, row.clone());
        Ok(row)
    }

    async fn list_signing_approvals(
        &self,
        request_id: &str,
    ) -> Result<Vec<PoolSigningApprovalRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut out = inner
            .signing_approvals_by_id
            .values()
            .filter(|row| row.request_id == request_id)
            .cloned()
            .collect::<Vec<_>>();
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(out)
    }

    async fn mark_signing_request_executed(
        &self,
        request_id: &str,
        status: &str,
        execution_result_json: Value,
        executed_at: DateTime<Utc>,
    ) -> Result<PoolSigningRequestRow, LiquidityPoolStoreError> {
        let mut inner = self.inner.lock().await;
        let Some((row, fingerprint)) = inner.signing_requests.get(request_id).cloned() else {
            return Err(LiquidityPoolStoreError::NotFound(
                "signing_request".to_string(),
            ));
        };

        let mut updated = row.clone();
        updated.status = status.to_string();
        updated.execution_result_json = Some(execution_result_json);
        updated.executed_at = Some(executed_at);

        inner
            .signing_requests
            .insert(request_id.to_string(), (updated.clone(), fingerprint));
        Ok(updated)
    }
}

struct PostgresLiquidityPoolStore {
    db: Arc<RuntimeDb>,
}

#[async_trait]
impl LiquidityPoolStore for PostgresLiquidityPoolStore {
    async fn create_or_get_pool(&self, pool: PoolRow) -> Result<PoolRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT pool_id, pool_kind, operator_id, status, config, created_at
                  FROM runtime.liquidity_pools
                 WHERE pool_id = $1
                "#,
                &[&pool.pool_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let out = map_pool_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            if out.pool_kind != pool.pool_kind || out.operator_id != pool.operator_id {
                return Err(LiquidityPoolStoreError::Conflict(
                    "pool_id reused with different pool metadata".to_string(),
                ));
            }
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_pools (
                  pool_id,
                  pool_kind,
                  operator_id,
                  status,
                  config,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6)
                RETURNING pool_id, pool_kind, operator_id, status, config, created_at
                "#,
                &[
                    &pool.pool_id,
                    &pool.pool_kind,
                    &pool.operator_id,
                    &pool.status,
                    &pool.config,
                    &pool.created_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        map_pool_row(&inserted).map_err(LiquidityPoolStoreError::Db)
    }

    async fn get_pool(&self, pool_id: &str) -> Result<Option<PoolRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT pool_id, pool_kind, operator_id, status, config, created_at
                  FROM runtime.liquidity_pools
                 WHERE pool_id = $1
                "#,
                &[&pool_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(
            map_pool_row(&row).map_err(LiquidityPoolStoreError::Db)?,
        ))
    }

    async fn create_or_get_deposit(
        &self,
        input: DepositInsertInput,
    ) -> Result<DepositRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT deposit_id,
                       pool_id,
                       partition_kind,
                       lp_id,
                       rail,
                       amount_sats,
                       share_price_sats,
                       shares_minted,
                       status,
                       request_fingerprint_sha256,
                       idempotency_key,
                       invoice_bolt11,
                       invoice_hash,
                       deposit_address,
                       created_at,
                       confirmed_at
                  FROM runtime.liquidity_deposits
                 WHERE pool_id = $1
                   AND partition_kind = $2
                   AND lp_id = $3
                   AND idempotency_key = $4
                "#,
                &[
                    &input.pool_id,
                    &input.partition_kind,
                    &input.lp_id,
                    &input.idempotency_key,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let fingerprint: String = row.get("request_fingerprint_sha256");
            if fingerprint != input.request_fingerprint_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "idempotency_key reused with different deposit parameters".to_string(),
                ));
            }
            let out = map_deposit_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_deposits (
                  deposit_id,
                  pool_id,
                  partition_kind,
                  lp_id,
                  rail,
                  amount_sats,
                  share_price_sats,
                  shares_minted,
                  status,
                  request_fingerprint_sha256,
                  idempotency_key,
                  invoice_bolt11,
                  invoice_hash,
                  deposit_address,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
                RETURNING deposit_id,
                          pool_id,
                          partition_kind,
                          lp_id,
                          rail,
                          amount_sats,
                          share_price_sats,
                          shares_minted,
                          status,
                          request_fingerprint_sha256,
                          idempotency_key,
                          invoice_bolt11,
                          invoice_hash,
                          deposit_address,
                          created_at,
                          confirmed_at
                "#,
                &[
                    &input.deposit_id,
                    &input.pool_id,
                    &input.partition_kind,
                    &input.lp_id,
                    &input.rail,
                    &input.amount_sats,
                    &input.share_price_sats,
                    &input.shares_minted,
                    &input.status,
                    &input.request_fingerprint_sha256,
                    &input.idempotency_key,
                    &input.invoice_bolt11,
                    &input.invoice_hash,
                    &input.deposit_address,
                    &input.created_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        map_deposit_row(&inserted).map_err(LiquidityPoolStoreError::Db)
    }

    async fn confirm_deposit_and_mint_shares(
        &self,
        pool_id: &str,
        deposit_id: &str,
        confirmed_at: DateTime<Utc>,
    ) -> Result<(DepositRow, bool), LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let row = tx
            .query_opt(
                r#"
                SELECT deposit_id,
                       pool_id,
                       partition_kind,
                       lp_id,
                       rail,
                       amount_sats,
                       share_price_sats,
                       shares_minted,
                       status,
                       request_fingerprint_sha256,
                       idempotency_key,
                       invoice_bolt11,
                       invoice_hash,
                       deposit_address,
                       created_at,
                       confirmed_at
                  FROM runtime.liquidity_deposits
                 WHERE deposit_id = $1
                 FOR UPDATE
                "#,
                &[&deposit_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let Some(row) = row else {
            return Err(LiquidityPoolStoreError::NotFound("deposit".to_string()));
        };

        let row_pool_id: String = row.get("pool_id");
        if row_pool_id != pool_id {
            return Err(LiquidityPoolStoreError::NotFound("deposit".to_string()));
        }

        let status: String = row.get("status");
        if status == "confirmed" {
            let out = map_deposit_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok((out, false));
        }

        if status != "quoted" && status != "pending" {
            return Err(LiquidityPoolStoreError::Conflict(
                "deposit not in a confirmable state".to_string(),
            ));
        }

        let shares_minted: i64 = row.get("shares_minted");
        let lp_id: String = row.get("lp_id");
        let partition_kind: String = row.get("partition_kind");

        let updated = tx
            .query_one(
                r#"
                UPDATE runtime.liquidity_deposits
                   SET status = 'confirmed',
                       confirmed_at = $2
                 WHERE deposit_id = $1
                 RETURNING deposit_id,
                          pool_id,
                          partition_kind,
                          lp_id,
                          rail,
                          amount_sats,
                          share_price_sats,
                          shares_minted,
                          status,
                          request_fingerprint_sha256,
                          idempotency_key,
                          invoice_bolt11,
                          invoice_hash,
                          deposit_address,
                          created_at,
                          confirmed_at
                "#,
                &[&deposit_id, &confirmed_at],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        // Upsert LP account, then credit shares.
        tx.execute(
            r#"
            INSERT INTO runtime.liquidity_lp_accounts (pool_id, partition_kind, lp_id, shares_total, updated_at)
            VALUES ($1,$2,$3,0,$4)
            ON CONFLICT (pool_id, lp_id, partition_kind) DO NOTHING
            "#,
            &[&pool_id, &partition_kind, &lp_id, &confirmed_at],
        )
        .await
        .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.execute(
            r#"
            UPDATE runtime.liquidity_lp_accounts
               SET shares_total = shares_total + $3,
                   updated_at = $4
             WHERE pool_id = $1
               AND partition_kind = $2
               AND lp_id = $5
            "#,
            &[
                &pool_id,
                &partition_kind,
                &shares_minted,
                &confirmed_at,
                &lp_id,
            ],
        )
        .await
        .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        map_deposit_row(&updated)
            .map(|out| (out, true))
            .map_err(LiquidityPoolStoreError::Db)
    }

    async fn create_or_get_withdrawal(
        &self,
        input: WithdrawalInsertInput,
    ) -> Result<WithdrawalRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT withdrawal_id,
                       pool_id,
                       partition_kind,
                       lp_id,
                       shares_burned,
                       amount_sats_estimate,
                       rail_preference,
                       status,
                       request_fingerprint_sha256,
                       idempotency_key,
                       earliest_settlement_at,
                       payout_invoice_bolt11,
                       payout_invoice_hash,
                       payout_address,
                       wallet_receipt_sha256,
                       created_at,
                       paid_at
                  FROM runtime.liquidity_withdrawals
                 WHERE pool_id = $1
                   AND partition_kind = $2
                   AND lp_id = $3
                   AND idempotency_key = $4
                "#,
                &[
                    &input.pool_id,
                    &input.partition_kind,
                    &input.lp_id,
                    &input.idempotency_key,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let fingerprint: String = row.get("request_fingerprint_sha256");
            if fingerprint != input.request_fingerprint_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "idempotency_key reused with different withdrawal parameters".to_string(),
                ));
            }
            let out = map_withdrawal_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_withdrawals (
                  withdrawal_id,
                  pool_id,
                  partition_kind,
                  lp_id,
                  shares_burned,
                  amount_sats_estimate,
                  rail_preference,
                  status,
                  request_fingerprint_sha256,
                  idempotency_key,
                  earliest_settlement_at,
                  payout_invoice_bolt11,
                  payout_invoice_hash,
                  payout_address,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
                RETURNING withdrawal_id,
                          pool_id,
                          partition_kind,
                          lp_id,
                          shares_burned,
                          amount_sats_estimate,
                          rail_preference,
                          status,
                          request_fingerprint_sha256,
                          idempotency_key,
                          earliest_settlement_at,
                          payout_invoice_bolt11,
                          payout_invoice_hash,
                          payout_address,
                          wallet_receipt_sha256,
                          created_at,
                          paid_at
                "#,
                &[
                    &input.withdrawal_id,
                    &input.pool_id,
                    &input.partition_kind,
                    &input.lp_id,
                    &input.shares_burned,
                    &input.amount_sats_estimate,
                    &input.rail_preference,
                    &input.status,
                    &input.request_fingerprint_sha256,
                    &input.idempotency_key,
                    &input.earliest_settlement_at,
                    &input.payout_invoice_bolt11,
                    &input.payout_invoice_hash,
                    &input.payout_address,
                    &input.created_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        map_withdrawal_row(&inserted).map_err(LiquidityPoolStoreError::Db)
    }

    async fn get_withdrawal(
        &self,
        pool_id: &str,
        withdrawal_id: &str,
    ) -> Result<Option<WithdrawalRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT withdrawal_id,
                       pool_id,
                       partition_kind,
                       lp_id,
                       shares_burned,
                       amount_sats_estimate,
                       rail_preference,
                       status,
                       request_fingerprint_sha256,
                       idempotency_key,
                       earliest_settlement_at,
                       payout_invoice_bolt11,
                       payout_invoice_hash,
                       payout_address,
                       wallet_receipt_sha256,
                       created_at,
                       paid_at
                  FROM runtime.liquidity_withdrawals
                 WHERE pool_id = $1
                   AND withdrawal_id = $2
                "#,
                &[&pool_id, &withdrawal_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(row
            .as_ref()
            .map(map_withdrawal_row)
            .transpose()
            .map_err(LiquidityPoolStoreError::Db)?)
    }

    async fn list_due_withdrawals(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<WithdrawalRow>, LiquidityPoolStoreError> {
        let limit = limit.max(1).min(5_000);
        let client = self.db.client();
        let client = client.lock().await;
        let rows = client
            .query(
                r#"
                SELECT withdrawal_id,
                       pool_id,
                       partition_kind,
                       lp_id,
                       shares_burned,
                       amount_sats_estimate,
                       rail_preference,
                       status,
                       request_fingerprint_sha256,
                       idempotency_key,
                       earliest_settlement_at,
                       payout_invoice_bolt11,
                       payout_invoice_hash,
                       payout_address,
                       wallet_receipt_sha256,
                       created_at,
                       paid_at
                  FROM runtime.liquidity_withdrawals
                 WHERE (status = 'queued' OR status = 'approved')
                   AND paid_at IS NULL
                   AND earliest_settlement_at <= $1
                 ORDER BY earliest_settlement_at ASC, created_at ASC
                 LIMIT $2
                "#,
                &[&now, &limit],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_withdrawal_row(&row).map_err(LiquidityPoolStoreError::Db)?);
        }
        Ok(out)
    }

    async fn mark_withdrawal_paid_and_burn_shares(
        &self,
        pool_id: &str,
        withdrawal_id: &str,
        wallet_receipt_sha256: &str,
        paid_at: DateTime<Utc>,
    ) -> Result<WithdrawalRow, LiquidityPoolStoreError> {
        let wallet_receipt_sha256 = wallet_receipt_sha256.trim().to_string();
        if wallet_receipt_sha256.is_empty() {
            return Err(LiquidityPoolStoreError::Conflict(
                "wallet_receipt_sha256 is required".to_string(),
            ));
        }

        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT withdrawal_id,
                       pool_id,
                       partition_kind,
                       lp_id,
                       shares_burned,
                       amount_sats_estimate,
                       rail_preference,
                       status,
                       request_fingerprint_sha256,
                       idempotency_key,
                       earliest_settlement_at,
                       payout_invoice_bolt11,
                       payout_invoice_hash,
                       payout_address,
                       wallet_receipt_sha256,
                       created_at,
                       paid_at
                  FROM runtime.liquidity_withdrawals
                 WHERE pool_id = $1
                   AND withdrawal_id = $2
                 FOR UPDATE
                "#,
                &[&pool_id, &withdrawal_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let Some(existing) = existing else {
            return Err(LiquidityPoolStoreError::NotFound(
                "withdrawal not found".to_string(),
            ));
        };

        let existing_row = map_withdrawal_row(&existing).map_err(LiquidityPoolStoreError::Db)?;
        if existing_row.status == "paid" || existing_row.paid_at.is_some() {
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(existing_row);
        }
        if existing_row.status != "queued" && existing_row.status != "approved" {
            return Err(LiquidityPoolStoreError::Conflict(
                "withdrawal not in a payable state".to_string(),
            ));
        }

        // Burn shares.
        let burned = existing_row.shares_burned;
        if burned <= 0 {
            return Err(LiquidityPoolStoreError::Db(
                "shares_burned invalid".to_string(),
            ));
        }

        let updated_lp = tx
            .execute(
                r#"
                UPDATE runtime.liquidity_lp_accounts
                   SET shares_total = shares_total - $4,
                       updated_at = $5
                 WHERE pool_id = $1
                   AND partition_kind = $2
                   AND lp_id = $3
                   AND shares_total >= $4
                "#,
                &[
                    &existing_row.pool_id,
                    &existing_row.partition_kind,
                    &existing_row.lp_id,
                    &burned,
                    &paid_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        if updated_lp == 0 {
            return Err(LiquidityPoolStoreError::Conflict(
                "insufficient shares".to_string(),
            ));
        }

        let updated = tx
            .query_one(
                r#"
                UPDATE runtime.liquidity_withdrawals
                   SET status = 'paid',
                       wallet_receipt_sha256 = $3,
                       paid_at = $4
                 WHERE pool_id = $1
                   AND withdrawal_id = $2
                RETURNING withdrawal_id,
                          pool_id,
                          partition_kind,
                          lp_id,
                          shares_burned,
                          amount_sats_estimate,
                          rail_preference,
                          status,
                          request_fingerprint_sha256,
                          idempotency_key,
                          earliest_settlement_at,
                          payout_invoice_bolt11,
                          payout_invoice_hash,
                          payout_address,
                          wallet_receipt_sha256,
                          created_at,
                          paid_at
                "#,
                &[&pool_id, &withdrawal_id, &wallet_receipt_sha256, &paid_at],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        map_withdrawal_row(&updated).map_err(LiquidityPoolStoreError::Db)
    }

    async fn get_lp_account(
        &self,
        pool_id: &str,
        partition_kind: &str,
        lp_id: &str,
    ) -> Result<Option<LpAccountRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT pool_id, partition_kind, lp_id, shares_total, updated_at
                  FROM runtime.liquidity_lp_accounts
                 WHERE pool_id = $1
                   AND partition_kind = $2
                   AND lp_id = $3
                "#,
                &[&pool_id, &partition_kind, &lp_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(
            map_lp_account_row(&row).map_err(LiquidityPoolStoreError::Db)?,
        ))
    }

    async fn get_total_shares(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_one(
                r#"
                SELECT COALESCE(SUM(shares_total), 0)::BIGINT AS total_shares
                  FROM runtime.liquidity_lp_accounts
                 WHERE pool_id = $1
                   AND partition_kind = $2
                "#,
                &[&pool_id, &partition_kind],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(row.get::<_, i64>("total_shares"))
    }

    async fn get_confirmed_deposits_total_sats(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_one(
                r#"
                SELECT COALESCE(SUM(amount_sats), 0)::BIGINT AS total_sats
                  FROM runtime.liquidity_deposits
                 WHERE pool_id = $1
                   AND partition_kind = $2
                   AND status = 'confirmed'
                "#,
                &[&pool_id, &partition_kind],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(row.get::<_, i64>("total_sats"))
    }

    async fn get_pending_withdrawals_estimate_sats(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_one(
                r#"
                SELECT COALESCE(SUM(amount_sats_estimate), 0)::BIGINT AS total_sats
                  FROM runtime.liquidity_withdrawals
                 WHERE pool_id = $1
                   AND partition_kind = $2
                   AND status IN ('requested','queued','approved')
                "#,
                &[&pool_id, &partition_kind],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(row.get::<_, i64>("total_sats"))
    }

    async fn create_or_get_snapshot(
        &self,
        snapshot: PoolSnapshotRow,
    ) -> Result<PoolSnapshotRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT snapshot_id,
                       pool_id,
                       partition_kind,
                       as_of,
                       assets_json,
                       liabilities_json,
                       share_price_sats,
                       canonical_json_sha256,
                       signature_json,
                       created_at
                  FROM runtime.liquidity_pool_snapshots
                 WHERE snapshot_id = $1
                "#,
                &[&snapshot.snapshot_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let out = map_snapshot_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_pool_snapshots (
                  snapshot_id,
                  pool_id,
                  partition_kind,
                  as_of,
                  assets_json,
                  liabilities_json,
                  share_price_sats,
                  canonical_json_sha256,
                  signature_json,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
                RETURNING snapshot_id,
                          pool_id,
                          partition_kind,
                          as_of,
                          assets_json,
                          liabilities_json,
                          share_price_sats,
                          canonical_json_sha256,
                          signature_json,
                          created_at
                "#,
                &[
                    &snapshot.snapshot_id,
                    &snapshot.pool_id,
                    &snapshot.partition_kind,
                    &snapshot.as_of,
                    &snapshot.assets_json,
                    &snapshot.liabilities_json,
                    &snapshot.share_price_sats,
                    &snapshot.canonical_json_sha256,
                    &snapshot.signature_json,
                    &snapshot.created_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        map_snapshot_row(&inserted).map_err(LiquidityPoolStoreError::Db)
    }

    async fn get_latest_snapshot(
        &self,
        pool_id: &str,
        partition_kind: &str,
    ) -> Result<Option<PoolSnapshotRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT snapshot_id,
                       pool_id,
                       partition_kind,
                       as_of,
                       assets_json,
                       liabilities_json,
                       share_price_sats,
                       canonical_json_sha256,
                       signature_json,
                       created_at
                  FROM runtime.liquidity_pool_snapshots
                 WHERE pool_id = $1
                   AND partition_kind = $2
                 ORDER BY as_of DESC
                 LIMIT 1
                "#,
                &[&pool_id, &partition_kind],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(
            map_snapshot_row(&row).map_err(LiquidityPoolStoreError::Db)?,
        ))
    }

    async fn prune_snapshots_keep_latest(
        &self,
        pool_id: &str,
        partition_kind: &str,
        keep_latest: i64,
    ) -> Result<u64, LiquidityPoolStoreError> {
        let keep_latest = keep_latest.max(1);
        let client = self.db.client();
        let client = client.lock().await;
        let deleted = client
            .execute(
                r#"
                DELETE FROM runtime.liquidity_pool_snapshots
                 WHERE snapshot_id IN (
                   SELECT snapshot_id
                     FROM runtime.liquidity_pool_snapshots
                    WHERE pool_id = $1
                      AND partition_kind = $2
                    ORDER BY as_of DESC, created_at DESC, snapshot_id DESC
                    OFFSET $3
                 )
                "#,
                &[&pool_id, &partition_kind, &keep_latest],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(deleted)
    }

    async fn get_credit_reserved_commitments_sats(
        &self,
        pool_id: &str,
        now: DateTime<Utc>,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_one(
                r#"
                SELECT COALESCE(SUM(max_sats), 0)::BIGINT AS total_sats
                  FROM runtime.credit_envelopes
                 WHERE pool_id = $1
                   AND status = 'accepted'
                   AND exp > $2
                "#,
                &[&pool_id, &now],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(row.get::<_, i64>("total_sats"))
    }

    async fn put_receipt(
        &self,
        receipt: ReceiptInsertInput,
    ) -> Result<(), LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT canonical_json_sha256
                  FROM runtime.liquidity_pool_receipts
                 WHERE entity_kind = $1
                   AND entity_id = $2
                   AND schema = $3
                "#,
                &[&receipt.entity_kind, &receipt.entity_id, &receipt.schema],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let existing_digest: String = row.get("canonical_json_sha256");
            if existing_digest != receipt.canonical_json_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "receipt already exists for entity_kind/entity_id/schema with different digest"
                        .to_string(),
                ));
            }

            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(());
        }

        tx.execute(
            r#"
            INSERT INTO runtime.liquidity_pool_receipts (
              receipt_id,
              entity_kind,
              entity_id,
              schema,
              canonical_json_sha256,
              signature_json,
              receipt_json,
              created_at
            )
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            "#,
            &[
                &receipt.receipt_id,
                &receipt.entity_kind,
                &receipt.entity_id,
                &receipt.schema,
                &receipt.canonical_json_sha256,
                &receipt.signature_json,
                &receipt.receipt_json,
                &receipt.created_at,
            ],
        )
        .await
        .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(())
    }

    async fn upsert_signer_set(
        &self,
        input: SignerSetUpsertInput,
    ) -> Result<PoolSignerSetRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;

        let row = client
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_pool_signer_sets (
                  pool_id,
                  schema,
                  threshold,
                  signers_json,
                  policy_json,
                  canonical_json_sha256,
                  created_at,
                  updated_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$7)
                ON CONFLICT (pool_id)
                DO UPDATE SET
                  schema = EXCLUDED.schema,
                  threshold = EXCLUDED.threshold,
                  signers_json = EXCLUDED.signers_json,
                  policy_json = EXCLUDED.policy_json,
                  canonical_json_sha256 = EXCLUDED.canonical_json_sha256,
                  updated_at = EXCLUDED.updated_at
                RETURNING pool_id,
                          schema,
                          threshold,
                          signers_json,
                          policy_json,
                          canonical_json_sha256,
                          created_at,
                          updated_at
                "#,
                &[
                    &input.pool_id,
                    &input.schema,
                    &input.threshold,
                    &input.signers_json,
                    &input.policy_json,
                    &input.canonical_json_sha256,
                    &input.updated_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        map_signer_set_row(&row).map_err(LiquidityPoolStoreError::Db)
    }

    async fn get_signer_set(
        &self,
        pool_id: &str,
    ) -> Result<Option<PoolSignerSetRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;

        let row = client
            .query_opt(
                r#"
                SELECT pool_id,
                       schema,
                       threshold,
                       signers_json,
                       policy_json,
                       canonical_json_sha256,
                       created_at,
                       updated_at
                  FROM runtime.liquidity_pool_signer_sets
                 WHERE pool_id = $1
                "#,
                &[&pool_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(
            map_signer_set_row(&row).map_err(LiquidityPoolStoreError::Db)?,
        ))
    }

    async fn create_or_get_signing_request(
        &self,
        input: SigningRequestInsertInput,
    ) -> Result<PoolSigningRequestRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT request_id,
                       pool_id,
                       action_class,
                       idempotency_key,
                       payload_json,
                       payload_sha256,
                       required_signatures,
                       status,
                       execution_result_json,
                       executed_at,
                       created_at
                  FROM runtime.liquidity_pool_signing_requests
                 WHERE pool_id = $1
                   AND action_class = $2
                   AND idempotency_key = $3
                 FOR UPDATE
                "#,
                &[&input.pool_id, &input.action_class, &input.idempotency_key],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let payload_sha256: String = row.get("payload_sha256");
            if payload_sha256 != input.payload_sha256 {
                return Err(LiquidityPoolStoreError::Conflict(
                    "idempotency_key reused with different signing payload".to_string(),
                ));
            }

            let out = map_signing_request_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_pool_signing_requests (
                  request_id,
                  pool_id,
                  action_class,
                  idempotency_key,
                  payload_json,
                  payload_sha256,
                  required_signatures,
                  status,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                RETURNING request_id,
                          pool_id,
                          action_class,
                          idempotency_key,
                          payload_json,
                          payload_sha256,
                          required_signatures,
                          status,
                          execution_result_json,
                          executed_at,
                          created_at
                "#,
                &[
                    &input.request_id,
                    &input.pool_id,
                    &input.action_class,
                    &input.idempotency_key,
                    &input.payload_json,
                    &input.payload_sha256,
                    &input.required_signatures,
                    &input.status,
                    &input.created_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        map_signing_request_row(&inserted).map_err(LiquidityPoolStoreError::Db)
    }

    async fn get_signing_request(
        &self,
        pool_id: &str,
        request_id: &str,
    ) -> Result<Option<PoolSigningRequestRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;

        let row = client
            .query_opt(
                r#"
                SELECT request_id,
                       pool_id,
                       action_class,
                       idempotency_key,
                       payload_json,
                       payload_sha256,
                       required_signatures,
                       status,
                       execution_result_json,
                       executed_at,
                       created_at
                  FROM runtime.liquidity_pool_signing_requests
                 WHERE request_id = $1
                   AND pool_id = $2
                "#,
                &[&request_id, &pool_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(
            map_signing_request_row(&row).map_err(LiquidityPoolStoreError::Db)?,
        ))
    }

    async fn list_signing_requests(
        &self,
        pool_id: &str,
        status: Option<&str>,
        limit: i64,
    ) -> Result<Vec<PoolSigningRequestRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;

        let rows = match status {
            Some(status) => {
                client
                    .query(
                        r#"
                        SELECT request_id,
                               pool_id,
                               action_class,
                               idempotency_key,
                               payload_json,
                               payload_sha256,
                               required_signatures,
                               status,
                               execution_result_json,
                               executed_at,
                               created_at
                          FROM runtime.liquidity_pool_signing_requests
                         WHERE pool_id = $1
                           AND status = $2
                         ORDER BY created_at DESC
                         LIMIT $3
                        "#,
                        &[&pool_id, &status, &limit],
                    )
                    .await
            }
            None => {
                client
                    .query(
                        r#"
                        SELECT request_id,
                               pool_id,
                               action_class,
                               idempotency_key,
                               payload_json,
                               payload_sha256,
                               required_signatures,
                               status,
                               execution_result_json,
                               executed_at,
                               created_at
                          FROM runtime.liquidity_pool_signing_requests
                         WHERE pool_id = $1
                         ORDER BY created_at DESC
                         LIMIT $2
                        "#,
                        &[&pool_id, &limit],
                    )
                    .await
            }
        }
        .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_signing_request_row(&row).map_err(LiquidityPoolStoreError::Db)?);
        }
        Ok(out)
    }

    async fn create_or_get_signing_approval(
        &self,
        input: SigningApprovalInsertInput,
    ) -> Result<PoolSigningApprovalRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT approval_id,
                       request_id,
                       signer_pubkey,
                       signature_json,
                       created_at
                  FROM runtime.liquidity_pool_signing_approvals
                 WHERE request_id = $1
                   AND signer_pubkey = $2
                 FOR UPDATE
                "#,
                &[&input.request_id, &input.signer_pubkey],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let out = map_signing_approval_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            let signature_digest_existing = out.signature.signature_hex.clone();
            let signature_digest_new: crate::artifacts::ReceiptSignatureV1 =
                serde_json::from_value(input.signature_json.clone())
                    .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            if signature_digest_existing != signature_digest_new.signature_hex
                || out.signature.signed_sha256 != signature_digest_new.signed_sha256
                || out.signature.scheme != signature_digest_new.scheme
            {
                return Err(LiquidityPoolStoreError::Conflict(
                    "approval already exists with different signature".to_string(),
                ));
            }

            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_pool_signing_approvals (
                  approval_id,
                  request_id,
                  signer_pubkey,
                  signature_json,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5)
                RETURNING approval_id,
                          request_id,
                          signer_pubkey,
                          signature_json,
                          created_at
                "#,
                &[
                    &input.approval_id,
                    &input.request_id,
                    &input.signer_pubkey,
                    &input.signature_json,
                    &input.created_at,
                ],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        map_signing_approval_row(&inserted).map_err(LiquidityPoolStoreError::Db)
    }

    async fn list_signing_approvals(
        &self,
        request_id: &str,
    ) -> Result<Vec<PoolSigningApprovalRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let rows = client
            .query(
                r#"
                SELECT approval_id,
                       request_id,
                       signer_pubkey,
                       signature_json,
                       created_at
                  FROM runtime.liquidity_pool_signing_approvals
                 WHERE request_id = $1
                 ORDER BY created_at ASC
                "#,
                &[&request_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(map_signing_approval_row(&row).map_err(LiquidityPoolStoreError::Db)?);
        }
        Ok(out)
    }

    async fn mark_signing_request_executed(
        &self,
        request_id: &str,
        status: &str,
        execution_result_json: Value,
        executed_at: DateTime<Utc>,
    ) -> Result<PoolSigningRequestRow, LiquidityPoolStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        let row = tx
            .query_opt(
                r#"
                SELECT request_id,
                       pool_id,
                       action_class,
                       idempotency_key,
                       payload_json,
                       payload_sha256,
                       required_signatures,
                       status,
                       execution_result_json,
                       executed_at,
                       created_at
                  FROM runtime.liquidity_pool_signing_requests
                 WHERE request_id = $1
                 FOR UPDATE
                "#,
                &[&request_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Err(LiquidityPoolStoreError::NotFound(
                "signing_request".to_string(),
            ));
        };

        let current_status: String = row.get("status");
        if current_status == status {
            let out = map_signing_request_row(&row).map_err(LiquidityPoolStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let updated = tx
            .query_one(
                r#"
                UPDATE runtime.liquidity_pool_signing_requests
                   SET status = $2,
                       execution_result_json = $3,
                       executed_at = $4
                 WHERE request_id = $1
                RETURNING request_id,
                          pool_id,
                          action_class,
                          idempotency_key,
                          payload_json,
                          payload_sha256,
                          required_signatures,
                          status,
                          execution_result_json,
                          executed_at,
                          created_at
                "#,
                &[&request_id, &status, &execution_result_json, &executed_at],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        map_signing_request_row(&updated).map_err(LiquidityPoolStoreError::Db)
    }
}

fn map_pool_row(row: &tokio_postgres::Row) -> Result<PoolRow, String> {
    Ok(PoolRow {
        pool_id: row.get("pool_id"),
        pool_kind: row.get("pool_kind"),
        operator_id: row.get("operator_id"),
        status: row.get("status"),
        config: row.get("config"),
        created_at: row.get("created_at"),
    })
}

fn map_lp_account_row(row: &tokio_postgres::Row) -> Result<LpAccountRow, String> {
    Ok(LpAccountRow {
        pool_id: row.get("pool_id"),
        partition_kind: row.get("partition_kind"),
        lp_id: row.get("lp_id"),
        shares_total: row.get("shares_total"),
        updated_at: row.get("updated_at"),
    })
}

fn map_deposit_row(row: &tokio_postgres::Row) -> Result<DepositRow, String> {
    Ok(DepositRow {
        deposit_id: row.get("deposit_id"),
        pool_id: row.get("pool_id"),
        partition_kind: row.get("partition_kind"),
        lp_id: row.get("lp_id"),
        rail: row.get("rail"),
        amount_sats: row.get("amount_sats"),
        share_price_sats: row.get("share_price_sats"),
        shares_minted: row.get("shares_minted"),
        status: row.get("status"),
        idempotency_key: row.get("idempotency_key"),
        invoice_bolt11: row.get("invoice_bolt11"),
        invoice_hash: row.get("invoice_hash"),
        deposit_address: row.get("deposit_address"),
        created_at: row.get("created_at"),
        confirmed_at: row.get("confirmed_at"),
    })
}

fn map_withdrawal_row(row: &tokio_postgres::Row) -> Result<WithdrawalRow, String> {
    Ok(WithdrawalRow {
        withdrawal_id: row.get("withdrawal_id"),
        pool_id: row.get("pool_id"),
        partition_kind: row.get("partition_kind"),
        lp_id: row.get("lp_id"),
        shares_burned: row.get("shares_burned"),
        amount_sats_estimate: row.get("amount_sats_estimate"),
        rail_preference: row.get("rail_preference"),
        status: row.get("status"),
        idempotency_key: row.get("idempotency_key"),
        earliest_settlement_at: row.get("earliest_settlement_at"),
        payout_invoice_bolt11: row.get("payout_invoice_bolt11"),
        payout_invoice_hash: row.get("payout_invoice_hash"),
        payout_address: row.get("payout_address"),
        wallet_receipt_sha256: row.get("wallet_receipt_sha256"),
        created_at: row.get("created_at"),
        paid_at: row.get("paid_at"),
    })
}

fn map_snapshot_row(row: &tokio_postgres::Row) -> Result<PoolSnapshotRow, String> {
    Ok(PoolSnapshotRow {
        snapshot_id: row.get("snapshot_id"),
        pool_id: row.get("pool_id"),
        partition_kind: row.get("partition_kind"),
        as_of: row.get("as_of"),
        assets_json: row.get("assets_json"),
        liabilities_json: row.get("liabilities_json"),
        share_price_sats: row.get("share_price_sats"),
        canonical_json_sha256: row.get("canonical_json_sha256"),
        signature_json: row.get("signature_json"),
        created_at: row.get("created_at"),
    })
}

fn map_signer_set_row(row: &tokio_postgres::Row) -> Result<PoolSignerSetRow, String> {
    let signers_json: Value = row.get("signers_json");
    let signers: Vec<crate::liquidity_pool::types::PoolSignerV1> =
        serde_json::from_value(signers_json).map_err(|error| error.to_string())?;

    let policy_json: Value = row.get("policy_json");
    let policy: crate::liquidity_pool::types::PoolSignerPolicyV1 =
        serde_json::from_value(policy_json).map_err(|error| error.to_string())?;

    let threshold: i32 = row.get("threshold");
    Ok(PoolSignerSetRow {
        pool_id: row.get("pool_id"),
        schema: row.get("schema"),
        threshold: u32::try_from(threshold).map_err(|_| "threshold invalid".to_string())?,
        signers,
        policy,
        canonical_json_sha256: row.get("canonical_json_sha256"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn map_signing_request_row(row: &tokio_postgres::Row) -> Result<PoolSigningRequestRow, String> {
    let required_signatures: i32 = row.get("required_signatures");
    Ok(PoolSigningRequestRow {
        request_id: row.get("request_id"),
        pool_id: row.get("pool_id"),
        action_class: row.get("action_class"),
        idempotency_key: row.get("idempotency_key"),
        payload_json: row.get("payload_json"),
        payload_sha256: row.get("payload_sha256"),
        required_signatures: u32::try_from(required_signatures)
            .map_err(|_| "required_signatures invalid".to_string())?,
        status: row.get("status"),
        execution_result_json: row.get("execution_result_json"),
        created_at: row.get("created_at"),
        executed_at: row.get("executed_at"),
    })
}

fn map_signing_approval_row(row: &tokio_postgres::Row) -> Result<PoolSigningApprovalRow, String> {
    let signature_json: Value = row.get("signature_json");
    let signature: crate::artifacts::ReceiptSignatureV1 =
        serde_json::from_value(signature_json).map_err(|error| error.to_string())?;

    Ok(PoolSigningApprovalRow {
        approval_id: row.get("approval_id"),
        request_id: row.get("request_id"),
        signer_pubkey: row.get("signer_pubkey"),
        signature,
        created_at: row.get("created_at"),
    })
}
