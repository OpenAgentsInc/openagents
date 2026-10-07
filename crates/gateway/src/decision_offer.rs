//! One configured native decision offer over the existing price and ledgers.
//! Configuration is attributable; production qualification and expense remain
//! unknown until the operator retains their own evidence.

use crate::{money::Priced, purchase, serve::ServeState};
use axum::http::HeaderMap;
use receipts::decision_metering::Metering;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tenancy::money::{Ledger, Resource};
use tenancy::{Binding, Expected, Lane};

pub const SCHEMA: &str = "openagents.decision-offer.v1";
pub const TERMS_SCHEMA: &str = "openagents.decision-offer-terms.v1";
pub const MODEL: &str = "kev-0.6b";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectedOffer {
    pub schema: String,
    pub id: String,
    pub version: String,
    /// The loaded-bytes identity, independently checked against the card on
    /// every dispatch. The retained artifact lock identifies the supported
    /// family; its digest is not a substitute for this runtime content pin.
    pub identity: Expected,
    pub requests_per_minute: u64,
}
impl SelectedOffer {
    pub fn check(&self, priced: &Priced) -> Result<(), String> {
        let valid_id = |v: &str| {
            !v.is_empty()
                && v.len() <= 128
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        };
        let signature = self
            .identity
            .artifact_signature
            .strip_prefix("sha256:")
            .unwrap_or("");
        if self.schema != SCHEMA
            || !valid_id(&self.id)
            || !valid_id(&self.version)
            || self.identity.model != MODEL
            || self.identity.adapter.is_some()
            || signature.len() != 64
            || !signature
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || self.requests_per_minute == 0
            || self.requests_per_minute > 1_000_000
        {
            return Err("The selected offer needs a supported model, exact content pin, and explicit capacity.".into());
        }
        for (key, value) in [("backend", "cpu"), ("dtype", "f32"), ("head_dtype", "f32")] {
            if self.identity.execution.get(key).map(String::as_str) != Some(value) {
                return Err(
                    "The selected offer supports the native CPU f32 Kev execution only.".into(),
                );
            }
        }
        let keys = [
            "backend",
            "dtype",
            "head_dtype",
            "attention",
            "bucket_size",
            "lora_merge",
            "option_isolation",
            "max_state",
            "max_branch",
        ];
        if self.identity.execution.len() != keys.len()
            || keys.iter().any(|key| {
                self.identity
                    .execution
                    .get(*key)
                    .is_none_or(|v| v.is_empty() || v.len() > 128)
            })
        {
            return Err("The selected offer must pin every native execution setting.".into());
        }
        if priced.price.model != MODEL
            || priced.price.capacity != "dedicated"
            || priced.price.policy != crate::money::POLICY
            || priced.price.rates.len() != 1
            || !priced.price.rates.contains_key(&Resource::InputTokens)
            || priced.price.rates[&Resource::InputTokens].millionths == 0
            || priced.maximum_usage.len() != 1
            || priced
                .maximum_usage
                .get(&Resource::InputTokens)
                .is_none_or(|v| *v == 0)
        {
            return Err(
                "The selected offer prices only bounded packed input tokens on its dedicated lane."
                    .into(),
            );
        }
        priced.price.quote(&priced.maximum_usage)?;
        Ok(())
    }
    pub fn check_binding(&self, binding: &Binding) -> Result<(), String> {
        if binding.lane != Lane::Dedicated
            || binding.artifact != self.identity
            || binding.capacity.as_ref().is_none_or(|cap| {
                cap.concurrency != Some(1)
                    || cap.requests_per_minute != Some(self.requests_per_minute)
            })
        {
            return Err(
                "The selected offer does not match the admitted artifact or one-call capacity."
                    .into(),
            );
        }
        Ok(())
    }
    pub fn check_card(&self, priced: &Priced, card: &Value) -> Result<(), String> {
        self.check(priced)?;
        let meter: Metering = serde_json::from_value(card["metering"].clone())
            .map_err(|_| "The selected backend has no compatible metering declaration.")?;
        let maximum = meter.validate_kev_input().map_err(str::to_owned)?;
        if card["limits"]["context_tokens"].as_u64() != Some(maximum)
            || card["limits"]["concurrent_calls"]
                .as_u64()
                .is_none_or(|v| v == 0)
            || maximum != priced.maximum_usage[&Resource::InputTokens]
            || card["batching"]["kind"] != "caller-loop"
        {
            return Err(
                "The backend cannot enforce the selected resource or capacity ceiling.".into(),
            );
        }
        Ok(())
    }
    /// The existing ledger retains full prices in holds. Reject a changed rate
    /// under the same identity/version without creating another price archive.
    pub fn check_history(&self, priced: &Priced, ledger: &Ledger) -> Result<(), String> {
        for workspace in ledger.workspaces() {
            for (_, hold) in ledger.holds(workspace) {
                let old = &hold.price;
                if old.model == priced.price.model
                    && old.capacity == priced.price.capacity
                    && old.version == priced.price.version
                    && old != &priced.price
                {
                    return Err("The selected price version has different retained terms; use a new price version.".into());
                }
            }
        }
        Ok(())
    }
}

