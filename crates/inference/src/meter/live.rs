//! Live rates per upstream and model over a rolling window: time to first
//! token (p50, p90, p95), total time (p50, p95), throughput, error rate,
//! uptime, and cost per million tokens (`docs/inference/gateway.md`,
//! section 6).

use std::collections::BTreeMap;

use serde::Serialize;

use super::{Attempt, Outcome};

/// The windows the gateway reports: 5 minutes, 1 hour, 24 hours.
pub const WINDOWS: [(&str, u64); 3] = [
    ("5m", 5 * 60_000),
    ("1h", 60 * 60_000),
    ("24h", 24 * 60 * 60_000),
];

/// One (upstream, model) over one window.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Rate {
    pub upstream: String,
    pub model: String,
    /// Attempts counted (canceled attempts excluded).
    pub attempts: u64,
    pub errors: u64,
    /// `errors / attempts`.
    pub error_rate: f64,
    /// Share of attempts the upstream did not fail on its own side
    /// (refusals of the request itself and rate limits do not count).
    pub uptime: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_p50_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_p90_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttft_p95_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_p50_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_p95_ms: Option<u64>,
    /// Median output tokens per second after the first token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_per_second_p50: Option<f64>,
    pub tokens: u64,
    /// Summed cost, micros of `currency`.
    pub cost: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub currency: String,
    /// Cost per million tokens, micros, when any tokens were priced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_per_million: Option<u64>,
}

/// Rates for every (upstream, model) seen in `attempts`.
pub fn rates<'a>(attempts: impl Iterator<Item = &'a Attempt>) -> Vec<Rate> {
    #[derive(Default)]
    struct Acc {
        attempts: u64,
        errors: u64,
        outages: u64,
        ttft: Vec<u64>,
        total: Vec<u64>,
        tps: Vec<f64>,
        tokens: u64,
        priced_tokens: u64,
        cost: u64,
        currency: String,
    }
    let mut groups: BTreeMap<(&str, &str), Acc> = BTreeMap::new();
    for attempt in attempts {
        if attempt.outcome == Outcome::Canceled {
            continue;
        }
        let acc = groups
            .entry((attempt.upstream.as_str(), attempt.model.as_str()))
            .or_default();
        acc.attempts += 1;
        if attempt.is_error() {
            acc.errors += 1;
            if attempt.error.is_none_or(|class| class.is_outage()) {
                acc.outages += 1;
            }
        }
        if let Some(first) = attempt.first_token_ms {
            acc.ttft.push(first);
        }
        if attempt.outcome == Outcome::Ok {
            acc.total.push(attempt.total_ms);
        }
        if let Some(tps) = attempt.throughput() {
            acc.tps.push(tps);
        }
        acc.tokens += attempt.tokens.total();
        if let Some(cost) = attempt.cost {
            acc.cost = acc.cost.saturating_add(cost);
            acc.priced_tokens += attempt.tokens.total();
            if acc.currency.is_empty() {
                acc.currency.clone_from(&attempt.currency);
            }
        }
    }
    groups
        .into_iter()
        .map(|((upstream, model), mut acc)| {
            acc.ttft.sort_unstable();
            acc.total.sort_unstable();
            acc.tps.sort_by(f64::total_cmp);
            let n = acc.attempts.max(1) as f64;
            Rate {
                upstream: upstream.to_owned(),
                model: model.to_owned(),
                attempts: acc.attempts,
                errors: acc.errors,
                error_rate: acc.errors as f64 / n,
                uptime: 1.0 - acc.outages as f64 / n,
                ttft_p50_ms: percentile(&acc.ttft, 50),
                ttft_p90_ms: percentile(&acc.ttft, 90),
                ttft_p95_ms: percentile(&acc.ttft, 95),
                total_p50_ms: percentile(&acc.total, 50),
                total_p95_ms: percentile(&acc.total, 95),
                tokens_per_second_p50: percentile(&acc.tps, 50),
                tokens: acc.tokens,
                cost: acc.cost,
                currency: acc.currency,
                cost_per_million: (acc.priced_tokens > 0).then(|| {
                    u64::try_from(acc.cost as u128 * 1_000_000 / acc.priced_tokens as u128)
                        .unwrap_or(u64::MAX)
                }),
            }
        })
        .collect()
}

/// Nearest-rank percentile of sorted values.
fn percentile<T: Copy>(sorted: &[T], p: usize) -> Option<T> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (p * sorted.len()).div_ceil(100).max(1);
    sorted.get(rank - 1).copied()
}

#[cfg(test)]
mod tests {
    use super::super::{ErrorClass, Tokens};
    use super::*;

    fn ok(first: u64, total: u64) -> Attempt {
        Attempt {
            first_token_ms: Some(first),
            total_ms: total,
            tokens: Tokens {
                input: 900,
                output: 100,
                ..Tokens::default()
            },
            cost: Some(10),
            currency: "USD".into(),
            ..Attempt::new("r", 1, "zai", "glm", 0)
        }
    }

    #[test]
    fn percentiles_errors_uptime_and_cost() {
        let mut attempts: Vec<Attempt> = (1..=10).map(|i| ok(i * 100, 2_000)).collect();
        let mut down = Attempt::new("r", 1, "zai", "glm", 0);
        down.outcome = Outcome::Fallback;
        down.error = Some(ErrorClass::Server);
        let mut limited = down.clone();
        limited.error = Some(ErrorClass::RateLimited);
        let mut canceled = down.clone();
        canceled.outcome = Outcome::Canceled;
        attempts.extend([down, limited, canceled]);
        let rates = rates(attempts.iter());
        assert_eq!(rates.len(), 1);
        let rate = &rates[0];
        assert_eq!(rate.attempts, 12);
        assert_eq!(rate.errors, 2);
        assert!((rate.error_rate - 2.0 / 12.0).abs() < 1e-9);
        assert!((rate.uptime - 11.0 / 12.0).abs() < 1e-9);
        assert_eq!(rate.ttft_p50_ms, Some(500));
        assert_eq!(rate.ttft_p90_ms, Some(900));
        assert_eq!(rate.ttft_p95_ms, Some(1_000));
        assert_eq!(rate.total_p95_ms, Some(2_000));
        // 100 tokens over 1.5 s at the median.
        assert!(rate.tokens_per_second_p50.unwrap() > 60.0);
        // 10 attempts, 10 micros each, 1,000 tokens each.
        assert_eq!(rate.cost_per_million, Some(10_000));
    }

    #[test]
    fn groups_by_upstream_and_model() {
        let a = ok(100, 200);
        let mut b = ok(100, 200);
        b.upstream = "vertex".into();
        assert_eq!(rates([a, b].iter()).len(), 2);
    }
}
