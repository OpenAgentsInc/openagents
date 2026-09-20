//! Whether the recorded rows cover the selection a run was meant to ask.
//!
//! A store only knows the rows it holds. An item lost to a timeout, a dead
//! door, or a killed process leaves no row, and a report over the surviving
//! rows alone reads complete whether it is or not. Completeness needs the
//! other half declared: which items the run was meant to ask, and of which
//! doors. This module is that declaration and the accounting against it.
//!
//! The rule the numbers follow is the one [`crate::spread`] and the
//! deployment reader already keep: a missing item is missing, never a wrong
//! answer and never a zero. Coverage is reported, not repaired — a run that
//! recorded a subset renders as the subset it is, labelled, and nothing
//! fills the gap silently.
//!
//! A report that cannot name its expected selection is unverifiable as a
//! completed evaluation. It still renders — a partial record is evidence —
//! but the missing declaration is the first thing it says.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::row::Row;
use crate::suite::{Partition, Suite};

/// The selection a run was meant to ask: which items, in which partitions,
/// and of which doors.
///
/// Built from the suite the run scored and the narrowing the run declared —
/// the same `partition`, `family`, and item-id subset `eval` accepts, so the
/// expectation reproduces what the run asked rather than a reconstruction
/// after the fact. The doors are named separately because a door that
/// recorded nothing still belongs in the expectation.
#[derive(Clone, Debug)]
pub struct Expected {
    /// The `(partition, item_id)` pairs the run was meant to record per door.
    items: BTreeSet<(String, String)>,
    /// `item_id` to `family`, for the per-family accounting.
    by_item: BTreeMap<String, String>,
    /// The families the selection covers, in the suite's own order.
    families: Vec<String>,
    /// The doors the run was meant to ask, in declared order.
    pub doors: Vec<String>,
}

impl Expected {
    /// The selection a run of `suite` over `partitions` was meant to cover,
    /// narrowed to `family` and then to `subset` when those were declared.
    ///
    /// A family or item id the suite does not hold is an error rather than
    /// an empty expectation, for the same reason `eval` refuses them: a
    /// selection that names nothing reads as a door that answered nothing.
    /// A subset id outside the selected partitions and family is refused —
    /// the declaration must be reachable by the narrowing it claims.
    pub fn of(
        suite: &Suite,
        partitions: &[Partition],
        family: Option<&str>,
        subset: Option<&BTreeSet<String>>,
        doors: Vec<String>,
    ) -> Result<Self, String> {
        if let Some(named) = family {
            let held = suite.families();
            if !held.iter().any(|family| family == named) {
                return Err(format!(
                    "`{}` holds no {named} family; it holds {}",
                    suite.name,
                    held.join(", ")
                ));
            }
        }
        // The items are enumerated from `suite.items` rather than
        // `Suite::partition`: that method's refusal of the locked partition
        // is about scoring it, and naming a locked item's id to check it was
        // covered reads nothing the ledger protects.
        let mut items = BTreeSet::new();
        let mut by_item = BTreeMap::new();
        for partition in partitions {
            for item in suite
                .items
                .iter()
                .filter(|item| item.partition == *partition)
            {
                if let Some(named) = family
                    && item.family != named
                {
                    continue;
                }
                items.insert((partition.as_str().to_string(), item.id.clone()));
                by_item.insert(item.id.clone(), item.family.clone());
            }
        }
        if let Some(subset) = subset {
            if subset.is_empty() {
                return Err("the expected subset names no items".to_string());
            }
            for id in subset {
                if !by_item.contains_key(id) {
                    return Err(format!(
                        "item {id} is outside the selected partitions and family"
                    ));
                }
            }
            items.retain(|(_, id)| subset.contains(id));
            by_item.retain(|id, _| subset.contains(id));
        }
        if items.is_empty() {
            return Err("the declared selection covers no items".to_string());
        }
        let families = suite
            .families()
            .into_iter()
            .filter(|family| by_item.values().any(|held| held == family))
            .collect();
        Ok(Self {
            items,
            by_item,
            families,
            doors,
        })
    }

    /// The `(partition, item_id)` pairs themselves.
    #[must_use]
    pub fn items(&self) -> &BTreeSet<(String, String)> {
        &self.items
    }

