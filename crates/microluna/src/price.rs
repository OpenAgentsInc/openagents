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

/// The list-price cost of `usage` on `model`, or `None` when the model has
/// no known price. An unknown price is absent, never zero.
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
}
