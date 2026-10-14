//! Checking a decision answer against a reference run of the same model.
//!
//! A decision is one forward pass and a short probability vector, so a
//! checker can recompute it cheaply on its own copy of the pinned model and
//! compare. The comparison is in log space:
//!
//! ```text
//! d(p, q) = max_i | (ln p_i - mean_j ln p_j) - (ln q_i - mean_j ln q_j) |
//! ```
//!
//! with probabilities floored at `1e-6`. `d` is the largest disagreement
//! between two options' log-odds after removing the common offset, so it
//! does not saturate when the model is confident: a near-certain answer
//! still carries the exact gaps between the unlikely options, and those
//! gaps fingerprint the weights, the quantisation and the prompt. The
//! idea and the round rule below (a mean bound that loses the round, a
//! higher mean bound and a per-item bound that strike) follow REAX's
//! subnet (`reaxlabs/reax-subnet`, MIT); this is our own implementation,
//! and the limits come from our own calibration of Clef-Flash
//! (`docs/research/decisionmodels/calibration.md`).
//!
//! Before any distance, [`consistent`] checks that the answer's derived
//! fields agree with its own probabilities (the choice is the most likely
//! option, a score is the expected level), so a pylon cannot pair a
//! plausible choice with made-up probabilities.

use serde_json::Value;

/// Probabilities below this are treated as equal: below it an option
/// carries no usable fingerprint, and float rounding dominates.
pub const LOG_FLOOR: f64 = 1e-6;

/// How far the probabilities may be from summing to one.
pub const SUM_TOLERANCE: f64 = 1e-3;

/// The probabilities of `answer` to `question`, in the question's option
/// order: a choice's criteria keys, a score's levels, `[true, false]` for
/// a noul.
///
/// # Errors
///
/// When the answer is not a well-formed answer to that question: the
/// wrong type, missing or extra options, numbers outside `[0, 1]` or not
/// finite, probabilities that do not sum to one.
pub fn probabilities(question: &Value, answer: &Value) -> Result<Vec<f64>, String> {
    let kind = question["type"].as_str().unwrap_or("noul");
    if answer["type"].as_str() != Some(kind) {
        return Err(format!(
            "a {kind} question got a {} answer",
            answer["type"].as_str().unwrap_or("typeless")
        ));
    }
    let unit = |value: &Value, what: &str| -> Result<f64, String> {
        let x = value
            .as_f64()
            .ok_or_else(|| format!("{what} is not a number"))?;
        if !x.is_finite() || !(0.0..=1.0).contains(&x) {
            return Err(format!("{what} is {x}, outside 0 to 1"));
        }
        Ok(x)
    };
    let keys: Vec<String> = match kind {
        "noul" => {
            let p = unit(&answer["noul"], "noul")?;
            return Ok(vec![p, 1.0 - p]);
        }
        "choice" => question["criteria"]
            .as_object()
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default(),
        "score" => (0..question["criteria"].as_array().map_or(0, Vec::len))
            .map(|i| i.to_string())
            .collect(),
        other => return Err(format!("unknown question type `{other}`")),
    };
    let given = answer["probabilities"]
        .as_object()
        .ok_or("the answer has no probabilities")?;
    if given.len() != keys.len() {
        return Err(format!(
            "the answer has {} probabilities for {} options",
            given.len(),
            keys.len()
        ));
    }
    let mut out = Vec::with_capacity(keys.len());
    for key in &keys {
        let value = given
            .get(key)
            .ok_or_else(|| format!("the answer has no probability for `{key}`"))?;
        out.push(unit(value, &format!("p({key})"))?);
    }
    let sum: f64 = out.iter().sum();
    if (sum - 1.0).abs() > SUM_TOLERANCE {
        return Err(format!("the probabilities sum to {sum:.6}, not 1"));
    }
    Ok(out)
}

