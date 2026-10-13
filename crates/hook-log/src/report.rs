//! Join decision lines with outcomes and compute calibration.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde::Deserialize;

use crate::Decision;

/// Reliability bins.
pub const BINS: usize = 5;

/// A label for one decision.
#[derive(Clone, Debug, Deserialize)]
pub struct Label {
    /// The decision's `ts`.
    #[serde(alias = "id")]
    pub decision_ts: u64,
    /// Optional narrowing when two decisions share a `ts`.
    #[serde(default)]
    pub hook: Option<String>,
    #[serde(default)]
    pub question: Option<String>,
    /// Was the decision right?
    pub correct: bool,
}

/// One run's outcome.
#[derive(Clone, Debug, Deserialize)]
pub struct Outcome {
    pub run_id: String,
    #[serde(default)]
    pub passed: bool,
    #[serde(default)]
    pub labels: Vec<Label>,
}

/// A decision with its label, when one matched.
#[derive(Clone, Debug)]
pub struct Joined {
    pub decision: Decision,
    pub correct: Option<bool>,
    pub passed: Option<bool>,
}

/// Parses JSON lines, skipping blank and malformed ones.
#[must_use]
pub fn parse_lines<T: for<'de> Deserialize<'de>>(text: &str) -> Vec<T> {
    text.lines()
        .filter_map(|l| serde_json::from_str(l.trim()).ok())
        .collect()
}

/// Attaches labels (by `decision_ts`, within the outcome's run) and the
/// run's pass flag to each decision.
#[must_use]
pub fn join(decisions: Vec<Decision>, outcomes: &[Outcome]) -> Vec<Joined> {
    decisions
        .into_iter()
        .map(|decision| {
            let outcome = decision
                .run_id
                .as_ref()
                .and_then(|id| outcomes.iter().find(|o| &o.run_id == id));
            let correct = outcome.and_then(|o| {
                o.labels
                    .iter()
                    .find(|l| {
                        l.decision_ts == decision.ts
                            && l.hook.as_ref().is_none_or(|h| *h == decision.hook)
                            && l.question.as_ref().is_none_or(|q| *q == decision.question)
                    })
                    .map(|l| l.correct)
            });
            Joined {
                passed: outcome.map(|o| o.passed),
                correct,
                decision,
            }
        })
        .collect()
}

/// One reliability bin.
#[derive(Clone, Debug, PartialEq)]
pub struct Bin {
    pub n: usize,
    pub mean_p: f64,
    pub observed: f64,
}

/// Bins `(p, correct)` pairs into [`BINS`] equal-width bins and returns the
/// bins (empty ones have `n == 0`) and the expected calibration error.
#[must_use]
pub fn reliability(pairs: &[(f64, bool)]) -> (Vec<Bin>, f64) {
    let mut sums = vec![(0usize, 0.0f64, 0usize); BINS];
    for &(p, ok) in pairs {
        let p = p.clamp(0.0, 1.0);
        let i = ((p * BINS as f64) as usize).min(BINS - 1);
        sums[i].0 += 1;
        sums[i].1 += p;
        sums[i].2 += usize::from(ok);
    }
    let total = pairs.len().max(1) as f64;
    let mut ece = 0.0;
    let bins = sums
        .into_iter()
        .map(|(n, sum_p, hits)| {
            if n == 0 {
                return Bin {
                    n,
                    mean_p: 0.0,
                    observed: 0.0,
                };
            }
            let mean_p = sum_p / n as f64;
            let observed = hits as f64 / n as f64;
            ece += n as f64 / total * (mean_p - observed).abs();
            Bin {
                n,
                mean_p,
                observed,
            }
        })
        .collect();
    (bins, ece)
}

fn range(i: usize) -> String {
    format!(
        "{:.1}-{:.1}",
        i as f64 / BINS as f64,
        (i + 1) as f64 / BINS as f64
    )
}