    /// The expected pairs that belong to one family.
    #[must_use]
    pub fn for_family(&self, family: &str) -> BTreeSet<(String, String)> {
        self.items
            .iter()
            .filter(|(_, id)| self.by_item.get(id).map(String::as_str) == Some(family))
            .cloned()
            .collect()
    }

    /// The families the selection covers, in the suite's own order.
    #[must_use]
    pub fn families(&self) -> &[String] {
        &self.families
    }
}

/// What the rows show against an expected selection, for one door's rows.
///
/// `answered` and `refused` are recorded outcomes — a refusal is the door's
/// result and stays a result. `missing` is the gap: expected items with no
/// row at all, which timeouts and dead doors produce. `duplicates` are
/// expected items with more than one row — a second pass, a retry the store
/// kept, or a merge that folded twice — each named rather than merged.
/// `unexpected` are rows for items the declared selection does not cover:
/// a wider run than declared, or rows from another run folded in.
#[derive(Clone, Debug, Default)]
pub struct Coverage {
    /// How many `(partition, item)` pairs the selection asked for.
    pub expected: usize,
    /// Expected items with a recorded answer.
    pub answered: usize,
    /// Expected items the door refused.
    pub refused: usize,
    /// Expected items with no row, `(partition, item_id)`, in order.
    pub missing: Vec<(String, String)>,
    /// Expected items with more than one row, `(partition, item_id)`.
    pub duplicates: Vec<(String, String)>,
    /// Recorded rows outside the selection, `(partition, item_id)`.
    pub unexpected: Vec<(String, String)>,
}

impl Coverage {
    /// The accounting of `rows` against `expected`.
    ///
    /// A row that is neither an answer nor a refusal — the half-written
    /// shape [`Row::check`] refuses — still counts as recorded evidence of
    /// an attempt; it is neither silently dropped nor scored.
    #[must_use]
    pub fn of(rows: &[Row], expected: &BTreeSet<(String, String)>) -> Self {
        let mut coverage = Self {
            expected: expected.len(),
            ..Self::default()
        };
        let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
        for row in rows {
            let key = (row.split.clone(), row.item_id.clone());
            if !expected.contains(&key) {
                coverage.unexpected.push(key);
                continue;
            }
            *seen.entry(key).or_default() += 1;
            if row.is_scored() {
                coverage.answered += 1;
            } else if row.is_refused() {
                coverage.refused += 1;
            }
        }
        for key in expected {
            match seen.get(key) {
                None => coverage.missing.push(key.clone()),
                Some(count) if *count >= 2 => coverage.duplicates.push(key.clone()),
                _ => {}
            }
        }
        coverage
    }

    /// Whether the rows recorded exactly the selection, once each.
    #[must_use]
    pub fn complete(&self) -> bool {
        self.missing.is_empty() && self.duplicates.is_empty() && self.unexpected.is_empty()
    }

    /// How many expected items carry any row.
    #[must_use]
    pub fn recorded(&self) -> usize {
        self.expected - self.missing.len()
    }
}

/// The workload a group of rows was run under: door identity, the question
/// and gate digests it pins, and the estimator's trial configuration.
///
/// Rows that disagree on any of these are different measurements and are
/// never averaged together — the same discipline
/// `examples/deployment_from_store.rs` enforces as an error. Here the key
/// only groups; what a report does with a mixture is its own choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunKey {
    /// What the rows claim the door was running.
    pub door_identity: String,
    /// The question-set digest, or none on an inline-question suite.
    pub question_digest: Option<String>,
    /// The gate digest, when a gate judged the rows.
    pub gate_digest: Option<String>,
    /// Which estimator produced the raw signal.
    pub estimator: String,
    /// How many draws each estimate rests on.
    pub samples: Option<u64>,
    /// The seed base the estimator started from.
    pub seed_base: Option<u64>,
}

impl RunKey {
    /// The key one row claims.
    #[must_use]
    pub fn of(row: &Row) -> Self {
        Self {
            // Strings, maps, and a boolean serialize without a failure.
            door_identity: serde_json::to_string(&row.door_identity)
                .expect("a door identity serializes"),
            question_digest: row.question_digest.clone(),
            gate_digest: row.gate_digest.clone(),
            estimator: row.estimator.clone(),
            samples: row.samples,
            seed_base: row.seed_base,
        }
    }
}

