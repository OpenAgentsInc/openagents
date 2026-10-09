//! The public rate card (`docs/inference/gateway.md`, section 8 and open
//! decisions 1 to 3) and the model catalog's price rows.
//!
//! One source: the meter's [`RateCard`]. The gateway serves
//! [`Card::from_rates`] over its meter's card at `GET /v1/rates`, the
//! catalog at `GET /v1/models` carries the same rows ([`catalog`]), and
//! the website's models page draws whichever card the gateway serves, or
//! [`Card::published`] (every adapter's own rows, the card a gateway
//! with no rate overrides serves) when it cannot reach one.
//!
//! Every row shows, per million tokens of input, cached input, and
//! output: the provider's list price, our margin, and their sum, the
//! price. Prices are US dollars as decimal strings converted from integer
//! micros, with sats beside them when a bitcoin price is set: requests
//! are charged in sats. A promotion is a separate row with its label,
//! never a change to the list row. The provider is named for the caller
//! ([`provider`]); the Pro door's proxy vendor is never named.

use serde::{Deserialize, Serialize};

use crate::meter::{Rate, RateCard, RateRow};
use crate::router::{Offering, micros_usd};
use crate::upstream::{ModelRow, openrouter, pro, vercel, vertex, zai};

/// The schema tag on [`Card`].
pub const SCHEMA: &str = "openagents.rates.v1";

/// The name a caller sees for an upstream id.
#[must_use]
pub fn provider(upstream: &str) -> &str {
    match upstream {
        "vertex" => "Google Vertex AI",
        "zai" => "Z.ai",
        "pro" => "OpenAgents (Pro)",
        "openrouter" => "OpenRouter",
        "vercel" => "Vercel AI Gateway",
        "local" => "Your computer",
        other => other,
    }
}

/// Every adapter's own model rows, with the upstream id that serves each.
#[must_use]
pub fn published_models() -> Vec<(&'static str, ModelRow)> {
    vertex::default_models()
        .into_iter()
        .map(|(row, _)| ("vertex", row))
        .chain(zai::default_models().into_iter().map(|row| ("zai", row)))
        .chain(pro::default_models().into_iter().map(|row| ("pro", row)))
        .chain(
            openrouter::default_models()
                .into_iter()
                .map(|row| ("openrouter", row)),
        )
        .chain(
            vercel::default_models()
                .into_iter()
                .map(|row| ("vercel", row)),
        )
        .collect()
}

/// Every adapter's own price rows at the default margin: the meter's rate
/// card when the gateway's config sets no rows of its own.
#[must_use]
pub fn published() -> RateCard {
    RateCard::new(
        published_models()
            .into_iter()
            .map(|(upstream, row)| row.price.rate_row(upstream, &row.id)),
    )
}

/// The bitcoin price sats are figured at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SatsRate {
    /// Whole US dollars per bitcoin.
    pub usd_per_btc: u64,
    /// When that price was read, as the operator wrote it (a date or a
    /// timestamp).
    pub as_of: String,
}

impl SatsRate {
    /// `micros` of a dollar in sats, to the nearest whole sat.
    #[must_use]
    pub fn sats(&self, micros: u64) -> Option<u64> {
        // sats = micros / 1e6 USD * 1e8 sats per BTC / usd_per_btc.
        let by = u128::from(self.usd_per_btc);
        if by == 0 {
            return None;
        }
        let scaled = u128::from(micros) * 100;
        u64::try_from((scaled + by / 2) / by).ok()
    }
}

/// The rate card: the JSON of `GET /v1/rates`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub v: String,
    /// What every amount is: `USD per million tokens`.
    pub unit: String,
    /// What a request is charged in: `sats`.
    pub charged_in: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sats_rate: Option<SatsRate>,
    pub rows: Vec<Row>,
}

/// A row is the list price or a promotion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    List,
    Promotion,
}

/// One model through one provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    pub model: String,
    /// The provider's name for a caller.
    pub provider: String,
    /// The upstream id `openagents.route` takes.
    pub upstream: String,
    pub kind: Kind,
    /// A promotion's label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Our margin, as a percent (`5`).
    pub margin_percent: String,
    pub input: Amount,
    pub cached_input: Amount,
    pub output: Amount,
}

