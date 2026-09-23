//! The search space and the proposal operators.
//!
//! A slot is one searchable manifest field ([`crate::policy::SEARCHABLE`]
//! lists them; each has a canary that shows a change reaches the executor
//! or what it's given). A proposal replaces slot values in the study's
//! baseline and becomes a complete new manifest, validated like any other,
//! or a recorded refusal. The operators, by their real names:
//!
//! - `grid`: every combination of the slots' listed values.
//! - `random`: uniform draws from the same lists, seeded.
//! - `climb`: coordinate ascent from the baseline, one step per slot per
//!   move, on the search partition's objective.
//! - `swap`: one component's implementation replaced, everything else
//!   fixed, such as the first packer against the coverage packer.
//! - `reflect`: text edits for instruction slots, read from a proposal file
//!   a person or a model wrote after reading failing traces. The record
//!   names the proposer; it is not GEPA, which this crate doesn't run.
//! - `router-refit`: a refit router table. The manifest has no route slot
//!   yet, so construction refuses it; `gym coder router` fits and scores
//!   routers on the outcome matrix.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Archive, Change, Proposer, Rng};
use crate::policy::{Manifest, PackPolicy, SEARCHABLE};

/// One searchable field and the values a study may give it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    /// The manifest field, as [`SEARCHABLE`] names it.
    pub id: String,
    /// NIP-OPT's surface: `parameters`, `composition`, or `instructions`.
    pub surface: String,
    /// The grid's values, in order.
    pub values: Vec<Value>,
    /// The climb's step and bounds, for a numeric slot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl Slot {
    /// A numeric slot with grid values and climb bounds.
    #[must_use]
    pub fn numeric(id: &str, values: &[f64], step: f64, min: f64, max: f64) -> Self {
        Slot {
            id: id.to_string(),
            surface: "parameters".to_string(),
            values: values.iter().map(|v| number(*v)).collect(),
            step: Some(step),
            min: Some(min),
            max: Some(max),
        }
    }

    /// A choice slot, such as a component's implementation.
    #[must_use]
    pub fn choice(id: &str, surface: &str, values: &[&str]) -> Self {
        Slot {
            id: id.to_string(),
            surface: surface.to_string(),
            values: values.iter().map(|v| json!(v)).collect(),
            step: None,
            min: None,
            max: None,
        }
    }
}

/// A whole number as a JSON integer, anything else as a float rounded to
/// two decimals.
#[must_use]
pub fn number(value: f64) -> Value {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        json!(value as i64)
    } else {
        json!((value * 100.0).round() / 100.0)
    }
}

/// Refuses a space whose slots aren't all searchable and distinct.
///
/// # Errors
///
/// Returns every problem found.
pub fn validate(slots: &[Slot]) -> Result<(), String> {
    let mut problems = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for slot in slots {
        if !SEARCHABLE.contains(&slot.id.as_str()) {
            problems.push(format!(
                "{} has no passing canary, so it cannot be searched",
                slot.id
            ));
        }
        if !seen.insert(&slot.id) {
            problems.push(format!("{} appears twice", slot.id));
        }
        if slot.values.is_empty() {
            problems.push(format!("{} lists no values", slot.id));
        }
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the search space is refused: {}",
            problems.join("; ")
        ))
    }
}

/// `base` with each change applied, validated.
///
/// # Errors
///
/// Returns why the result is not a valid manifest: an unsearchable slot,
/// a value of the wrong type, or a manifest validation refuses.
pub fn apply(base: &Manifest, changes: &[Change]) -> Result<Manifest, String> {
    let mut value = serde_json::to_value(base).map_err(|error| error.to_string())?;
    for change in changes {
        if !SEARCHABLE.contains(&change.slot.as_str()) {
            return Err(format!(
                "{} has no passing canary, so it cannot be searched",
                change.slot
            ));
        }
        let parts: Vec<&str> = change.slot.split('.').collect();
        // A `brief.pack` field starts from the packer's defaults.
        if change.slot.starts_with("policy.brief.pack.")
            && value
                .pointer("/policy/brief/pack")
                .is_none_or(Value::is_null)
        {
            value["policy"]["brief"]["pack"] =
                serde_json::to_value(PackPolicy::default()).map_err(|error| error.to_string())?;
        }
        let (last, parents) = parts.split_last().ok_or("an empty slot")?;
        let mut node = &mut value;
        for part in parents {
            node = node
                .get_mut(*part)
                .ok_or_else(|| format!("{} is not a manifest field", change.slot))?;
        }
        let object = node
            .as_object_mut()
            .ok_or_else(|| format!("{} is not a manifest field", change.slot))?;
        object.insert((*last).to_string(), change.value.clone());
    }
    let manifest: Manifest = serde_json::from_value(value)
        .map_err(|error| format!("the candidate manifest is invalid: {error}"))?;
    manifest.validate()?;
    Ok(manifest)
}