/// [`probabilities`], and the answer's derived fields agree with them: a
/// choice names the most likely option, a score is the expected level,
/// and a confidence lies in `[0, 1]`.
///
/// # Errors
///
/// As [`probabilities`], or the first derived field that disagrees.
pub fn consistent(question: &Value, answer: &Value) -> Result<Vec<f64>, String> {
    let p = probabilities(question, answer)?;
    let kind = question["type"].as_str().unwrap_or("noul");
    if let Some(confidence) = answer.get("confidence")
        && !confidence
            .as_f64()
            .is_some_and(|c| c.is_finite() && (0.0..=1.0).contains(&c))
    {
        return Err("the confidence is outside 0 to 1".into());
    }
    let top = p.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    match kind {
        "choice" => {
            let chosen = answer["choice"].as_str().ok_or("the choice is missing")?;
            let index = question["criteria"]
                .as_object()
                .and_then(|o| o.keys().position(|k| k == chosen))
                .ok_or_else(|| format!("the choice `{chosen}` is not an option"))?;
            if p[index] < top - 1e-9 {
                return Err(format!(
                    "the choice `{chosen}` has p {:.4}, but another option has {top:.4}",
                    p[index]
                ));
            }
        }
        "score" => {
            let expected: f64 = p.iter().enumerate().map(|(i, x)| i as f64 * x).sum();
            let given = answer["score"].as_f64().ok_or("the score is missing")?;
            if (given - expected).abs() > 1e-3 * p.len() as f64 {
                return Err(format!(
                    "the score {given:.4} is not the expected level {expected:.4}"
                ));
            }
        }
        _ => {}
    }
    Ok(p)
}

/// The centred-log distance between two probability vectors over the same
/// options (natural-log units).
///
/// # Panics
///
/// Never; vectors of different lengths are infinitely far apart.
#[must_use]
pub fn distance(p: &[f64], q: &[f64]) -> f64 {
    if p.len() != q.len() || p.is_empty() {
        return f64::INFINITY;
    }
    let centred = |v: &[f64]| -> Vec<f64> {
        let logs: Vec<f64> = v.iter().map(|x| x.max(LOG_FLOOR).ln()).collect();
        let mean = logs.iter().sum::<f64>() / logs.len() as f64;
        logs.into_iter().map(|x| x - mean).collect()
    };
    centred(p)
        .iter()
        .zip(centred(q))
        .map(|(a, b)| (a - b).abs())
        .fold(0.0, f64::max)
}

/// The round rule for one model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Limits {
    /// A round whose mean distance is above this is lost.
    pub mean: f64,
    /// A round whose mean distance is above this is a strike.
    pub strike: f64,
    /// Any one verified item above this is a strike.
    pub item: f64,
    /// Items sent per round.
    pub items: usize,
    /// Items the checker recomputes, drawn after the answers arrive.
    pub verified: usize,
}

/// Clef-Flash Q4_K_M served by Psionic, from the calibration in
/// `docs/research/decisionmodels/calibration.md` (2026-10-14, data in
/// `crates/pylon/data/calibration/`):
///
/// - honest pairs: Metal against itself and with f16 accumulation (0),
///   CPU against Metal (max 0.0016 on generated items), CPU against CUDA
///   with f32 accumulation (max 0.0037) and f16 accumulation (median
///   0.005, max 0.063): every simulated round passes;
/// - the same weights at Q8_0 (median 0.069) pass 3.5 % of rounds and
///   strike 21 %; the full-precision reference (what an upstream bf16/f32
///   API answers) passes 0.5 % and strikes 36 %; llama.cpp on the same
///   file passes 1.7 % and strikes 96 %.
///
/// The item bound is eight times the largest honest item seen. Every item
/// is verified: a reference run costs a fifth of a second on a Mac.
pub const CLEF_FLASH: Limits = Limits {
    mean: 0.05,
    strike: 0.15,
    item: 0.5,
    items: 8,
    verified: 8,
};

/// What a round came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Within the limits.
    Pass,
    /// Mean above [`Limits::mean`]: this round earns nothing.
    Lost,
    /// Mean above [`Limits::strike`], an item above [`Limits::item`], or
    /// an answer missing, malformed or inconsistent.
    Strike,
}

/// One scored round.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Round {
    pub verdict: Verdict,
    /// Mean distance over the verified items (infinite when one failed).
    pub mean: f64,
    /// The largest verified distance.
    pub worst: f64,
    /// Items verified.
    pub verified: usize,
}

