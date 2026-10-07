//! Normalize native one-time card collection and reversal evidence.
//! The billing adapter supplies original journal and Money admissions. Browser
//! returns and webhook payload fields are never collection authority.

use super::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use tenancy::money::funding::{
    AdmittedQuote, FeePayer, Finality, Funding, Rounding, Snapshot, Unit,
};

pub struct Original<'a> {
    pub checkout: &'a str,
    pub customer: &'a str,
    pub customer_reference: &'a str,
    pub quote: &'a AdmittedQuote,
}

/// Minimal native facts retained by the private billing journal. It contains
/// no card details, customer payload, dispute evidence, or processor secret.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub checkout: String,
    pub customer: String,
    pub intent: String,
    pub charge: String,
    pub refunds: Vec<String>,
    pub disputes: Vec<String>,
    pub transactions: Vec<String>,
    pub snapshot: Snapshot,
    /// Applicable signed native adjustment expense. A pending negative fee
    /// return remains withheld. These facts issue no customer credit and do
    /// not claim a complete merchant margin.
    pub adjustment_fee_units: i64,
    /// Native removed principal exceeding this purchase's convertible backing.
    pub excess_removed_units: u64,
}

fn uint(value: &Value, field: &str) -> Result<u64, String> {
    value[field].as_u64().ok_or_else(refusal)
}
fn reference(value: &Value, field: &str, prefix: &str) -> Result<String, String> {
    identifier(value[field].as_str().ok_or_else(refusal)?, prefix)
}
fn units(cents: u64) -> Result<u64, String> {
    cents.checked_mul(10_000).ok_or_else(refusal)
}
fn sum(total: &mut u64, amount: u64) -> Result<(), String> {
    *total = total.checked_add(amount).ok_or_else(refusal)?;
    Ok(())
}
struct Transaction {
    amount: i64,
    fee: i64,
    available: bool,
}
impl Transaction {
    fn expense(&self) -> i64 {
        // A native fee return cannot release exposure before it is available.
        if self.fee < 0 && !self.available {
            0
        } else {
            self.fee
        }
    }
}
impl Stripe {
    async fn transaction(
        &self,
        id: &str,
        source: &str,
        kind: &str,
        now: u64,
    ) -> Result<Transaction, String> {
        let value = self.get("balance_transactions", id).await?;
        let amount = value["amount"].as_i64().ok_or_else(refusal)?;
        let fee = value["fee"].as_i64().ok_or_else(refusal)?;
        let net = value["net"].as_i64().ok_or_else(refusal)?;
        let available = match value["status"].as_str() {
            Some("available") if uint(&value, "available_on")? <= now => true,
            Some("pending") => false,
            _ => return Err(refusal()),
        };
        if value["source"] != source
            || value["type"] != kind
            || value["currency"] != "usd"
            || !value["exchange_rate"].is_null()
            || amount.checked_sub(fee) != Some(net)
            || uint(&value, "created")? > now
        {
            return Err(refusal());
        }
        Ok(Transaction {
            amount,
            fee,
            available,
        })
    }

    /// Rebuild one original admitted purchase from the account-bound native
    /// API. A current incomplete or contradictory lookup refuses; the owner
    /// preserves its previous facts and quarantines uncommitted credit.
    /// This method appends no journal row and moves no money.
    pub async fn collect(
        &self,
        original: &Original<'_>,
        previous: Option<&Collection>,
        now: u64,
    ) -> Result<Option<Collection>, String> {
        tokio::time::timeout(
            std::time::Duration::from_secs(60),
            self.collect_inner(original, previous, now),
        )
        .await
        .map_err(|_| refusal())?
    }

