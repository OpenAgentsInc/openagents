//! The usage record a turn or an episode reports, derived from its
//! trajectory: tokens and cost by component, and the total, with every
//! call's charge. Moved from Coder One's episode module, which re-exports
//! [`usage`].

use atif::document::{Source, Step};
use serde_json::{Value, json};

use crate::component::jev::USD_PER_MILLION_INPUT as JEV_USD_PER_MILLION_INPUT;
use crate::{credentials, delegate};

/// The usage record, derived from the trajectory so both say the same
/// thing. Unreported values are null, never zero. `delegating` adds the
/// delegate component, which a run with delegation off leaves out.
///
/// Every call is one of three charges: `priced`, with a cost; `zero`, a
/// known zero, such as a request never sent or refused before any work;
/// or `unknown`, a call that may have done billed work nobody reported,
/// such as a timed-out request or a session cut off by its deadline. A
/// component's cost is known only when none of its calls is unknown;
/// `cost_lower_bound_usd` sums what is known either way. `ledger` lists
/// every call with its charge and provenance.
pub fn usage(steps: &[Step], delegating: bool) -> Value {
    let generations: Vec<&Step> = steps
        .iter()
        .filter(|step| step.source == Source::Agent && step.call.is_none() && step.tokens.is_some())
        .collect();
    let failed_generation_steps: Vec<&Step> = steps
        .iter()
        .filter(|step| {
            step.source == Source::System && step.message.starts_with("generation failed")
        })
        .collect();
    let failed_generations = failed_generation_steps.len();
    let retries: u64 = generations
        .iter()
        .filter_map(|step| step.extensions.get("attempts").and_then(Value::as_u64))
        .map(|attempts| attempts.saturating_sub(1))
        .sum();

    let gen_input: u64 = generations
        .iter()
        .filter_map(|s| s.tokens)
        .map(|t| t.0)
        .sum();
    let gen_output: u64 = generations
        .iter()
        .filter_map(|s| s.tokens)
        .map(|t| t.1)
        .sum();
    let costs: Vec<Option<u64>> = generations
        .iter()
        .map(|step| step.extensions.get("cost_microusd").and_then(Value::as_u64))
        .collect();
    // A failed generation that may have done billed work leaves the cost
    // unknown; one refused before any work is a known zero.
    let gen_failed_unknown = failed_generation_steps
        .iter()
        .filter(|step| step.extensions.get("charge").and_then(Value::as_str) != Some("zero"))
        .count();
    // No generation at all is a known cost of zero, as in a Jev-brief
    // episode that delegates before the explorer runs; one unreported call
    // leaves the generation cost unknown.
    let priced = costs.iter().all(Option::is_some) && gen_failed_unknown == 0;
    let gen_cost_known_part = costs.iter().flatten().sum::<u64>() as f64 / 1_000_000.0;

    let jev = jev_usage(steps);
    let jev_cost = (jev.unknown == 0).then_some(jev.priced_usd);

    let cached: Vec<Option<u64>> = generations
        .iter()
        .map(|step| step.extensions.get("cached_tokens").and_then(Value::as_u64))
        .collect();
    let gen_cached = if !cached.is_empty() && cached.iter().all(Option::is_some) {
        json!(cached.iter().flatten().sum::<u64>())
    } else {
        Value::Null
    };

    let delegate = delegate_usage(steps);
    let delegations = delegate.dispatches.len();
    let delegate_failed = delegate
        .dispatches
        .iter()
        .filter(|dispatch| dispatch.outcome != atif::document::Outcome::Completed)
        .count();

    let gen_cost_known = priced.then_some(gen_cost_known_part);
    let delegate_cost_known = if delegations == 0 {
        Some(0.0)
    } else {
        delegate.cost_usd()
    };
    let total = match (gen_cost_known, jev_cost, delegate_cost_known) {
        (Some(generation), Some(jev), Some(delegated)) => json!(generation + jev + delegated),
        _ => Value::Null,
    };
    let lower_bound = gen_cost_known_part + jev.priced_usd + delegate.lower_bound_usd();
    let unknown_calls = gen_failed_unknown
        + costs.iter().filter(|cost| cost.is_none()).count()
        + jev.unknown
        + delegate.unknown();
    let delegate_input = if delegations == 0 {
        Some(0)
    } else {
        delegate.total_input()
    };

    let mut components = json!({
        "generation": {
            "input_tokens": gen_input,
            "cached_input_tokens": gen_cached,
            "output_tokens": gen_output,
            "cost_usd": gen_cost_known,
            "cost_lower_bound_usd": gen_cost_known_part,
            "cost_provenance": if priced { "provider_reported" } else { "unknown" },
            "unpriced_calls": costs.iter().filter(|cost| cost.is_none()).count(),
            "failed_calls_unknown_charge": gen_failed_unknown,
        },
        "jev": jev.record(),
    });
    if delegating || delegations > 0 {
        components["delegate"] = delegate.record();
    }

    let mut ledger: Vec<Value> = Vec::new();
    let mut generation_number = 0;
    for step in steps {
        if step.source == Source::Agent && step.call.is_none() && step.tokens.is_some() {
            generation_number += 1;
            let cost = step
                .extensions
                .get("cost_microusd")
                .and_then(Value::as_u64)
                .map(|micro| micro as f64 / 1_000_000.0);
            ledger.push(json!({
                "component": "generation",
                "id": format!("generation-{generation_number}"),
                "name": "generate",
                "model": step.model,
                "charge": if cost.is_some() { "priced" } else { "unknown" },
                "cost_usd": cost,
                "provenance": if cost.is_some() { "provider_reported" } else { "unknown" },
                "basis": if cost.is_some() { "the door reported cost_microusd" } else { "the door reported no cost" },
                "milliseconds": step.milliseconds,
                "input_tokens": step.tokens.map(|t| t.0),
                "output_tokens": step.tokens.map(|t| t.1),
            }));
        } else if step.source == Source::System && step.message.starts_with("generation failed") {
            generation_number += 1;
            let zero = step.extensions.get("charge").and_then(Value::as_str) == Some("zero");
            ledger.push(json!({
                "component": "generation",
                "id": format!("generation-{generation_number}"),
                "name": "generate",
                "model": Value::Null,
                "charge": if zero { "zero" } else { "unknown" },
                "cost_usd": if zero { json!(0.0) } else { Value::Null },
                "provenance": if zero { "none" } else { "unknown" },
                "basis": if zero { "refused or never sent: no billed work" } else { "the request failed; the door may have billed it" },
                "milliseconds": step.milliseconds,
            }));
        } else if let Some(call) = step.call.as_ref().filter(|call| call.is_decision()) {
            let charge = JevCharge::of(step, call);
            ledger.push(json!({
                "component": "jev",
                "id": call.id,
                "name": call.name,
                "model": credentials::JEV_MODEL,
                "outcome": call.outcome,
                "charge": charge.charge,
                "cost_usd": match charge.charge {
                    "priced" => json!(charge.usd()),
                    "zero" => json!(0.0),
                    _ => Value::Null,
                },
                "provenance": match charge.charge {
                    "priced" => "price_estimate",
                    "zero" => "none",
                    _ => "unknown",
                },
                "basis": charge.basis,
                "milliseconds": call.milliseconds,
                "input_tokens": charge.input_tokens,
            }));
        } else if let Some(dispatch) = step.call.as_ref().and_then(|call| Dispatch::of(step, call))
        {
            ledger.push(dispatch.record());
        }
    }

    json!({
        "tokens": {
            "input": match delegate_input {
                Some(delegated) => json!(gen_input + jev.input_tokens + delegated),
                None => Value::Null,
            },
            "cache": if delegations > 0 { json!(delegate.cache_read) } else { gen_cached.clone() },
            "output": match (delegations, delegate.output) {
                (0, _) => json!(gen_output),
                (_, Some(delegated)) => json!(gen_output + delegated),
                _ => Value::Null,
            },
            "note": "input sums generation, Jev, and delegate input tokens, the delegate's cache reads and writes included; cache is the delegate's cache reads when a delegation ran, and generation's reported cached input otherwise",
        },
        "cost": {
            "amount_usd": total,
            "lower_bound_usd": lower_bound,
            "unknown_calls": unknown_calls,
            "provenance": if total.is_null() { "unknown" } else { "mixed" },
            "covers": "generation (provider_reported), jev (price_estimate), and delegate (cli_list_price or cli_reported for Claude Code, price_estimate for Codex); each is under components, and ledger lists every call",
        },
        "calls": {
            "generation": generations.len(),
            "decisions": jev.requests,
            "decisions_skipped": jev.skipped,
            "delegates": delegations,
            "failed": failed_generations + jev.failed + delegate_failed,
            "retries": retries,
        },
        "components": components,
        "ledger": ledger,
    })
}

