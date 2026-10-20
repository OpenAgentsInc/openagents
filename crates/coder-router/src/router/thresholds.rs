//! The router's act-or-fall-through thresholds, derived from written costs
//! (#10386, step 2 of `docs/research/typesafe/2026-10-03-calibration.md`).
//!
//! A threshold on a calibrated probability is a cost decision, not a
//! taste: serving a prepared reply costs nothing when it is right and
//! [`Costs::wrong_action`] when it is wrong; falling through to the model
//! costs [`Costs::fall_through`] whatever the reading said. With `p` the
//! calibrated probability the reply is right, acting is cheaper exactly
//! when `(1 - p) * wrong_action < fall_through`, so the least `p` worth
//! acting on is `1 - fall_through / wrong_action` ([`Costs::act_at`]).
//!
//! That rule only holds on calibrated probabilities. A raw reading's `p`
//! is not the chance the reply is right (the `answer` question's raw 0.8–0.9
//! bin was right 75% of the time on the calibration partition), which is
//! why the raw thresholds in [`super::policy`] stay for the raw path and
//! the calibrated ones here serve when the map applied.
//!
//! The nightly refit (#10387) rewrites these from live outcomes; keep every
//! threshold the router derives from costs in this module.

use gym::calibrate::{Map, Observation};
use serde::Serialize;

/// The written cost of each outcome of one router decision, in units of
/// one model call.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Costs {
    /// Serving a prepared reply (or route) that is wrong: the person reads
    /// a wrong answer, has to notice it and ask again, and trusts the next
    /// reply less. Written as ten model calls.
    pub wrong_action: f64,
    /// Falling through to the model: one model call, a second or two and a
    /// fraction of a cent, with the right answer still likely.
    pub fall_through: f64,
}

impl Costs {
    /// The least calibrated probability at which acting is cheaper than
    /// falling through.
    #[must_use]
    pub fn act_at(self) -> f64 {
        1.0 - self.fall_through / self.wrong_action
    }
}

/// Serving a wrong whole prepared answer: ten model calls; falling through:
/// one.
pub const ANSWER_COSTS: Costs = Costs {
    wrong_action: 10.0,
    fall_through: 1.0,
};

/// Acting on a wrong route (a canned reply, `end`, or a refusal): ten model
/// calls; falling through: one.
pub const ROUTE_COSTS: Costs = Costs {
    wrong_action: 10.0,
    fall_through: 1.0,
};

/// The least calibrated `answer` probability for a whole prepared answer:
/// [`ANSWER_COSTS`]`.act_at()`, 0.90. Serves in place of
/// [`super::policy::ANSWER_CONFIDENCE`] when the reading went through the
/// answer map.
pub const CALIBRATED_ANSWER_CONFIDENCE: f64 = 0.90;

/// One operating point: how a threshold does on a set of observations.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Operating {
    pub threshold: f64,
    /// Whether the threshold was tested on the mapped probabilities.
    pub calibrated: bool,
    pub items: usize,
    /// Readings at or above the threshold: served.
    pub acted: usize,
    /// Served and right.
    pub right: usize,
    /// Served and wrong.
    pub wrong: usize,
    /// Below the threshold: fell through to the model.
    pub fell_through: usize,
    /// Mean cost per item under `costs`.
    pub cost_per_item: f64,
}

impl Operating {
    /// The share of served readings that were right, `None` with none
    /// served.
    #[must_use]
    pub fn precision(&self) -> Option<f64> {
        (self.acted > 0).then(|| self.right as f64 / self.acted as f64)
    }
}

/// How acting at `threshold` does on `observations`, mapping each raw
/// probability through `map` first when one is given.
#[must_use]
pub fn operate(
    observations: &[Observation],
    map: Option<&Map>,
    threshold: f64,
    costs: Costs,
) -> Operating {
    let mut acted = 0;
    let mut right = 0;
    for observation in observations {
        let p = map.map_or(observation.raw, |map| map.apply(observation.raw));
        if p >= threshold {
            acted += 1;
            if observation.correct {
                right += 1;
            }
        }
    }
    let wrong = acted - right;
    let fell_through = observations.len() - acted;
    let cost = wrong as f64 * costs.wrong_action + fell_through as f64 * costs.fall_through;
    Operating {
        threshold,
        calibrated: map.is_some(),
        items: observations.len(),
        acted,
        right,
        wrong,
        fell_through,
        cost_per_item: if observations.is_empty() {
            0.0
        } else {
            cost / observations.len() as f64
        },
    }
}

/// The calibrated threshold, from the candidates `0.50, 0.55, … 0.95`,
/// with the least cost on `observations` (ties to the higher threshold).
/// The derivation picks on the calibration partition and confirms on the
/// held-out split; the cost formula's [`Costs::act_at`] is the answer a
/// perfectly calibrated map would give.
#[must_use]
pub fn least_cost(observations: &[Observation], map: &Map, costs: Costs) -> Operating {
    (10..=19)
        .map(|step| f64::from(step) * 0.05)
        .map(|threshold| operate(observations, Some(map), threshold, costs))
        .fold(None::<Operating>, |best, next| match best {
            Some(best) if best.cost_per_item < next.cost_per_item - 1e-12 => Some(best),
            _ => Some(next),
        })
        .expect("ten candidates")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_written_costs_give_the_served_thresholds() {
        assert!((ANSWER_COSTS.act_at() - CALIBRATED_ANSWER_CONFIDENCE).abs() < 1e-12);
        assert!((ROUTE_COSTS.act_at() - 0.9).abs() < 1e-12);
    }

    #[test]
    fn an_operating_point_counts_served_right_wrong_and_fell_through() {
        let observations = [
            Observation::new(0.95, true),
            Observation::new(0.85, false),
            Observation::new(0.6, true),
        ];
        let point = operate(&observations, None, 0.8, ANSWER_COSTS);
        assert_eq!(
            (point.acted, point.right, point.wrong, point.fell_through),
            (2, 1, 1, 1)
        );
        assert!((point.cost_per_item - 11.0 / 3.0).abs() < 1e-12);
        assert_eq!(point.precision(), Some(0.5));
    }

    #[test]
    fn the_least_cost_threshold_skips_an_overconfident_band() {
        // Raw 0.8–0.9 is right a quarter of the time, 0.9–1.0 always: on a
        // map fitted to that, the least-cost threshold serves only the top.
        let mut fit = Vec::new();
        for _ in 0..4 {
            fit.push(Observation::new(0.85, false));
            fit.push(Observation::new(0.95, true));
        }
        fit.push(Observation::new(0.85, true));
        let map = Map::fit(&fit, 10);
        let best = least_cost(&fit, &map, ANSWER_COSTS);
        assert_eq!(best.wrong, 0);
        assert_eq!(best.acted, 4);
    }
}
