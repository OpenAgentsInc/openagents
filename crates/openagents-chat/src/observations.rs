//! Bounded observation state and its bridge into existing task presentation.

use std::collections::VecDeque;

use route_contract::observation as wire;
pub use route_contract::observation::{Body, Channel, Observation, TextMode};

use crate::{
    coder_events::{CoderEvent, Output, Step, StepKind, Verb},
    plan,
};

#[derive(Clone, Debug)]
struct Item {
    id: String,
    seq: u64,
    text: String,
    channel: Channel,
    complete: bool,
    history: bool,
    tool: Option<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Update {
    /// Stable row identity within a task, distinct from delivery sequence.
    pub key: Option<String>,
    pub event: Option<CoderEvent>,
}

/// Provider observations do not change task state or grant control rights.
#[derive(Clone, Debug)]
pub struct Projection {
    host: String,
    task: String,
    turn: usize,
    generation: u64,
    seq: u64,
    items: VecDeque<Item>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub version: Option<String>,
    pub native_session: Option<String>,
    pub plan: Option<Vec<plan::Item>>,
    pub context: Option<(Option<u64>, Option<u64>)>,
    pub usage: Option<wire::Usage>,
    pub rate_limit: Option<(String, Option<u64>)>,
    pub policy: Option<String>,
    pub incomplete: bool,
    pub ended: bool,
}

impl Projection {
    pub fn new(host: &str, task: &str, turn: usize) -> Self {
        Self {
            host: host.into(),
            task: task.into(),
            turn,
            generation: 0,
            seq: 0,
            items: VecDeque::new(),
            provider: None,
            model: None,
            version: None,
            native_session: None,
            plan: None,
            context: None,
            usage: None,
            rate_limit: None,
            policy: None,
            incomplete: false,
            ended: false,
        }
    }

    pub fn turn(&self) -> usize {
        self.turn
    }

