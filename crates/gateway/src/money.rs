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

use serde::{Deserialize, Serialize};
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
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Priced {
    /// Optional supported native offer. Absent preserves legacy monetary
    /// behavior without implying a commercially qualified product.
    #[serde(default)]
    pub offer: Option<crate::decision_offer::SelectedOffer>,
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
    /// This attempt already has a durable hold and cannot dispatch again.
    Duplicate,
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
            Self::Duplicate => write!(
                f,
                "This request and attempt were already charged. Send a new `X-Attempt` number, or a new `Idempotency-Key` for new work."
            ),
            Self::Funds(message) | Self::Price(message) | Self::Ledger(message) => {
                write!(f, "{message}")
            }
        }
    }
}

/// The hold one admitted call stands under — what a later settle,
/// release, or unknown marker names.
pub struct Hold {
    pub offer: Option<crate::decision_offer::SelectedOffer>,
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
/// An existing attempt refuses redispatch. Its original quota and monetary
/// reservations stay intact until the original execution or reconciliation
/// resolves them.
pub fn reserve(
    ledger: &mut Ledger,
    workspace: &str,
    request: &str,
    attempt: u32,
    request_digest: &str,
    priced: &Priced,
    binding: (&str, &str),
) -> Result<Hold, Refusal> {
    let (model, capacity) = binding;
    let key = format!("{request}#{attempt}");
    if ledger.hold(workspace, &key).is_some() {
        return Err(Refusal::Duplicate);
    }
    if let Some(offer) = &priced.offer {
        offer.check(priced).map_err(Refusal::Price)?;
    }
    let price = &priced.price;
    if price.policy != POLICY {
        return Err(Refusal::Price(format!(
            "The model's price uses the pricing policy `{}`, which this service doesn't support (it supports `{POLICY}`). Contact the operator.",
            price.policy
        )));
    }
    if price.model != model {
        return Err(Refusal::Price(format!(
            "The model's price is set for `{}`, not for the model `{model}` that serves it. Contact the operator.",
            price.model
        )));
    }
    if price.capacity != capacity {
        return Err(Refusal::Price(format!(
            "The model's price is set for `{}` capacity, not the `{capacity}` capacity it runs at. Contact the operator.",
            price.capacity
        )));
    }
    let worst = price.quote(&priced.maximum_usage).map_err(Refusal::Price)?;
    let balance = ledger
        .balance_for_price(workspace, price)
        .map_err(|cause| match classify(cause) {
            Refusal::Funds(_) => Refusal::Funds(format!(
                "Workspace `{workspace}` has no billing balance. Add credit before you make paid calls."
            )),
            other => other,
        })?;
    if worst > balance.available.min(balance.spend_remaining) {
        return Err(Refusal::Funds(format!(
            "Workspace `{workspace}` doesn't have enough credit for this call. Each call \
             sets aside its highest possible cost, {worst} millionths of {}, and the \
             workspace has {} available with {} left under its spend limit. Add credit \
             or raise the spend limit.",
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
        offer: priced.offer.clone(),
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
    // Reconciliation may revisit settlement. Preserve a terminal result.
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
    // Preserve a terminal settlement or unresolved dispatched work.
    if let Some(existing) = ledger.hold(&hold.workspace, &hold.attempt) {
        match existing.phase {
            Phase::Settled => return Settlement::Settled,
            Phase::Released => return Settlement::Released,
            Phase::Unknown => return Settlement::Outstanding,
            Phase::Held => {}
        }
    }
    let result = ledger.apply(Mutation {
        workspace: hold.workspace.clone(),
        source: format!("gateway:{}:release", hold.attempt),
        audit: hold.attempt.clone(),
        operation: Operation::Release {
            attempt: hold.attempt.clone(),
        },
    });
    if result.is_ok() {
        Settlement::Released
    } else {
        Settlement::Outstanding
    }
}

/// The usage one response reports under the price's resources — `None`
/// when any priced resource is unreported or reports a non-count. A
/// report naming other resources settles under the priced set alone.
pub fn observed(price: &Price, body: &Value) -> Option<Usage> {
    observed_total(price, &[body.get("usage")])
}

/// A selected native completion must still name the model that was admitted.
/// A conflicting response is dispatched work with unknown liability.
pub fn observed_hold(hold: &Hold, body: &Value) -> Option<Usage> {
    if hold.offer.is_some() && body["model"].as_str() != Some(hold.price.model.as_str()) {
        return None;
    }
    observed(&hold.price, body)
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
            offer: None,
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
    fn failed_release_does_not_claim_funds_were_returned() {
        let root = tempfile::tempdir().unwrap();
        let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
        let hold = Hold {
            offer: None,
            workspace: "missing-workspace".into(),
            attempt: "missing-attempt".into(),
            price: priced().price,
        };
        assert_eq!(release(&mut ledger, &hold), Settlement::Outstanding);
    }

    #[test]
    fn reserve_excludes_other_product_trials_and_release_does_not_restart_them() {
        use tenancy::money::funding::{
            self, Finality, Policy, Promotion, PromotionTerms, PurchaseTerms, SpentCreditLoss, Unit,
        };

        let root = tempfile::tempdir().unwrap();
        let mut ledger = Ledger::open(&root.path().join("money.jsonl")).unwrap();
        let mut apply = |source: &str, operation| {
            ledger
                .apply(Mutation {
                    workspace: "buyer".into(),
                    source: source.into(),
                    audit: format!("fixture:{source}"),
                    operation,
                })
                .unwrap();
        };
        apply(
            "create",
            Operation::Create {
                currency: "USD".into(),
                spend_limit: 100_000,
                topups_allowed: false,
            },
        );
        let mut policy = Policy {
            schema: funding::POLICY_SCHEMA.into(),
            version: "other-product-trial-v1".into(),
            unit: Unit::CurrencyMillionths {
                currency: "USD".into(),
            },
            conversions: Vec::new(),
            purchases: PurchaseTerms {
                required_finality: Finality::Final,
                refunds_allowed: false,
                disputes_allowed: false,
                spent_credit_loss: SpentCreditLoss::Operator,
            },
            promotions: PromotionTerms {
                total_cap: 28_000,
                grant_cap: 14_000,
                max_lifetime_seconds: 600,
                max_admissions: 1,
                price_policies: ["other-product-v1".into()].into(),
                reversible: true,
            },
        };
        apply(
            "policy",
            Operation::FundingPolicy {
                policy: policy.clone(),
            },
        );
        let expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 600;
        apply(
            "other-trial",
            Operation::Promotion {
                grant: Promotion {
                    id: "other-trial".into(),
                    origin: "synthetic-other-campaign".into(),
                    policy: policy.version.clone(),
                    amount: 14_000,
                    expires_at,
                },
            },
        );
        // The aggregate balance has credit for another admitted product, but
        // the gateway cannot spend it under observed-usage-v1.
        assert_eq!(ledger.balance("buyer").unwrap().available, 14_000);
        let mut wrong_currency = priced();
        wrong_currency.price.currency = "EUR".into();
        assert!(matches!(
            reserve(
                &mut ledger,
                "buyer",
                "wrong-currency",
                1,
                "digest",
                &wrong_currency,
                ("kev-0.6b", "dedicated"),
            ),
            Err(Refusal::Price(_))
        ));
        assert!(ledger.hold("buyer", "wrong-currency#1").is_none());
        let result = reserve(
            &mut ledger,
            "buyer",
            "denied",
            1,
            "digest",
            &priced(),
            ("kev-0.6b", "dedicated"),
        );
        assert!(matches!(result, Err(Refusal::Funds(_))));
        assert!(ledger.hold("buyer", "denied#1").is_none());
        policy.version = "gateway-trial-v1".into();
        policy.promotions.price_policies = [POLICY.into()].into();
        ledger
            .apply(Mutation {
                workspace: "buyer".into(),
                source: "gateway-policy".into(),
                audit: "fixture:gateway-policy".into(),
                operation: Operation::FundingPolicy {
                    policy: policy.clone(),
                },
            })
            .unwrap();
        ledger
            .apply(Mutation {
                workspace: "buyer".into(),
                source: "gateway-trial".into(),
                audit: "fixture:gateway-trial".into(),
                operation: Operation::Promotion {
                    grant: Promotion {
                        id: "gateway-trial".into(),
                        origin: "synthetic-gateway-campaign".into(),
                        policy: policy.version,
                        amount: 14_000,
                        expires_at,
                    },
                },
            })
            .unwrap();
        let hold = reserve(
            &mut ledger,
            "buyer",
            "accepted",
            1,
            "digest",
            &priced(),
            ("kev-0.6b", "dedicated"),
        )
        .unwrap();
        assert_eq!(
            ledger.hold("buyer", "accepted#1").unwrap().allocations[0].lot,
            "gateway-trial"
        );
        assert_eq!(release(&mut ledger, &hold), Settlement::Released);
        assert!(matches!(
            reserve(
                &mut ledger,
                "buyer",
                "released-trial",
                1,
                "digest",
                &priced(),
                ("kev-0.6b", "dedicated")
            ),
            Err(Refusal::Funds(_))
        ));
        assert!(ledger.hold("buyer", "released-trial#1").is_none());
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