/// What one Jev call cost.
struct JevCharge {
    charge: &'static str,
    basis: String,
    input_tokens: Option<u64>,
}

impl JevCharge {
    /// Reads the call's `jev_usage`. A record written before charges
    /// were recorded is priced when it carries input tokens; a failed call
    /// with no usage at all is unknown, never zero.
    fn of(step: &Step, call: &atif::document::Call) -> Self {
        let usage = step.extensions.get("jev_usage");
        let input_tokens = usage
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(Value::as_u64);
        let basis = usage
            .and_then(|usage| usage.get("basis"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let charge = match usage
            .and_then(|usage| usage.get("charge"))
            .and_then(Value::as_str)
        {
            Some("priced") if input_tokens.is_some() => "priced",
            Some("zero") => "zero",
            Some(_) => "unknown",
            None if input_tokens.is_some() => "priced",
            None => "unknown",
        };
        let basis = basis.unwrap_or_else(|| {
            match (charge, call.outcome) {
                ("priced", _) => "the response reported its input tokens",
                (_, atif::document::Outcome::Failed) => "the request failed and recorded no usage",
                _ => "the call recorded no usage",
            }
            .to_string()
        });
        Self {
            charge,
            basis,
            input_tokens,
        }
    }

    fn usd(&self) -> f64 {
        self.input_tokens.unwrap_or(0) as f64 * JEV_USD_PER_MILLION_INPUT / 1_000_000.0
    }

    fn skipped(&self) -> bool {
        self.charge == "zero" && self.basis.starts_with("not sent")
    }
}

/// The Jev component, summed over every decision call.
#[derive(Default)]
struct JevUsage {
    requests: usize,
    priced: usize,
    zero: usize,
    unknown: usize,
    skipped: usize,
    failed: usize,
    input_tokens: u64,
    priced_usd: f64,
}

fn jev_usage(steps: &[Step]) -> JevUsage {
    let mut usage = JevUsage::default();
    for step in steps {
        let Some(call) = step.call.as_ref().filter(|call| call.is_decision()) else {
            continue;
        };
        let charge = JevCharge::of(step, call);
        usage.requests += 1;
        match charge.charge {
            "priced" => {
                usage.priced += 1;
                usage.input_tokens += charge.input_tokens.unwrap_or(0);
                usage.priced_usd += charge.usd();
            }
            "zero" => usage.zero += 1,
            _ => usage.unknown += 1,
        }
        if charge.skipped() {
            usage.skipped += 1;
        } else if call.outcome == atif::document::Outcome::Failed {
            usage.failed += 1;
        }
    }
    usage
}

impl JevUsage {
    fn record(&self) -> Value {
        let known = self.unknown == 0;
        json!({
            "model": credentials::JEV_MODEL,
            "requests": self.requests,
            "priced": self.priced,
            "known_zero": self.zero,
            "unknown": self.unknown,
            "skipped": self.skipped,
            "input_tokens": known.then_some(self.input_tokens),
            "input_tokens_priced": self.input_tokens,
            "output_tokens_billed": false,
            "cost_usd": known.then_some(self.priced_usd),
            "cost_lower_bound_usd": self.priced_usd,
            "cost_provenance": if known { "price_estimate" } else { "unknown" },
            "rate": "$0.042 per million input tokens, retrieved 2026-09-22",
        })
    }
}

/// One delegate dispatch, with its own identity and charge.
struct Dispatch<'a> {
    call: &'a atif::document::Call,
    step: &'a Step,
    outcome: atif::document::Outcome,
    charge: &'static str,
    cost_usd: Option<f64>,
    lower_bound_usd: f64,
    provenance: String,
}

impl<'a> Dispatch<'a> {
    fn of(step: &'a Step, call: &'a atif::document::Call) -> Option<Self> {
        if call.name != "delegate"
            || call.extra.get("schema").and_then(Value::as_str) != Some(delegate::CALL_SCHEMA)
        {
            return None;
        }
        let reported = call.extra.get("total_cost_usd").and_then(Value::as_f64);
        // A record written before charges were recorded is priced when it
        // carries a cost.
        let charge = match call.extra.get("charge").and_then(Value::as_str) {
            Some("priced") if reported.is_some() => "priced",
            Some("zero") => "zero",
            Some(_) => "unknown",
            None if reported.is_some() => "priced",
            None => "unknown",
        };
        let partial = call
            .extra
            .get("cost_lower_bound_usd")
            .and_then(Value::as_f64);
        Some(Self {
            call,
            step,
            outcome: call.outcome,
            charge,
            cost_usd: match charge {
                "priced" => reported,
                "zero" => Some(0.0),
                _ => None,
            },
            lower_bound_usd: reported.or(partial).unwrap_or(0.0),
            provenance: match charge {
                "priced" => call
                    .extra
                    .get("cost_provenance")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_string(),
                "zero" => "none".to_string(),
                _ => "unknown".to_string(),
            },
        })
    }