/// The distinct run configurations `rows` were recorded under, in
/// first-seen order, each with its rows.
///
/// One group is the ordinary case. More than one means the store folds
/// measurements that disagree on what ran or what was asked — a report
/// renders each apart rather than pooling them into one denominator.
#[must_use]
pub fn run_groups(rows: &[Row]) -> Vec<(RunKey, Vec<Row>)> {
    let mut groups: Vec<(RunKey, Vec<Row>)> = Vec::new();
    for row in rows {
        let key = RunKey::of(row);
        match groups.iter_mut().find(|(held, _)| *held == key) {
            Some((_, inside)) => inside.push(row.clone()),
            None => groups.push((key, vec![row.clone()])),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::row::RefusalCode;

    fn row(split: &str, item: &str) -> Row {
        Row {
            split: split.to_string(),
            item_id: item.to_string(),
            answered: true,
            correct: Some(true),
            ..Row::default()
        }
    }

    fn expected() -> BTreeSet<(String, String)> {
        [
            ("development".to_string(), "a".to_string()),
            ("development".to_string(), "b".to_string()),
            ("development".to_string(), "c".to_string()),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn a_full_pass_is_complete() {
        let rows = vec![
            row("development", "a"),
            row("development", "b"),
            row("development", "c"),
        ];
        let coverage = Coverage::of(&rows, &expected());
        assert!(coverage.complete());
        assert_eq!((coverage.answered, coverage.recorded()), (3, 3));
    }

    #[test]
    fn a_missing_item_is_named_and_never_scored() {
        let rows = vec![row("development", "a"), row("development", "c")];
        let coverage = Coverage::of(&rows, &expected());
        assert!(!coverage.complete());
        assert_eq!(
            coverage.missing,
            [("development".to_string(), "b".to_string())]
        );
        assert_eq!(coverage.recorded(), 2);
    }

    #[test]
    fn a_refusal_is_a_recorded_outcome_not_a_gap() {
        let refused = row("development", "b").refused(RefusalCode::Busy);
        let rows = vec![row("development", "a"), refused, row("development", "c")];
        let coverage = Coverage::of(&rows, &expected());
        assert!(coverage.complete());
        assert_eq!((coverage.answered, coverage.refused), (2, 1));
    }

    #[test]
    fn duplicates_and_unexpected_rows_are_named() {
        let mut rows = vec![
            row("development", "a"),
            row("development", "b"),
            row("development", "c"),
        ];
        rows.push(row("development", "a"));
        rows.push(row("development", "other"));
        let coverage = Coverage::of(&rows, &expected());
        assert!(!coverage.complete());
        assert_eq!(
            coverage.duplicates,
            [("development".to_string(), "a".to_string())]
        );
        assert_eq!(
            coverage.unexpected,
            [("development".to_string(), "other".to_string())]
        );
    }

    #[test]
    fn rows_under_two_identities_form_two_groups() {
        let mut other = row("development", "a");
        other.door_identity.model = "changed".to_string();
        let rows = vec![row("development", "a"), other, row("development", "b")];
        let groups = run_groups(&rows);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].1.len(), 2);
        assert_eq!(groups[1].1.len(), 1);
    }

    #[test]
    fn a_subset_is_the_declared_selection_not_a_gap() {
        let suite = crate::suite::support_v2_three_way().unwrap();
        let subset: BTreeSet<String> = suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Development)
            .take(3)
            .map(|item| item.id.clone())
            .collect();
        let expected = Expected::of(
            &suite,
            &[Partition::Development],
            None,
            Some(&subset),
            vec![],
        )
        .unwrap();
        assert_eq!(expected.items().len(), 3);
        let rows: Vec<Row> = expected
            .items()
            .iter()
            .map(|(split, id)| row(split, id))
            .collect();
        assert!(Coverage::of(&rows, expected.items()).complete());
    }

    #[test]
    fn a_subset_outside_the_selection_is_refused() {
        let suite = crate::suite::support_v2_three_way().unwrap();
        let calibration: BTreeSet<String> = suite
            .partition(Partition::Calibration)
            .unwrap()
            .iter()
            .map(|item| item.id.clone())
            .take(1)
            .collect();
        let trouble = Expected::of(
            &suite,
            &[Partition::Development],
            None,
            Some(&calibration),
            vec![],
        )
        .unwrap_err();
        assert!(trouble.contains("outside the selected"), "{trouble}");
    }
}