    /// Verify original merchant, customer, quote, amount, and native checkout.
    async fn checked_checkout(&self, original: &Original<'_>) -> Result<Value, String> {
        let quoted = original.quote;
        identifier(
            original.checkout,
            if self.mode == Some(true) {
                "cs_live_"
            } else {
                "cs_test_"
            },
        )?;
        let quote = &quoted.quote;
        let conversion = &quoted.conversion;
        let usd = Unit::CurrencyMillionths {
            currency: "USD".into(),
        };
        if conversion.source != usd
            || conversion.target != usd
            || conversion.numerator != 1
            || conversion.denominator != 1
            || conversion.rounding != Rounding::Exact
            || conversion.fee_payer != FeePayer::Customer
            || quote.gross_units % 10_000 != 0
            || quote.gross_units == 0
            || quote.expires_at <= quoted.quoted_at
        {
            return Err(refusal());
        }
        self.admitted_account().await?;
        let customer = self.get("customers", original.customer).await?;
        if customer["deleted"] == true
            || customer["metadata"]["oa_customer"] != original.customer_reference
        {
            return Err(refusal());
        }
        let checkout = self.get("checkout/sessions", original.checkout).await?;
        if checkout["mode"] != "payment"
            || checkout["customer"] != original.customer
            || checkout["currency"] != "usd"
            || units(uint(&checkout, "amount_total")?)? != quote.gross_units
            || checkout["metadata"]["oa_quote"] != quote.id
            || checkout["client_reference_id"] != quote.id
            || uint(&checkout, "expires_at")? != quote.expires_at
            || !checkout["subscription"].is_null()
            || !checkout["setup_intent"].is_null()
        {
            return Err(refusal());
        }
        Ok(checkout)
    }

