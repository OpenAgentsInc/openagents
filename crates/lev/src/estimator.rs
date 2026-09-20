//! Turning observable behavior into a distribution.
//!
//! Apple's runtime returns a selected option. It returns no logits, no
//! log-probabilities, and no hidden states, so there is no distribution to
//! read. These estimators manufacture one:
//!
//! - **L1** takes one greedy call and returns the choice alone. It produces
//!   no distribution, so it cannot serve the contract: `jev::NoulAnswer`
//!   carries only a probability, and both other answer types require a
//!   `confidence`. L1 is an internal fast path for callers that want a typed
//!   choice and nothing else.
//! - **L2** takes `N` seeded samples and counts them. Its resolution is
//!   `1/N`, which is carried with the estimate rather than rounded away. The
//!   seeds come from a block a caller names, because a fixed seed set has no
//!   trial-to-trial variance to average over; see [`seed_block`].
//! - **L3** takes one call that also selects an ordered certainty band.
//!
//! None of these is a calibrated predictive distribution, and that is the
//! whole reason `calibrate` exists. L2 measures decoding entropy: how
//! consistently the model answers, not how often it is right. A model can be
//! wrong at every seed and report a unanimous 1.00.

use indexmap::IndexMap;

use crate::api::{Answer, MAX_SCORE_LEVELS};
use crate::bridge::{Bridge, Call, Pool, Sampling};
use crate::error::{Refusal, RefusalCode, Result};
use crate::schema::{BANDS, Compiled, Kind};
use serde_json::Value;

/// Which estimator produced a raw signal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Estimator {
    /// One greedy call, no distribution.
    L1,
    /// `N` seeded samples, counted.
    L2,
    /// One call that also selects a certainty band.
    L3,
}

impl Estimator {
    /// The stable wire label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::L1 => "l1",
            Self::L2 => "l2",
            Self::L3 => "l3",
        }
    }
}

/// What an estimator observed, before any calibration is applied.
///
/// This is deliberately not called a probability. It is a raw signal, and
/// `crate::calibrate` is what turns it into a number a caller may act on.
#[derive(Clone, Debug, PartialEq)]
pub struct Raw {
    /// Which estimator ran.
    pub estimator: Estimator,
    /// The option that won.
    pub choice: String,
    /// The observed frequency per option, over the admitted set in order.
    pub frequency: IndexMap<String, f64>,
    /// The smallest difference this estimate can express: `1/N` for L2.
    pub resolution: f64,
    /// The seeds that produced it, in order.
    pub seeds: Vec<u64>,
    /// The block those seeds were drawn from.
    pub seed_base: u64,
    /// The certainty band, for L3.
    pub band: Option<String>,
    /// Draws the runtime refused, usually on a guardrail.
    pub refused: u64,
    /// Total time the calls took inside the helper.
    pub latency_ms: f64,
}

impl Raw {
    /// The frequency the winning option carried.
    #[must_use]
    pub fn top(&self) -> f64 {
        self.frequency
            .get(&self.choice)
            .copied()
            .unwrap_or_default()
    }
}

/// Runs one greedy call and returns the choice with no distribution.
pub fn l1(bridge: &mut Bridge, compiled: &Compiled) -> Result<Raw> {
    let outcome = bridge.decide(&Call::decide(compiled, Sampling::Greedy))?;
    let choice = outcome.choice.ok_or_else(|| {
        Refusal::new(
            RefusalCode::DecodingFailure,
            "the runtime selected no option",
        )
    })?;
    Ok(Raw {
        estimator: Estimator::L1,
        choice,
        frequency: IndexMap::new(),
        resolution: 1.0,
        seeds: Vec::new(),
        seed_base: 0,
        band: None,
        refused: 0,
        latency_ms: outcome.latency_ms.unwrap_or_default(),
    })
}

