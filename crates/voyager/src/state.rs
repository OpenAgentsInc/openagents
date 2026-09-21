//! What the agent knows about itself, decoded from a `state` answer.
//!
//! The fields mirror what the Voyager paper feeds its curriculum and
//! critic: where the agent is, how healthy and fed it is, what it holds,
//! and what is around it. Everything here parses tolerantly — a field the
//! helper did not send is empty, not an error — because the observation
//! is evidence, and missing evidence is itself a fact.

use std::collections::BTreeMap;

use serde_json::Value;

/// One `state` response, decoded.
#[derive(Clone, Debug, Default)]
pub struct AgentState {
    /// Where the bot stands, `x`/`y`/`z`.
    pub position: [f64; 3],
    /// Half-hearts, as vanilla reports: 20 is full.
    pub health: f64,
    /// Hunger points, 20 is full.
    pub food: i64,
    /// Saturation behind the hunger bar.
    pub saturation: f64,
    /// Item name to count, aggregated across the inventory.
    pub inventory: BTreeMap<String, i64>,
    /// Distinct non-air block names within the scan radius.
    pub nearby_blocks: Vec<String>,
    /// Entity kinds within roughly twice the scan radius.
    pub nearby_entities: Vec<String>,
}

impl AgentState {
    /// Decodes the `result` of a `state` call.
    #[must_use]
    pub fn from_result(result: &Value) -> Self {
        let mut state = AgentState::default();
        if let Some(position) = result.get("position") {
            state.position = [
                position.get("x").and_then(Value::as_f64).unwrap_or(0.0),
                position.get("y").and_then(Value::as_f64).unwrap_or(0.0),
                position.get("z").and_then(Value::as_f64).unwrap_or(0.0),
            ];
        }
        state.health = result.get("health").and_then(Value::as_f64).unwrap_or(0.0);
        state.food = result.get("food").and_then(Value::as_i64).unwrap_or(0);
        state.saturation = result
            .get("saturation")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if let Some(inventory) = result.get("inventory").and_then(Value::as_object) {
            for (name, count) in inventory {
                state
                    .inventory
                    .insert(name.clone(), count.as_i64().unwrap_or(0));
            }
        }
        for (key, target) in [
            ("nearby_blocks", &mut state.nearby_blocks),
            ("nearby_entities", &mut state.nearby_entities),
        ] {
            if let Some(list) = result.get(key).and_then(Value::as_array) {
                target.extend(list.iter().filter_map(Value::as_str).map(str::to_string));
            }
        }
        state
    }

    /// How many of `item` the inventory holds. The bridge reports registry
    /// names (`minecraft:oak_log`), so a bare name checks both spellings.
    #[must_use]
    pub fn count(&self, item: &str) -> i64 {
        if item.contains(':') {
            return self.inventory.get(item).copied().unwrap_or(0);
        }
        self.inventory
            .iter()
            .filter(|(name, _)| name.strip_prefix("minecraft:").unwrap_or(name.as_str()) == item)
            .map(|(_, count)| *count)
            .sum()
    }

    /// Whether any nearby block name ends with `suffix` — `oak_log`,
    /// `birch_log`, and friends all answer `*_log`.
    #[must_use]
    pub fn sees(&self, suffix: &str) -> bool {
        self.nearby_blocks.iter().any(|name| name.ends_with(suffix))
    }

    /// The nearby block names that end with `suffix`.
    #[must_use]
    pub fn matching(&self, suffix: &str) -> Vec<String> {
        self.nearby_blocks
            .iter()
            .filter(|name| name.ends_with(suffix))
            .cloned()
            .collect()
    }

    /// A one-line summary for chat and traces.
    #[must_use]
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        parts.push(format!(
            "at ({:.0}, {:.0}, {:.0})",
            self.position[0], self.position[1], self.position[2]
        ));
        parts.push(format!("health {:.0} food {}", self.health, self.food));
        if !self.inventory.is_empty() {
            let items: Vec<String> = self
                .inventory
                .iter()
                .map(|(name, count)| format!("{name}x{count}"))
                .collect();
            parts.push(format!("holding {}", items.join(", ")));
        }
        if !self.nearby_blocks.is_empty() {
            let shown = 12.min(self.nearby_blocks.len());
            let mut list: Vec<String> = self.nearby_blocks[..shown]
                .iter()
                .map(|name| name.trim_start_matches("minecraft:").to_string())
                .collect();
            if self.nearby_blocks.len() > shown {
                list.push(format!("+{} more", self.nearby_blocks.len() - shown));
            }
            parts.push(format!("sees {}", list.join(", ")));
        }
        parts.join("; ")
    }
}
