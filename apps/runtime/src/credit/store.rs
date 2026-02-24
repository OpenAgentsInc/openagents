use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::db::RuntimeDb;
use crate::credit::types::{CreditEnvelopeRow, CreditOfferRow, CreditSettlementRow};

#[derive(Debug, thiserror::Error)]
pub enum CreditStoreError {
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("db error: {0}")]
    Db(String),
}

#[derive(Debug, Clone)]
pub struct CreditReceiptInsertInput {
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
pub trait CreditStore: Send + Sync {
    async fn create_or_get_offer(
        &self,
        offer: CreditOfferRow,
        request_fingerprint_sha256: String,
    ) -> Result<CreditOfferRow, CreditStoreError>;

    async fn get_offer(&self, offer_id: &str) -> Result<Option<CreditOfferRow>, CreditStoreError>;

    async fn update_offer_status(
        &self,
        offer_id: &str,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<(), CreditStoreError>;

    async fn create_or_get_envelope(
        &self,
        envelope: CreditEnvelopeRow,
        request_fingerprint_sha256: String,
    ) -> Result<CreditEnvelopeRow, CreditStoreError>;

    async fn get_envelope(
        &self,
        envelope_id: &str,
    ) -> Result<Option<CreditEnvelopeRow>, CreditStoreError>;

    async fn update_envelope_status(
        &self,
        envelope_id: &str,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<(), CreditStoreError>;

    async fn create_or_get_settlement(
        &self,
        settlement: CreditSettlementRow,
        request_fingerprint_sha256: String,
    ) -> Result<(CreditSettlementRow, bool), CreditStoreError>;

    async fn get_settlement_by_envelope(
        &self,
        envelope_id: &str,
    ) -> Result<Option<CreditSettlementRow>, CreditStoreError>;

    async fn put_receipt(&self, receipt: CreditReceiptInsertInput)
        -> Result<(), CreditStoreError>;

    async fn get_receipt_by_unique(
        &self,
        entity_kind: &str,
        entity_id: &str,
        schema: &str,
    ) -> Result<Option<CreditReceiptInsertInput>, CreditStoreError>;
}

pub fn memory() -> Arc<dyn CreditStore> {
    Arc::new(MemoryCreditStore::default())
}

pub fn postgres(db: Arc<RuntimeDb>) -> Arc<dyn CreditStore> {
    Arc::new(PostgresCreditStore { db })
}

#[derive(Default)]
struct MemoryCreditStore {
    inner: Mutex<MemoryCreditStoreInner>,
}

#[derive(Default)]
struct MemoryCreditStoreInner {
    offers: HashMap<String, (CreditOfferRow, String)>,
    envelopes: HashMap<String, (CreditEnvelopeRow, String)>,
    settlements_by_envelope: HashMap<String, (CreditSettlementRow, String)>,
    receipts_by_unique: HashMap<(String, String, String), String>,
    receipts_by_id: HashMap<String, CreditReceiptInsertInput>,
}

#[async_trait]
impl CreditStore for MemoryCreditStore {
    async fn create_or_get_offer(
        &self,
        offer: CreditOfferRow,
        request_fingerprint_sha256: String,
    ) -> Result<CreditOfferRow, CreditStoreError> {
        let mut inner = self.inner.lock().await;
        if let Some((existing, fingerprint)) = inner.offers.get(&offer.offer_id).cloned() {
            if fingerprint != request_fingerprint_sha256 {
                return Err(CreditStoreError::Conflict(
                    "offer_id reused with different offer parameters".to_string(),
                ));
            }
            return Ok(existing);
        }
        inner
            .offers
            .insert(offer.offer_id.clone(), (offer.clone(), request_fingerprint_sha256));
        Ok(offer)
    }

    async fn get_offer(&self, offer_id: &str) -> Result<Option<CreditOfferRow>, CreditStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner.offers.get(offer_id).map(|(row, _)| row.clone()))
    }

    async fn update_offer_status(
        &self,
        offer_id: &str,
        status: &str,
        _now: DateTime<Utc>,
    ) -> Result<(), CreditStoreError> {
        let mut inner = self.inner.lock().await;
        let Some((mut row, fingerprint)) = inner.offers.get(offer_id).cloned() else {
            return Err(CreditStoreError::NotFound("offer".to_string()));
        };
        row.status = status.to_string();
        inner.offers.insert(offer_id.to_string(), (row, fingerprint));
        Ok(())
    }