    pub fn apply(&mut self, observation: &Observation) -> Option<Update> {
        if !observation.applies_to(&self.host, &self.task, self.turn) {
            return None;
        }
        // A child's output cannot replace parent state, including its session.
        if observation.child.is_some() {
            if observation.scope.generation != self.generation
                || observation.seq <= self.seq
                || self.ended
            {
                return None;
            }
            if observation.seq != self.seq + 1 {
                self.gap();
            }
            self.seq = observation.seq;
            return Some(Update {
                key: None,
                event: None,
            });
        }
        if !matches!(observation.body, Body::Started { .. })
            && self
                .native_session
                .as_ref()
                .zip(observation.native_session.as_ref())
                .is_some_and(|(known, incoming)| known != incoming)
        {
            return None;
        }
        let starts = matches!(observation.body, Body::Started { .. });
        if starts {
            if observation.seq != 1 || observation.scope.generation <= self.generation {
                return None;
            }
            let generation = observation.scope.generation;
            *self = Self::new(&self.host, &self.task, self.turn);
            self.generation = generation;
        } else if observation.scope.generation != self.generation
            || observation.seq <= self.seq
            || self.ended
        {
            return None;
        }
        if observation.seq != self.seq + 1 {
            self.gap();
        }
        self.seq = observation.seq;
        if let Some(session) = &observation.native_session {
            self.native_session = Some(session.chars().take(256).collect());
        }
        let mut update = Update {
            key: None,
            event: None,
        };
        let turn = self.turn;
        let step = |kind, text: String, id| {
            CoderEvent::Step(Step {
                turn,
                step_id: id,
                kind,
                source: "agent".into(),
                text,
                call: None,
                plan: None,
            })
        };
        match &observation.body {
            Body::Started { provider, model } => {
                self.provider = Some(provider.chars().take(256).collect());
                self.model = Some(model.chars().take(256).collect());
                if self.generation > 1 {
                    update.key = Some(self.key("attempt"));
                    update.event = Some(step(
                        StepKind::Note,
                        crate::coder_events::starting(provider),
                        observation.seq,
                    ));
                }
            }
            Body::Session { model, version } => {
                if let Some(model) = model {
                    self.model = Some(model.chars().take(256).collect());
                }
                if let Some(version) = version {
                    self.version = Some(version.chars().take(256).collect());
                }
            }
            Body::Text {
                channel,
                mode,
                text,
                complete,
                truncated,
            } => {
                let Some(id) = observation
                    .item
                    .as_deref()
                    .filter(|id| !id.is_empty() && id.len() <= 256)
                else {
                    self.gap();
                    return Some(update);
                };
                let at = self.items.iter().position(|item| item.id == id);
                let at = match at {
                    Some(at) => at,
                    None if *mode == TextMode::Delta => {
                        self.gap();
                        return Some(update);
                    }
                    None => self.insert(id, observation.seq),
                };
                let item = &mut self.items[at];
                if item.tool.is_some() || (item.channel != *channel && !item.text.is_empty()) {
                    self.gap();
                    return Some(update);
                }
                if *mode == TextMode::Delta && (item.complete || !item.history) {
                    self.gap();
                    return Some(update);
                }
                item.channel = *channel;
                match mode {
                    TextMode::Snapshot => {
                        item.text = wire::bounded(text);
                        item.history = true;
                    }
                    TextMode::Delta => {
                        let remaining = wire::MAX_TEXT.saturating_sub(item.text.len());
                        if text.len() > remaining {
                            self.incomplete = true;
                        }
                        item.text = wire::bounded(&format!("{}{}", item.text, wire::bounded(text)));
                    }
                }
                item.complete = *complete;
                if *truncated || text.len() > wire::MAX_TEXT {
                    self.incomplete = true;
                }
                let text = item.text.clone();
                let seq = item.seq;
                update.key = Some(self.key(&format!("text/{channel:?}/{id}")));
                update.event = Some(step(
                    if *channel == Channel::Text {
                        StepKind::Reply
                    } else {
                        StepKind::Thinking
                    },
                    text,
                    seq,
                ));
            }
            Body::ToolStarted { name, input } => {
                let Some(id) = observation
                    .item
                    .as_deref()
                    .filter(|id| !id.is_empty() && id.len() <= 256)
                else {
                    self.gap();
                    return Some(update);
                };
                let at = self
                    .items
                    .iter()
                    .position(|item| item.id == id)
                    .unwrap_or_else(|| self.insert(id, observation.seq));
                let item = &mut self.items[at];
                if item.complete {
                    return Some(update);
                }
                let arguments = serde_json::from_str::<serde_json::Value>(input)
                    .unwrap_or_else(|_| serde_json::Value::String(wire::bounded(input)));
                let kind = match name.as_str() {
                    "Edit" | "Write" => "edit",
                    "Delete" => "delete",
                    "Move" => "move",
                    _ => "",
                };
                let (label, call) = crate::coder_events::tool_call(
                    &serde_json::json!({"function_name": name, "arguments": arguments}),
                    &serde_json::json!({"observation_tool": {"kind": kind}}),
                );
                item.tool = Some((name.chars().take(256).collect(), call.target.clone()));
                let bash = call.verb == Verb::Run;
                let mut event = step(
                    if bash {
                        StepKind::Command
                    } else {
                        StepKind::ToolCall
                    },
                    if bash { call.target.clone() } else { label },
                    item.seq,
                );
                if let CoderEvent::Step(step) = &mut event {
                    step.call = Some(call);
                }
                update.key = Some(self.key(&format!("tool/{id}/start")));
                update.event = Some(event);
            }
            Body::ToolResult {
                text,
                failed,
                truncated,
            } => {
                let Some(id) = observation.item.as_deref() else {
                    self.gap();
                    return Some(update);
                };
                let Some(item) = self
                    .items
                    .iter_mut()
                    .find(|item| item.id == id && item.tool.is_some())
                else {
                    self.gap();
                    return Some(update);
                };
                item.complete = true;
                let command = item.tool.as_ref().unwrap().1.clone();
                let seq = item.seq;
                update.key = Some(self.key(&format!("tool/{id}/result")));
                update.event = Some(CoderEvent::Output(Output {
                    turn: self.turn,
                    step_id: seq,
                    command,
                    exit: Some(if *failed { 1 } else { 0 }),
                    timed_out: false,
                    seconds: 0.0,
                    text: wire::bounded(text),
                    truncated: *truncated || text.len() > wire::MAX_TEXT,
                }));
            }
            Body::Plan { items } => {
                let items: Vec<_> = items
                    .iter()
                    .take(plan::MAX_ITEMS)
                    .map(|item| {
                        plan::Item::new(
                            item.text.chars().take(plan::MAX_TEXT).collect::<String>(),
                            match item.status {
                                wire::PlanStatus::Pending => plan::Status::Pending,
                                wire::PlanStatus::InProgress => plan::Status::InProgress,
                                wire::PlanStatus::Completed => plan::Status::Completed,
                            },
                        )
                    })
                    .collect();
                self.plan = Some(items.clone());
                let mut event = step(StepKind::Note, String::new(), observation.seq);
                if let CoderEvent::Step(step) = &mut event {
                    step.plan = Some(items);
                }
                update.key = Some(self.key("state/plan"));
                update.event = Some(event);
            }
            Body::Context { used, capacity } => self.context = Some((*used, *capacity)),
            Body::Usage { usage } => self.usage = Some(usage.clone()),
            Body::RateLimit { status, resets_at } => {
                self.rate_limit = Some((wire::bounded(status), *resets_at))
            }
            Body::Policy { tool, decision } => {
                self.policy = Some(format!(
                    "{}: {}",
                    wire::bounded(tool),
                    wire::bounded(decision)
                ));
            }
            Body::Request { id, text, .. } => {
                update.key = Some(self.key(&format!("request/{id}")));
                update.event = Some(step(
                    StepKind::Note,
                    format!("Coder asks: {}", wire::bounded(text)),
                    observation.seq,
                ));
            }
            Body::Artifact { path, change } => {
                update.key = Some(self.key(&format!("artifact/{path}")));
                update.event = Some(step(
                    StepKind::Note,
                    format!("{}: {}", wire::bounded(path), wire::bounded(change)),
                    observation.seq,
                ));
            }
            Body::Ended { .. } => self.ended = true,
            Body::Gap { .. } => self.gap(),
            Body::Child { .. } | Body::Resource { .. } => {}
            Body::Unknown => self.gap(),
        }
        Some(update)
    }

