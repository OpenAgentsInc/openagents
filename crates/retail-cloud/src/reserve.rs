//! Reserving a funded request's maximum charge before anything is
//! provisioned (#10710).
//!
//! The hold lives in the central ledger ([`pay_ledger::compute::hold`]),
//! bound to the account, the quote's digest, the funded request, the
//! execution identity, and the confirmed offer's digest. A retry of the
//! same funded request reuses the hold. Paid-call funding (x402), task
//! dispatch, and provider bills are separate journals; decision-gateway
//! quota is never money here.

use pay_ledger::Ledger;
use pay_ledger::compute::{Hold, HoldRequest};
use route_contract::digest_of;

use crate::authority::{self, Current, Step};
use crate::offer::FundedRequest;
use crate::{Error, Result};

/// The hold request for `funded`.
#[must_use]
pub fn hold_request(funded: &FundedRequest, at: i64) -> HoldRequest {
    HoldRequest {
        id: funded.request.clone(),
        account: funded.account.clone(),
        quote: digest_of(&funded.quote).to_string(),
        execution: funded.execution.clone(),
        terms: funded.offer_digest.to_string(),
        amount_msat: i64::try_from(funded.quote.max_sats.saturating_mul(1000)).unwrap_or(i64::MAX),
        at,
    }
}

/// Hold the funded request's maximum charge, after checking the spend
/// right as it stands now.
///
/// # Errors
///
/// [`Error::Denied`] without the spend right, and the ledger's
/// [`pay_ledger::Error::Insufficient`] or [`pay_ledger::Error::Conflict`].
pub fn reserve(
    ledger: &mut Ledger,
    funded: &FundedRequest,
    current: &Current,
    at: i64,
) -> Result<Hold> {
    authority::check(Step::Reserve, &funded.admission, current).map_err(Error::Denied)?;
    Ok(ledger.reserve(&hold_request(funded, at))?)
}
