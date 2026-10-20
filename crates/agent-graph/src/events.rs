//! Chat activity is a separate input; it never changes the graph definition.

use serde::{Deserialize, Serialize};

use crate::{Detail, GraphSpec};

/// One chat event, with its recorded time in Unix milliseconds.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Event {
    pub at: u64,
    #[serde(flatten)]
    pub kind: Kind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    CheckIn,
    ToolCall { name: String },
    DecisionRecorded,
    SuiteCase,
}

/// Reads recorded chat steps. Host events use `extensions.agent_graph`;
/// tool calls use the chat's structured `call`, never message text.
pub fn from_chat(json: &str) -> Result<Vec<Event>, serde_json::Error> {
    #[derive(Deserialize)]
    struct Step {
        at: u64,
        #[serde(default)]
        call: Option<Call>,
        #[serde(default)]
        extensions: serde_json::Map<String, serde_json::Value>,
    }
    #[derive(Deserialize)]
    struct Call {
        name: String,
        outcome: String,
    }
    let steps: Vec<Step> = serde_json::from_str(json)?;
    let mut events = Vec::new();
    for step in steps {
        if let Some(value) = step.extensions.get("agent_graph") {
            let kind: Kind = serde_json::from_value(value.clone())?;
            events.push(Event { at: step.at, kind });
        }
        if let Some(call) = step.call {
            if call.name == "record_decision" && call.outcome == "Completed" {
                events.push(Event {
                    at: step.at,
                    kind: Kind::DecisionRecorded,
                });
            }
            events.push(Event {
                at: step.at,
                kind: Kind::ToolCall { name: call.name },
            });
        }
    }
    Ok(events)
}

/// A single piece of data travels for one second, then disappears.
pub fn phase(at: u64, now: u64) -> Option<f32> {
    let age = now.checked_sub(at)?;
    (age < 1000).then_some(age as f32 / 1000.0)
}

impl Event {
    /// Indices of existing links this event travels. Unknown tools light nothing.
    pub fn links(&self, spec: &GraphSpec) -> Vec<usize> {
        let mut ends = Vec::new();
        match &self.kind {
            Kind::CheckIn => ends.push(("check-ins", "wake", "runtime", "wake")),
            Kind::DecisionRecorded => ends.push(("tools-memory", "record", "memory", "record")),
            Kind::SuiteCase => ends.push(("runtime", "reply", "tests", "reply")),
            Kind::ToolCall { name } => {
                for unit in &spec.units {
                    if unit.parent.as_deref() == Some("tools")
                        && unit.details.iter().any(
                            |d| matches!(d, Detail::Chips { items, .. } if items.contains(name)),
                        )
                    {
                        ends.push(("runtime", "call", "authority", "call"));
                        ends.push(("authority", "allowed", unit.id.as_str(), "call"));
                    }
                }
            }
        }
        spec.links
            .iter()
            .enumerate()
            .filter_map(|(i, link)| {
                ends.contains(&(
                    link.from.unit.as_str(),
                    link.from.pin.as_str(),
                    link.to.unit.as_str(),
                    link.to.pin.as_str(),
                ))
                .then_some(i)
            })
            .collect()
    }
}

#[cfg(all(test, feature = "lab"))]
mod tests {
    use super::*;

    #[test]
    fn events_light_only_existing_links_once() {
        let events = from_chat(r#"[{"at":100,"message":"record_decision"},{"at":200,"call":{"name":"record_decision","outcome":"Completed"}},{"at":300,"extensions":{"agent_graph":{"type":"check_in"}}}]"#).unwrap();
        assert_eq!(events.len(), 3);
        let spec = crate::foreman::graph();
        let before = serde_json::to_string(&spec).unwrap();
        for (kind, count) in [
            (Kind::CheckIn, 1),
            (
                Kind::ToolCall {
                    name: "read_chat".into(),
                },
                2,
            ),
            (
                Kind::ToolCall {
                    name: "unknown".into(),
                },
                0,
            ),
            (Kind::DecisionRecorded, 1),
            (Kind::SuiteCase, 1),
        ] {
            assert_eq!(Event { at: 100, kind }.links(&spec).len(), count);
        }
        assert_eq!(phase(100, 99), None);
        assert_eq!(phase(100, 100), Some(0.0));
        assert_eq!(phase(100, 600), Some(0.5));
        assert_eq!(phase(100, 1100), None);
        assert_eq!(before, serde_json::to_string(&spec).unwrap());
    }
}