    fn extra(&self, key: &str) -> Value {
        self.call.extra.get(key).cloned().unwrap_or(Value::Null)
    }

    fn usage(&self, key: &str) -> Option<u64> {
        self.call.extra.get("usage")?.get(key)?.as_u64()
    }

    /// Native turns, model calls, and completed items. A record written
    /// before units were separated counted Codex's completed items as
    /// turns; it reads as completed items there.
    fn units(&self) -> (Option<u64>, Option<u64>, Option<u64>) {
        if let Some(units) = self.call.extra.get("units") {
            let read = |key: &str| units.get(key).and_then(Value::as_u64);
            return (
                read("native_turns"),
                read("model_calls"),
                read("completed_items"),
            );
        }
        let turns = self.extra("num_turns").as_u64();
        let calls = self.extra("api_calls").as_u64();
        if self.extra("capability").as_str() == Some("codex") {
            (self.usage("codex_turns"), None, turns)
        } else {
            (turns, calls, None)
        }
    }

    fn record(&self) -> Value {
        let (turns, calls, items) = self.units();
        json!({
            "component": "delegate",
            "id": self.call.id,
            "name": "delegate",
            "agent": self.extra("capability"),
            "model": self.step.model.clone().map_or_else(|| self.extra("model"), Value::String),
            "model_requested": self.extra("model"),
            "credential": self.extra("credential"),
            "status": self.extra("status"),
            "outcome": self.outcome,
            "charge": self.charge,
            "cost_usd": self.cost_usd,
            "cost_lower_bound_usd": self.lower_bound_usd,
            "provenance": self.provenance,
            "basis": self.extra("charge_basis"),
            "milliseconds": self.call.milliseconds,
            "units": {
                "native_turns": turns,
                "model_calls": calls,
                "completed_items": items,
            },
            "deadline": self.extra("deadline"),
        })
    }
}

/// The delegate component, summed over every dispatch.
#[derive(Default)]
struct DelegateUsage<'a> {
    dispatches: Vec<Dispatch<'a>>,
    input: Option<u64>,
    cache_read: Option<u64>,
    cache_creation: Option<u64>,
    output: Option<u64>,
    per_call: Vec<u64>,
}

