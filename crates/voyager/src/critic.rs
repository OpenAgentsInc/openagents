//! The critic: did the task actually succeed?
//!
//! The paper asks a model whether the agent's last program achieved
//! its goal. This stack can do better — where a ruleset declares a
//! mechanical check, a deterministic answer beats a calibrated one —
//! so a task's `verify` spec is a menu, not a single call:
//!
//! - [`Spec::Moved`]: the position delta reached a minimum.
//! - [`Spec::Inventory`]: an item count grew by a minimum.
//! - [`Spec::BlockAt`]: a position holds the declared kind.
//! - [`Spec::Noul`]: a `noul` question to the decision door over the
//!   before/after state — the paper's own shape, for goals no
//!   mechanical rule covers.
//! - [`Spec::Ran`]: the program ran to completion — the weakest
//!   honest answer, recorded as what it is.
//!
//! A verification is evidence either way: the spec, the readings, and
//! the verdict land in the task's record, and a `noul` call keeps its
//! full request and response beside the other decision records.

use serde::Deserialize;
use serde_json::Value;

use crate::decide::Door;
use crate::error::Result;
use crate::state::AgentState;

/// How a task proves it finished — the manifest's `verify` value.
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
#[derive(Default)]
pub enum Spec {
    /// The bot moved at least `min_blocks` horizontally.
    #[serde(rename = "moved")]
    Moved {
        /// The smallest horizontal distance that counts.
        min_blocks: f64,
    },
    /// An inventory item grew by at least `at_least`. `item` is a bare
    /// name or a `_`-suffix match — `_log` covers every log kind.
    #[serde(rename = "inventory")]
    Inventory {
        /// The item name or suffix to count.
        item: String,
        /// The smallest gain that counts.
        at_least: i64,
    },
    /// A position holds a block kind after the task.
    #[serde(rename = "block_at")]
    BlockAt {
        /// `[x, y, z]` to read.
        position: [i32; 3],
        /// The kind it must hold, such as `oak_planks`.
        block: String,
    },
    /// The decision door answers whether the task succeeded, given
    /// the goal and the before/after state.
    #[serde(rename = "noul")]
    Noul,
    /// No check — the program running to completion is the record.
    #[serde(rename = "ran")]
    #[default]
    Ran,
}

/// What one verification concluded.
#[derive(Clone, Debug)]
pub struct Verdict {
    /// Whether the task succeeded.
    pub ok: bool,
    /// The evidence line — the numbers the check read, or the door's
    /// answer and its probability.
    pub detail: String,
}

/// What the check reads: the agent's state before the program ran and
/// after, plus the blocks the program read back where a spec asks.
pub struct Readings<'a> {
    /// State before the attempt.
    pub before: &'a AgentState,
    /// State after the attempt.
    pub after: &'a AgentState,
    /// `block_at` readings the attempt took: `(position, kind)`.
    pub blocks: &'a [([i32; 3], String)],
}

/// Runs a mechanical spec against the readings. `Noul` is not
/// answerable here — [`verify`] routes it to the door.
#[must_use]
pub fn check(spec: &Spec, readings: &Readings<'_>) -> Option<Verdict> {
    match spec {
        Spec::Moved { min_blocks } => {
            let [bx, _, bz] = readings.before.position;
            let [ax, _, az] = readings.after.position;
            let moved = ((ax - bx).powi(2) + (az - bz).powi(2)).sqrt();
            Some(Verdict {
                ok: moved >= *min_blocks,
                detail: format!("moved {moved:.1} blocks (needed {min_blocks})"),
            })
        }
        Spec::Inventory { item, at_least } => {
            let (before, after) = (count(readings.before, item), count(readings.after, item));
            let gained = after - before;
            Some(Verdict {
                ok: gained >= *at_least,
                detail: format!("holds {after} {item} (+{gained}, needed {at_least})"),
            })
        }
        Spec::BlockAt { position, block } => {
            let found = readings
                .blocks
                .iter()
                .find(|(pos, _)| pos == position)
                .map(|(_, kind)| kind.as_str());
            fn normalized(kind: &str) -> &str {
                kind.strip_prefix("minecraft:").unwrap_or(kind)
            }
            let expected = normalized(block);
            Some(Verdict {
                ok: found.is_some_and(|kind| normalized(kind) == expected),
                detail: format!(
                    "{position:?} holds {} (wanted {expected})",
                    found.unwrap_or("nothing")
                ),
            })
        }
        Spec::Noul => None,
        Spec::Ran => Some(Verdict {
            ok: true,
            detail: "the program ran to completion".to_string(),
        }),
    }
}

