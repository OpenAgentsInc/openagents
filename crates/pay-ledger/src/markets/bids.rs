//! Explicit, inert negotiated bid selection. No losing offer can create an obligation.
use super::exact;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub rfq: String,
    pub capability: String,
    pub source: String,
    pub disclosure: String,
    pub max_all_in_msat: i64,
    pub expires_at: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bid {
    pub quote: String,
    pub rfq: String,
    pub provider: String,
    pub labor_terms: String,
    pub capability: String,
    pub source: String,
    pub disclosure: String,
    pub price_msat: i64,
    pub fee_limit_msat: i64,
    pub coordination_cost_msat: i64,
    pub expires_at: i64,
    pub available_capacity: u32,
}
impl Bid {
    pub fn fingerprint(&self) -> Result<String> {
        let value = serde_json::to_value(self).map_err(|_| Error::Invalid("bid encoding"))?;
        let bytes = nostr::contracts::jcs(&value).map_err(|_| Error::Invalid("canonical bid"))?;
        Ok(nostr::contracts::digest_bytes(&bytes)
            .trim_start_matches("sha256:")
            .to_owned())
    }

    pub fn all_in(&self) -> Result<i64> {
        if self.price_msat < 0 || self.fee_limit_msat < 0 || self.coordination_cost_msat < 0 {
            return Err(Error::Invalid("bid cost"));
        }
        self.price_msat
            .checked_add(self.fee_limit_msat)
            .and_then(|p| p.checked_add(self.coordination_cost_msat))
            .ok_or(Error::Invalid("bid cost overflow"))
    }
}
/// Private buyer comparison result. It conveys no grant, order, or payment approval.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub suggested_quote: String,
    pub nonwinners: Vec<String>,
    pub all_in_msat: i64,
}
pub fn compare(request: &Request, bids: &[Bid], now: i64) -> Result<Comparison> {
    for pin in [
        &request.rfq,
        &request.capability,
        &request.source,
        &request.disclosure,
    ] {
        exact(pin)?;
    }
    if request.expires_at <= now || request.max_all_in_msat < 0 || bids.len() > 64 {
        return Err(Error::Invalid("RFQ expiry, budget, or quote bound"));
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut eligible = vec![];
    for bid in bids {
        for pin in [
            &bid.quote,
            &bid.rfq,
            &bid.provider,
            &bid.labor_terms,
            &bid.capability,
            &bid.source,
            &bid.disclosure,
        ] {
            exact(pin)?;
        }
        if !seen.insert(&bid.quote) {
            return Err(Error::Conflict("duplicate quote"));
        }
        let cost = bid.all_in()?;
        if bid.rfq == request.rfq
            && bid.capability == request.capability
            && bid.source == request.source
            && bid.disclosure == request.disclosure
            && bid.expires_at > now
            && bid.available_capacity > 0
            && cost <= request.max_all_in_msat
        {
            eligible.push((cost, &bid.quote));
        }
    }
    eligible.sort();
    let (cost, quote) = eligible.first().ok_or(Error::Denied("no eligible bid"))?;
    Ok(Comparison {
        suggested_quote: (*quote).clone(),
        all_in_msat: *cost,
        nonwinners: bids
            .iter()
            .filter(|b| &b.quote != *quote)
            .map(|b| b.quote.clone())
            .collect(),
    })
}
/// Exact buyer-approved selection, distinct from the comparison suggestion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub bid_fingerprint: String,
    pub quote: String,
    pub order: String,
    pub admission: String,
    pub payer: String,
    pub approved_price_msat: i64,
    pub approved_fee_limit_msat: i64,
}
impl Selection {
    pub fn validate(&self, bid: &Bid, now: i64) -> Result<()> {
        for pin in [
            &self.bid_fingerprint,
            &self.quote,
            &self.order,
            &self.admission,
            &self.payer,
        ] {
            exact(pin)?;
        }
        if self.bid_fingerprint != bid.fingerprint()?
            || self.quote != bid.quote
            || self.approved_price_msat != bid.price_msat
            || self.approved_fee_limit_msat != bid.fee_limit_msat
            || bid.expires_at <= now
            || bid.available_capacity == 0
        {
            return Err(Error::Denied("exact accepted bid terms"));
        }
        bid.all_in()?;
        Ok(())
    }
}
