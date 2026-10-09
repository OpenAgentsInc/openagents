//! The rate table: what each (upstream, model) costs us per million
//! tokens, and the margin we add (`docs/inference/gateway.md`, section 8).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::Tokens;

/// One row: list prices in micros of `currency` per million tokens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateRow {
    pub upstream: String,
    pub model: String,
    #[serde(default = "usd")]
    pub currency: String,
    /// Uncached input, micros per million tokens.
    pub input: u64,
    /// Cached input; defaults to `input`.
    #[serde(default)]
    pub cached_input: Option<u64>,
    /// Cache write; defaults to `input`.
    #[serde(default)]
    pub cache_write: Option<u64>,
    /// Output (reasoning included).
    pub output: u64,
    /// Our margin in basis points (500 is 5%).
    #[serde(default)]
    pub margin_bps: u32,
    /// A promotion on this row: the caller pays its prices instead of
    /// list plus margin. The rate card shows it as its own labeled row
    /// beside the list row.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<Promotion>,
}

/// A promotional price: what the caller pays, in micros of the row's
/// currency per million tokens, margin included. It never changes the
/// row's list price, which stays our cost.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Promotion {
    /// The line the rate card shows, such as "Free while our launch
    /// credit lasts".
    pub label: String,
    pub input: u64,
    /// Cached input; defaults to `input`.
    #[serde(default)]
    pub cached_input: Option<u64>,
    pub output: u64,
}

fn usd() -> String {
    "USD".into()
}

/// What one attempt cost, in micros of `currency`. `price` is what the
/// caller pays: `cost + margin`, or under a promotion the promotion's
/// price, with `margin` what is left over the cost (zero when the
/// promotion is at or below cost).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Priced {
    pub currency: String,
    pub cost: u64,
    pub margin: u64,
    pub price: u64,
}

impl RateRow {
    /// Price `tokens` at this row, rounding each amount to the nearest micro.
    pub fn price(&self, tokens: &Tokens) -> Priced {
        let cached = tokens.cached_input.min(tokens.input);
        let write = tokens.cache_write.min(tokens.input - cached);
        let plain = tokens.input - cached - write;
        let scaled = plain as u128 * self.input as u128
            + cached as u128 * self.cached_input.unwrap_or(self.input) as u128
            + write as u128 * self.cache_write.unwrap_or(self.input) as u128
            + tokens.output as u128 * self.output as u128;
        let cost = round_div(scaled, 1_000_000);
        if let Some(promotion) = &self.promotion {
            // Cache writes are input at the promotion's price.
            let scaled = (plain + write) as u128 * promotion.input as u128
                + cached as u128 * promotion.cached_input.unwrap_or(promotion.input) as u128
                + tokens.output as u128 * promotion.output as u128;
            let price = round_div(scaled, 1_000_000);
            return Priced {
                currency: self.currency.clone(),
                cost,
                margin: price.saturating_sub(cost),
                price,
            };
        }
        let margin = round_div(cost as u128 * self.margin_bps as u128, 10_000);
        Priced {
            currency: self.currency.clone(),
            cost,
            margin,
            price: cost.saturating_add(margin),
        }
    }

    /// One million tokens of each kind at list price: (list, margin,
    /// price) for input, cached input, and output, the margin rounded as
    /// [`RateRow::price`] rounds it. A promotion is not applied.
    #[must_use]
    pub fn per_million(&self) -> [(u64, u64, u64); 3] {
        [
            self.input,
            self.cached_input.unwrap_or(self.input),
            self.output,
        ]
        .map(|list| {
            let margin = round_div(list as u128 * self.margin_bps as u128, 10_000);
            (list, margin, list.saturating_add(margin))
        })
    }
}

fn round_div(value: u128, by: u128) -> u64 {
    u64::try_from((value + by / 2) / by).unwrap_or(u64::MAX)
}

/// Every row, keyed by (upstream, model).
#[derive(Clone, Debug, Default)]
pub struct RateCard {
    rows: BTreeMap<(String, String), RateRow>,
}

impl RateCard {
    pub fn new(rows: impl IntoIterator<Item = RateRow>) -> Self {
        let mut card = Self::default();
        for row in rows {
            card.set(row);
        }
        card
    }

    /// Add or replace a row.
    pub fn set(&mut self, row: RateRow) {
        self.rows
            .insert((row.upstream.clone(), row.model.clone()), row);
    }

    pub fn get(&self, upstream: &str, model: &str) -> Option<&RateRow> {
        self.rows.get(&(upstream.to_owned(), model.to_owned()))
    }

    pub fn rows(&self) -> impl Iterator<Item = &RateRow> {
        self.rows.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glm() -> RateRow {
        RateRow {
            upstream: "zai".into(),
            model: "zai/glm-5.3-flash".into(),
            currency: "USD".into(),
            input: 150_000,
            cached_input: Some(30_000),
            cache_write: None,
            output: 500_000,
            margin_bps: 500,
            promotion: None,
        }
    }

    #[test]
    fn prices_the_published_example() {
        // One million of each at the spec's GLM row: $0.15 + $0.50.
        let priced = glm().price(&Tokens {
            input: 1_000_000,
            output: 1_000_000,
            ..Tokens::default()
        });
        assert_eq!(priced.cost, 650_000);
        assert_eq!(priced.margin, 32_500);
        assert_eq!(priced.price, 682_500);
    }

    #[test]
    fn cached_input_is_cheaper_and_never_exceeds_input() {
        let row = glm();
        let cached = row.price(&Tokens {
            input: 1_000_000,
            cached_input: 1_000_000,
            ..Tokens::default()
        });
        assert_eq!(cached.cost, 30_000);
        let overclaimed = row.price(&Tokens {
            input: 10,
            cached_input: 1_000_000,
            ..Tokens::default()
        });
        assert_eq!(overclaimed.cost, 0); // 10 * 0.03 micros rounds to 0
    }

    #[test]
    fn a_promotion_sets_the_price_and_keeps_the_cost() {
        let mut row = glm();
        row.promotion = Some(Promotion {
            label: "Free this week".into(),
            input: 0,
            cached_input: None,
            output: 0,
        });
        let priced = row.price(&Tokens {
            input: 1_000_000,
            output: 1_000_000,
            ..Tokens::default()
        });
        assert_eq!((priced.cost, priced.margin, priced.price), (650_000, 0, 0));
        assert_eq!(
            glm().per_million(),
            [
                (150_000, 7_500, 157_500),
                (30_000, 1_500, 31_500),
                (500_000, 25_000, 525_000)
            ]
        );
    }
}
