//! Our extensions, following the spec's rules: an optional `openagents`
//! object on requests and responses, and two prefixed stream events
//! (`openagents:route`, `openagents:cost`).
//!
//! Money is a decimal string in US dollars (`"0.0000282"`), as upstreams
//! report it, so no amount passes through a float; sats are whole numbers.

use serde::{Deserialize, Serialize};

use crate::wire::{Extra, open_enum};

open_enum! {
    /// How the router orders candidate upstreams.
    pub enum Sort {
        Quality = "quality",
        Price = "price",
        Latency = "latency",
    }
}

open_enum! {
    /// Data-retention posture an upstream must meet.
    pub enum Privacy {
        Strict = "strict",
        Standard = "standard",
    }
}

open_enum! {
    /// Whose account pays the upstream.
    pub enum Payer {
        Ours = "ours",
        Mine = "mine",
    }
}

/// Routing preferences.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RoutePreferences {
    /// Upstreams to try first, in this order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Upstreams that may serve the request; no others.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub only: Vec<String>,
    /// Upstreams that may not serve the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ignore: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<Sort>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// The most the caller will pay, in US dollars per million tokens.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MaxPrice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// The request's `openagents` object. Every field is optional.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RequestOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<RoutePreferences>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub privacy: Option<Privacy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pay: Option<Payer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_price: Option<MaxPrice>,
    /// Model ids to fall back to, in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

open_enum! {
    /// How one attempt at an upstream ended.
    pub enum AttemptOutcome {
        Ok = "ok",
        Fallback = "fallback",
        Failed = "failed",
        Canceled = "canceled",
    }
}

/// One upstream tried.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    pub model: String,
    pub upstream: String,
    pub outcome: AttemptOutcome,
    /// Milliseconds from sending to the attempt's end.
    pub ms: u64,
    /// Why it fell back or failed, when it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// What a request cost: the upstream's charge, our margin, and the price
/// (their sum), in dollars and sats.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cost {
    pub upstream_usd: String,
    pub margin_usd: String,
    pub price_usd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_sats: Option<u64>,
    #[serde(flatten)]
    pub extra: Extra,
}

impl Cost {
    /// A cost from micros of US dollars, as the meter's rate card prices
    /// an attempt (`meter::card::Priced`).
    #[must_use]
    pub fn from_micros(upstream: u64, margin: u64, price_sats: Option<u64>) -> Self {
        use crate::router::micros_usd;
        Self {
            upstream_usd: micros_usd(upstream),
            margin_usd: micros_usd(margin),
            price_usd: micros_usd(upstream.saturating_add(margin)),
            price_sats,
            extra: Extra::new(),
        }
    }
}

/// The response's `openagents` object.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ResponseInfo {
    /// The model that answered.
    pub model: String,
    /// The upstream account that answered.
    pub upstream: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attempts: Vec<Attempt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `openagents:route`: which model and upstream took the request. Sent
/// before the first output item.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RouteEvent {
    pub model: String,
    pub upstream: String,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `openagents:cost`: the cost object. Sent before the terminal event.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CostEvent {
    pub cost: Cost,
    #[serde(flatten)]
    pub extra: Extra,
}
