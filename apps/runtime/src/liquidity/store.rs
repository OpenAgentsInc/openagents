use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::db::RuntimeDb;
use crate::liquidity::types::{LiquidityPaymentRow, LiquidityQuoteRow, LiquidityReceiptRow};

#[derive(Debug, thiserror::Error)]
pub enum LiquidityStoreError {
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("db error: {0}")]
    Db(String),
}

#[derive(Debug, Clone)]
pub struct PaymentFinalizeInput {
    pub quote_id: String,
    pub status: String,
    pub completed_at: DateTime<Utc>,
    pub latency_ms: u64,
    pub wallet_response_json: Option<Value>,
    pub wallet_receipt_sha256: Option<String>,
    pub preimage_sha256: Option<String>,
    pub paid_at_ms: Option<i64>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub receipt: LiquidityReceiptRow,
}

#[async_trait]
pub trait LiquidityStore: Send + Sync {
    async fn create_or_get_quote(
        &self,
        quote: LiquidityQuoteRow,
    ) -> Result<LiquidityQuoteRow, LiquidityStoreError>;

    async fn get_quote(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityQuoteRow>, LiquidityStoreError>;

    async fn create_or_get_payment_in_flight(
        &self,
        quote_id: &str,
        request_fingerprint_sha256: &str,
        run_id: Option<String>,
        trajectory_hash: Option<String>,
        wallet_request_id: &str,
        started_at: DateTime<Utc>,
    ) -> Result<(LiquidityPaymentRow, bool), LiquidityStoreError>;

    async fn finalize_payment(
        &self,
        input: PaymentFinalizeInput,
    ) -> Result<(LiquidityPaymentRow, LiquidityReceiptRow), LiquidityStoreError>;

    async fn get_payment(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityPaymentRow>, LiquidityStoreError>;
    async fn get_receipt(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityReceiptRow>, LiquidityStoreError>;
}

pub fn memory() -> Arc<dyn LiquidityStore> {
    Arc::new(MemoryLiquidityStore::default())
}

pub fn postgres(db: Arc<RuntimeDb>) -> Arc<dyn LiquidityStore> {
    Arc::new(PostgresLiquidityStore { db })
}

#[derive(Default)]
struct MemoryLiquidityStore {
    inner: Mutex<MemoryLiquidityInner>,
}

#[derive(Default)]
struct MemoryLiquidityInner {
    quotes_by_id: HashMap<String, LiquidityQuoteRow>,
    quote_id_by_idempotency: HashMap<String, String>,
    payments_by_quote_id: HashMap<String, LiquidityPaymentRow>,
    receipts_by_quote_id: HashMap<String, LiquidityReceiptRow>,
}

#[async_trait]
impl LiquidityStore for MemoryLiquidityStore {
    async fn create_or_get_quote(
        &self,
        quote: LiquidityQuoteRow,
    ) -> Result<LiquidityQuoteRow, LiquidityStoreError> {
        let mut inner = self.inner.lock().await;

        if let Some(existing_id) = inner.quote_id_by_idempotency.get(&quote.idempotency_key) {
            let existing = inner
                .quotes_by_id
                .get(existing_id)
                .cloned()
                .ok_or_else(|| LiquidityStoreError::Db("missing quote row".to_string()))?;
            if existing.request_fingerprint_sha256 != quote.request_fingerprint_sha256 {
                return Err(LiquidityStoreError::Conflict(
                    "idempotency_key reused with different quote parameters".to_string(),
                ));
            }
            return Ok(existing);
        }

        inner
            .quote_id_by_idempotency
            .insert(quote.idempotency_key.clone(), quote.quote_id.clone());
        inner
            .quotes_by_id
            .insert(quote.quote_id.clone(), quote.clone());
        Ok(quote)
    }

    async fn get_quote(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityQuoteRow>, LiquidityStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner.quotes_by_id.get(quote_id).cloned())
    }

    async fn create_or_get_payment_in_flight(
        &self,
        quote_id: &str,
        request_fingerprint_sha256: &str,
        run_id: Option<String>,
        trajectory_hash: Option<String>,
        wallet_request_id: &str,
        started_at: DateTime<Utc>,
    ) -> Result<(LiquidityPaymentRow, bool), LiquidityStoreError> {
        let mut inner = self.inner.lock().await;

        if let Some(existing) = inner.payments_by_quote_id.get(quote_id).cloned() {
            if existing.request_fingerprint_sha256 != request_fingerprint_sha256 {
                return Err(LiquidityStoreError::Conflict(
                    "quote_id reused with different pay context".to_string(),
                ));
            }
            return Ok((existing, false));
        }

        let row = LiquidityPaymentRow {
            quote_id: quote_id.to_string(),
            status: "in_flight".to_string(),
            request_fingerprint_sha256: request_fingerprint_sha256.to_string(),
            run_id,
            trajectory_hash,
            wallet_request_id: wallet_request_id.to_string(),
            started_at,
            completed_at: None,
            latency_ms: None,
            wallet_response_json: None,
            wallet_receipt_sha256: None,
            preimage_sha256: None,
            paid_at_ms: None,
            error_code: None,
            error_message: None,
            updated_at: started_at,
        };

        inner
            .payments_by_quote_id
            .insert(quote_id.to_string(), row.clone());
        Ok((row, true))
    }

    async fn finalize_payment(
        &self,
        input: PaymentFinalizeInput,
    ) -> Result<(LiquidityPaymentRow, LiquidityReceiptRow), LiquidityStoreError> {
        let mut inner = self.inner.lock().await;
        let existing = inner
            .payments_by_quote_id
            .get(&input.quote_id)
            .cloned()
            .ok_or_else(|| LiquidityStoreError::NotFound("payment missing".to_string()))?;

        let mut updated = existing.clone();
        updated.status = input.status;
        updated.completed_at = Some(input.completed_at);
        updated.latency_ms = Some(input.latency_ms);
        updated.wallet_response_json = input.wallet_response_json;
        updated.wallet_receipt_sha256 = input.wallet_receipt_sha256;
        updated.preimage_sha256 = input.preimage_sha256;
        updated.paid_at_ms = input.paid_at_ms;
        updated.error_code = input.error_code;
        updated.error_message = input.error_message;
        updated.updated_at = Utc::now();

        inner
            .payments_by_quote_id
            .insert(input.quote_id.clone(), updated.clone());

        if let Some(existing_receipt) = inner.receipts_by_quote_id.get(&input.quote_id) {
            if existing_receipt.canonical_json_sha256 != input.receipt.canonical_json_sha256 {
                return Err(LiquidityStoreError::Conflict(
                    "receipt canonical hash mismatch for quote_id".to_string(),
                ));
            }
            return Ok((updated, existing_receipt.clone()));
        }

        inner
            .receipts_by_quote_id
            .insert(input.quote_id.clone(), input.receipt.clone());
        Ok((updated, input.receipt))
    }

    async fn get_payment(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityPaymentRow>, LiquidityStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner.payments_by_quote_id.get(quote_id).cloned())
    }

    async fn get_receipt(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityReceiptRow>, LiquidityStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner.receipts_by_quote_id.get(quote_id).cloned())
    }
}