    fn insert(&mut self, id: &str, seq: u64) -> usize {
        if self.items.len() >= wire::MAX_ITEMS {
            self.items.pop_front();
            self.incomplete = true;
        }
        self.items.push_back(Item {
            id: id.into(),
            seq,
            text: String::new(),
            channel: Channel::Text,
            complete: false,
            history: true,
            tool: None,
        });
        self.items.len() - 1
    }

    fn key(&self, id: &str) -> String {
        format!("{}/{}/{id}", self.turn, self.generation)
    }

    fn gap(&mut self) {
        self.incomplete = true;
        for item in &mut self.items {
            item.history = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(seq: u64, body: Body) -> Observation {
        Observation {
            schema: wire::SCHEMA.into(),
            scope: wire::Scope {
                host: "local".into(),
                task: "task".into(),
                turn: 1,
                generation: 1,
            },
            seq,
            native_session: None,
            item: Some("item".into()),
            child: None,
            body,
        }
    }

    fn started() -> Observation {
        event(
            1,
            Body::Started {
                provider: "claude".into(),
                model: "model".into(),
            },
        )
    }

    fn text(seq: u64, mode: TextMode, text: &str, complete: bool) -> Observation {
        event(
            seq,
            Body::Text {
                channel: Channel::Text,
                mode,
                text: text.into(),
                complete,
                truncated: false,
            },
        )
    }

    fn words(update: Update) -> String {
        match update.event.unwrap() {
            CoderEvent::Step(step) => step.text,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn snapshots_replace_deltas_append_and_closed_generations_reject_late_output() {
        let mut state = Projection::new("local", "task", 1);
        state.apply(&started()).unwrap();
        let first = state
            .apply(&text(2, TextMode::Snapshot, "Hello", false))
            .unwrap();
        let second = state
            .apply(&text(3, TextMode::Delta, " world", true))
            .unwrap();
        assert_eq!(first.key, second.key);
        assert_eq!(words(second), "Hello world");
        assert!(
            state
                .apply(&text(3, TextMode::Delta, " duplicate", false))
                .is_none()
        );
        assert_eq!(
            words(
                state
                    .apply(&text(4, TextMode::Snapshot, "Corrected", true))
                    .unwrap()
            ),
            "Corrected"
        );
        state
            .apply(&event(
                5,
                Body::Ended {
                    ending: wire::Ending::Completed,
                    message: None,
                },
            ))
            .unwrap();
        assert!(
            state
                .apply(&text(6, TextMode::Snapshot, "late", true))
                .is_none()
        );
        let mut next = started();
        next.scope.generation = 2;
        state.apply(&next).unwrap();
        assert!(!state.ended);
        assert!(
            state
                .apply(&text(7, TextMode::Snapshot, "old generation", true))
                .is_none()
        );
    }

    #[test]
    fn a_gap_refuses_partial_deltas_and_a_snapshot_recovers_the_item() {
        let mut state = Projection::new("local", "task", 1);
        state.apply(&started()).unwrap();
        state
            .apply(&text(2, TextMode::Snapshot, "Hello", false))
            .unwrap();
        assert!(
            state
                .apply(&text(4, TextMode::Delta, "lost middle", false))
                .unwrap()
                .event
                .is_none()
        );
        assert!(state.incomplete);
        assert_eq!(
            words(
                state
                    .apply(&text(5, TextMode::Snapshot, "Whole", false))
                    .unwrap()
            ),
            "Whole"
        );
        assert_eq!(
            words(
                state
                    .apply(&text(6, TextMode::Delta, " again", true))
                    .unwrap()
            ),
            "Whole again"
        );
        assert!(
            state.incomplete,
            "recovering an item cannot recover missing earlier history"
        );
    }

    #[test]
    fn wrong_identity_and_child_output_cannot_change_parent_state() {
        let mut state = Projection::new("local", "task", 1);
        state.apply(&started()).unwrap();
        for field in ["schema", "host", "task", "turn"] {
            let mut bad = text(2, TextMode::Snapshot, "foreign", true);
            match field {
                "schema" => bad.schema = "future".into(),
                "host" => bad.scope.host = "other".into(),
                "task" => bad.scope.task = "other".into(),
                _ => bad.scope.turn = 2,
            }
            assert!(state.apply(&bad).is_none());
        }
        let mut child = event(
            2,
            Body::Context {
                used: Some(99),
                capacity: Some(100),
            },
        );
        child.child = Some("child".into());
        child.native_session = Some("child-session".into());
        state.apply(&child).unwrap();
        assert!(state.context.is_none());
        assert!(state.native_session.is_none());
        let mut session = event(
            3,
            Body::Session {
                model: None,
                version: None,
            },
        );
        session.native_session = Some("parent".into());
        state.apply(&session).unwrap();
        let mut foreign = text(4, TextMode::Snapshot, "foreign session", true);
        foreign.native_session = Some("other".into());
        assert!(state.apply(&foreign).is_none());
        assert_eq!(state.native_session.as_deref(), Some("parent"));
    }

    #[test]
    fn tools_join_by_call_identity_and_requests_do_not_end_a_turn() {
        let mut state = Projection::new("local", "task", 1);
        state.apply(&started()).unwrap();
        let start = state
            .apply(&event(
                2,
                Body::ToolStarted {
                    name: "Bash".into(),
                    input: r#"{"command":"cargo test"}"#.into(),
                },
            ))
            .unwrap();
        let result = state
            .apply(&event(
                3,
                Body::ToolResult {
                    text: "passed".into(),
                    failed: false,
                    truncated: false,
                },
            ))
            .unwrap();
        let CoderEvent::Step(start) = start.event.unwrap() else {
            panic!("call")
        };
        let CoderEvent::Output(result) = result.event.unwrap() else {
            panic!("result")
        };
        assert_eq!(start.step_id, result.step_id);
        assert_eq!(start.text, result.command);
        let request = state
            .apply(&event(
                4,
                Body::Request {
                    id: "ask".into(),
                    request: wire::RequestKind::Approval,
                    text: "Run it?".into(),
                    choices: vec!["Allow".into()],
                },
            ))
            .unwrap()
            .event
            .unwrap();
        assert!(!request.ends_turn());
        assert!(!state.ended);
        state
            .apply(&event(
                5,
                Body::Usage {
                    usage: wire::Usage {
                        output: Some(0),
                        ..Default::default()
                    },
                },
            ))
            .unwrap();
        assert_eq!(state.usage.unwrap().input, None);
    }

    #[test]
    fn native_file_tools_keep_the_existing_read_and_edit_presentation() {
        let mut state = Projection::new("local", "task", 1);
        state.apply(&started()).unwrap();
        for (seq, name, verb) in [(2, "Read", Verb::Read), (3, "Edit", Verb::Edit)] {
            let mut observation = event(
                seq,
                Body::ToolStarted {
                    name: name.into(),
                    input: r#"{"file_path":"src/lib.rs"}"#.into(),
                },
            );
            observation.item = Some(name.into());
            let CoderEvent::Step(step) = state.apply(&observation).unwrap().event.unwrap() else {
                panic!("tool")
            };
            assert_eq!(step.call.as_ref().unwrap().verb, verb);
            assert_eq!(step.call.as_ref().unwrap().target, "src/lib.rs");
        }
    }

    #[test]
    fn retained_items_and_text_are_bounded_and_evicted_deltas_need_a_snapshot() {
        let mut state = Projection::new("local", "task", 1);
        state.apply(&started()).unwrap();
        for n in 0..wire::MAX_ITEMS + 1 {
            let mut observation = text(n as u64 + 2, TextMode::Snapshot, "x", false);
            observation.item = Some(format!("item-{n}"));
            state.apply(&observation).unwrap();
        }
        assert_eq!(state.items.len(), wire::MAX_ITEMS);
        assert!(state.incomplete);
        let seq = wire::MAX_ITEMS as u64 + 3;
        let mut delta = text(seq, TextMode::Delta, "tail", false);
        delta.item = Some("item-0".into());
        assert!(state.apply(&delta).unwrap().event.is_none());
        let oversized = "日".repeat(wire::MAX_TEXT);
        let update = state
            .apply(&text(seq + 1, TextMode::Snapshot, &oversized, true))
            .unwrap();
        assert!(words(update).len() <= wire::MAX_TEXT);
    }
}