/// The block of seeds `seed_base` names, for an ensemble of `n` draws.
///
/// Block `b` draws `b * n .. b * n + n`, so two different bases never share
/// a seed, and block 0 draws `0..n` — the seeds every number in
/// `docs/lev/measurements/` was produced with.
///
/// The block exists because a seed reproduces its sample: 16 of 16 in one
/// process and across a fresh one, in the behavior record under
/// `docs/lev/measurements/`. That is what makes an L2 estimate replayable,
/// and it also means that running the same block again is not a fresh trial.
/// It is the same arithmetic. A caller that wants independent evidence — a
/// confirmation run, a resampling spread, an effect size — asks for a block
/// it has not drawn yet.
pub fn seed_block(seed_base: u64, n: u64) -> Result<std::ops::Range<u64>> {
    if n == 0 {
        return Err(Refusal::new(
            RefusalCode::InvalidRequest,
            "an ensemble draws at least one sample",
        ));
    }
    let overflow = || {
        Refusal::new(
            RefusalCode::InvalidRequest,
            format!("seed base {seed_base} over {n} samples runs past the seed space"),
        )
    };
    let start = seed_base.checked_mul(n).ok_or_else(overflow)?;
    let end = start.checked_add(n).ok_or_else(overflow)?;
    Ok(start..end)
}

/// Runs `n` seeded samples and counts them.
///
/// The seeds are the block [`seed_block`] gives for `seed_base`, recorded
/// with the estimate so the same numbers can be produced again. Whether the
/// runtime honors a seed is a measured fact, not an assumption; see
/// `docs/lev/measurements/`.
pub fn l2(bridge: &mut Bridge, compiled: &Compiled, n: u64, seed_base: u64) -> Result<Raw> {
    let block = seed_block(seed_base, n)?;
    let mut counts: IndexMap<String, u64> = compiled
        .options
        .iter()
        .map(|option| (option.clone(), 0))
        .collect();
    let mut seeds = Vec::with_capacity(n as usize);
    let mut latency = 0.0;

    // A guardrail can fire on one draw and not another, so an ensemble of N
    // draws is N chances to trip it. The comparison run caught this on a
    // renewal question: seven draws answered and one came back unsafe. A
    // single refusal must not lose the other seven, so a minority of refused
    // draws is recorded and skipped, and only an ensemble that refuses
    // outright refuses the question.
    let mut refused = 0_u64;
    let mut last: Option<Refusal> = None;
    for seed in block {
        let call = Call::decide(
            compiled,
            Sampling::Random {
                seed,
                temperature: None,
            },
        );
        let outcome = match bridge.decide(&call) {
            Ok(outcome) => outcome,
            Err(refusal) if refusal.code == RefusalCode::Guardrail => {
                refused += 1;
                last = Some(refusal);
                continue;
            }
            Err(refusal) => return Err(refusal),
        };
        let choice = outcome.choice.ok_or_else(|| {
            Refusal::new(
                RefusalCode::DecodingFailure,
                "the runtime selected no option",
            )
        })?;
        let slot = counts.get_mut(&choice).ok_or_else(|| {
            Refusal::new(
                RefusalCode::DecodingFailure,
                format!("the runtime selected '{choice}', which is not an admitted option"),
            )
        })?;
        *slot += 1;
        seeds.push(seed);
        latency += outcome.latency_ms.unwrap_or_default();
    }

    let drawn = n - refused;
    if drawn == 0 {
        return Err(
            last.unwrap_or_else(|| Refusal::new(RefusalCode::Guardrail, "every draw was refused"))
        );
    }
    let mut raw = finish(
        Estimator::L2,
        counts,
        drawn,
        seeds,
        seed_base,
        None,
        latency,
    );
    raw.refused = refused;
    Ok(raw)
}

/// Runs `n` seeded samples across a pool of helpers.
///
/// Identical to [`l2`] in what it computes and in the seeds it records; the
/// only difference is that the draws run concurrently. The same seeds produce
/// the same estimate either way, which `tests/pool.rs` checks rather than
/// assumes.
pub fn l2_pool(pool: &Pool, compiled: &Compiled, n: u64, seed_base: u64) -> Result<Raw> {
    l2_pool_with(pool, compiled, n, seed_base, None)
}