/// The text report.
#[must_use]
pub fn render(joined: &[Joined]) -> String {
    let mut groups: BTreeMap<(String, String), Vec<&Joined>> = BTreeMap::new();
    for j in joined {
        groups
            .entry((j.decision.hook.clone(), j.decision.question.clone()))
            .or_default()
            .push(j);
    }
    let mut out = String::new();
    for ((hook, question), rows) in groups {
        let ps: Vec<f64> = rows.iter().filter_map(|j| j.decision.p).collect();
        let mean = if ps.is_empty() {
            "n/a".to_string()
        } else {
            format!("{:.3}", ps.iter().sum::<f64>() / ps.len() as f64)
        };
        let _ = writeln!(
            out,
            "{hook} / {question}: {} decisions, mean p {mean}",
            rows.len()
        );
        let labeled: Vec<(f64, bool)> = rows
            .iter()
            .filter_map(|j| Some((j.decision.p?, j.correct?)))
            .collect();
        if labeled.is_empty() {
            if !ps.is_empty() {
                let (bins, _) = reliability(&ps.iter().map(|&p| (p, false)).collect::<Vec<_>>());
                let _ = writeln!(out, "  p histogram:");
                for (i, b) in bins.iter().enumerate() {
                    let _ = writeln!(out, "    {}  {}", range(i), b.n);
                }
            }
            continue;
        }
        let (bins, ece) = reliability(&labeled);
        let _ = writeln!(out, "  {} labeled", labeled.len());
        for (i, b) in bins.iter().enumerate() {
            let _ = writeln!(
                out,
                "    {}  n={}  p={:.3}  observed={:.3}",
                range(i),
                b.n,
                b.mean_p,
                b.observed
            );
        }
        let _ = writeln!(out, "  ECE {ece:.3}");
    }
    if out.is_empty() {
        out.push_str("no decisions\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::How;

    fn dec(ts: u64, run: &str, p: f64) -> Decision {
        let mut d = Decision::new("gate", "q", "allow", How::Decision);
        d.ts = ts;
        d.run_id = Some(run.to_string());
        d.p = Some(p);
        d
    }

    #[test]
    fn join_matches_by_run_and_ts() {
        let outcomes = parse_lines::<Outcome>(
            r#"{"run_id":"r1","passed":true,"labels":[{"decision_ts":1,"correct":true},{"id":2,"correct":false}]}
not json
{"run_id":"r2","passed":false}"#,
        );
        assert_eq!(outcomes.len(), 2);
        let joined = join(
            vec![
                dec(1, "r1", 0.9),
                dec(2, "r1", 0.8),
                dec(3, "r1", 0.5),
                dec(1, "r2", 0.5),
                dec(1, "zz", 0.5),
            ],
            &outcomes,
        );
        assert_eq!(joined[0].correct, Some(true));
        assert_eq!(joined[1].correct, Some(false));
        assert_eq!(joined[2].correct, None);
        assert_eq!(joined[3].correct, None);
        assert_eq!(joined[3].passed, Some(false));
        assert_eq!(joined[4].passed, None);
    }

    #[test]
    fn ece_of_perfectly_calibrated_is_zero() {
        let pairs: Vec<(f64, bool)> = (0..10).map(|i| (0.5, i < 5)).collect();
        let (bins, ece) = reliability(&pairs);
        assert_eq!(bins[2].n, 10);
        assert!(ece.abs() < 1e-9);
    }

    #[test]
    fn ece_of_overconfident_is_the_gap() {
        let pairs: Vec<(f64, bool)> = (0..10).map(|i| (0.9, i < 5)).collect();
        let (bins, ece) = reliability(&pairs);
        assert_eq!(bins[4].n, 10);
        assert!((ece - 0.4).abs() < 1e-9);
    }

    #[test]
    fn ece_weights_bins_by_size_and_p_one_lands_in_last_bin() {
        let pairs = [(1.0, true), (1.0, true), (1.0, true), (0.1, true)];
        let (bins, ece) = reliability(&pairs);
        assert_eq!(bins[4].n, 3);
        assert_eq!(bins[0].n, 1);
        // Only bin 0 is off: |0.1 - 1| * 1/4.
        assert!((ece - 0.225).abs() < 1e-9);
    }

    #[test]
    fn render_without_labels_gives_counts_and_histogram() {
        let text = render(&join(vec![dec(1, "r", 0.1), dec(2, "r", 0.95)], &[]));
        assert!(text.contains("gate / q: 2 decisions, mean p 0.525"));
        assert!(text.contains("p histogram"));
        assert!(!text.contains("ECE"));
    }

    #[test]
    fn render_with_labels_has_ece() {
        let outcomes = parse_lines::<Outcome>(
            r#"{"run_id":"r","passed":true,"labels":[{"decision_ts":1,"correct":true}]}"#,
        );
        let text = render(&join(vec![dec(1, "r", 0.9)], &outcomes));
        assert!(text.contains("1 labeled"));
        assert!(text.contains("ECE 0.100"));
    }
}
