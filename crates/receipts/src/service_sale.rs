//! Private service-sale metadata. Storage, authentication, external payment
//! verification, and publication authority stay with the recording owner.

use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

#[path = "service_sale_sources.rs"]
mod sources;
pub use sources::{Facts, verify_fulfillment, verify_sources};

pub const SCHEMA: &str = "openagents.sales.service-sale.v1";
pub const EXPORT_SCHEMA: &str = "openagents.sales.service-sale-export.v1";
pub const MAX_PAYMENTS: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub path: String,
    pub sha256: String,
}
impl Reference {
    pub fn validate(&self) -> Result<(), String> {
        if self.path.is_empty()
            || self.path.len() > 512
            || !Path::new(&self.path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            || self.path.chars().any(char::is_control)
            || self.sha256.len() != 64
            || !self.sha256.bytes().all(|c| c.is_ascii_hexdigit())
        {
            return Err("invalid private service evidence reference".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub agreement: Reference,
    pub agreement_acceptance: Reference,
    pub pilot_review: Reference,
    pub handoff: Reference,
    pub customer_acceptance: Reference,
    pub support_acceptance: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Invoice {
    pub id: String,
    pub external_reference: String,
    pub currency: String,
    pub currency_scale: u64,
    pub amount_minor: u64,
    pub issued_at: u64,
    pub due_at: u64,
    pub payment_route_reference: String,
    pub evidence: Reference,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentTrigger {
    AcceptedDelivery,
    VerifiedServicePayment,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Fulfillment {
    pub id: String,
    pub responsible_human: String,
    pub currency: String,
    pub currency_scale: u64,
    pub amount_minor: u64,
    pub trigger: FulfillmentTrigger,
    pub agreement: Reference,
    pub acceptance: Reference,
    pub bill: Option<Reference>,
    pub payment: Option<Reference>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub id: String,
    pub offer_version: String,
    pub invoice: Invoice,
    pub sources: Evidence,
    pub fulfillment: Option<Fulfillment>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Disposition {
    Pending,
    Unknown,
    Paid,
    Reversed,
    Disputed,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PaymentInput {
    pub disposition: Disposition,
    pub external_reference: Option<String>,
    /// Verified original gross collection. V1 supports one fully paid invoice;
    /// a partial or unverified claim remains pending or unknown.
    pub paid_minor: Option<u64>,
    /// Verified cumulative refund/clawback, not an assumed disputed amount.
    pub reversed_minor: Option<u64>,
    pub evidence: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    pub input: PaymentInput,
    pub verified_by: String,
    pub verified_at: u64,
    pub command_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FulfillmentInput {
    pub bill: Reference,
    pub payment: Option<Reference>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FulfillmentVerification {
    pub input: FulfillmentInput,
    pub verified_by: String,
    pub verified_at: u64,
    pub command_digest: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Sale {
    pub schema: String,
    pub admission: Admission,
    pub pipeline_lead: String,
    pub pipeline_revision_at_admission: u64,
    pub account: String,
    pub admitted_by: String,
    pub admitted_at: u64,
    pub admission_command_digest: String,
    pub admitted_recipients: Vec<String>,
    pub retain_until: u64,
    pub facts: Facts,
    pub payments: Vec<Verification>,
    pub fulfillment_reconciliations: Vec<FulfillmentVerification>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Export {
    pub schema: String,
    pub sale: Sale,
    pub exported_by: String,
    pub exported_at: u64,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PaymentSummary {
    pub paid_minor: u64,
    pub refunded_minor: u64,
    pub refund_reversals_minor: u64,
    pub unresolved: bool,
}

pub(crate) fn identifier(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err("invalid bounded service identifier".into());
    }
    Ok(())
}
pub(crate) fn digest(value: &str) -> Result<(), String> {
    if value.len() != 64 || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid service source digest".into());
    }
    Ok(())
}
/// Convert declared USD cents to the finance report's fixed denomination.
/// This conversion changes precision only; it never exchanges currencies.
pub fn usd_millionths(currency: &str, scale: u64, minor: u64) -> Result<u64, String> {
    if currency != "USD" || scale != 100 {
        return Err("service v1 requires declared USD minor units with scale 100".into());
    }
    minor
        .checked_mul(10_000)
        .ok_or_else(|| "service amount overflow".into())
}

/// Validate an explicitly denominated fulfillment charge without converting
/// Bitcoin to the service invoice's USD reporting unit.
pub fn validate_fulfillment_amount(currency: &str, scale: u64, minor: u64) -> Result<(), String> {
    if currency == "BTC" && scale == 100_000_000_000 && minor > 0 && minor <= i64::MAX as u64 {
        return Ok(());
    }
    usd_millionths(currency, scale, minor).map(|_| ())
}
impl Admission {
    pub fn validate(&self) -> Result<(), String> {
        for s in [
            &self.id,
            &self.offer_version,
            &self.invoice.id,
            &self.invoice.external_reference,
            &self.invoice.payment_route_reference,
        ] {
            identifier(s)?;
        }
        usd_millionths(
            &self.invoice.currency,
            self.invoice.currency_scale,
            self.invoice.amount_minor,
        )?;
        if self.invoice.amount_minor == 0 || self.invoice.issued_at > self.invoice.due_at {
            return Err("service invoice needs a positive agreed charge and valid dates".into());
        }
        self.invoice.evidence.validate()?;
        for r in [
            &self.sources.agreement,
            &self.sources.agreement_acceptance,
            &self.sources.pilot_review,
            &self.sources.handoff,
            &self.sources.customer_acceptance,
            &self.sources.support_acceptance,
        ] {
            r.validate()?;
        }
        if let Some(f) = &self.fulfillment {
            for s in [&f.id, &f.responsible_human] {
                identifier(s)?;
            }
            validate_fulfillment_amount(&f.currency, f.currency_scale, f.amount_minor)?;
            if f.amount_minor == 0 {
                return Err("fulfillment needs its own agreed positive price".into());
            }
            f.agreement.validate()?;
            f.acceptance.validate()?;
            for r in [&f.bill, &f.payment].into_iter().flatten() {
                r.validate()?;
            }
            if f.payment.is_some() && f.bill.is_none() {
                return Err("fulfillment payment requires its billed obligation".into());
            }
        }
        Ok(())
    }
}
impl Sale {
    pub fn validate(&self) -> Result<(), String> {
        self.admission.validate()?;
        if self.schema != SCHEMA
            || self.pipeline_revision_at_admission == 0
            || self.payments.len() > MAX_PAYMENTS
            || self.admitted_recipients.is_empty()
            || self.admitted_recipients.len() > 16
            || self.retain_until <= self.admitted_at
        {
            return Err("unsupported or oversized service sale".into());
        }
        for s in [&self.pipeline_lead, &self.account, &self.admitted_by] {
            identifier(s)?;
        }
        digest(&self.admission_command_digest)?;
        if !self
            .admitted_recipients
            .contains(&format!("human:{}", self.admitted_by))
        {
            return Err("service issuer is outside the admitted recipients".into());
        }
        for s in &self.admitted_recipients {
            identifier(s)?;
        }
        self.facts.validate()?;
        let summary = self.summary()?;
        if self.admission.fulfillment.as_ref().is_some_and(|f| {
            f.payment.is_some()
                && f.trigger == FulfillmentTrigger::VerifiedServicePayment
                && summary.paid_minor == 0
        }) {
            return Err("fulfillment payment precedes its verified service payment trigger".into());
        }
        self.effective_fulfillment()?;
        Ok(())
    }
    pub fn effective_fulfillment(&self) -> Result<Option<Fulfillment>, String> {
        if self.fulfillment_reconciliations.len() > MAX_PAYMENTS {
            return Err("fulfillment history exceeds bound".into());
        }
        let mut current = self.admission.fulfillment.clone();
        let mut previous_at = self.admitted_at;
        for verification in &self.fulfillment_reconciliations {
            let f = current
                .as_mut()
                .ok_or("fulfillment reconciliation needs an admitted obligation")?;
            identifier(&verification.verified_by)?;
            digest(&verification.command_digest)?;
            verification.input.bill.validate()?;
            if let Some(payment) = &verification.input.payment {
                payment.validate()?;
            }
            if verification.verified_at < previous_at
                || verification.verified_at >= self.retain_until
                || !self
                    .admitted_recipients
                    .contains(&format!("human:{}", verification.verified_by))
                || f.bill
                    .as_ref()
                    .is_some_and(|r| r != &verification.input.bill)
                || f.payment
                    .as_ref()
                    .is_some_and(|r| Some(r) != verification.input.payment.as_ref())
            {
                return Err("fulfillment evidence changes or exceeds admitted authority".into());
            }
            if verification.input.payment.is_some()
                && f.trigger == FulfillmentTrigger::VerifiedServicePayment
                && !self.payments.iter().any(|v| {
                    v.input.disposition == Disposition::Paid
                        && v.verified_at <= verification.verified_at
                })
            {
                return Err(
                    "fulfillment payment precedes its verified service payment trigger".into(),
                );
            }
            f.bill = Some(verification.input.bill.clone());
            f.payment = verification.input.payment.clone();
            previous_at = verification.verified_at;
        }
        Ok(current)
    }
    pub fn validate_next(&self, input: &PaymentInput) -> Result<(), String> {
        input.evidence.validate()?;
        if let Some(r) = &input.external_reference {
            identifier(r)?;
        }
        if self.payments.len() >= MAX_PAYMENTS {
            return Err("service payment history exceeds bound".into());
        }
        if let Some(reference) = self
            .payments
            .iter()
            .find_map(|v| v.input.external_reference.as_ref())
        {
            if input.external_reference.as_ref() != Some(reference) {
                return Err("verified service payment identity is immutable".into());
            }
        }
        let prior_paid = self
            .payments
            .iter()
            .any(|v| v.input.disposition == Disposition::Paid);
        match input.disposition {
            Disposition::Pending | Disposition::Unknown => {
                if input.paid_minor.is_some() || input.reversed_minor.is_some() {
                    return Err(
                        "unverified service disposition cannot manufacture collected money".into(),
                    );
                }
            }
            Disposition::Paid => {
                if input.external_reference.is_none()
                    || input.paid_minor != Some(self.admission.invoice.amount_minor)
                    || input.reversed_minor.unwrap_or(0) > self.admission.invoice.amount_minor
                {
                    return Err("paid disposition requires the exact fully collected invoice and external reference".into());
                }
            }
            Disposition::Reversed => {
                if !prior_paid
                    || input.paid_minor != Some(self.admission.invoice.amount_minor)
                    || input.reversed_minor.unwrap_or(0) == 0
                    || input.reversed_minor.unwrap_or(0) > self.admission.invoice.amount_minor
                {
                    return Err("service reversal requires the previously verified collection and exact cumulative refund".into());
                }
            }
            Disposition::Disputed => {
                if input.external_reference.is_none()
                    || input
                        .paid_minor
                        .is_some_and(|v| !prior_paid || v != self.admission.invoice.amount_minor)
                    || input
                        .reversed_minor
                        .is_some_and(|v| !prior_paid || v > self.admission.invoice.amount_minor)
                {
                    return Err("a dispute does not establish a payment or assumed clawback".into());
                }
            }
        }
        Ok(())
    }
    pub fn summary(&self) -> Result<PaymentSummary, String> {
        let mut prefix = self.clone();
        prefix.payments.clear();
        let mut summary = PaymentSummary::default();
        let mut refund_position = 0;
        let mut previous_at = self.admitted_at;
        for verification in &self.payments {
            prefix.validate_next(&verification.input)?;
            identifier(&verification.verified_by)?;
            digest(&verification.command_digest)?;
            if verification.verified_at < previous_at
                || verification.verified_at >= self.retain_until
                || !self
                    .admitted_recipients
                    .contains(&format!("human:{}", verification.verified_by))
            {
                return Err("service verification is outside retained admission authority".into());
            }
            if verification.input.disposition == Disposition::Paid {
                summary.paid_minor = self.admission.invoice.amount_minor;
            }
            if let Some(now) = verification.input.reversed_minor {
                if now >= refund_position {
                    summary.refunded_minor = summary
                        .refunded_minor
                        .checked_add(now - refund_position)
                        .ok_or("service refund overflow")?;
                } else {
                    summary.refund_reversals_minor = summary
                        .refund_reversals_minor
                        .checked_add(refund_position - now)
                        .ok_or("service refund reversal overflow")?;
                }
                refund_position = now;
            }
            summary.unresolved = matches!(
                verification.input.disposition,
                Disposition::Pending | Disposition::Unknown | Disposition::Disputed
            );
            prefix.payments.push(verification.clone());
            previous_at = verification.verified_at;
        }
        if self.payments.is_empty() {
            summary.unresolved = true;
        }
        Ok(summary)
    }
}
impl Export {
    pub fn validate(&self) -> Result<(), String> {
        self.sale.validate()?;
        identifier(&self.exported_by)?;
        if self.schema != EXPORT_SCHEMA
            || self.exported_at < self.sale.admitted_at
            || self
                .sale
                .payments
                .last()
                .is_some_and(|v| v.verified_at > self.exported_at)
            || self
                .sale
                .fulfillment_reconciliations
                .last()
                .is_some_and(|v| v.verified_at > self.exported_at)
            || self.exported_at >= self.sale.retain_until
            || !self
                .sale
                .admitted_recipients
                .contains(&format!("human:{}", self.exported_by))
        {
            return Err(
                "private service export exceeds its admitted disclosure or retention".into(),
            );
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn service_cash_conversion_is_exact_bounded_and_never_implies_fx() {
        assert_eq!(usd_millionths("USD", 100, 101).unwrap(), 1_010_000);
        assert!(usd_millionths("EUR", 100, 101).is_err());
        assert!(usd_millionths("USD", 1000, 101).is_err());
        assert!(usd_millionths("USD", 100, u64::MAX).is_err());
        assert!(
            Reference {
                path: "../private".into(),
                sha256: "a".repeat(64)
            }
            .validate()
            .is_err()
        );
    }
    #[test]
    fn btc_fulfillment_is_exact_without_changing_usd_invoice_conversion() {
        assert!(validate_fulfillment_amount("BTC", 100_000_000_000, 10_000).is_ok());
        for (currency, scale, amount) in [
            ("BTC", 100_000_000, 10_000),
            ("BTC", 100_000_000_000, 0),
            ("BTC", 100_000_000_000, u64::MAX),
            ("EUR", 100, 10_000),
        ] {
            assert!(validate_fulfillment_amount(currency, scale, amount).is_err());
        }
        assert!(usd_millionths("BTC", 100_000_000_000, 10_000).is_err());
        assert_eq!(usd_millionths("USD", 100, 101).unwrap(), 1_010_000);
    }
}
