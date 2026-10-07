//! Bounded native adjustment references for one original merchant charge.
//! These reads authorize neither refunds nor credit. The funding owner must
//! retrieve and normalize every referenced native record before reconciliation.

use super::*;
use std::collections::BTreeSet;

/// Native references only. No customer, card, dispute evidence, or message
/// payload leaves this lookup or becomes a journal record.
pub struct Adjustments {
    pub refunds: Vec<String>,
    pub disputes: Vec<String>,
}

fn head(charge: &Value) -> Vec<Value> {
    [
        "id",
        "livemode",
        "amount",
        "amount_captured",
        "currency",
        "customer",
        "payment_intent",
        "balance_transaction",
        "paid",
        "captured",
        "status",
        "amount_refunded",
        "refunded",
        "disputed",
    ]
    .iter()
    .map(|key| charge[*key].clone())
    .collect()
}

impl Stripe {
    /// Find complete native refund and dispute reference sets for the original
    /// charge. An incomplete page or changed charge requires reconciliation;
    /// partial rows never establish that the remaining backing is available.
    pub async fn adjustments(&self, charge: &str) -> Result<Adjustments, String> {
        identifier(charge, "ch_")?;
        self.admitted_account().await?;
        let before = self.get("charges", charge).await?;
        let refunds = self.related("refunds", charge).await?;
        let disputes = self.related("disputes", charge).await?;
        let after = self.get("charges", charge).await?;
        self.admitted_account().await?;
        if head(&before) != head(&after) {
            return Err(refusal());
        }
        Ok(Adjustments { refunds, disputes })
    }

    async fn related(&self, resource: &str, charge: &str) -> Result<Vec<String>, String> {
        let (object, prefix) = match resource {
            "refunds" => ("refund", "re_"),
            "disputes" => ("dispute", "dp_"),
            _ => return Err(refusal()),
        };
        let value = self
            .exchange(
                self.client
                    .get(format!("{}/v1/{resource}", self.origin))
                    .query(&[("charge", charge), ("limit", "100")]),
            )
            .await?;
        let rows = value["data"].as_array().ok_or_else(refusal)?;
        if value["object"] != "list"
            || value["url"] != format!("/v1/{resource}")
            || value["has_more"] != false
            || rows.len() > 100
        {
            return Err(refusal());
        }
        let mut seen = BTreeSet::new();
        let mut refs = Vec::with_capacity(rows.len());
        for row in rows {
            let id = row["id"].as_str().ok_or_else(refusal)?;
            identifier(id, prefix)?;
            if row["object"] != object || row["charge"] != charge || !seen.insert(id) {
                return Err(refusal());
            }
            if resource == "disputes" {
                self.check_mode(row)?;
            }
            refs.push(id.into());
        }
        Ok(refs)
    }
}