/// An authenticated model card exposes exact configured terms for this payer.
/// It never probes inference, reserves funds, or infers live qualification.
pub(crate) async fn terms(
    state: &ServeState,
    headers: &HeaderMap,
    door: &str,
    balance_permitted: bool,
) -> Value {
    let Some(priced) = state.config.money.as_ref().and_then(|m| m.doors.get(door)) else {
        return Value::Null;
    };
    let Some(offer) = &priced.offer else {
        return Value::Null;
    };
    let context = match purchase::current(state, headers, door) {
        Ok(context) => context,
        Err(_) => {
            return json!({"schema": TERMS_SCHEMA, "status":"unavailable", "reason":"Current customer, workspace, resource, or price admission is unavailable."});
        }
    };
    let ledger = if balance_permitted {
        state.money_lock().await
    } else {
        None
    };
    let (position, account) = match ledger {
        Some(ledger) => (
            ledger
                .balance_for_price(&context.workspace, &priced.price)
                .ok(),
            ledger.balance(&context.workspace).ok(),
        ),
        None => (None, None),
    };
    let compatible = account
        .as_ref()
        .map(|p| p.currency == priced.price.currency);
    let spendable = position
        .as_ref()
        .map(|p| p.available.min(p.spend_remaining));
    json!({
        "schema": TERMS_SCHEMA,
        "status": "configured",
        "offer": offer,
        "product": "typed-text-decisions",
        "route": "/v1/systemone",
        "primitives": ["noul", "choice", "score"],
        "price": priced.price,
        "currency_scale": 1_000_000,
        "maximum_usage": priced.maximum_usage,
        "maximum_charge": context.price.maximum_charge,
        "counter": {"wire":"input_tokens", "unit":"tokens", "basis":"packed-input-token-ids", "overlaps":[]},
        "unpriced_counters": ["output_tokens"],
        "capacity": {"lane":"dedicated", "concurrent_calls":1, "requests_per_minute":offer.requests_per_minute},
        "account_currency": account.as_ref().map(|p| &p.currency),
        "currency_compatible": compatible,
        "balance_visibility": if balance_permitted { "permitted" } else { "out-of-scope" },
        "spendable_for_this_price": spendable,
        "outstanding_reserved": account.as_ref().map(|p| p.reserved),
        "funding_admission": {"status": match (compatible, spendable) {
            (Some(false), _) => "currency-incompatible",
            (Some(true), Some(amount)) if amount >= context.price.maximum_charge => "ceiling-affordable",
            (Some(true), Some(_)) => "insufficient-funds",
            _ => "unknown"
        }, "authorizes_dispatch":false},
        "payer_workspace": context.payer_workspace,
        "purchase_context": context,
        "settlement": {"policy":crate::money::POLICY, "rounding":"ceil-per-resource-per-attempt-to-currency-millionth", "unpriceable_dispatch":"full-hold-outstanding-until-evidenced-operator-reconciliation", "pre_dispatch_refusal":"release-fresh-hold", "replay":"same-request-attempt-never-dispatches-twice"},
        "expense": {"provider":{"status":"unknown"}, "hosting":{"status":"unknown"}, "retail_charge_is_expense":false},
        "qualification": {"state":"unknown", "source":"configuration-only", "production_price_provider_funding_result_evidence":"owner-required-O5", "backend_probe_performed":false},
        "limits": ["Decision access supplies no hosted text generation.", "Typed probabilities establish no correctness or execution authority.", "A dedicated registry lane establishes no physically reserved hardware.", "This ceiling is not observed spend, an expense estimate, or proof of purchased funds.", "A model card and receipt are attributable claims, not remote attestation."]
    })
}