/// One million tokens of one kind.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Amount {
    /// The provider's list price, dollars.
    pub list_usd: String,
    /// Our margin, dollars.
    pub margin_usd: String,
    /// What the caller pays, dollars: list plus margin, or a promotion's
    /// price.
    pub price_usd: String,
    /// The price in sats at the card's [`SatsRate`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_sats: Option<u64>,
}

/// A basis-point margin as a percent string: 500 is `5`, 550 is `5.5`.
#[must_use]
pub fn percent(bps: u32) -> String {
    let whole = bps / 100;
    let fraction = bps % 100;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{}", format!("{fraction:02}").trim_end_matches('0'))
    }
}

fn amount(list: u64, margin: u64, price: u64, sats: Option<&SatsRate>) -> Amount {
    Amount {
        list_usd: micros_usd(list),
        margin_usd: micros_usd(margin),
        price_usd: micros_usd(price),
        price_sats: sats.and_then(|rate| rate.sats(price)),
    }
}

/// A rate row's card rows: the list row, then its promotion when it has
/// one.
#[must_use]
pub fn rows(row: &RateRow, sats: Option<&SatsRate>) -> Vec<Row> {
    let [input, cached, output] = row.per_million();
    let list = Row {
        model: row.model.clone(),
        provider: provider(&row.upstream).to_owned(),
        upstream: row.upstream.clone(),
        kind: Kind::List,
        label: None,
        margin_percent: percent(row.margin_bps),
        input: amount(input.0, input.1, input.2, sats),
        cached_input: amount(cached.0, cached.1, cached.2, sats),
        output: amount(output.0, output.1, output.2, sats),
    };
    let mut out = vec![list];
    if let Some(promotion) = &row.promotion {
        let promo = |list: u64, price: u64| amount(list, price.saturating_sub(list), price, sats);
        out.push(Row {
            kind: Kind::Promotion,
            label: Some(promotion.label.clone()),
            input: promo(input.0, promotion.input),
            cached_input: promo(cached.0, promotion.cached_input.unwrap_or(promotion.input)),
            output: promo(output.0, promotion.output),
            ..out[0].clone()
        });
    }
    out
}

impl Card {
    /// The card over `rates`, in model order, each model's providers in
    /// upstream order.
    #[must_use]
    pub fn from_rates(rates: &RateCard, sats: Option<&SatsRate>) -> Self {
        let mut ordered: Vec<&RateRow> = rates.rows().collect();
        ordered.sort_by(|a, b| (&a.model, &a.upstream).cmp(&(&b.model, &b.upstream)));
        Self {
            v: SCHEMA.to_owned(),
            unit: "USD per million tokens".to_owned(),
            charged_in: "sats".to_owned(),
            sats_rate: sats.cloned(),
            rows: ordered
                .into_iter()
                .flat_map(|row| rows(row, sats))
                .collect(),
        }
    }

    /// The card over [`published`].
    #[must_use]
    pub fn published(sats: Option<&SatsRate>) -> Self {
        Self::from_rates(&published(), sats)
    }
}