/// One value when every dispatch agrees, `mixed` when they differ, and
/// `null` with none.
fn agreed(values: &[Value]) -> Value {
    let mut distinct: Vec<&Value> = Vec::new();
    for value in values {
        if !distinct.contains(&value) {
            distinct.push(value);
        }
    }
    match distinct.as_slice() {
        [] => Value::Null,
        [one] => (*one).clone(),
        _ => json!("mixed"),
    }
}

impl DelegateUsage<'_> {
    fn total_input(&self) -> Option<u64> {
        Some(self.input? + self.cache_read? + self.cache_creation?)
    }

    fn unknown(&self) -> usize {
        self.dispatches
            .iter()
            .filter(|dispatch| dispatch.charge == "unknown")
            .count()
    }

    /// The summed cost, known only when no dispatch's charge is unknown.
    fn cost_usd(&self) -> Option<f64> {
        self.dispatches
            .iter()
            .map(|dispatch| dispatch.cost_usd)
            .sum()
    }

    fn lower_bound_usd(&self) -> f64 {
        self.dispatches
            .iter()
            .map(|dispatch| dispatch.lower_bound_usd)
            .sum()
    }

    fn units(&self) -> (Option<u64>, Option<u64>, Option<u64>) {
        let units: Vec<_> = self.dispatches.iter().map(Dispatch::units).collect();
        (
            units.iter().map(|u| u.0).sum(),
            units.iter().map(|u| u.1).sum(),
            units.iter().map(|u| u.2).sum(),
        )
    }

    fn record(&self) -> Value {
        let each = |key: &str| -> Vec<Value> {
            self.dispatches
                .iter()
                .map(|dispatch| dispatch.extra(key))
                .collect()
        };
        let priced: Vec<Value> = self
            .dispatches
            .iter()
            .filter(|dispatch| dispatch.charge == "priced")
            .map(|dispatch| json!(dispatch.provenance))
            .collect();
        let provenance = if self.dispatches.is_empty() {
            json!("none")
        } else if self.unknown() > 0 {
            json!("unknown")
        } else if priced.is_empty() {
            json!("none")
        } else {
            agreed(&priced)
        };
        let cost_note: Vec<Value> = each("cost_note");
        let (turns, calls, items) = self.units();
        let models: Vec<Value> = self
            .dispatches
            .iter()
            .map(|dispatch| {
                dispatch
                    .step
                    .model
                    .clone()
                    .map_or_else(|| dispatch.extra("model"), Value::String)
            })
            .collect();
        json!({
            "agent": agreed(&each("capability")),
            "model": agreed(&each("model")),
            "credential": agreed(&each("credential")),
            "agents": each("capability"),
            "models": models,
            "delegations": self.dispatches.len(),
            "turns": turns,
            "api_calls": calls,
            "units": {
                "native_turns": turns,
                "model_calls": calls,
                "completed_items": items,
            },
            "input_tokens": self.input,
            "cache_read_input_tokens": self.cache_read,
            "cache_creation_input_tokens": self.cache_creation,
            "total_input_tokens": self.total_input(),
            "output_tokens": self.output,
            "input_tokens_per_call": self.per_call,
            "max_input_tokens_per_call": self.per_call.iter().max(),
            "cost_usd": if self.dispatches.is_empty() { json!(0.0) } else { json!(self.cost_usd()) },
            "cost_lower_bound_usd": self.lower_bound_usd(),
            "cost_provenance": provenance,
            "unknown_dispatches": self.unknown(),
            "cost_note": agreed(&cost_note),
            "dispatches": self.dispatches.iter().map(Dispatch::record).collect::<Vec<_>>(),
        })
    }
}

fn delegate_usage(steps: &[Step]) -> DelegateUsage<'_> {
    let dispatches: Vec<Dispatch<'_>> = steps
        .iter()
        .filter_map(|step| step.call.as_ref().and_then(|call| Dispatch::of(step, call)))
        .collect();
    let sum = |read: &dyn Fn(&Dispatch<'_>) -> Option<u64>| -> Option<u64> {
        dispatches.iter().map(read).sum()
    };
    DelegateUsage {
        input: sum(&|d| d.usage("input_tokens")),
        cache_read: sum(&|d| d.usage("cache_read_input_tokens")),
        cache_creation: sum(&|d| d.usage("cache_creation_input_tokens")),
        output: sum(&|d| d.usage("output_tokens")),
        per_call: dispatches
            .iter()
            .filter_map(|d| {
                d.call
                    .extra
                    .get("input_tokens_per_call")?
                    .as_array()
                    .cloned()
            })
            .flatten()
            .filter_map(|value| value.as_u64())
            .collect(),
        dispatches,
    }
}