    /// Only native unpaid state can identify a pending or expired checkout.
    pub(crate) async fn unpaid_status(&self, original: &Original<'_>) -> Result<String, String> {
        let checkout = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            self.checked_checkout(original),
        )
        .await
        .map_err(|_| refusal())??;
        let status = match (
            checkout["status"].as_str(),
            checkout["payment_status"].as_str(),
        ) {
            (Some("open"), Some("unpaid")) => "pending",
            (Some("expired"), Some("unpaid")) => "expired",
            _ => return Err(refusal()),
        };
        self.admitted_account().await?;
        Ok(status.into())
    }

    async fn collect_inner(
        &self,
        original: &Original<'_>,
        previous: Option<&Collection>,
        now: u64,
    ) -> Result<Option<Collection>, String> {
        let quoted = original.quote;
        let quote = &quoted.quote;
        let conversion = &quoted.conversion;
        let checkout = self.checked_checkout(original).await?;
        if checkout["status"] != "complete" || checkout["payment_status"] != "paid" {
            if previous.is_some() {
                return Err(refusal());
            }
            if matches!(checkout["status"].as_str(), Some("open" | "expired"))
                && checkout["payment_status"] == "unpaid"
            {
                self.admitted_account().await?;
                return Ok(None);
            }
            return Err(refusal());
        }
        let intent = reference(&checkout, "payment_intent", "pi_")?;
        let payment = self.get("payment_intents", &intent).await?;
        let cents = quote.gross_units / 10_000;
        if payment["status"] != "succeeded"
            || payment["capture_method"] != "automatic"
            || payment["customer"] != original.customer
            || payment["currency"] != "usd"
            || uint(&payment, "amount")? != cents
            || uint(&payment, "amount_received")? != cents
            || uint(&payment, "amount_capturable")? != 0
            || payment["metadata"]["oa_quote"] != quote.id
            || !payment["application_fee_amount"].is_null()
            || !payment["on_behalf_of"].is_null()
            || !payment["transfer_data"].is_null()
            || !payment["setup_future_usage"].is_null()
        {
            return Err(refusal());
        }
        let charge = reference(&payment, "latest_charge", "ch_")?;
        let paid = self.get("charges", &charge).await?;
        let paid_at = uint(&paid, "created")?;
        if paid["payment_intent"] != intent
            || paid["customer"] != original.customer
            || paid["currency"] != "usd"
            || paid["status"] != "succeeded"
            || paid["paid"] != true
            || paid["captured"] != true
            || paid["payment_method_details"]["type"] != "card"
            || uint(&paid, "amount")? != cents
            || uint(&paid, "amount_captured")? != cents
            || paid_at < quoted.quoted_at
            || paid_at >= quote.expires_at
            || paid_at > now
            || !paid["application_fee"].is_null()
            || !paid["transfer_data"].is_null()
        {
            return Err(refusal());
        }
        let transaction = reference(&paid, "balance_transaction", "txn_")?;
        let native = self
            .transaction(&transaction, &charge, "charge", now)
            .await?;
        if native.amount != i64::try_from(cents).map_err(|_| refusal())? || native.fee < 0 {
            return Err(refusal());
        }
        let fee_units = units(native.fee as u64)?;
        if fee_units > quote.maximum_fee_units {
            return Err(refusal());
        }
        let converted = conversion.quote(quote.gross_units, fee_units, quoted.quoted_at)?;
        let refs = self.adjustments(&charge).await?;
        let mut transactions = BTreeSet::from([transaction]);
        let mut refunded = 0;
        let mut successful_refunds = 0;
        let mut possible_refunds = 0;
        let mut pending = false;
        let mut disputed = 0;
        let mut adjustment_fees = 0_i64;
        let mut recovery_proofs = previous
            .map(|old| old.snapshot.refund_recovery_proofs.clone())
            .unwrap_or_default();
        for id in &refs.refunds {
            let value = self.get("refunds", id).await?;
            if value["charge"] != charge
                || value["payment_intent"] != intent
                || value["currency"] != "usd"
            {
                return Err(refusal());
            }
            let status = value["status"].as_str().ok_or_else(refusal)?;
            match status {
                "failed" | "canceled" => {
                    if value["balance_transaction"].is_null()
                        && value["failure_balance_transaction"].is_null()
                    {
                        continue;
                    }
                }
                "succeeded" => {}
                "pending" | "requires_action" => {
                    pending = true;
                }
                _ => return Err(refusal()),
            }
            let amount = uint(&value, "amount")?;
            if amount == 0 || amount > cents {
                return Err(refusal());
            }
            let txid = reference(&value, "balance_transaction", "txn_")?;
            let tx = self.transaction(&txid, id, "refund", now).await?;
            if tx.amount != -i64::try_from(amount).map_err(|_| refusal())?
                || !transactions.insert(txid)
            {
                return Err(refusal());
            }
            adjustment_fees = adjustment_fees
                .checked_add(tx.expense())
                .ok_or_else(refusal)?;
            if matches!(status, "failed" | "canceled") {
                let return_id = reference(&value, "failure_balance_transaction", "txn_")?;
                let returned = self
                    .transaction(&return_id, id, "refund_failure", now)
                    .await?;
                if returned.amount != i64::try_from(amount).map_err(|_| refusal())?
                    || !transactions.insert(return_id.clone())
                {
                    return Err(refusal());
                }
                adjustment_fees = adjustment_fees
                    .checked_add(returned.expense())
                    .ok_or_else(refusal)?;
                if returned.available {
                    let proof = format!(
                        "stripe:{}:{return_id}",
                        self.verified_account.as_ref().ok_or_else(refusal)?
                    );
                    let amount_units = units(amount)?;
                    if recovery_proofs
                        .insert(proof, amount_units)
                        .is_some_and(|old| old != amount_units)
                    {
                        return Err(refusal());
                    }
                } else {
                    pending = true;
                    sum(&mut refunded, amount)?;
                }
            } else {
                sum(&mut refunded, amount)?;
                sum(&mut possible_refunds, amount)?;
                if status == "succeeded" {
                    sum(&mut successful_refunds, amount)?;
                }
            }
        }
        let reported_refund = uint(&paid, "amount_refunded")?;
        if reported_refund < successful_refunds
            || reported_refund > possible_refunds
            || reported_refund > cents
        {
            return Err(refusal());
        }
        for id in &refs.disputes {
            let value = self.get("disputes", id).await?;
            if value["charge"] != charge
                || value["payment_intent"] != intent
                || value["currency"] != "usd"
            {
                return Err(refusal());
            }
            let amount = uint(&value, "amount")?;
            if amount == 0 || amount > cents {
                return Err(refusal());
            }
            let rows = value["balance_transactions"]
                .as_array()
                .ok_or_else(refusal)?;
            if rows.len() > 2 {
                return Err(refusal());
            }
            let mut removed = 0;
            let mut restored = 0;
            for row in rows {
                let txid = reference(row, "id", "txn_")?;
                let tx = self.transaction(&txid, id, "adjustment", now).await?;
                if tx.amount.unsigned_abs() != amount || !transactions.insert(txid) {
                    return Err(refusal());
                }
                if tx.amount < 0 {
                    sum(&mut removed, amount)?;
                } else if tx.available {
                    sum(&mut restored, amount)?;
                }
                adjustment_fees = adjustment_fees
                    .checked_add(tx.expense())
                    .ok_or_else(refusal)?;
            }
            if removed > amount || restored > removed {
                return Err(refusal());
            }
            match value["status"].as_str() {
                Some("warning_closed") if removed == 0 && restored == 0 => {}
                Some("needs_response" | "under_review" | "lost")
                    if removed == amount && restored == 0 => {}
                Some("won") if removed == amount => {}
                _ => return Err(refusal()),
            }
            sum(&mut disputed, removed - restored)?;
        }
        let checked = self.get("charges", &charge).await?;
        let after = self.adjustments(&charge).await?;
        self.admitted_account().await?;
        if super::adjustments::head(&paid) != super::adjustments::head(&checked)
            || refs.refunds != after.refunds
            || refs.disputes != after.disputes
        {
            return Err(refusal());
        }
        let removed = units(refunded.checked_add(disputed).ok_or_else(refusal)?)?;
        let refund_units = units(refunded)?.min(converted.convertible_units);
        let dispute_units = units(disputed)?.min(converted.convertible_units - refund_units);
        let funding = Funding {
            id: quote.id.clone(),
            origin: quote.origin.clone(),
            payment: format!(
                "stripe:{}:{charge}",
                self.verified_account.as_ref().ok_or_else(refusal)?
            ),
            policy: quote.policy.clone(),
            conversion: quote.conversion.clone(),
            gross_units: quote.gross_units,
            fee_units,
        };
        let revision = if let Some(old) = previous {
            if old.checkout != original.checkout
                || old.customer != original.customer
                || old.intent != intent
                || old.charge != charge
                || old.snapshot.funding != funding
                || old.snapshot.paid_at != paid_at
                || !old.refunds.iter().all(|id| refs.refunds.contains(id))
                || !old.disputes.iter().all(|id| refs.disputes.contains(id))
                || !old.transactions.iter().all(|id| transactions.contains(id))
                || (old.snapshot.finality == Finality::Final && !native.available)
            {
                return Err(refusal());
            }
            let restored = recovery_proofs
                .iter()
                .filter(|(proof, _)| !old.snapshot.refund_recovery_proofs.contains_key(*proof))
                .try_fold(0_u64, |sum, (_, amount)| sum.checked_add(*amount))
                .ok_or_else(refusal)?;
            if old
                .snapshot
                .refunded_source_units
                .saturating_sub(refund_units)
                > restored
            {
                return Err(refusal());
            }
            old.snapshot.revision.checked_add(1).ok_or_else(refusal)?
        } else {
            1
        };
        let transactions = transactions.into_iter().collect::<Vec<_>>();
        let evidence = receipts::execution::digest_request(
            &json!({"checkout":original.checkout,"intent":intent,"charge":charge,
            "transactions":transactions,"gross":quote.gross_units,"fee":fee_units,"paid_at":paid_at,
            "refund":refund_units,"dispute":dispute_units,"available":native.available,"adjustment_fees":adjustment_fees,
            "refund_recoveries":recovery_proofs,"reconciliation_pending":pending}),
        );
        Ok(Some(Collection {
            checkout: original.checkout.into(),
            customer: original.customer.into(),
            intent,
            charge,
            refunds: refs.refunds,
            disputes: refs.disputes,
            transactions,
            snapshot: Snapshot {
                quote: quote.id.clone(),
                funding,
                paid_at,
                finality: if native.available {
                    Finality::Final
                } else {
                    Finality::Confirmed
                },
                evidence,
                revision,
                refunded_source_units: refund_units,
                refund_recovery_proofs: recovery_proofs,
                disputed_source_units: dispute_units,
                reconciliation_pending: pending,
                processor_expense_units: adjustment_fees.checked_mul(10_000).ok_or_else(refusal)?,
            },
            adjustment_fee_units: adjustment_fees.checked_mul(10_000).ok_or_else(refusal)?,
            excess_removed_units: removed.saturating_sub(converted.convertible_units),
        }))
    }
}