/// Every combination of the slots' values, in slot order.
#[must_use]
pub fn grid(slots: &[Slot]) -> Vec<Vec<Change>> {
    let mut out: Vec<Vec<Change>> = vec![Vec::new()];
    for slot in slots {
        let mut next = Vec::with_capacity(out.len() * slot.values.len());
        for prefix in &out {
            for value in &slot.values {
                let mut changes = prefix.clone();
                changes.push(Change {
                    slot: slot.id.clone(),
                    value: value.clone(),
                });
                next.push(changes);
            }
        }
        out = next;
    }
    out.retain(|changes| !changes.is_empty());
    out
}

/// `n` uniform draws from the slots' value lists.
#[must_use]
pub fn random(slots: &[Slot], n: usize, seed: u64) -> Vec<Vec<Change>> {
    let mut rng = Rng::new(seed);
    (0..n)
        .map(|_| {
            slots
                .iter()
                .map(|slot| Change {
                    slot: slot.id.clone(),
                    value: slot.values[rng.below(slot.values.len())].clone(),
                })
                .collect()
        })
        .collect()
}

/// One step of a numeric slot from `current`, up or down, within bounds.
fn neighbors(slot: &Slot, current: f64) -> Vec<f64> {
    let (Some(step), Some(min), Some(max)) = (slot.step, slot.min, slot.max) else {
        return Vec::new();
    };
    [current - step, current + step]
        .into_iter()
        .map(|v| (v * 100.0).round() / 100.0)
        .filter(|v| *v >= min - 1e-9 && *v <= max + 1e-9)
        .collect()
}

/// What a climb did.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Climb {
    /// The incumbent after each accepted move, starting at the baseline.
    pub path: Vec<String>,
    /// Candidates the climb evaluated, including rejected neighbors.
    pub evaluated: usize,
    /// Why it stopped.
    pub stopped: String,
}

/// Coordinate ascent: from the baseline, evaluate one step up and down in
/// each numeric slot, move to the best neighbor when it beats the
/// incumbent by more than `min_gain`, and stop when none does or after
/// `max_moves`. `score` returns a candidate's objective on the search
/// partition, or `None` when it can't be evaluated.
pub fn climb(
    archive: &mut Archive,
    base: &Manifest,
    baseline: &str,
    slots: &[Slot],
    max_moves: usize,
    min_gain: f64,
    score: &mut dyn FnMut(&str, &Manifest) -> Option<f64>,
) -> Climb {
    let proposer = Proposer {
        operator: "climb".to_string(),
        algorithm: format!(
            "coordinate ascent: one step up and down per numeric slot, best neighbor, \
             stop below a gain of {min_gain} or after {max_moves} moves"
        ),
    };
    let current_value = |slot: &Slot, changes: &BTreeMap<String, Value>| -> Option<f64> {
        if let Some(value) = changes.get(&slot.id) {
            return value.as_f64();
        }
        let value = serde_json::to_value(base).ok()?;
        let pointer = format!("/{}", slot.id.replace('.', "/"));
        value.pointer(&pointer).and_then(Value::as_f64).or_else(|| {
            // An unset `brief.pack` field reads as the packer's default.
            let defaults = serde_json::to_value(PackPolicy::default()).ok()?;
            let field = slot.id.strip_prefix("policy.brief.pack.")?;
            defaults.get(field).and_then(Value::as_f64)
        })
    };
    let mut incumbent = baseline.to_string();
    let mut incumbent_changes: BTreeMap<String, Value> = BTreeMap::new();
    let Some(mut best) = score(baseline, base) else {
        return Climb {
            path: vec![incumbent],
            evaluated: 0,
            stopped: "the baseline could not be evaluated".to_string(),
        };
    };
    let mut path = vec![incumbent.clone()];
    let mut evaluated = 0;
    for _ in 0..max_moves {
        let mut round_best: Option<(f64, String, BTreeMap<String, Value>)> = None;
        for slot in slots {
            let Some(current) = current_value(slot, &incumbent_changes) else {
                continue;
            };
            for next in neighbors(slot, current) {
                let mut changes = incumbent_changes.clone();
                changes.insert(slot.id.clone(), super::space::number(next));
                let list: Vec<Change> = changes
                    .iter()
                    .map(|(slot, value)| Change {
                        slot: slot.clone(),
                        value: value.clone(),
                    })
                    .collect();
                let Some(digest) =
                    archive.propose(base, vec![incumbent.clone()], list, proposer.clone())
                else {
                    continue;
                };
                let Some(manifest) = archive.get(&digest).and_then(|p| p.manifest.clone()) else {
                    continue;
                };
                evaluated += 1;
                if let Some(value) = score(&digest, &manifest)
                    && round_best.as_ref().is_none_or(|(b, ..)| value > *b)
                {
                    round_best = Some((value, digest, changes));
                }
            }
        }
        match round_best {
            Some((value, digest, changes)) if value > best + min_gain => {
                best = value;
                incumbent = digest;
                incumbent_changes = changes;
                path.push(incumbent.clone());
            }
            _ => {
                return Climb {
                    path,
                    evaluated,
                    stopped: format!("no neighbor gained more than {min_gain}"),
                };
            }
        }
    }
    Climb {
        path,
        evaluated,
        stopped: format!("reached {max_moves} moves"),
    }
}

