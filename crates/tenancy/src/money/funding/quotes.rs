//! Accepted funding terms in the native monetary journal. A quote issues no
//! credit; provider verification remains the funding adapter's responsibility.

use super::*;

const MAX_QUOTES: usize = 4096;
const MAX_CHECKOUT_SECONDS: u64 = 86_400;

/// Exact accepted checkout terms. Timestamps are admitted by the native writer,
/// rather than supplied later by a payment callback to backdate its terms.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub id: String,
    pub origin: String,
    pub policy: String,
    pub conversion: String,
    pub gross_units: u64,
    pub maximum_fee_units: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct AdmittedQuote {
    pub quote: Quote,
    pub quoted_at: u64,
    pub policy_digest: String,
    pub conversion: Conversion,
    pub minimum_credit: Converted,
    /// The exact funding identity once the adapter presents verified payment.
    /// None is unpaid or unobserved, not a statement about processor failure.
    pub funding: Option<String>,
}

impl Book {
    pub fn admit_quote(&mut self, quote: &Quote, at: u64) -> Result<(), String> {
        for value in [&quote.id, &quote.origin, &quote.policy, &quote.conversion] {
            identity(value)?;
        }
        if self.quotes.contains_key(&quote.id)
            || self.funding.contains_key(&quote.id)
            || self.lots.contains_key(&quote.id)
            || self.quotes.len() >= MAX_QUOTES
        {
            return Err("funding quote identity exists or its retained bound is full".into());
        }
        let policy = self
            .policies
            .get(&self.active)
            .ok_or("funding policy is missing")?;
        if quote.policy != self.active
            || quote.expires_at <= at
            || quote.expires_at - at > MAX_CHECKOUT_SECONDS
        {
            return Err(
                "funding quote must pin current terms and a bounded checkout window".into(),
            );
        }
        let conversion = policy
            .conversions
            .iter()
            .find(|c| c.version == quote.conversion)
            .ok_or("funding quote conversion is unavailable")?;
        if quote.expires_at > conversion.valid_until
            || quote.maximum_fee_units > conversion.max_fee_units
        {
            return Err("funding quote exceeds its conversion validity or fee cap".into());
        }
        let minimum_credit = conversion.quote(quote.gross_units, quote.maximum_fee_units, at)?;
        self.quotes.insert(
            quote.id.clone(),
            AdmittedQuote {
                quote: quote.clone(),
                quoted_at: at,
                policy_digest: policy.digest()?,
                conversion: conversion.clone(),
                minimum_credit,
                funding: None,
            },
        );
        Ok(())
    }

    /// A delayed observation may honor an original admitted quote only when the
    /// processor verifies that the payment itself occurred inside that window.
    /// Changed policy, amount, fees, or origin cannot relabel the obligation.
    pub fn begin_quoted(
        &mut self,
        id: &str,
        funding: &Funding,
        paid_at: u64,
        observed_at: u64,
    ) -> Result<(), String> {
        let admitted = self
            .quotes
            .get(id)
            .ok_or("native funding quote is missing")?;
        let quote = &admitted.quote;
        if admitted.funding.is_some()
            || funding.id != quote.id
            || funding.origin != quote.origin
            || funding.policy != quote.policy
            || funding.conversion != quote.conversion
            || funding.gross_units != quote.gross_units
            || funding.fee_units > quote.maximum_fee_units
            || paid_at < admitted.quoted_at
            || paid_at >= quote.expires_at
            || paid_at > observed_at
        {
            return Err(
                "verified payment differs from its original native checkout admission".into(),
            );
        }
        let quoted_at = admitted.quoted_at;
        self.begin_admitted(funding, quoted_at)?;
        self.quotes
            .get_mut(id)
            .expect("the admitted quote remains present")
            .funding = Some(funding.id.clone());
        Ok(())
    }
}
