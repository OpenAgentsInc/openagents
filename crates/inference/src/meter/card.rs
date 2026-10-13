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
    /// A list price change announced ahead (Gemini 4 Argon's introductory
    /// price ends on a date): from its day on, the row's list prices are
    /// the change's. Every price the row gives is the one in force today.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub change: Option<RateChange>,
}

/// A dated list price change, in micros of the row's currency per million
/// tokens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RateChange {
    /// The first UTC day the new prices apply, `YYYY-MM-DD`.
    pub from: String,
    pub input: u64,
    /// Cached input; defaults to `input`.
    #[serde(default)]
    pub cached_input: Option<u64>,
    pub output: u64,
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

/// The UTC day of `unix_secs`, `YYYY-MM-DD`.
#[must_use]
pub fn utc_day(unix_secs: u64) -> String {
    // Howard Hinnant's civil_from_days.
    let days = i64::try_from(unix_secs / 86_400).unwrap_or(i64::MAX) + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Today's UTC day.
#[must_use]
pub fn today() -> String {
    utc_day(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs()),
    )
}

impl RateRow {
    /// This row as it stands on `day` (`YYYY-MM-DD`): its change applied
    /// once the change's day has come.
    #[must_use]
    pub fn on(&self, day: &str) -> RateRow {
        let mut row = self.clone();
        if let Some(change) = &self.change
            && day >= change.from.as_str()
        {
            row.input = change.input;
            row.cached_input = Some(change.cached_input.unwrap_or(change.input));
            row.output = change.output;
            row.change = None;
        }
        row
    }

    /// This row as it stands today.
    #[must_use]
    pub fn current(&self) -> RateRow {
        self.on(&today())
    }

    /// Price `tokens` at this row's prices in force today, rounding each
    /// amount to the nearest micro.
    pub fn price(&self, tokens: &Tokens) -> Priced {
        match &self.change {
            Some(_) => self.current().price_listed(tokens),
            None => self.price_listed(tokens),
        }
    }

    /// Price `tokens` at this row's prices on `day`.
    pub fn price_on(&self, tokens: &Tokens, day: &str) -> Priced {
        self.on(day).price_listed(tokens)
    }

    fn price_listed(&self, tokens: &Tokens) -> Priced {
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
        let row = self.current();
        [row.input, row.cached_input.unwrap_or(row.input), row.output].map(|list| {
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
            change: None,
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

    /// #11546: Gemini 4 Argon's introductory $2/$10 becomes $4/$20 on
    /// 2026-12-01, by date, with no edit on the day.
    #[test]
    fn a_dated_change_takes_over_on_its_day() {
        let mut row = glm();
        row.input = 2_000_000;
        row.cached_input = Some(100_000);
        row.output = 10_000_000;
        row.change = Some(RateChange {
            from: "2026-12-01".into(),
            input: 4_000_000,
            cached_input: Some(200_000),
            output: 20_000_000,
        });
        let million = Tokens {
            input: 1_000_000,
            output: 1_000_000,
            ..Tokens::default()
        };
        assert_eq!(row.price_on(&million, "2026-11-30").cost, 12_000_000);
        assert_eq!(row.price_on(&million, "2026-12-01").cost, 24_000_000);
        assert_eq!(row.price_on(&million, "2027-01-15").cost, 24_000_000);
        let cached = Tokens {
            input: 1_000_000,
            cached_input: 1_000_000,
            ..Tokens::default()
        };
        assert_eq!(row.price_on(&cached, "2026-11-30").cost, 100_000);
        assert_eq!(row.price_on(&cached, "2026-12-01").cost, 200_000);
        assert_eq!(row.on("2026-12-01").change, None);
        assert_eq!(row.on("2026-11-30"), row);
    }

    #[test]
    fn utc_days_are_calendar_dates() {
        assert_eq!(utc_day(0), "1970-01-01");
        // 2026-11-30T23:59:59Z and the second after.
        assert_eq!(utc_day(1_796_083_199), "2026-11-30");
        assert_eq!(utc_day(1_796_083_200), "2026-12-01");
        assert_eq!(utc_day(951_782_400), "2000-02-29");
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