/// `GET /v1/models`: the OpenAI list shape, one entry per model id with an
/// `openagents` object naming each provider that serves it, its
/// capabilities, privacy, rate card rows, and the last hour's live rates.
#[must_use]
pub fn catalog(
    offerings: &[Offering],
    rates: &RateCard,
    live: &[Rate],
    sats: Option<&SatsRate>,
) -> serde_json::Value {
    let mut models: Vec<&str> = offerings.iter().map(|o| o.model.as_str()).collect();
    models.sort_unstable();
    models.dedup();
    let data: Vec<serde_json::Value> = models
        .into_iter()
        .map(|model| {
            let providers: Vec<serde_json::Value> = offerings
                .iter()
                .filter(|o| o.model == model)
                .map(|o| {
                    let prices = rates
                        .get(&o.upstream, &o.model)
                        .map(|row| rows(row, sats))
                        .unwrap_or_default();
                    let speed = live
                        .iter()
                        .find(|rate| rate.upstream == o.upstream && rate.model == o.model);
                    serde_json::json!({
                        "provider": provider(&o.upstream),
                        "upstream": o.upstream,
                        "context": o.capabilities.context,
                        "max_output": o.capabilities.max_output,
                        "capabilities": {
                            "tools": o.capabilities.tools,
                            "json_schema": o.capabilities.json_schema,
                            "images": o.capabilities.images,
                            "reasoning": o.capabilities.reasoning,
                        },
                        "zero_retention": o.zero_retention,
                        "prices": prices,
                        "live": speed.map(|rate| serde_json::json!({
                            "window": "1h",
                            "attempts": rate.attempts,
                            "uptime": rate.uptime,
                            "ttft_p50_ms": rate.ttft_p50_ms,
                            "tokens_per_second_p50": rate.tokens_per_second_p50,
                        })),
                    })
                })
                .collect();
            serde_json::json!({
                "id": model,
                "object": "model",
                "created": 0,
                "owned_by": model.split('/').next().unwrap_or(model),
                "openagents": {"providers": providers},
            })
        })
        .collect();
    serde_json::json!({"object": "list", "data": data})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meter::Promotion;

    #[test]
    fn every_row_shows_list_margin_and_price() {
        let card = Card::published(Some(&SatsRate {
            usd_per_btc: 100_000,
            as_of: "2026-10-09".into(),
        }));
        let glm = card
            .rows
            .iter()
            .find(|row| row.upstream == "zai" && row.model == "zai/glm-5.3-flash")
            .unwrap();
        assert_eq!(glm.provider, "Z.ai");
        assert_eq!(glm.margin_percent, "5");
        assert_eq!(
            (
                glm.input.list_usd.as_str(),
                glm.input.margin_usd.as_str(),
                glm.input.price_usd.as_str()
            ),
            ("0.15", "0.0075", "0.1575")
        );
        assert_eq!(glm.cached_input.price_usd, "0.0315");
        assert_eq!(glm.output.price_usd, "0.525");
        // $0.1575 at $100,000 a bitcoin is 157.5 sats.
        assert_eq!(glm.input.price_sats, Some(158));
        assert!(card.rows.iter().all(|row| row.kind == Kind::List));
        // The Pro door's models are named as ours, never the proxy's vendor.
        for row in card.rows.iter().filter(|row| row.upstream == "pro") {
            assert_eq!(row.provider, "OpenAgents (Pro)");
        }
        let text = serde_json::to_string(&card).unwrap().to_lowercase();
        assert!(!text.contains("stripe"), "{text}");
    }

    #[test]
    fn a_promotion_is_its_own_labeled_row() {
        let mut row = published()
            .get("zai", "zai/glm-5.3-flash")
            .cloned()
            .unwrap();
        row.promotion = Some(Promotion {
            label: "Free while our launch credit lasts".into(),
            input: 0,
            cached_input: None,
            output: 0,
        });
        let card = Card::from_rates(&RateCard::new([row]), None);
        assert_eq!(card.rows.len(), 2);
        assert_eq!(card.rows[0].kind, Kind::List);
        assert_eq!(card.rows[0].input.price_usd, "0.1575");
        assert_eq!(card.rows[1].kind, Kind::Promotion);
        assert_eq!(
            card.rows[1].label.as_deref(),
            Some("Free while our launch credit lasts")
        );
        assert_eq!(card.rows[1].input.list_usd, "0.15");
        assert_eq!(card.rows[1].input.price_usd, "0");
        assert_eq!(card.rows[1].input.price_sats, None);
    }

    #[test]
    fn percents_and_sats() {
        assert_eq!(percent(500), "5");
        assert_eq!(percent(550), "5.5");
        assert_eq!(percent(1_025), "10.25");
        let rate = SatsRate {
            usd_per_btc: 62_500,
            as_of: String::new(),
        };
        assert_eq!(rate.sats(1_000_000), Some(1_600));
        assert_eq!(
            SatsRate {
                usd_per_btc: 0,
                as_of: String::new()
            }
            .sats(1),
            None
        );
    }
}