    async fn create_or_get_envelope(
        &self,
        envelope: CreditEnvelopeRow,
        request_fingerprint_sha256: String,
    ) -> Result<CreditEnvelopeRow, CreditStoreError> {
        let mut inner = self.inner.lock().await;
        if let Some((existing, fingerprint)) = inner.envelopes.get(&envelope.envelope_id).cloned()
        {
            if fingerprint != request_fingerprint_sha256 {
                return Err(CreditStoreError::Conflict(
                    "envelope_id reused with different envelope parameters".to_string(),
                ));
            }
            return Ok(existing);
        }
        inner.envelopes.insert(
            envelope.envelope_id.clone(),
            (envelope.clone(), request_fingerprint_sha256),
        );
        Ok(envelope)
    }

    async fn get_envelope(
        &self,
        envelope_id: &str,
    ) -> Result<Option<CreditEnvelopeRow>, CreditStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .envelopes
            .get(envelope_id)
            .map(|(row, _)| row.clone()))
    }

    async fn update_envelope_status(
        &self,
        envelope_id: &str,
        status: &str,
        _now: DateTime<Utc>,
    ) -> Result<(), CreditStoreError> {
        let mut inner = self.inner.lock().await;
        let Some((mut row, fingerprint)) = inner.envelopes.get(envelope_id).cloned() else {
            return Err(CreditStoreError::NotFound("envelope".to_string()));
        };
        row.status = status.to_string();
        inner
            .envelopes
            .insert(envelope_id.to_string(), (row, fingerprint));
        Ok(())
    }

    async fn create_or_get_settlement(
        &self,
        settlement: CreditSettlementRow,
        request_fingerprint_sha256: String,
    ) -> Result<(CreditSettlementRow, bool), CreditStoreError> {
        let mut inner = self.inner.lock().await;
        if let Some((existing, fingerprint)) = inner
            .settlements_by_envelope
            .get(&settlement.envelope_id)
            .cloned()
        {
            if fingerprint != request_fingerprint_sha256 {
                return Err(CreditStoreError::Conflict(
                    "settlement already exists for envelope with different parameters".to_string(),
                ));
            }
            return Ok((existing, false));
        }
        inner.settlements_by_envelope.insert(
            settlement.envelope_id.clone(),
            (settlement.clone(), request_fingerprint_sha256),
        );
        Ok((settlement, true))
    }

    async fn get_settlement_by_envelope(
        &self,
        envelope_id: &str,
    ) -> Result<Option<CreditSettlementRow>, CreditStoreError> {
        let inner = self.inner.lock().await;
        Ok(inner
            .settlements_by_envelope
            .get(envelope_id)
            .map(|(row, _)| row.clone()))
    }

    async fn put_receipt(
        &self,
        receipt: CreditReceiptInsertInput,
    ) -> Result<(), CreditStoreError> {
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
                .ok_or_else(|| CreditStoreError::Db("missing receipt row".to_string()))?;
            if existing.canonical_json_sha256 != receipt.canonical_json_sha256 {
                return Err(CreditStoreError::Conflict(
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

    async fn get_receipt_by_unique(
        &self,
        entity_kind: &str,
        entity_id: &str,
        schema: &str,
    ) -> Result<Option<CreditReceiptInsertInput>, CreditStoreError> {
        let inner = self.inner.lock().await;
        let key = (entity_kind.to_string(), entity_id.to_string(), schema.to_string());
        let Some(receipt_id) = inner.receipts_by_unique.get(&key) else {
            return Ok(None);
        };
        Ok(inner.receipts_by_id.get(receipt_id).cloned())
    }
}

struct PostgresCreditStore {
    db: Arc<RuntimeDb>,
}

#[async_trait]
impl CreditStore for PostgresCreditStore {
    async fn create_or_get_offer(
        &self,
        offer: CreditOfferRow,
        request_fingerprint_sha256: String,
    ) -> Result<CreditOfferRow, CreditStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT offer_id, agent_id, pool_id, scope_type, scope_id, max_sats, fee_bps,
                       requires_verifier, exp, status, issued_at, request_fingerprint_sha256
                  FROM runtime.credit_offers
                 WHERE offer_id = $1
                "#,
                &[&offer.offer_id],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let fingerprint: String = row.get("request_fingerprint_sha256");
            if fingerprint != request_fingerprint_sha256 {
                return Err(CreditStoreError::Conflict(
                    "offer_id reused with different offer parameters".to_string(),
                ));
            }
            let out = map_offer_row(&row).map_err(CreditStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| CreditStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        tx.execute(
            r#"
            INSERT INTO runtime.credit_offers (
                offer_id, agent_id, pool_id, scope_type, scope_id, max_sats, fee_bps,
                requires_verifier, exp, status, issued_at, request_fingerprint_sha256
            ) VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12
            )
            "#,
            &[
                &offer.offer_id,
                &offer.agent_id,
                &offer.pool_id,
                &offer.scope_type,
                &offer.scope_id,
                &offer.max_sats,
                &offer.fee_bps,
                &offer.requires_verifier,
                &offer.exp,
                &offer.status,
                &offer.issued_at,
                &request_fingerprint_sha256,
            ],
        )
        .await
        .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        Ok(offer)
    }

    async fn get_offer(&self, offer_id: &str) -> Result<Option<CreditOfferRow>, CreditStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT offer_id, agent_id, pool_id, scope_type, scope_id, max_sats, fee_bps,
                       requires_verifier, exp, status, issued_at
                  FROM runtime.credit_offers
                 WHERE offer_id = $1
                "#,
                &[&offer_id],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        Ok(row
            .as_ref()
            .map(map_offer_row)
            .transpose()
            .map_err(CreditStoreError::Db)?)
    }

    async fn update_offer_status(
        &self,
        offer_id: &str,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<(), CreditStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let updated = client
            .execute(
                r#"
                UPDATE runtime.credit_offers
                   SET status = $2, issued_at = issued_at, exp = exp
                 WHERE offer_id = $1
                "#,
                &[&offer_id, &status],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        if updated == 0 {
            return Err(CreditStoreError::NotFound("offer".to_string()));
        }

        // Touch a consistent "updated_at" via an explicit no-op insert into receipts table isn't
        // necessary; keep minimal.
        let _ = now;
        Ok(())
    }

    async fn create_or_get_envelope(
        &self,
        envelope: CreditEnvelopeRow,
        request_fingerprint_sha256: String,
    ) -> Result<CreditEnvelopeRow, CreditStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT envelope_id, offer_id, agent_id, pool_id, provider_id, scope_type, scope_id,
                       max_sats, fee_bps, exp, status, issued_at, request_fingerprint_sha256
                  FROM runtime.credit_envelopes
                 WHERE envelope_id = $1
                "#,
                &[&envelope.envelope_id],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        if let Some(row) = existing {
            let fingerprint: String = row.get("request_fingerprint_sha256");
            if fingerprint != request_fingerprint_sha256 {
                return Err(CreditStoreError::Conflict(
                    "envelope_id reused with different envelope parameters".to_string(),
                ));
            }
            let out = map_envelope_row(&row).map_err(CreditStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| CreditStoreError::Db(error.to_string()))?;
            return Ok(out);
        }

        tx.execute(
            r#"
            INSERT INTO runtime.credit_envelopes (
                envelope_id, offer_id, agent_id, pool_id, provider_id, scope_type, scope_id,
                max_sats, fee_bps, exp, status, issued_at, request_fingerprint_sha256
            ) VALUES (
                $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13
            )
            "#,
            &[
                &envelope.envelope_id,
                &envelope.offer_id,
                &envelope.agent_id,
                &envelope.pool_id,
                &envelope.provider_id,
                &envelope.scope_type,
                &envelope.scope_id,
                &envelope.max_sats,
                &envelope.fee_bps,
                &envelope.exp,
                &envelope.status,
                &envelope.issued_at,
                &request_fingerprint_sha256,
            ],
        )
        .await
        .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        Ok(envelope)
    }

    async fn get_envelope(
        &self,
        envelope_id: &str,
    ) -> Result<Option<CreditEnvelopeRow>, CreditStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT envelope_id, offer_id, agent_id, pool_id, provider_id, scope_type, scope_id,
                       max_sats, fee_bps, exp, status, issued_at
                  FROM runtime.credit_envelopes
                 WHERE envelope_id = $1
                "#,
                &[&envelope_id],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        Ok(row
            .as_ref()
            .map(map_envelope_row)
            .transpose()
            .map_err(CreditStoreError::Db)?)
    }

    async fn update_envelope_status(
        &self,
        envelope_id: &str,
        status: &str,
        now: DateTime<Utc>,
    ) -> Result<(), CreditStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let updated = client
            .execute(
                r#"
                UPDATE runtime.credit_envelopes
                   SET status = $2
                 WHERE envelope_id = $1
                "#,
                &[&envelope_id, &status],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        if updated == 0 {
            return Err(CreditStoreError::NotFound("envelope".to_string()));
        }
        let _ = now;
        Ok(())
    }

    async fn create_or_get_settlement(
        &self,
        settlement: CreditSettlementRow,
        request_fingerprint_sha256: String,
    ) -> Result<(CreditSettlementRow, bool), CreditStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT settlement_id, envelope_id, outcome, spent_sats, fee_sats,
                       verification_receipt_sha256, liquidity_receipt_sha256, created_at,
                       request_fingerprint_sha256
                  FROM runtime.credit_settlements
                 WHERE envelope_id = $1
                "#,
                &[&settlement.envelope_id],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let fingerprint: String = row.get("request_fingerprint_sha256");
            if fingerprint != request_fingerprint_sha256 {
                return Err(CreditStoreError::Conflict(
                    "settlement already exists for envelope with different parameters".to_string(),
                ));
            }
            let out = map_settlement_row(&row).map_err(CreditStoreError::Db)?;
            tx.commit()
                .await
                .map_err(|error| CreditStoreError::Db(error.to_string()))?;
            return Ok((out, false));
        }

        tx.execute(
            r#"
            INSERT INTO runtime.credit_settlements (
                settlement_id, envelope_id, outcome, spent_sats, fee_sats,
                verification_receipt_sha256, liquidity_receipt_sha256, created_at,
                request_fingerprint_sha256
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
            &[
                &settlement.settlement_id,
                &settlement.envelope_id,
                &settlement.outcome,
                &settlement.spent_sats,
                &settlement.fee_sats,
                &settlement.verification_receipt_sha256,
                &settlement.liquidity_receipt_sha256,
                &settlement.created_at,
                &request_fingerprint_sha256,
            ],
        )
        .await
        .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        Ok((settlement, true))
    }

    async fn get_settlement_by_envelope(
        &self,
        envelope_id: &str,
    ) -> Result<Option<CreditSettlementRow>, CreditStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT settlement_id, envelope_id, outcome, spent_sats, fee_sats,
                       verification_receipt_sha256, liquidity_receipt_sha256, created_at
                  FROM runtime.credit_settlements
                 WHERE envelope_id = $1
                "#,
                &[&envelope_id],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        Ok(row
            .as_ref()
            .map(map_settlement_row)
            .transpose()
            .map_err(CreditStoreError::Db)?)
    }

    async fn put_receipt(
        &self,
        receipt: CreditReceiptInsertInput,
    ) -> Result<(), CreditStoreError> {
        let client = self.db.client();
        let mut client = client.lock().await;
        let tx = client
            .transaction()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        let existing = tx
            .query_opt(
                r#"
                SELECT receipt_id, canonical_json_sha256
                  FROM runtime.credit_receipts
                 WHERE entity_kind = $1 AND entity_id = $2 AND schema = $3
                "#,
                &[&receipt.entity_kind, &receipt.entity_id, &receipt.schema],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        if let Some(row) = existing {
            let existing_sha: String = row.get("canonical_json_sha256");
            if existing_sha != receipt.canonical_json_sha256 {
                return Err(CreditStoreError::Conflict(
                    "receipt already exists for entity_kind/entity_id/schema with different digest"
                        .to_string(),
                ));
            }
            tx.commit()
                .await
                .map_err(|error| CreditStoreError::Db(error.to_string()))?;
            return Ok(());
        }

        tx.execute(
            r#"
            INSERT INTO runtime.credit_receipts (
                receipt_id, entity_kind, entity_id, schema, canonical_json_sha256,
                signature_json, receipt_json, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
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
        .map_err(|error| CreditStoreError::Db(error.to_string()))?;

        tx.commit()
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        Ok(())
    }

    async fn get_receipt_by_unique(
        &self,
        entity_kind: &str,
        entity_id: &str,
        schema: &str,
    ) -> Result<Option<CreditReceiptInsertInput>, CreditStoreError> {
        let client = self.db.client();
        let client = client.lock().await;
        let row = client
            .query_opt(
                r#"
                SELECT receipt_id, entity_kind, entity_id, schema, canonical_json_sha256,
                       signature_json, receipt_json, created_at
                  FROM runtime.credit_receipts
                 WHERE entity_kind = $1 AND entity_id = $2 AND schema = $3
                "#,
                &[&entity_kind, &entity_id, &schema],
            )
            .await
            .map_err(|error| CreditStoreError::Db(error.to_string()))?;
        Ok(row
            .as_ref()
            .map(map_receipt_row)
            .transpose()
            .map_err(CreditStoreError::Db)?)
    }
}

fn map_offer_row(row: &tokio_postgres::Row) -> Result<CreditOfferRow, String> {
    Ok(CreditOfferRow {
        offer_id: row
            .try_get("offer_id")
            .map_err(|e| e.to_string())?,
        agent_id: row
            .try_get("agent_id")
            .map_err(|e| e.to_string())?,
        pool_id: row.try_get("pool_id").map_err(|e| e.to_string())?,
        scope_type: row
            .try_get("scope_type")
            .map_err(|e| e.to_string())?,
        scope_id: row.try_get("scope_id").map_err(|e| e.to_string())?,
        max_sats: row.try_get("max_sats").map_err(|e| e.to_string())?,
        fee_bps: row.try_get("fee_bps").map_err(|e| e.to_string())?,
        requires_verifier: row
            .try_get("requires_verifier")
            .map_err(|e| e.to_string())?,
        exp: row.try_get("exp").map_err(|e| e.to_string())?,
        status: row.try_get("status").map_err(|e| e.to_string())?,
        issued_at: row.try_get("issued_at").map_err(|e| e.to_string())?,
    })
}

fn map_envelope_row(row: &tokio_postgres::Row) -> Result<CreditEnvelopeRow, String> {
    Ok(CreditEnvelopeRow {
        envelope_id: row
            .try_get("envelope_id")
            .map_err(|e| e.to_string())?,
        offer_id: row.try_get("offer_id").map_err(|e| e.to_string())?,
        agent_id: row
            .try_get("agent_id")
            .map_err(|e| e.to_string())?,
        pool_id: row.try_get("pool_id").map_err(|e| e.to_string())?,
        provider_id: row
            .try_get("provider_id")
            .map_err(|e| e.to_string())?,
        scope_type: row
            .try_get("scope_type")
            .map_err(|e| e.to_string())?,
        scope_id: row.try_get("scope_id").map_err(|e| e.to_string())?,
        max_sats: row.try_get("max_sats").map_err(|e| e.to_string())?,
        fee_bps: row.try_get("fee_bps").map_err(|e| e.to_string())?,
        exp: row.try_get("exp").map_err(|e| e.to_string())?,
        status: row.try_get("status").map_err(|e| e.to_string())?,
        issued_at: row.try_get("issued_at").map_err(|e| e.to_string())?,
    })
}

fn map_settlement_row(row: &tokio_postgres::Row) -> Result<CreditSettlementRow, String> {
    Ok(CreditSettlementRow {
        settlement_id: row
            .try_get("settlement_id")
            .map_err(|e| e.to_string())?,
        envelope_id: row
            .try_get("envelope_id")
            .map_err(|e| e.to_string())?,
        outcome: row.try_get("outcome").map_err(|e| e.to_string())?,
        spent_sats: row.try_get("spent_sats").map_err(|e| e.to_string())?,
        fee_sats: row.try_get("fee_sats").map_err(|e| e.to_string())?,
        verification_receipt_sha256: row
            .try_get("verification_receipt_sha256")
            .map_err(|e| e.to_string())?,
        liquidity_receipt_sha256: row
            .try_get("liquidity_receipt_sha256")
            .map_err(|e| e.to_string())?,
        created_at: row
            .try_get("created_at")
            .map_err(|e| e.to_string())?,
    })
}

fn map_receipt_row(row: &tokio_postgres::Row) -> Result<CreditReceiptInsertInput, String> {
    Ok(CreditReceiptInsertInput {
        receipt_id: row
            .try_get("receipt_id")
            .map_err(|e| e.to_string())?,
        entity_kind: row
            .try_get("entity_kind")
            .map_err(|e| e.to_string())?,
        entity_id: row.try_get("entity_id").map_err(|e| e.to_string())?,
        schema: row.try_get("schema").map_err(|e| e.to_string())?,
        canonical_json_sha256: row
            .try_get("canonical_json_sha256")
            .map_err(|e| e.to_string())?,
        signature_json: row
            .try_get("signature_json")
            .map_err(|e| e.to_string())?,
        receipt_json: row
            .try_get("receipt_json")
            .map_err(|e| e.to_string())?,
        created_at: row.try_get("created_at").map_err(|e| e.to_string())?,
    })
}
