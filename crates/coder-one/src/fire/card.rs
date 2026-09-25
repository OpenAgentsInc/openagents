//! A strategy card: how Fable 5.1's winning runs solved one task, in
//! phases, with the times they reached each one.
//!
//! Cards live in `bench/terminal-bench/fire/cards/`. Only the host-side
//! judge reads a card; nothing in it reaches the run under test.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// The card's schema name.
pub const SCHEMA: &str = "openagents.fire.card.v1";

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Card {
    pub schema: String,
    pub task: String,
    pub sources: Vec<String>,
    pub fable: Fable,
    pub strategy: String,
    pub phases: Vec<Phase>,
    pub independent_check: String,
    pub must_not: Vec<String>,
    pub pitfalls: Vec<Pitfall>,
    pub budget: Budget,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Fable {
    pub passes: String,
    pub effort: String,
    pub median_seconds: f64,
    pub median_steps: f64,
    pub median_cost_usd: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Phase {
    pub id: String,
    pub what: String,
    pub done_when: String,
    pub fable_seconds: Span,
    pub signals: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Span {
    pub start: f64,
    pub end: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Pitfall {
    pub id: String,
    pub what: String,
}

/// Seconds from the start by which the winners had reached each point.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Budget {
    pub first_read_done_s: f64,
    pub first_edit_s: f64,
    pub first_check_s: f64,
    pub done_s: f64,
}

impl Card {
    /// Reads a card.
    ///
    /// # Errors
    ///
    /// A message when the file can't be read, isn't JSON, has another
    /// schema, or has no phases.
    pub fn load(path: &Path) -> Result<Card, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("can't read the card {}: {error}", path.display()))?;
        let card: Card = serde_json::from_str(&text)
            .map_err(|error| format!("the card {} isn't valid: {error}", path.display()))?;
        if card.schema != SCHEMA {
            return Err(format!(
                "the card {} has schema {:?}, not {SCHEMA}",
                path.display(),
                card.schema
            ));
        }
        if card.phases.is_empty() {
            return Err(format!("the card {} lists no phases", path.display()));
        }
        Ok(card)
    }

    /// The phase with `id`.
    #[must_use]
    pub fn phase(&self, id: &str) -> Option<&Phase> {
        self.phases.iter().find(|phase| phase.id == id)
    }
}