/// A reflective edit file: text edits a person or a model proposed after
/// reading failing traces and per-component metrics, never the verifier's
/// tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reflection {
    /// Who proposed the edits: a person's handle, or a model and its
    /// prompt's digest.
    pub proposer: String,
    pub edits: Vec<ReflectedEdit>,
}

/// One reflective edit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReflectedEdit {
    pub slot: String,
    pub value: Value,
    /// What the proposer read and why the edit should help.
    pub rationale: String,
}

/// The slots a reflective edit may change: those whose surface is
/// instructions.
pub const REFLECTABLE: &[&str] = &["policy.brief.directions", "policy.executor.system"];

/// Records each edit in `path` as a proposal from the study baseline.
///
/// # Errors
///
/// Returns a message when the file can't be read.
pub fn reflect(
    archive: &mut Archive,
    base: &Manifest,
    baseline: &str,
    path: &Path,
) -> Result<Vec<String>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let reflection: Reflection = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not a reflection file: {error}", path.display()))?;
    let proposer = Proposer {
        operator: "reflect".to_string(),
        algorithm: format!(
            "reflective text edit proposed by {}; not GEPA",
            reflection.proposer
        ),
    };
    let mut built = Vec::new();
    for edit in reflection.edits {
        let changes = vec![Change {
            slot: edit.slot.clone(),
            value: edit.value,
        }];
        if !REFLECTABLE.contains(&edit.slot.as_str()) {
            archive.refuse(
                changes,
                proposer.clone(),
                format!(
                    "{} is not an instruction slot; reflective edits change only {}",
                    edit.slot,
                    REFLECTABLE.join(" and ")
                ),
            );
            continue;
        }
        if let Some(digest) =
            archive.propose(base, vec![baseline.to_string()], changes, proposer.clone())
        {
            built.push(digest);
        }
    }
    Ok(built)
}