/// The full critic: mechanical first, the door for what mechanics
/// cannot say. `door` may be absent — a `Noul` spec with no door
/// reports itself unanswerable rather than guessing.
///
/// # Errors
///
/// The door's own errors pass through; everything else is a verdict.
pub fn verify(
    spec: &Spec,
    readings: &Readings<'_>,
    goal: &str,
    door: Option<&mut Door>,
    purpose: &str,
) -> Result<Verdict> {
    if let Some(verdict) = check(spec, readings) {
        return Ok(verdict);
    }
    let Some(door) = door else {
        return Ok(Verdict {
            ok: false,
            detail: "the task's only check is a decision call and no door is wired".to_string(),
        });
    };
    let state = serde_json::json!({
        "goal": goal,
        "before": describe(readings.before),
        "after": describe(readings.after),
        "blocks_read": readings.blocks.iter().map(|(pos, kind)| {
            serde_json::json!({"position": pos, "kind": kind})
        }).collect::<Vec<_>>(),
    });
    let noul = door.verify(
        state,
        "Did the agent accomplish the stated goal? Answer by what the state shows, not what was attempted.",
        purpose,
    )?;
    Ok(Verdict {
        ok: noul.probability >= 0.5,
        detail: format!("the door answers {:.2}", noul.probability),
    })
}

/// Count an item spec — a bare name matches its `minecraft:` form, a
/// leading `_` matches every kind with that suffix.
fn count(state: &AgentState, item: &str) -> i64 {
    if let Some(suffix) = item.strip_prefix('_') {
        return state
            .inventory
            .iter()
            .filter(|(name, _)| name.ends_with(&format!("_{suffix}")))
            .map(|(_, count)| *count)
            .sum();
    }
    state.count(item)
}

/// The state summary the `noul` question reads.
fn describe(state: &AgentState) -> Value {
    serde_json::json!({
        "position": state.position,
        "health": state.health,
        "food": state.food,
        "inventory": state.inventory,
        "nearby_blocks": state.nearby_blocks,
        "nearby_entities": state.nearby_entities,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_at(x: f64, z: f64, items: &[(&str, i64)]) -> AgentState {
        let mut state = AgentState {
            position: [x, 0.0, z],
            ..Default::default()
        };
        for (name, count) in items {
            state.inventory.insert(name.to_string(), *count);
        }
        state
    }

    #[test]
    fn moved_reads_the_horizontal_delta() {
        let before = state_at(0.0, 0.0, &[]);
        let after = state_at(10.0, 0.0, &[]);
        let readings = Readings {
            before: &before,
            after: &after,
            blocks: &[],
        };
        let verdict = check(&Spec::Moved { min_blocks: 8.0 }, &readings).unwrap();
        assert!(verdict.ok);
        let verdict = check(&Spec::Moved { min_blocks: 12.0 }, &readings).unwrap();
        assert!(!verdict.ok);
    }

    #[test]
    fn inventory_counts_suffix_matches() {
        let before = state_at(0.0, 0.0, &[("minecraft:oak_log", 1)]);
        let after = state_at(
            0.0,
            0.0,
            &[("minecraft:oak_log", 3), ("minecraft:birch_log", 2)],
        );
        let readings = Readings {
            before: &before,
            after: &after,
            blocks: &[],
        };
        let verdict = check(
            &Spec::Inventory {
                item: "_log".to_string(),
                at_least: 3,
            },
            &readings,
        )
        .unwrap();
        assert!(verdict.ok, "{}", verdict.detail);
    }

    #[test]
    fn block_at_normalizes_the_prefix() {
        let before = state_at(0.0, 0.0, &[]);
        let after = state_at(0.0, 0.0, &[]);
        let blocks = vec![([1, 0, -5], "minecraft:oak_planks".to_string())];
        let readings = Readings {
            before: &before,
            after: &after,
            blocks: &blocks,
        };
        let verdict = check(
            &Spec::BlockAt {
                position: [1, 0, -5],
                block: "oak_planks".to_string(),
            },
            &readings,
        )
        .unwrap();
        assert!(verdict.ok);
    }

    #[test]
    fn noul_needs_the_door() {
        let before = state_at(0.0, 0.0, &[]);
        let readings = Readings {
            before: &before,
            after: &before,
            blocks: &[],
        };
        assert!(check(&Spec::Noul, &readings).is_none());
        let verdict = verify(&Spec::Noul, &readings, "do a thing", None, "test").unwrap();
        assert!(!verdict.ok);
    }
}
