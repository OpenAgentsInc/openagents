use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::db::RuntimeDb;
use crate::liquidity_pool::types::{DepositRow, LpAccountRow, PoolRow, PoolSnapshotRow, WithdrawalRow};

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
    pub lp_id: String,
    pub shares_burned: i64,
    pub amount_sats_estimate: i64,
    pub rail_preference: String,
    pub status: String,
    pub request_fingerprint_sha256: String,
    pub idempotency_key: String,
    pub earliest_settlement_at: DateTime<Utc>,
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

    async fn get_lp_account(
        &self,
        pool_id: &str,
        lp_id: &str,
    ) -> Result<Option<LpAccountRow>, LiquidityPoolStoreError>;

    async fn get_total_shares(&self, pool_id: &str) -> Result<i64, LiquidityPoolStoreError>;

    async fn get_pending_withdrawals_estimate_sats(
        &self,
        pool_id: &str,
    ) -> Result<i64, LiquidityPoolStoreError>;

    async fn create_or_get_snapshot(
        &self,
        snapshot: PoolSnapshotRow,
    ) -> Result<PoolSnapshotRow, LiquidityPoolStoreError>;

    async fn get_latest_snapshot(
        &self,
        pool_id: &str,
    ) -> Result<Option<PoolSnapshotRow>, LiquidityPoolStoreError>;

    async fn put_receipt(
        &self,
        receipt: ReceiptInsertInput,
    ) -> Result<(), LiquidityPoolStoreError>;
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
    lp_accounts: HashMap<(String, String), LpAccountRow>,
    deposits: HashMap<String, (DepositRow, String)>,
    deposit_by_idempotency: HashMap<(String, String, String), String>,
    withdrawals: HashMap<String, (WithdrawalRow, String)>,
    withdrawal_by_idempotency: HashMap<(String, String, String), String>,
    latest_snapshot_by_pool: HashMap<String, PoolSnapshotRow>,
    receipts_by_unique: HashMap<(String, String, String), String>,
    receipts_by_id: HashMap<String, ReceiptInsertInput>,
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
            input.lp_id.clone(),
            input.idempotency_key.clone(),
        );
        if let Some(existing_id) = inner.deposit_by_idempotency.get(&key) {
            let (row, fingerprint) = inner
                .deposits
                .get(existing_id)
                .cloned()
                .ok_or_else(|| LiquidityPoolStoreError::Db("missing deposit row".to_string()))?;
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
        inner
            .deposits
            .insert(input.deposit_id.clone(), (row.clone(), input.request_fingerprint_sha256));
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

        let lp_key = (updated.pool_id.clone(), updated.lp_id.clone());
        let account = inner.lp_accounts.entry(lp_key.clone()).or_insert_with(|| LpAccountRow {
            pool_id: updated.pool_id.clone(),
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
            input.lp_id.clone(),
            input.idempotency_key.clone(),
        );
        if let Some(existing_id) = inner.withdrawal_by_idempotency.get(&key) {
            let (row, fingerprint) = inner
                .withdrawals
                .get(existing_id)
                .cloned()
                .ok_or_else(|| LiquidityPoolStoreError::Db("missing withdrawal row".to_string()))?;
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
            lp_id: input.lp_id.clone(),
            shares_burned: input.shares_burned,
            amount_sats_estimate: input.amount_sats_estimate,
            rail_preference: input.rail_preference.clone(),
            status: input.status.clone(),
            idempotency_key: input.idempotency_key.clone(),
            earliest_settlement_at: input.earliest_settlement_at,
            payout_invoice_hash: None,
            payout_address: None,
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

    async fn get_lp_account(
        &self,
        pool_id: &str,
        lp_id: &str,
    ) -> Result<Option<LpAccountRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .lp_accounts
            .get(&(pool_id.to_string(), lp_id.to_string()))
            .cloned())
    }

    async fn get_total_shares(&self, pool_id: &str) -> Result<i64, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut total = 0_i64;
        for ((pool, _), row) in inner.lp_accounts.iter() {
            if pool == pool_id {
                total = total
                    .checked_add(row.shares_total)
                    .ok_or_else(|| LiquidityPoolStoreError::Db("share accounting overflow".to_string()))?;
            }
        }
        Ok(total)
    }

    async fn get_pending_withdrawals_estimate_sats(
        &self,
        pool_id: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        let mut total = 0_i64;
        for (row, _) in inner.withdrawals.values() {
            if row.pool_id == pool_id
                && (row.status == "requested"
                    || row.status == "queued"
                    || row.status == "approved")
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
            .latest_snapshot_by_pool
            .get(&snapshot.pool_id)
            .filter(|existing| existing.snapshot_id == snapshot.snapshot_id)
            .cloned()
        {
            return Ok(existing);
        }
        inner
            .latest_snapshot_by_pool
            .insert(snapshot.pool_id.clone(), snapshot.clone());
        Ok(snapshot)
    }

    async fn get_latest_snapshot(
        &self,
        pool_id: &str,
    ) -> Result<Option<PoolSnapshotRow>, LiquidityPoolStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner.latest_snapshot_by_pool.get(pool_id).cloned())
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
        Ok(Some(map_pool_row(&row).map_err(LiquidityPoolStoreError::Db)?))
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
                   AND lp_id = $2
                   AND idempotency_key = $3
                "#,
                &[&input.pool_id, &input.lp_id, &input.idempotency_key],
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
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                RETURNING deposit_id,
                          pool_id,
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

        let updated = tx
            .query_one(
                r#"
                UPDATE runtime.liquidity_deposits
                   SET status = 'confirmed',
                       confirmed_at = $2
                 WHERE deposit_id = $1
                 RETURNING deposit_id,
                          pool_id,
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
            INSERT INTO runtime.liquidity_lp_accounts (pool_id, lp_id, shares_total, updated_at)
            VALUES ($1,$2,0,$3)
            ON CONFLICT (pool_id, lp_id) DO NOTHING
            "#,
            &[&pool_id, &lp_id, &confirmed_at],
        )
        .await
        .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;

        tx.execute(
            r#"
            UPDATE runtime.liquidity_lp_accounts
               SET shares_total = shares_total + $3,
                   updated_at = $4
             WHERE pool_id = $1
               AND lp_id = $2
            "#,
            &[&pool_id, &lp_id, &shares_minted, &confirmed_at],
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
                       lp_id,
                       shares_burned,
                       amount_sats_estimate,
                       rail_preference,
                       status,
                       request_fingerprint_sha256,
                       idempotency_key,
                       earliest_settlement_at,
                       payout_invoice_hash,
                       payout_address,
                       wallet_receipt_sha256,
                       created_at,
                       paid_at
                  FROM runtime.liquidity_withdrawals
                 WHERE pool_id = $1
                   AND lp_id = $2
                   AND idempotency_key = $3
                "#,
                &[&input.pool_id, &input.lp_id, &input.idempotency_key],
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
                  lp_id,
                  shares_burned,
                  amount_sats_estimate,
                  rail_preference,
                  status,
                  request_fingerprint_sha256,
                  idempotency_key,
                  earliest_settlement_at,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
                RETURNING withdrawal_id,
                          pool_id,
                          lp_id,
                          shares_burned,
                          amount_sats_estimate,
                          rail_preference,
                          status,
                          request_fingerprint_sha256,
                          idempotency_key,
                          earliest_settlement_at,
                          payout_invoice_hash,
                          payout_address,
                          wallet_receipt_sha256,
                          created_at,
                          paid_at
                "#,
                &[
                    &input.withdrawal_id,
                    &input.pool_id,
                    &input.lp_id,
                    &input.shares_burned,
                    &input.amount_sats_estimate,
                    &input.rail_preference,
                    &input.status,
                    &input.request_fingerprint_sha256,
                    &input.idempotency_key,
                    &input.earliest_settlement_at,
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

    async fn get_lp_account(
        &self,
        pool_id: &str,
        lp_id: &str,
    ) -> Result<Option<LpAccountRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT pool_id, lp_id, shares_total, updated_at
                  FROM runtime.liquidity_lp_accounts
                 WHERE pool_id = $1
                   AND lp_id = $2
                "#,
                &[&pool_id, &lp_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(map_lp_account_row(&row).map_err(LiquidityPoolStoreError::Db)?))
    }

    async fn get_total_shares(&self, pool_id: &str) -> Result<i64, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_one(
                r#"
                SELECT COALESCE(SUM(shares_total), 0)::BIGINT AS total_shares
                  FROM runtime.liquidity_lp_accounts
                 WHERE pool_id = $1
                "#,
                &[&pool_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        Ok(row.get::<_, i64>("total_shares"))
    }

    async fn get_pending_withdrawals_estimate_sats(
        &self,
        pool_id: &str,
    ) -> Result<i64, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_one(
                r#"
                SELECT COALESCE(SUM(amount_sats_estimate), 0)::BIGINT AS total_sats
                  FROM runtime.liquidity_withdrawals
                 WHERE pool_id = $1
                   AND status IN ('requested','queued','approved')
                "#,
                &[&pool_id],
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
                  as_of,
                  assets_json,
                  liabilities_json,
                  share_price_sats,
                  canonical_json_sha256,
                  signature_json,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
                RETURNING snapshot_id,
                          pool_id,
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
    ) -> Result<Option<PoolSnapshotRow>, LiquidityPoolStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT snapshot_id,
                       pool_id,
                       as_of,
                       assets_json,
                       liabilities_json,
                       share_price_sats,
                       canonical_json_sha256,
                       signature_json,
                       created_at
                  FROM runtime.liquidity_pool_snapshots
                 WHERE pool_id = $1
                 ORDER BY as_of DESC
                 LIMIT 1
                "#,
                &[&pool_id],
            )
            .await
            .map_err(|error| LiquidityPoolStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(map_snapshot_row(&row).map_err(LiquidityPoolStoreError::Db)?))
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
        lp_id: row.get("lp_id"),
        shares_total: row.get("shares_total"),
        updated_at: row.get("updated_at"),
    })
}

fn map_deposit_row(row: &tokio_postgres::Row) -> Result<DepositRow, String> {
    Ok(DepositRow {
        deposit_id: row.get("deposit_id"),
        pool_id: row.get("pool_id"),
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
        lp_id: row.get("lp_id"),
        shares_burned: row.get("shares_burned"),
        amount_sats_estimate: row.get("amount_sats_estimate"),
        rail_preference: row.get("rail_preference"),
        status: row.get("status"),
        idempotency_key: row.get("idempotency_key"),
        earliest_settlement_at: row.get("earliest_settlement_at"),
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
        as_of: row.get("as_of"),
        assets_json: row.get("assets_json"),
        liabilities_json: row.get("liabilities_json"),
        share_price_sats: row.get("share_price_sats"),
        canonical_json_sha256: row.get("canonical_json_sha256"),
        signature_json: row.get("signature_json"),
        created_at: row.get("created_at"),
    })
}