/// Runs `n` seeded samples across a pool, optionally through an adapter.
pub fn l2_pool_with(
    pool: &Pool,
    compiled: &Compiled,
    n: u64,
    seed_base: u64,
    adapter: Option<&str>,
) -> Result<Raw> {
    let block = seed_block(seed_base, n)?;
    let first = block.start;
    let calls: Vec<Call> = block
        .map(|seed| {
            let call = Call::decide(
                compiled,
                Sampling::Random {
                    seed,
                    temperature: None,
                },
            );
            match adapter {
                Some(path) => call.with_adapter(path),
                None => call,
            }
        })
        .collect();
    let outcomes = pool.decide_all(&calls);

    let mut counts: IndexMap<String, u64> = compiled
        .options
        .iter()
        .map(|option| (option.clone(), 0))
        .collect();
    let mut seeds = Vec::new();
    let mut latency = 0.0_f64;
    let mut refused = 0_u64;
    let mut last: Option<Refusal> = None;

    for (index, outcome) in outcomes.into_iter().enumerate() {
        match outcome {
            Ok(outcome) => {
                let choice = outcome.choice.ok_or_else(|| {
                    Refusal::new(
                        RefusalCode::DecodingFailure,
                        "the runtime selected no option",
                    )
                })?;
                let slot = counts.get_mut(&choice).ok_or_else(|| {
                    Refusal::new(
                        RefusalCode::DecodingFailure,
                        format!("the runtime selected '{choice}', which is not an admitted option"),
                    )
                })?;
                *slot += 1;
                seeds.push(first + index as u64);
                // The lanes overlap, so this sums helper time rather than
                // wall clock. Wall clock is what the caller measures.
                latency += outcome.latency_ms.unwrap_or_default();
            }
            Err(refusal) if refusal.code == RefusalCode::Guardrail => {
                refused += 1;
                last = Some(refusal);
            }
            Err(refusal) => return Err(refusal),
        }
    }

    let drawn = n - refused;
    if drawn == 0 {
        return Err(
            last.unwrap_or_else(|| Refusal::new(RefusalCode::Guardrail, "every draw was refused"))
        );
    }
    let mut raw = finish(
        Estimator::L2,
        counts,
        drawn,
        seeds,
        seed_base,
        None,
        latency,
    );
    raw.refused = refused;
    Ok(raw)
}

/// Runs one call that also selects an ordered certainty band.
pub fn l3(bridge: &mut Bridge, compiled: &Compiled) -> Result<Raw> {
    let bands: Vec<String> = BANDS.iter().map(|band| (*band).to_string()).collect();
    let call = Call::decide(compiled, Sampling::Greedy).with_band(bands);
    let outcome = bridge.decide(&call)?;
    let choice = outcome.choice.ok_or_else(|| {
        Refusal::new(
            RefusalCode::DecodingFailure,
            "the runtime selected no option",
        )
    })?;
    let mut counts: IndexMap<String, u64> = compiled
        .options
        .iter()
        .map(|option| (option.clone(), 0))
        .collect();
    if let Some(slot) = counts.get_mut(&choice) {
        *slot = 1;
    }
    let latency = outcome.latency_ms.unwrap_or_default();
    Ok(finish(
        Estimator::L3,
        counts,
        1,
        Vec::new(),
        0,
        outcome.band,
        latency,
    ))
}

fn finish(
    estimator: Estimator,
    counts: IndexMap<String, u64>,
    n: u64,
    seeds: Vec<u64>,
    seed_base: u64,
    band: Option<String>,
    latency_ms: f64,
) -> Raw {
    let total = n as f64;
    let frequency: IndexMap<String, f64> = counts
        .iter()
        .map(|(option, count)| (option.clone(), *count as f64 / total))
        .collect();
    let choice = counts
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(option, _)| option.clone())
        .unwrap_or_default();
    Raw {
        estimator,
        choice,
        frequency,
        resolution: 1.0 / total,
        seeds,
        seed_base,
        band,
        refused: 0,
        latency_ms,
    }
}

/// How sharp a distribution is, as TypeSafe's own adapter computes it and as
/// kev reproduces it: how far the leader stands above uniform.
#[must_use]
pub fn confidence(probabilities: &IndexMap<String, f64>) -> f64 {
    let k = probabilities.len();
    if k < 2 {
        return 0.0;
    }
    let top = probabilities.values().copied().fold(0.0_f64, f64::max);
    let uniform = 1.0 / k as f64;
    ((top - uniform) / (1.0 - uniform)).clamp(0.0, 1.0)
}

