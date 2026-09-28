//! Dollars for tokens, at OpenAI's list prices.
//!
//! The rates match `CODEX_PRICES` in `crates/coder-one/src/delegate.rs`,
//! which the operator supplied on 2026-09-22. They're repeated here
//! because Coder One will depend on this crate as an executor adapter,
//! not the other way round.

use crate::transport::TokenUsage;

/// Dollars per million tokens: uncached input, cached input, and output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rates {
    /// Uncached input.
    pub input: f64,
    /// Cached input.
    pub cached: f64,
    /// Output, reasoning included.
    pub output: f64,
}

/// How a dollar figure was reached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// Estimated from reported tokens at list prices: the provider
    /// reported no cost.
    ListPrice,
    /// The cost the provider reported it billed.
    Billed,
}

impl Basis {
    /// `list_price` or `billed`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Basis::ListPrice => "list_price",
            Basis::Billed => "billed",
        }
    }
}

impl std::fmt::Display for Basis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a cost figure says about itself.
pub const COST_NOTE: &str = "A list-price estimate from reported token usage: the \
provider reports no cost. Rates are OpenAI's standard short-context list prices, \
supplied by the operator on 2026-09-22.";

const PRICES: &[(&str, Rates)] = &[
    (
        "gpt-6-astra",
        Rates {
            input: 10.00,
            cached: 1.00,
            output: 50.00,
        },
    ),
    (
        "gpt-6-sol",
        Rates {
            input: 2.00,
            cached: 0.20,
            output: 10.00,
        },
    ),
    (
        "gpt-6-luna",
        Rates {
            input: 0.10,
            cached: 0.01,
            output: 0.50,
        },
    ),
];

/// The rates for `model`, with or without a provider prefix, or `None` for
/// a model with no known price. A `-pro` model costs what its base model
/// does, as OpenRouter listed them on 2026-09-25 (issue #9665).
#[must_use]
pub fn rates(model: &str) -> Option<Rates> {
    let model = model.rsplit('/').next().unwrap_or(model);
    let model = model.strip_suffix("-pro").unwrap_or(model);
    PRICES
        .iter()
        .find(|(name, _)| *name == model)
        .map(|(_, rates)| *rates)
}

/// The most output tokens one response of a priced model may produce,
/// reasoning included: 128,000 for GPT-6 Astra, Sol, and Luna, from
/// OpenAI's model pages (retrieved 2026-09-26).
pub const MAX_OUTPUT_TOKENS: u64 = 128_000;

/// Input tokens above which a whole request is priced at the long-context
/// rates: twice the input and cached-input rates and 1.5 times the output
/// rate. The same for all three models, from the same pages. The bound
/// applies them.
pub const LONG_CONTEXT_INPUT_TOKENS: u64 = 272_000;

/// Bytes of request text per token, at least: an upper bound on tokens
/// divides by this. OpenAI's tokenizers are byte-level BPE, where every
/// token covers at least one byte of UTF-8, so a text of `n` bytes is at
/// most `n` tokens. Real prompts run near 4 bytes a token (a step prompt of
/// 60,136 bytes was 15,751 tokens), so the bound is about four times the
/// truth on input.
pub const BYTES_PER_TOKEN: u64 = 1;

/// Tokens a request may carry beyond its text, added to the bound: message
/// framing, the syntax the provider renders tool declarations in, and any
/// preamble the provider adds.
pub const REQUEST_OVERHEAD_TOKENS: u64 = 4_096;

/// The most output tokens `model` may produce in one response, or `None`
/// for a model with no known price.
#[must_use]
pub fn max_output_tokens(model: &str) -> Option<u64> {
    rates(model).map(|_| MAX_OUTPUT_TOKENS)
}

/// The rates that apply to a request with `input` tokens.
fn rates_for(rates: Rates, input: u64) -> Rates {
    if input > LONG_CONTEXT_INPUT_TOKENS {
        Rates {
            input: rates.input * 2.0,
            cached: rates.cached * 2.0,
            output: rates.output * 1.5,
        }
    } else {
        rates
    }
}

/// The list-price cost of `usage` on `model`, or `None` when the model has
/// no known price. An unknown price is absent, never zero. These are the
/// short-context rates; [`upper_bound`] also applies the long-context ones.
#[must_use]
pub fn cost(model: &str, usage: TokenUsage) -> Option<f64> {
    let rates = rates(model)?;
    Some(
        (usage.uncached() as f64 * rates.input
            + usage.cached as f64 * rates.cached
            + usage.output as f64 * rates.output)
            / 1_000_000.0,
    )
}

/// The most a request of `request_bytes` bytes to `model` could cost at
/// list price, whatever the provider did with it: every input token
/// uncached, at most `request_bytes / BYTES_PER_TOKEN +
/// REQUEST_OVERHEAD_TOKENS` of them, and `max_output` output tokens, or
/// the model's [`max_output_tokens`] when the request sets none. `None`
/// for a model with no known price: its cost has no bound.
#[must_use]
pub fn upper_bound(model: &str, request_bytes: u64, max_output: Option<u64>) -> Option<f64> {
    let input = request_bytes.div_ceil(BYTES_PER_TOKEN) + REQUEST_OVERHEAD_TOKENS;
    let output = max_output.or_else(|| max_output_tokens(model))?;
    let rates = rates_for(rates(model)?, input);
    Some((input as f64 * rates.input + output as f64 * rates.output) / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn luna_costs_its_list_price_with_cached_input_apart() {
        let usage = TokenUsage {
            input: 2_000_000,
            cached: 1_000_000,
            output: 1_000_000,
            reasoning: 0,
        };
        let cost = cost("openai/gpt-6-luna", usage).unwrap();
        assert!((cost - 0.61).abs() < 1e-9);
        assert_eq!(super::cost("gpt-9-unknown", usage), None);
        assert_eq!(super::cost("openai/gpt-6-luna-pro", usage), Some(cost));
    }

    #[test]
    fn the_upper_bound_counts_a_token_a_byte_and_the_whole_output_cap() {
        // 10,000 bytes: at most 14,096 input tokens, and 128,000 output.
        let bound = upper_bound("gpt-6-luna", 10_000, None).unwrap();
        assert!((bound - (14_096.0 * 0.10 + 128_000.0 * 0.50) / 1e6).abs() < 1e-12);
        let capped = upper_bound("gpt-6-luna", 10_000, Some(1_000)).unwrap();
        assert!((capped - (14_096.0 * 0.10 + 1_000.0 * 0.50) / 1e6).abs() < 1e-12);
        // A bound over the long-context threshold uses those rates.
        let long = upper_bound("gpt-6-luna", 300_000, None).unwrap();
        assert!((long - (304_096.0 * 0.20 + 128_000.0 * 0.75) / 1e6).abs() < 1e-12);
        assert_eq!(upper_bound("gpt-9-unknown", 10, None), None);
    }
}