struct PostgresLiquidityStore {
    db: Arc<RuntimeDb>,
}

#[async_trait]
impl LiquidityStore for PostgresLiquidityStore {
    async fn create_or_get_quote(
        &self,
        quote: LiquidityQuoteRow,
    ) -> Result<LiquidityQuoteRow, LiquidityStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT quote_id,
                       idempotency_key,
                       request_fingerprint_sha256,
                       invoice,
                       invoice_hash,
                       host,
                       quoted_amount_msats,
                       max_amount_msats,
                       max_fee_msats,
                       urgency,
                       policy_context_json,
                       policy_context_sha256,
                       valid_until,
                       created_at
                  FROM runtime.liquidity_quotes
                 WHERE idempotency_key = $1
                "#,
                &[&quote.idempotency_key],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let existing_fingerprint: String = row.get("request_fingerprint_sha256");
            if existing_fingerprint != quote.request_fingerprint_sha256 {
                return Err(LiquidityStoreError::Conflict(
                    "idempotency_key reused with different quote parameters".to_string(),
                ));
            }

            let out = map_quote_row(&row).map_err(LiquidityStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_quotes (
                  quote_id,
                  idempotency_key,
                  request_fingerprint_sha256,
                  invoice,
                  invoice_hash,
                  host,
                  quoted_amount_msats,
                  max_amount_msats,
                  max_fee_msats,
                  urgency,
                  policy_context_json,
                  policy_context_sha256,
                  valid_until,
                  created_at
                )
                VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
                RETURNING quote_id,
                          idempotency_key,
                          request_fingerprint_sha256,
                          invoice,
                          invoice_hash,
                          host,
                          quoted_amount_msats,
                          max_amount_msats,
                          max_fee_msats,
                          urgency,
                          policy_context_json,
                          policy_context_sha256,
                          valid_until,
                          created_at
                "#,
                &[
                    &quote.quote_id,
                    &quote.idempotency_key,
                    &quote.request_fingerprint_sha256,
                    &quote.invoice,
                    &quote.invoice_hash,
                    &quote.host,
                    &i64::try_from(quote.quoted_amount_msats).unwrap_or(i64::MAX),
                    &i64::try_from(quote.max_amount_msats).unwrap_or(i64::MAX),
                    &i64::try_from(quote.max_fee_msats).unwrap_or(i64::MAX),
                    &quote.urgency,
                    &quote.policy_context_json,
                    &quote.policy_context_sha256,
                    &quote.valid_until,
                    &quote.created_at,
                ],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        map_quote_row(&inserted).map_err(LiquidityStoreError::Db)
    }