/// Records a router refit as a proposal the manifest can't carry yet.
pub fn router_refit(archive: &mut Archive, table: Value) {
    archive.refuse(
        vec![Change {
            slot: "policy.control.route".to_string(),
            value: table,
        }],
        Proposer {
            operator: "router-refit".to_string(),
            algorithm: "decision stump on Jev task features, leave-one-task-out".to_string(),
        },
        "the manifest has no policy.control.route slot, so a router table can't become a \
         candidate; fit and score routers with `gym coder router`"
            .to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::Packer;

    fn base() -> Manifest {
        let text =
            std::fs::read_to_string(crate::policy::reference_dir().join("pack-luna.json")).unwrap();
        let mut manifest = Manifest::parse(&text).unwrap();
        manifest.policy.brief.packer = Packer::Coverage;
        manifest
    }

    fn change(slot: &str, value: Value) -> Change {
        Change {
            slot: slot.to_string(),
            value,
        }
    }

    #[test]
    fn apply_builds_a_valid_manifest_or_says_why_not() {
        let base = base();
        let built = apply(
            &base,
            &[
                change("policy.brief.cap", json!(9000)),
                change("policy.brief.pack.slice", json!(800)),
            ],
        )
        .unwrap();
        assert_eq!(built.policy.brief.cap, 9000);
        let pack = built.policy.brief.pack.unwrap();
        assert_eq!(pack.slice, 800);
        assert_eq!(pack.item_max, PackPolicy::default().item_max);
        assert_ne!(built.digest(), base.digest());
        // A protected or canary-less field is refused.
        let error = apply(&base, &[change("protected.acceptance", json!("x"))]).unwrap_err();
        assert!(error.contains("no passing canary"), "{error}");
        let error = apply(&base, &[change("policy.brief.pack.slice", json!(50))]).unwrap_err();
        assert!(error.contains("slice must be 200"), "{error}");
        let error = apply(&base, &[change("policy.brief.cap", json!("big"))]).unwrap_err();
        assert!(error.contains("invalid"), "{error}");
    }

    #[test]
    fn the_archive_files_candidates_by_digest_and_keeps_refusals() {
        let base = base();
        let mut archive = Archive::default();
        let proposer = Proposer {
            operator: "grid".to_string(),
            algorithm: "grid".to_string(),
        };
        let a = archive
            .propose(
                &base,
                vec![],
                vec![change("policy.brief.cap", json!(9000))],
                proposer.clone(),
            )
            .unwrap();
        let again = archive
            .propose(
                &base,
                vec![],
                vec![change("policy.brief.cap", json!(9000))],
                proposer.clone(),
            )
            .unwrap();
        assert_eq!(a, again);
        assert!(
            archive
                .propose(
                    &base,
                    vec![],
                    vec![change("policy.brief.cap", json!(10))],
                    proposer
                )
                .is_none()
        );
        assert_eq!(archive.proposals.len(), 3);
        assert_eq!(archive.built(), vec![a.clone()]);
        assert!(matches!(
            archive.proposals[1].construction,
            super::super::Construction::Duplicate { .. }
        ));
        assert!(matches!(
            archive.proposals[2].construction,
            super::super::Construction::Refused { .. }
        ));
    }

    #[test]
    fn grid_and_random_cover_the_listed_values() {
        let slots = vec![
            Slot::numeric(
                "policy.brief.cap",
                &[6000.0, 12000.0],
                1000.0,
                4000.0,
                16000.0,
            ),
            Slot::numeric(
                "policy.brief.pack.slice",
                &[400.0, 800.0, 1200.0],
                100.0,
                200.0,
                2000.0,
            ),
        ];
        validate(&slots).unwrap();
        assert_eq!(grid(&slots).len(), 6);
        let draws = random(&slots, 5, 3);
        assert_eq!(draws, random(&slots, 5, 3));
        assert!(draws.iter().all(|d| d.len() == 2));
        let bad = vec![Slot::numeric(
            "policy.control.max_steps",
            &[1.0],
            1.0,
            1.0,
            9.0,
        )];
        assert!(validate(&bad).unwrap_err().contains("no passing canary"));
    }

    #[test]
    fn the_climb_moves_uphill_and_stops() {
        let base = base();
        let baseline = base.digest();
        let mut archive = Archive::default();
        archive.propose(
            &base,
            vec![],
            vec![],
            Proposer {
                operator: "baseline".to_string(),
                algorithm: "hand-authored".to_string(),
            },
        );
        let slots = vec![Slot::numeric(
            "policy.brief.cap",
            &[12000.0],
            1000.0,
            8000.0,
            16000.0,
        )];
        // The objective peaks at a cap of 10,000.
        let mut score =
            |_: &str, m: &Manifest| Some(-((m.policy.brief.cap as f64 - 10_000.0).abs()));
        let climb = climb(&mut archive, &base, &baseline, &slots, 10, 1e-6, &mut score);
        let last = archive.get(climb.path.last().unwrap()).unwrap();
        assert_eq!(last.manifest.as_ref().unwrap().policy.brief.cap, 10_000);
        assert_eq!(climb.path.len(), 3);
        assert!(climb.stopped.contains("no neighbor"));
    }

    #[test]
    fn reflective_edits_touch_only_instruction_slots_and_name_their_proposer() {
        let base = base();
        let baseline = base.digest();
        let mut archive = Archive::default();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("edits.json");
        std::fs::write(
            &path,
            json!({
                "proposer": "a person, by hand",
                "edits": [
                    { "slot": "policy.brief.directions", "value": "plain", "rationale": "shorter" },
                    { "slot": "policy.brief.cap", "value": 9000, "rationale": "not text" },
                ]
            })
            .to_string(),
        )
        .unwrap();
        let built = reflect(&mut archive, &base, &baseline, &path).unwrap();
        assert_eq!(built.len(), 1);
        assert!(
            archive
                .proposals
                .iter()
                .all(|p| p.proposer.algorithm.contains("not GEPA"))
        );
        assert_eq!(archive.proposals.len(), 2);
        router_refit(&mut archive, json!({ "stump": "concurrency" }));
        assert!(matches!(
            &archive.proposals[2].construction,
            super::super::Construction::Refused { reason } if reason.contains("control.route")
        ));
    }
}
