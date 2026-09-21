//! Monetary admission: the opt-in bridge from a served call to the
//! workspace spending ledger in `tenancy::money`.
//!
//! The mode is off unless `gateway.json` names a `money` document. When
//! it is absent nothing here runs — no ledger opens, no workspace is
//! charged, and no balance route exists. When it is present, every
//! admitted call reserves a bounded worst-case spend from its
//! authenticated workspace before any backend dispatch, then settles
//! the usage the serving process actually reported. A response without
//! a complete priceable report leaves the hold outstanding rather than
//! settling at zero.
//!
//! This module chooses no price, grants no credit, and exposes no
//! payment operation. Account creation, credit, debit, release of an
//! unknown hold, and refunds are operator mutations on the ledger,
//! never request-time behavior.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;
use tenancy::money::{Ledger, Mutation, Operation, Phase, Price, Resource, Usage};

/// The settlement policy this build implements under monetary
/// admission: a dispatched attempt is charged exactly the usage the
/// serving process reported, priced under the configured schedule —
/// nothing estimated, nothing invented. Completion that cannot be
/// priced stays outstanding as the full reservation, never zero.
pub const POLICY: &str = "observed-usage-v1";

/// The `money` document in `gateway.json` — the explicit operator
/// opt-in to charging workspaces.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Money {
    /// The spending ledger's file: one `tenancy::money` append-only log
    /// the process opens exclusively and holds for its lifetime. Keep it
    /// in a protected directory outside every executor write grant — the
    /// ledger refuses a symlink, a shared permission bit, and a second
    /// writer, and a damaged log refuses to open rather than discarding
    /// a tail.
    pub ledger: PathBuf,
    /// Door name to the admission it charges under. A configured door
    /// missing from this map refuses every call as `unpriced` — the
    /// gateway never invents a price to keep a door serving.
    #[serde(default)]
    pub doors: BTreeMap<String, Priced>,
}

/// What one door charges: the versioned price schedule and the
/// worst-case usage an attempt may reserve.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Priced {
    /// The price the door's calls are quoted under. Its `model` and
    /// `capacity` must equal the binding's artifact id and lane, and its
    /// `policy` must be [`POLICY`] — a price naming anything else is one
    /// this gateway cannot serve and refuses before dispatch.
    pub price: Price,
    /// The largest usage one attempt may be billed for: every priced
    /// resource, bounded explicitly. The reservation holds the price's
    /// quote of this map, so it must name exactly the priced resources.
    pub maximum_usage: Usage,
}

/// Why a monetary reservation was refused.
#[derive(Debug)]
pub enum Refusal {
    /// The workspace cannot fund the hold — no provisioned account, or
    /// the hold exceeds its available balance or remaining spend.
    Funds(String),
    /// The configured price cannot govern this call — its identity or
    /// terms disagree with the binding or the account's currency.
    Price(String),
    /// The ledger itself refused — a write failure or a poisoned
    /// instance. Dispatch must stop until an operator reconciles.
    Ledger(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Funds(message) | Self::Price(message) | Self::Ledger(message) => {
                write!(f, "{message}")
            }
        }
    }
}

/// The hold one admitted call stands under — what a later settle,
/// release, or unknown marker names.
pub struct Hold {
    /// The workspace account charged.
    pub workspace: String,
    /// The hold's attempt identity: `{request}#{attempt}` — the same
    /// reference the quota reservation and the receipt's usage field
    /// carry, so one name joins all three records.
    pub attempt: String,
    /// The price the hold was taken under — what settlement quotes.
    pub price: Price,
}

/// The counter name a response's `usage` object carries for a priced
/// resource — the only counters monetary admission can read. A provider
/// counter outside this set is unpriced and unsettleable here.
fn wire(resource: Resource) -> &'static str {
    match resource {
        Resource::InputTokens => "input_tokens",
        Resource::CachedInputTokens => "cached_input_tokens",
        Resource::OutputTokens => "output_tokens",
        Resource::ReasoningTokens => "reasoning_tokens",
        Resource::ComputeMilliseconds => "compute_milliseconds",
    }
}