    async fn get_quote(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityQuoteRow>, LiquidityStoreError> {
        let client = self.db.client();
        let row = client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT quote_id,
                       idempotency_key,
                       request_fingerprint_sha256,
                       invoice,
                       invoice_hash,
                       host,
                       quoted_amount_msats,
                       max_amount_msats,
                       max_fee_msats,
                       urgency,
                       policy_context_json,
                       policy_context_sha256,
                       valid_until,
                       created_at
                  FROM runtime.liquidity_quotes
                 WHERE quote_id = $1
                "#,
                &[&quote_id],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(map_quote_row(&row).map_err(LiquidityStoreError::Db)?))
    }

    async fn create_or_get_payment_in_flight(
        &self,
        quote_id: &str,
        request_fingerprint_sha256: &str,
        run_id: Option<String>,
        trajectory_hash: Option<String>,
        wallet_request_id: &str,
        started_at: DateTime<Utc>,
    ) -> Result<(LiquidityPaymentRow, bool), LiquidityStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT quote_id,
                       status,
                       request_fingerprint_sha256,
                       run_id,
                       trajectory_hash,
                       wallet_request_id,
                       started_at,
                       completed_at,
                       latency_ms,
                       wallet_response_json,
                       wallet_receipt_sha256,
                       preimage_sha256,
                       paid_at_ms,
                       error_code,
                       error_message,
                       updated_at
                  FROM runtime.liquidity_payments
                 WHERE quote_id = $1
                "#,
                &[&quote_id],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;
        if let Some(row) = existing {
            let existing_fingerprint: String = row.get("request_fingerprint_sha256");
            if existing_fingerprint != request_fingerprint_sha256 {
                return Err(LiquidityStoreError::Conflict(
                    "quote_id reused with different pay context".to_string(),
                ));
            }
            let out = map_payment_row(&row).map_err(LiquidityStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;
            return Ok((out, false));
        }

        let inserted = tx
            .query_one(
                r#"
                INSERT INTO runtime.liquidity_payments (
                  quote_id,
                  status,
                  request_fingerprint_sha256,
                  run_id,
                  trajectory_hash,
                  wallet_request_id,
                  started_at,
                  updated_at
                )
                VALUES ($1,'in_flight',$2,$3,$4,$5,$6,$6)
                RETURNING quote_id,
                          status,
                          request_fingerprint_sha256,
                          run_id,
                          trajectory_hash,
                          wallet_request_id,
                          started_at,
                          completed_at,
                          latency_ms,
                          wallet_response_json,
                          wallet_receipt_sha256,
                          preimage_sha256,
                          paid_at_ms,
                          error_code,
                          error_message,
                          updated_at
                "#,
                &[
                    &quote_id,
                    &request_fingerprint_sha256,
                    &run_id,
                    &trajectory_hash,
                    &wallet_request_id,
                    &started_at,
                ],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        map_payment_row(&inserted)
            .map(|row| (row, true))
            .map_err(LiquidityStoreError::Db)
    }

    async fn finalize_payment(
        &self,
        input: PaymentFinalizeInput,
    ) -> Result<(LiquidityPaymentRow, LiquidityReceiptRow), LiquidityStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        let updated = tx
            .query_one(
                r#"
                UPDATE runtime.liquidity_payments
                   SET status = $2,
                       completed_at = $3,
                       latency_ms = $4,
                       wallet_response_json = $5,
                       wallet_receipt_sha256 = $6,
                       preimage_sha256 = $7,
                       paid_at_ms = $8,
                       error_code = $9,
                       error_message = $10,
                       updated_at = NOW()
                 WHERE quote_id = $1
                 RETURNING quote_id,
                           status,
                           request_fingerprint_sha256,
                           run_id,
                           trajectory_hash,
                           wallet_request_id,
                           started_at,
                           completed_at,
                           latency_ms,
                           wallet_response_json,
                           wallet_receipt_sha256,
                           preimage_sha256,
                           paid_at_ms,
                           error_code,
                           error_message,
                           updated_at
                "#,
                &[
                    &input.quote_id,
                    &input.status,
                    &input.completed_at,
                    &i64::try_from(input.latency_ms).unwrap_or(i64::MAX),
                    &input.wallet_response_json,
                    &input.wallet_receipt_sha256,
                    &input.preimage_sha256,
                    &input.paid_at_ms,
                    &input.error_code,
                    &input.error_message,
                ],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        let existing_receipt = tx
            .query_opt(
                r#"
                SELECT quote_id,
                       schema,
                       canonical_json_sha256,
                       signature_json,
                       receipt_json,
                       created_at
                  FROM runtime.liquidity_receipts
                 WHERE quote_id = $1
                "#,
                &[&input.quote_id],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        let receipt_row = if let Some(row) = existing_receipt {
            let existing_hash: String = row.get("canonical_json_sha256");
            if existing_hash != input.receipt.canonical_json_sha256 {
                return Err(LiquidityStoreError::Conflict(
                    "receipt canonical hash mismatch for quote_id".to_string(),
                ));
            }
            map_receipt_row(&row).map_err(LiquidityStoreError::Db)?
        } else {
            let inserted = tx
                .query_one(
                    r#"
                    INSERT INTO runtime.liquidity_receipts (
                      quote_id,
                      schema,
                      canonical_json_sha256,
                      signature_json,
                      receipt_json,
                      created_at
                    )
                    VALUES ($1,$2,$3,$4,$5,$6)
                    RETURNING quote_id,
                              schema,
                              canonical_json_sha256,
                              signature_json,
                              receipt_json,
                              created_at
                    "#,
                    &[
                        &input.quote_id,
                        &input.receipt.schema,
                        &input.receipt.canonical_json_sha256,
                        &input.receipt.signature_json,
                        &input.receipt.receipt_json,
                        &input.receipt.created_at,
                    ],
                )
                .await
                .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;
            map_receipt_row(&inserted).map_err(LiquidityStoreError::Db)?
        };

        tx.commit()
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;

        Ok((
            map_payment_row(&updated).map_err(LiquidityStoreError::Db)?,
            receipt_row,
        ))
    }

    async fn get_payment(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityPaymentRow>, LiquidityStoreError> {
        let client = self.db.client();
        let row = client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT quote_id,
                       status,
                       request_fingerprint_sha256,
                       run_id,
                       trajectory_hash,
                       wallet_request_id,
                       started_at,
                       completed_at,
                       latency_ms,
                       wallet_response_json,
                       wallet_receipt_sha256,
                       preimage_sha256,
                       paid_at_ms,
                       error_code,
                       error_message,
                       updated_at
                  FROM runtime.liquidity_payments
                 WHERE quote_id = $1
                "#,
                &[&quote_id],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(
            map_payment_row(&row).map_err(LiquidityStoreError::Db)?,
        ))
    }

    async fn get_receipt(
        &self,
        quote_id: &str,
    ) -> Result<Option<LiquidityReceiptRow>, LiquidityStoreError> {
        let client = self.db.client();
        let row = client
            .lock()
            .await
            .query_opt(
                r#"
                SELECT quote_id,
                       schema,
                       canonical_json_sha256,
                       signature_json,
                       receipt_json,
                       created_at
                  FROM runtime.liquidity_receipts
                 WHERE quote_id = $1
                "#,
                &[&quote_id],
            )
            .await
            .map_err(|error| LiquidityStoreError::Db(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(
            map_receipt_row(&row).map_err(LiquidityStoreError::Db)?,
        ))
    }
}

fn map_quote_row(row: &tokio_postgres::Row) -> Result<LiquidityQuoteRow, String> {
    Ok(LiquidityQuoteRow {
        quote_id: row.get("quote_id"),
        idempotency_key: row.get("idempotency_key"),
        request_fingerprint_sha256: row.get("request_fingerprint_sha256"),
        invoice: row.get("invoice"),
        invoice_hash: row.get("invoice_hash"),
        host: row.get("host"),
        quoted_amount_msats: u64_from_i64(row.get("quoted_amount_msats"))?,
        max_amount_msats: u64_from_i64(row.get("max_amount_msats"))?,
        max_fee_msats: u64_from_i64(row.get("max_fee_msats"))?,
        urgency: row.get("urgency"),
        policy_context_json: row.get("policy_context_json"),
        policy_context_sha256: row.get("policy_context_sha256"),
        valid_until: row.get("valid_until"),
        created_at: row.get("created_at"),
    })
}

fn map_payment_row(row: &tokio_postgres::Row) -> Result<LiquidityPaymentRow, String> {
    Ok(LiquidityPaymentRow {
        quote_id: row.get("quote_id"),
        status: row.get("status"),
        request_fingerprint_sha256: row.get("request_fingerprint_sha256"),
        run_id: row.get("run_id"),
        trajectory_hash: row.get("trajectory_hash"),
        wallet_request_id: row.get("wallet_request_id"),
        started_at: row.get("started_at"),
        completed_at: row.get("completed_at"),
        latency_ms: row
            .try_get::<_, Option<i64>>("latency_ms")
            .map_err(|error| error.to_string())?
            .map(|value| u64_from_i64(value).unwrap_or(0)),
        wallet_response_json: row.get("wallet_response_json"),
        wallet_receipt_sha256: row.get("wallet_receipt_sha256"),
        preimage_sha256: row.get("preimage_sha256"),
        paid_at_ms: row.get("paid_at_ms"),
        error_code: row.get("error_code"),
        error_message: row.get("error_message"),
        updated_at: row.get("updated_at"),
    })
}

fn map_receipt_row(row: &tokio_postgres::Row) -> Result<LiquidityReceiptRow, String> {
    Ok(LiquidityReceiptRow {
        quote_id: row.get("quote_id"),
        schema: row.get("schema"),
        canonical_json_sha256: row.get("canonical_json_sha256"),
        signature_json: row.get("signature_json"),
        receipt_json: row.get("receipt_json"),
        created_at: row.get("created_at"),
    })
}

fn u64_from_i64(value: i64) -> Result<u64, String> {
    u64::try_from(value).map_err(|_| format!("negative bigint value {value}"))
}