/// The same sharpness, read on a named option rather than on the leader.
///
/// A calibration map can leave the option the estimator selected holding
/// less than a runner-up, so "how far the leader stands above uniform" and
/// "how far the answer stands above uniform" are two numbers. A door reports
/// the second, and it floors at zero rather than going negative: an answer
/// below uniform carries no confidence, and there is no such thing as less
/// than none.
#[must_use]
pub fn confidence_in(probabilities: &IndexMap<String, f64>, option: &str) -> f64 {
    let k = probabilities.len();
    if k < 2 {
        return 0.0;
    }
    let held = probabilities.get(option).copied().unwrap_or(0.0);
    let uniform = 1.0 / k as f64;
    ((held - uniform) / (1.0 - uniform)).clamp(0.0, 1.0)
}

/// Builds the contract answer from a calibrated distribution.
///
/// The distribution that arrives here has already been through a calibration
/// map. The derivations are identical to kev's, because two implementations
/// of one contract should agree on what a question means.
///
/// `selected` is the option the estimator chose, read from the **raw**
/// distribution before any map touched it. A map calibrates how sure a door
/// is about a fixed answer and never picks a different one, and a rescale
/// can leave the selected option below a runner-up, so this is passed in
/// rather than re-derived here — and it goes out on the wire: `choice`
/// carries it on a Choice and `selected` carries it on a Noul or a Score,
/// which have no other field that can. `crates/gym/src/calibrate.rs` carries
/// the contract.
pub fn answer(
    kind: Kind,
    probabilities: &IndexMap<String, f64>,
    legend: &IndexMap<String, Value>,
    selected: &str,
) -> Result<Answer> {
    if !probabilities.contains_key(selected) {
        return Err(Refusal::new(
            RefusalCode::DecodingFailure,
            format!("the distribution names no '{selected}'"),
        ));
    }
    match kind {
        Kind::Noul => {
            let yes = probabilities.get("yes").copied().ok_or_else(|| {
                Refusal::new(
                    RefusalCode::DecodingFailure,
                    "a Noul distribution names no 'yes'",
                )
            })?;
            Ok(Answer::Noul {
                noul: yes,
                selected: Some(selected.to_string()),
            })
        }
        Kind::Choice => Ok(Answer::Choice {
            choice: selected.to_string(),
            confidence: Some(confidence_in(probabilities, selected)),
            probabilities: Some(probabilities.clone()),
        }),
        Kind::Score => {
            let mut score = 0.0;
            for (key, probability) in probabilities {
                let level: u32 = key.parse().map_err(|_| {
                    Refusal::new(
                        RefusalCode::DecodingFailure,
                        format!("a Score distribution is keyed by '{key}', which is not a level"),
                    )
                })?;
                if level as usize >= MAX_SCORE_LEVELS {
                    return Err(Refusal::new(
                        RefusalCode::DecodingFailure,
                        format!("a Score distribution names level {level}"),
                    ));
                }
                score += f64::from(level) * probability;
            }
            Ok(Answer::Score {
                score,
                confidence: Some(confidence(probabilities)),
                selected: Some(selected.to_string()),
                legend: legend.clone(),
                probabilities: Some(probabilities.clone()),
            })
        }
    }
}