/// Score a round from the verified items' distances (`f64::INFINITY` for
/// an item with no usable answer).
#[must_use]
pub fn score(distances: &[f64], limits: &Limits) -> Round {
    if distances.is_empty() {
        return Round {
            verdict: Verdict::Strike,
            mean: f64::INFINITY,
            worst: f64::INFINITY,
            verified: 0,
        };
    }
    let worst = distances.iter().copied().fold(0.0, f64::max);
    let mean = if worst.is_finite() {
        distances.iter().sum::<f64>() / distances.len() as f64
    } else {
        f64::INFINITY
    };
    let verdict = if worst > limits.item || mean > limits.strike {
        Verdict::Strike
    } else if mean > limits.mean {
        Verdict::Lost
    } else {
        Verdict::Pass
    };
    Round {
        verdict,
        mean,
        worst,
        verified: distances.len(),
    }
}

/// `k` of the indices `0..n`, drawn from the operating system's random
/// source. A checker calls this after every answer has arrived, so a pylon
/// cannot tell which items are checked.
#[must_use]
pub fn choose_verified(n: usize, k: usize) -> Vec<usize> {
    let mut rng = crate::items::Rng::new(crate::items::fresh_seed());
    let mut pool: Vec<usize> = (0..n).collect();
    let mut out = Vec::new();
    while out.len() < k.min(n) {
        out.push(pool.remove(rng.below(pool.len())));
    }
    out.sort_unstable();
    out
}

/// The share of simulated rounds that pass and that strike, drawing
/// `limits.verified` items at a time from `distances` (with replacement),
/// as a validator's rounds would on this hardware.
#[must_use]
pub fn simulate(distances: &[f64], limits: &Limits, rounds: usize, seed: u64) -> (f64, f64) {
    if distances.is_empty() || rounds == 0 {
        return (0.0, 1.0);
    }
    let mut rng = crate::items::Rng::new(seed);
    let (mut passed, mut struck) = (0usize, 0usize);
    let mut draw = Vec::with_capacity(limits.verified);
    for _ in 0..rounds {
        draw.clear();
        for _ in 0..limits.verified {
            draw.push(distances[rng.below(distances.len())]);
        }
        match score(&draw, limits).verdict {
            Verdict::Pass => passed += 1,
            Verdict::Strike => struck += 1,
            Verdict::Lost => {}
        }
    }
    (passed as f64 / rounds as f64, struck as f64 / rounds as f64)
}

/// A distance distribution's summary.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Spread {
    pub n: usize,
    pub median: f64,
    pub p90: f64,
    pub p99: f64,
    pub max: f64,
}

