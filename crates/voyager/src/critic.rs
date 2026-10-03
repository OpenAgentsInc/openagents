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
    /// Minimum server-reported XP earned during this task.
    #[serde(rename = "xp_gained")]
    XpGained { at_least: u64 },
    /// Minimum observed character level.
    #[serde(rename = "level_at_least")]
    LevelAtLeast { level: u64 },
    /// Server quest-log status or confirmed reward event.
    #[serde(rename = "quest_status")]
    QuestStatus { quest: u32, status: String },
    /// Absolute item count by WoW item entry.
    #[serde(rename = "item_count")]
    ItemCount { entry: u32, at_least: u64 },
    /// Attributed kills during this task.
    #[serde(rename = "killed")]
    Killed { entry: u32, count: u64 },
    /// Map and distance from a declared WoW coordinate.
    #[serde(rename = "at_position")]
    AtPosition {
        map: u32,
        position: [f64; 3],
        radius: f64,
    },
    /// All checks must pass.
    #[serde(rename = "all")]
    All { checks: Vec<Spec> },
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
        Spec::All { checks } => {
            let verdicts: Option<Vec<_>> = checks.iter().map(|s| check(s, readings)).collect();
            verdicts.map(|vs| Verdict {
                ok: !vs.is_empty() && vs.iter().all(|v| v.ok),
                detail: vs
                    .iter()
                    .map(|v| v.detail.as_str())
                    .collect::<Vec<_>>()
                    .join("; "),
            })
        }
        Spec::XpGained { at_least } => wow_number(readings, "earned_xp", None, *at_least, true),
        Spec::LevelAtLeast { level } => wow_number(readings, "level", None, *level, false),
        Spec::ItemCount { entry, at_least } => wow_number(
            readings,
            "inventory",
            Some(entry.to_string()),
            *at_least,
            false,
        ),
        Spec::Killed { entry, count } => {
            wow_number(readings, "killed", Some(entry.to_string()), *count, true)
        }
        Spec::QuestStatus { quest, status } => {
            let w = &readings.after.wow;
            let q = w["quests"]
                .as_array()
                .and_then(|qs| qs.iter().find(|q| q["id"] == *quest));
            let ok = match status.as_str() {
                "accepted" => q.is_some(),
                "complete" => q
                    .and_then(|q| q["state"].as_u64())
                    .is_some_and(|s| s & 1 != 0),
                "turned_in" => w["turned_in"]
                    .as_array()
                    .is_some_and(|qs| qs.iter().any(|q| *q == *quest)),
                _ => false,
            };
            Some(Verdict {
                ok,
                detail: format!("quest {quest} {status}: {ok}"),
            })
        }
        Spec::AtPosition {
            map,
            position,
            radius,
        } => {
            let w = &readings.after.wow;
            let p = w["position"].as_array().filter(|p| p.len() == 3);
            let coords: Option<Vec<f64>> = p.and_then(|p| p.iter().map(Value::as_f64).collect());
            let distance = coords.map(|p| {
                p.iter()
                    .zip(position)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt()
            });
            Some(Verdict {
                ok: *radius >= 0.0 && w["map"] == *map && distance.is_some_and(|d| d <= *radius),
                detail: format!("map {map}, distance {distance:?}, radius {radius}"),
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
        "wow": state.wow,
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

fn wow_number(
    r: &Readings<'_>,
    key: &str,
    entry: Option<String>,
    threshold: u64,
    delta: bool,
) -> Option<Verdict> {
    let read = |s: &AgentState| {
        let v = &s.wow[key];
        if let Some(e) = &entry {
            if !v.is_object() {
                None
            } else {
                Some(v[e].as_u64().unwrap_or(0))
            }
        } else {
            v.as_u64()
        }
    };
    let after = read(r.after);
    let measured = if delta {
        read(r.before).zip(after).map(|(b, a)| a.saturating_sub(b))
    } else {
        after
    };
    Some(Verdict {
        ok: measured.is_some_and(|n| n >= threshold),
        detail: format!("{key} {entry:?}: {measured:?}, needed {threshold}"),
    })
}

#[cfg(test)]
mod wow_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn missing_evidence_never_passes_wow_checks() {
        let s = AgentState::default();
        let r = Readings {
            before: &s,
            after: &s,
            blocks: &[],
        };
        for spec in [
            Spec::XpGained { at_least: 0 },
            Spec::LevelAtLeast { level: 0 },
            Spec::ItemCount {
                entry: 117,
                at_least: 0,
            },
            Spec::Killed { entry: 6, count: 0 },
            Spec::QuestStatus {
                quest: 783,
                status: "turned_in".into(),
            },
            Spec::AtPosition {
                map: 0,
                position: [0.0; 3],
                radius: 10.0,
            },
        ] {
            assert!(!check(&spec, &r).unwrap().ok);
        }
    }
    #[test]
    fn server_outcomes_and_task_deltas_grade() {
        let before = AgentState::from_result(&json!({"map":0,"earned_xp":40,"killed":{"6":1}}));
        let after = AgentState::from_result(
            &json!({"map":0,"position":[1.5,2.5,3.5],"level":2,"earned_xp":600,"killed":{"6":11},"inventory":{"117":3},"quests":[{"id":7,"state":1}],"turned_in":[783]}),
        );
        let r = Readings {
            before: &before,
            after: &after,
            blocks: &[],
        };
        let specs = vec![
            Spec::XpGained { at_least: 560 },
            Spec::Killed {
                entry: 6,
                count: 10,
            },
            Spec::LevelAtLeast { level: 2 },
            Spec::ItemCount {
                entry: 117,
                at_least: 3,
            },
            Spec::QuestStatus {
                quest: 7,
                status: "complete".into(),
            },
            Spec::QuestStatus {
                quest: 783,
                status: "turned_in".into(),
            },
            Spec::AtPosition {
                map: 0,
                position: [1.5, 2.5, 3.5],
                radius: 0.0,
            },
        ];
        assert!(check(&Spec::All { checks: specs }, &r).unwrap().ok);
        assert!(
            !check(
                &Spec::Killed {
                    entry: 6,
                    count: 11
                },
                &r
            )
            .unwrap()
            .ok
        );
        assert!(
            !check(
                &Spec::AtPosition {
                    map: 1,
                    position: [1.5, 2.5, 3.5],
                    radius: 1.0
                },
                &r
            )
            .unwrap()
            .ok
        );
    }
}