/// Reserve the worst-case authorized spend for one attempt — durably,
/// before any backend dispatch.
///
/// A hold already standing under this attempt is the same reservation
/// replaying — an in-flight retry is not a second spend — and the
/// result says which happened so the caller knows whether a later
/// refusal may free the call's quota reservation.
pub fn reserve(
    ledger: &mut Ledger,
    workspace: &str,
    request: &str,
    attempt: u32,
    request_digest: &str,
    priced: &Priced,
    model: &str,
    capacity: &str,
) -> Result<Hold, Refusal> {
    let key = format!("{request}#{attempt}");
    if ledger.hold(workspace, &key).is_some() {
        return Ok(Hold {
            workspace: workspace.to_string(),
            attempt: key,
            price: priced.price.clone(),
        });
    }
    let price = &priced.price;
    if price.policy != POLICY {
        return Err(Refusal::Price(format!(
            "the price names policy `{}`, which this gateway does not implement (`{POLICY}`)",
            price.policy
        )));
    }
    if price.model != model {
        return Err(Refusal::Price(format!(
            "the price's model `{}` does not match the bound artifact `{model}`",
            price.model
        )));
    }
    if price.capacity != capacity {
        return Err(Refusal::Price(format!(
            "the price's capacity `{}` does not match the door's `{capacity}` lane",
            price.capacity
        )));
    }
    let worst = price.quote(&priced.maximum_usage).map_err(Refusal::Price)?;
    let balance = ledger.balance(workspace).map_err(|_| {
        Refusal::Funds(format!(
            "workspace `{workspace}` holds no provisioned monetary account"
        ))
    })?;
    if worst > balance.available.min(balance.spend_remaining) {
        return Err(Refusal::Funds(format!(
            "workspace `{workspace}` cannot cover the worst-case hold of {worst} {} \
             millionths ({} available, {} spend remaining)",
            balance.currency, balance.available, balance.spend_remaining
        )));
    }
    ledger
        .apply(Mutation {
            workspace: workspace.to_string(),
            source: format!("gateway:{key}:reserve"),
            audit: request_digest.to_string(),
            operation: Operation::Reserve {
                attempt: key.clone(),
                request_digest: request_digest.to_string(),
                price: price.clone(),
                maximum_usage: priced.maximum_usage.clone(),
            },
        })
        .map_err(classify)?;
    Ok(Hold {
        workspace: workspace.to_string(),
        attempt: key,
        price: price.clone(),
    })
}

/// Map a ledger refusal to its admission meaning. The affordability
/// pre-check catches the funding cases deterministically; anything left
/// is a price fault or a ledger fault.
fn classify(error: String) -> Refusal {
    for needle in ["credit", "spend limit", "account is missing"] {
        if error.contains(needle) {
            return Refusal::Funds(error);
        }
    }
    for needle in ["currency", "price", "usage", "version", "identifier"] {
        if error.contains(needle) {
            return Refusal::Price(error);
        }
    }
    Refusal::Ledger(error)
}

/// What the ledger was told about a dispatched attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Settlement {
    /// Observed usage priced and settled.
    Settled,
    /// Completion could not be priced — the attempt is marked unknown
    /// and its full reservation stays outstanding, never zero.
    Outstanding,
    /// No work dispatched — the hold was released back to the account.
    Released,
}

impl Settlement {
    /// The wire label the `x-settlement` response header carries.
    pub fn label(self) -> &'static str {
        match self {
            Self::Settled => "settled",
            Self::Outstanding => "outstanding",
            Self::Released => "released",
        }
    }
}

/// Settle the hold against observed usage — `None` when the completion
/// carried no complete priceable report. A settle the ledger refuses
/// (over-bound usage, a write failure) marks the attempt unknown: the
/// reservation stays outstanding either way.
pub fn settle(ledger: &mut Ledger, hold: &Hold, usage: Option<Usage>, receipt: &str) -> Settlement {
    // A duplicated attempt can reach settlement twice — an in-flight
    // retry shares the hold. Report the resolution that already stands
    // rather than rewriting a terminal phase.
    if let Some(existing) = ledger.hold(&hold.workspace, &hold.attempt) {
        match existing.phase {
            Phase::Settled => return Settlement::Settled,
            Phase::Released => return Settlement::Released,
            Phase::Held | Phase::Unknown => {}
        }
    }
    let Some(usage) = usage else {
        return unknown(ledger, hold);
    };
    match ledger.apply(Mutation {
        workspace: hold.workspace.clone(),
        source: format!("gateway:{}:settle", hold.attempt),
        audit: receipt.to_string(),
        operation: Operation::Settle {
            attempt: hold.attempt.clone(),
            usage,
            receipt: receipt.to_string(),
            provider_cost: None,
            hosting_cost: None,
        },
    }) {
        Ok(_) => Settlement::Settled,
        Err(_) => unknown(ledger, hold),
    }
}

/// Mark the hold's completion unknown — the whole reservation stays
/// outstanding as a liability until an operator reconciles it.
pub fn unknown(ledger: &mut Ledger, hold: &Hold) -> Settlement {
    ledger
        .apply(Mutation {
            workspace: hold.workspace.clone(),
            source: format!("gateway:{}:unknown", hold.attempt),
            audit: hold.attempt.clone(),
            operation: Operation::Unknown {
                attempt: hold.attempt.clone(),
            },
        })
        .ok();
    Settlement::Outstanding
}