/// Summarise `distances` (finite ones; an infinite one counts as the max).
#[must_use]
pub fn spread(distances: &[f64]) -> Spread {
    let mut sorted: Vec<f64> = distances.to_vec();
    sorted.sort_by(f64::total_cmp);
    let at = |q: f64| -> f64 {
        if sorted.is_empty() {
            return f64::NAN;
        }
        let i = ((q * (sorted.len() - 1) as f64).round() as usize).min(sorted.len() - 1);
        sorted[i]
    };
    Spread {
        n: sorted.len(),
        median: at(0.5),
        p90: at(0.9),
        p99: at(0.99),
        max: sorted.last().copied().unwrap_or(f64::NAN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn choice_q() -> Value {
        json!({"type": "choice", "criteria": {"a": "first", "b": "second", "c": "third"}})
    }

    #[test]
    fn distance_is_zero_for_equal_answers_and_ignores_scale() {
        let p = [0.7, 0.2, 0.1];
        assert!(distance(&p, &p) < 1e-12);
        // The same logits at another temperature are not the same answer,
        // but the same logits shifted by a constant are.
        let shifted: Vec<f64> = {
            let raw = [2.0f64, 0.5, -1.0];
            let z: f64 = raw.iter().map(|x| x.exp()).sum();
            raw.iter().map(|x| x.exp() / z).collect()
        };
        let again: Vec<f64> = {
            let raw = [12.0f64, 10.5, 9.0];
            let z: f64 = raw.iter().map(|x| x.exp()).sum();
            raw.iter().map(|x| x.exp() / z).collect()
        };
        assert!(distance(&shifted, &again) < 1e-9);
    }

    #[test]
    fn distance_sees_gaps_in_a_confident_answer() {
        // Both put 0.998 on the first option; the tails disagree by e.
        let p = [0.998, 0.001, 0.001];
        let q = [
            0.998,
            0.001 * std::f64::consts::E / (1.0 + 0.001 * 1.718),
            0.001,
        ];
        assert!(distance(&p, &q) > 0.5);
    }

    #[test]
    fn mismatched_lengths_are_infinitely_far() {
        assert!(distance(&[0.5, 0.5], &[0.2, 0.3, 0.5]).is_infinite());
    }

    #[test]
    fn consistent_checks_the_derived_fields() {
        let q = choice_q();
        let ok = json!({"type": "choice", "choice": "b", "confidence": 0.6,
                        "probabilities": {"a": 0.3, "b": 0.6, "c": 0.1}});
        assert_eq!(consistent(&q, &ok).unwrap(), vec![0.3, 0.6, 0.1]);
        let wrong_choice = json!({"type": "choice", "choice": "a", "confidence": 0.6,
                                  "probabilities": {"a": 0.3, "b": 0.6, "c": 0.1}});
        assert!(consistent(&q, &wrong_choice).is_err());
        let not_summing = json!({"type": "choice", "choice": "b",
                                 "probabilities": {"a": 0.3, "b": 0.6, "c": 0.3}});
        assert!(consistent(&q, &not_summing).is_err());
        let missing = json!({"type": "choice", "choice": "b",
                             "probabilities": {"a": 0.4, "b": 0.6}});
        assert!(consistent(&q, &missing).is_err());
        let score_q = json!({"type": "score", "criteria": ["low", "mid", "high"]});
        let good = json!({"type": "score", "score": 1.5, "confidence": 0.6,
                          "probabilities": {"0": 0.1, "1": 0.3, "2": 0.6}});
        assert!(consistent(&score_q, &good).is_ok());
        let bad = json!({"type": "score", "score": 2.0, "confidence": 0.6,
                         "probabilities": {"0": 0.1, "1": 0.3, "2": 0.6}});
        assert!(consistent(&score_q, &bad).is_err());
        let noul_q = json!({"type": "noul"});
        assert_eq!(
            consistent(&noul_q, &json!({"type": "noul", "noul": 0.25})).unwrap(),
            vec![0.25, 0.75]
        );
        assert!(consistent(&noul_q, &json!({"type": "noul", "noul": 1.5})).is_err());
        assert!(consistent(&noul_q, &json!({"type": "choice"})).is_err());
    }

    #[test]
    fn the_round_rule() {
        let l = CLEF_FLASH;
        assert_eq!(score(&[0.01; 8], &l).verdict, Verdict::Pass);
        assert_eq!(score(&[l.mean + 0.01; 8], &l).verdict, Verdict::Lost);
        assert_eq!(score(&[l.strike + 0.01; 8], &l).verdict, Verdict::Strike);
        let mut one_bad = [0.0; 8];
        one_bad[3] = l.item + 0.01;
        assert_eq!(score(&one_bad, &l).verdict, Verdict::Strike);
        let mut missing = [0.0; 8];
        missing[0] = f64::INFINITY;
        assert_eq!(score(&missing, &l).verdict, Verdict::Strike);
        assert_eq!(score(&[], &l).verdict, Verdict::Strike);
    }

    #[test]
    fn verified_items_are_distinct_and_in_range() {
        for _ in 0..50 {
            let v = choose_verified(8, 3);
            assert_eq!(v.len(), 3);
            assert!(v.windows(2).all(|w| w[0] < w[1]));
            assert!(v.iter().all(|&i| i < 8));
        }
        assert_eq!(choose_verified(3, 8), vec![0, 1, 2]);
    }

    #[test]
    fn simulation_separates_honest_noise_from_a_substitute() {
        let honest = vec![0.005, 0.01, 0.02, 0.004, 0.03, 0.001, 0.06];
        let (pass, strike) = simulate(&honest, &CLEF_FLASH, 5000, 1);
        assert!(pass > 0.999 && strike == 0.0);
        let other = vec![0.1, 0.2, 0.3, 0.15, 0.4, 0.05, 0.6];
        let (pass, _) = simulate(&other, &CLEF_FLASH, 5000, 1);
        assert!(pass < 0.05);
    }

    #[test]
    fn spread_summarises() {
        let s = spread(&[3.0, 1.0, 2.0]);
        assert_eq!((s.n, s.median, s.max), (3, 2.0, 3.0));
    }
}