/// The option a distribution selects: its argmax, with equal leaders
/// resolving to the last of them.
///
/// This is `gym::calibrate::selected` on the serving side, and the two agree
/// by construction. A door reads it from the raw estimator distribution.
///
/// # Errors
///
/// Returns a decoding refusal for an empty distribution, which is not an
/// answer.
pub fn argmax(probabilities: &IndexMap<String, f64>) -> Result<String> {
    probabilities
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(option, _)| option.clone())
        .ok_or_else(|| Refusal::new(RefusalCode::DecodingFailure, "an empty distribution"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn distribution(pairs: &[(&str, f64)]) -> IndexMap<String, f64> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), *value))
            .collect()
    }

    #[test]
    fn confidence_is_distance_above_uniform() {
        let uniform = distribution(&[("a", 0.5), ("b", 0.5)]);
        assert!((confidence(&uniform) - 0.0).abs() < 1e-12);

        let certain = distribution(&[("a", 1.0), ("b", 0.0)]);
        assert!((confidence(&certain) - 1.0).abs() < 1e-12);

        // Four options, leader at 0.85: (0.85 - 0.25) / 0.75 = 0.8.
        let four = distribution(&[("a", 0.85), ("b", 0.05), ("c", 0.05), ("d", 0.05)]);
        assert!((confidence(&four) - 0.8).abs() < 1e-12);
    }

    #[test]
    fn a_noul_answer_is_the_probability_of_yes() {
        let probabilities = distribution(&[("no", 0.08), ("yes", 0.92)]);
        let answer = answer(Kind::Noul, &probabilities, &IndexMap::new(), "yes").unwrap();
        assert_eq!(
            answer,
            Answer::Noul {
                noul: 0.92,
                selected: Some("yes".to_string())
            }
        );
    }

    #[test]
    fn a_score_is_the_probability_weighted_mean() {
        let probabilities = distribution(&[("0", 0.05), ("1", 0.30), ("2", 0.65)]);
        let mut legend = IndexMap::new();
        legend.insert("0".to_string(), json!("Calm"));
        legend.insert("1".to_string(), json!("Frustrated"));
        legend.insert("2".to_string(), json!("Very angry"));
        let Answer::Score { score, .. } =
            answer(Kind::Score, &probabilities, &legend, "2").unwrap()
        else {
            panic!("expected a Score");
        };
        assert!((score - 1.6).abs() < 1e-12);
    }

    #[test]
    fn a_tied_argmax_resolves_to_the_last_level_listed() {
        // The convention shared by `gym::calibrate::selected` and this
        // estimator: equal leaders resolve to the
        // last of them, which for a Score's ordered levels is the highest
        // tied level. An 8-sample grid makes exact halves common — the
        // distribution below is what `ramp/search-box/3` returned — and the
        // tie is a fact about the distribution, not a refusal.
        let tied = distribution(&[("0", 0.0), ("1", 0.0), ("2", 0.5), ("3", 0.5), ("4", 0.0)]);
        assert_eq!(argmax(&tied).unwrap(), "3");
        assert_eq!(gym::calibrate::tied(&tied), 2);
    }

    #[test]
    fn a_choice_answer_takes_the_selected_option() {
        let probabilities =
            distribution(&[("billing", 0.08), ("technical", 0.85), ("sales", 0.07)]);
        let selected = argmax(&probabilities).unwrap();
        let Answer::Choice {
            choice, confidence, ..
        } = answer(Kind::Choice, &probabilities, &IndexMap::new(), &selected).unwrap()
        else {
            panic!("expected a Choice");
        };
        assert_eq!(choice, "technical");
        assert!(confidence.expect("a confidence") > 0.7);
    }

    #[test]
    fn a_map_that_sinks_the_selected_option_does_not_change_the_answer() {
        // The contract in `gym::calibrate`: a map calibrates how sure a door
        // is about a fixed answer and never picks a different one. The
        // estimator chose `yes` at 0.8; a map reading that signal at 0.25
        // leaves `no` holding the larger share of the rescaled distribution,
        // and the door still answers `yes` — at a confidence of zero, which
        // is what a caller's refusal threshold reads.
        let raw = distribution(&[("yes", 0.8), ("no", 0.2)]);
        let selected = argmax(&raw).unwrap();
        assert_eq!(selected, "yes");

        let map = gym::calibrate::Map::fit(&[gym::calibrate::Observation::new(0.8, false)], 1);
        let rescaled = map.apply_distribution(&raw);
        assert!(rescaled["no"] > rescaled["yes"], "{rescaled:?}");

        let Answer::Choice {
            choice, confidence, ..
        } = answer(Kind::Choice, &rescaled, &IndexMap::new(), &selected).unwrap()
        else {
            panic!("expected a Choice");
        };
        assert_eq!(
            choice, "yes",
            "the map rescaled the answer rather than replacing it"
        );
        let confidence = confidence.expect("a confidence");
        assert!(
            (confidence - 0.0).abs() < 1e-12,
            "confidence was {confidence}"
        );
    }

    #[test]
    fn a_noul_or_score_carries_the_selected_option_through_an_inversion() {
        // The wire field that makes the Choice `choice` field's job work on
        // the other two kinds: the answer's own numbers no longer name the
        // pick once a map has sunk it, so `selected` does. `noul` and
        // `score` stay what the contract says they are — the calibrated
        // probability of yes and the weighted position of the mapped
        // distribution.
        let map = gym::calibrate::Map::fit(&[gym::calibrate::Observation::new(0.8, false)], 1);

        let raw = distribution(&[("no", 0.2), ("yes", 0.8)]);
        let rescaled = map.apply_distribution(&raw);
        assert!(rescaled["no"] > rescaled["yes"], "{rescaled:?}");
        let Answer::Noul { noul, selected } =
            answer(Kind::Noul, &rescaled, &IndexMap::new(), "yes").unwrap()
        else {
            panic!("expected a Noul");
        };
        assert!(
            (noul - 0.25).abs() < 1e-12,
            "the calibrated probability of yes: {noul}"
        );
        assert_eq!(
            selected.as_deref(),
            Some("yes"),
            "the answer survived the wire"
        );

        let raw = distribution(&[("0", 0.8), ("1", 0.15), ("2", 0.05)]);
        let rescaled = map.apply_distribution(&raw);
        assert!(rescaled["1"] > rescaled["0"], "{rescaled:?}");
        let Answer::Score {
            score, selected, ..
        } = answer(Kind::Score, &rescaled, &IndexMap::new(), "0").unwrap()
        else {
            panic!("expected a Score");
        };
        // The mapped distribution is 0.25, 0.5625, 0.1875, so the weighted
        // position is 0.9375 — a `score` that names level 1 to anyone
        // rounding it, while the answer stays level 0.
        assert!(
            (score - 0.9375).abs() < 1e-12,
            "the weighted position: {score}"
        );
        assert_eq!(
            selected.as_deref(),
            Some("0"),
            "the answer survived the wire"
        );
    }

    #[test]
    fn a_choice_for_an_option_the_distribution_does_not_name_refuses() {
        let probabilities = distribution(&[("billing", 0.6), ("sales", 0.4)]);
        let refused = answer(Kind::Choice, &probabilities, &IndexMap::new(), "technical")
            .expect_err("an option nothing scored is not an answer");
        assert_eq!(refused.code, RefusalCode::DecodingFailure);
    }

    #[test]
    fn a_block_is_contiguous_and_starts_where_the_last_one_ended() {
        assert_eq!(seed_block(0, 8).unwrap(), 0..8);
        assert_eq!(seed_block(1, 8).unwrap(), 8..16);
        assert_eq!(seed_block(7, 16).unwrap(), 112..128);
    }

    #[test]
    fn two_blocks_never_share_a_seed() {
        let n = 8;
        let blocks: Vec<Vec<u64>> = (0..8)
            .map(|base| seed_block(base, n).unwrap().collect())
            .collect();
        for (left, first) in blocks.iter().enumerate() {
            for (right, second) in blocks.iter().enumerate().skip(left + 1) {
                assert!(
                    first.iter().all(|seed| !second.contains(seed)),
                    "blocks {left} and {right} share a seed: {first:?} against {second:?}"
                );
            }
        }
        // Every seed up to the last block is used exactly once, so the blocks
        // tile the space rather than leaving gaps a later base falls into.
        let all: Vec<u64> = blocks.into_iter().flatten().collect();
        assert_eq!(all, (0..8 * n).collect::<Vec<u64>>());
    }

    #[test]
    fn a_block_past_the_seed_space_is_refused_rather_than_wrapped() {
        let refusal = seed_block(u64::MAX, 8).expect_err("it does not fit");
        assert_eq!(refusal.code, RefusalCode::InvalidRequest);
        let refusal = seed_block(1, 0).expect_err("an empty ensemble is refused");
        assert_eq!(refusal.code, RefusalCode::InvalidRequest);
    }

    #[test]
    fn an_l2_estimate_carries_its_own_resolution() {
        let counts: IndexMap<String, u64> = [("a".to_string(), 5_u64), ("b".to_string(), 3)]
            .into_iter()
            .collect();
        let raw = finish(Estimator::L2, counts, 8, (0..8).collect(), 0, None, 0.0);
        assert_eq!(raw.choice, "a");
        assert!((raw.top() - 0.625).abs() < 1e-12);
        // Eight samples cannot express a difference finer than 0.125, which
        // is coarser than the two decimals the contract reports.
        assert!((raw.resolution - 0.125).abs() < 1e-12);
        assert_eq!(raw.seeds.len(), 8);
        assert_eq!(raw.seed_base, 0);
    }
}