/// Release a hold whose work was never dispatched — the identity check
/// refused, or the call's fan-out sent nothing. The gateway writes no
/// release for any other cause; an unknown hold is reconciled only by
/// an operator with evidence no charge is due.
pub fn release(ledger: &mut Ledger, hold: &Hold) -> Settlement {
    // The same duplicated-attempt guard as settle: a twin that never
    // dispatched must not rewrite a hold the original already resolved.
    if let Some(existing) = ledger.hold(&hold.workspace, &hold.attempt) {
        match existing.phase {
            Phase::Settled => return Settlement::Settled,
            Phase::Released => return Settlement::Released,
            Phase::Unknown => return Settlement::Outstanding,
            Phase::Held => {}
        }
    }
    ledger
        .apply(Mutation {
            workspace: hold.workspace.clone(),
            source: format!("gateway:{}:release", hold.attempt),
            audit: hold.attempt.clone(),
            operation: Operation::Release {
                attempt: hold.attempt.clone(),
            },
        })
        .ok();
    Settlement::Released
}

/// The usage one response reports under the price's resources — `None`
/// when any priced resource is unreported or reports a non-count. A
/// report naming other resources settles under the priced set alone.
pub fn observed(price: &Price, body: &Value) -> Option<Usage> {
    observed_total(price, &[body.get("usage")])
}

/// The same read over a fan-out: every dispatched item must report
/// every priced resource for a total to exist — one silent item leaves
/// the whole settlement outstanding rather than partially priced.
pub fn observed_total(price: &Price, reports: &[Option<&Value>]) -> Option<Usage> {
    if reports.is_empty() {
        return None;
    }
    let mut usage = Usage::new();
    for resource in price.rates.keys() {
        let mut total = 0_u64;
        for report in reports {
            let units = report
                .and_then(|usage| usage.get(wire(*resource)))
                .and_then(Value::as_u64)?;
            total = total.checked_add(units)?;
        }
        usage.insert(*resource, total);
    }
    Some(usage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tenancy::money::Rate;

    fn priced() -> Priced {
        Priced {
            price: Price {
                version: "synthetic-fixture-v1".into(),
                currency: "USD".into(),
                model: "kev-0.6b".into(),
                capacity: "dedicated".into(),
                policy: POLICY.into(),
                rates: [
                    (
                        Resource::InputTokens,
                        Rate {
                            millionths: 10,
                            per_units: 1,
                        },
                    ),
                    (
                        Resource::OutputTokens,
                        Rate {
                            millionths: 40,
                            per_units: 1,
                        },
                    ),
                ]
                .into(),
            },
            maximum_usage: [
                (Resource::InputTokens, 1_000),
                (Resource::OutputTokens, 100),
            ]
            .into(),
        }
    }

    #[test]
    fn observed_reads_exactly_the_priced_resources() {
        let priced = priced();
        let body = serde_json::json!({
            "answers": {},
            "usage": {"input_tokens": 3, "output_tokens": 1, "vendor_magic": 9},
        });
        let usage = observed(&priced.price, &body).unwrap();
        assert_eq!(
            usage,
            [(Resource::InputTokens, 3), (Resource::OutputTokens, 1)].into()
        );
        // A missing priced resource or a non-count makes the whole
        // report unpriceable — never a partial or zero charge.
        for usage in [
            serde_json::json!({"input_tokens": 3}),
            serde_json::json!({"input_tokens": 3, "output_tokens": "1"}),
            serde_json::json!({}),
        ] {
            assert!(observed(&priced.price, &serde_json::json!({"usage": usage})).is_none());
        }
        assert!(observed(&priced.price, &serde_json::json!({"answers": {}})).is_none());
    }

    #[test]
    fn observed_total_needs_every_dispatched_report() {
        let priced = priced();
        let one = serde_json::json!({"input_tokens": 3, "output_tokens": 1});
        let two = serde_json::json!({"input_tokens": 4, "output_tokens": 2});
        let usage = observed_total(&priced.price, &[Some(&one), Some(&two)]).unwrap();
        assert_eq!(
            usage,
            [(Resource::InputTokens, 7), (Resource::OutputTokens, 3)].into()
        );
        assert!(observed_total(&priced.price, &[Some(&one), None]).is_none());
        // No reports is no observation — never a zero charge.
        assert!(observed_total(&priced.price, &[]).is_none());
    }
}
