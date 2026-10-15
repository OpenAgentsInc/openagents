//! Bounded child activity, independent of task completion and control rights.

pub use route_contract::children::*;
use route_contract::observation::{Body, Channel, Ending, Observation, TextMode};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct Projection {
    host: String,
    task: String,
    scope: Option<(usize, u64)>,
    seq: u64,
    pub children: Vec<Child>,
    items: BTreeMap<route_contract::Digest, (String, bool)>,
    revision: u64,
    sdk_revision: Option<u64>,
    pub truncated: bool,
    closed: bool,
}
impl Projection {
    pub fn new(host: &str, task: &str) -> Self {
        Self {
            host: host.into(),
            task: task.into(),
            ..Default::default()
        }
    }
    fn close_native(&mut self) {
        for child in &mut self.children {
            if child.reference.source == Source::Native && child.status == Status::Running {
                child.status = Status::Unknown;
                child.incomplete = true;
            }
        }
    }
    fn room(&mut self) {
        if self.children.len() >= MAX_CHILDREN {
            let removed = self.children.remove(0);
            self.items.remove(&removed.reference.id);
            self.truncated = true;
        }
    }
    pub fn consume(&mut self, observation: &Observation) -> bool {
        if !observation.applies_to(&self.host, &self.task, observation.scope.turn) {
            return false;
        }
        let version = (observation.scope.turn, observation.scope.generation);
        if self.scope.is_some_and(|known| version < known) {
            return false;
        }
        if self.scope != Some(version) {
            self.close_native();
            self.scope = Some(version);
            self.seq = 0;
            self.closed = false;
        }
        if observation.seq <= self.seq {
            return false;
        }
        let gap = observation.seq != self.seq + 1 || matches!(observation.body, Body::Gap { .. });
        self.seq = observation.seq;
        if gap {
            for child in &mut self.children {
                if (child.reference.scope.turn, child.reference.scope.generation) == version
                    && child.reference.source == Source::Native
                {
                    child.incomplete = true;
                }
            }
            for (_, history) in self.items.values_mut() {
                *history = false;
            }
        }
        let Some(id) = observation.child.as_ref().filter(|child| valid_id(child)) else {
            if matches!(observation.body, Body::Ended { .. }) {
                self.close_native();
                self.closed = true;
                self.revision += 1;
                return true;
            }
            return gap;
        };
        if self.closed {
            return false;
        }
        let reference = Reference::new(observation.scope.clone(), Source::Native, id.clone());
        let index = if let Some(index) = self
            .children
            .iter()
            .position(|child| child.reference == reference)
        {
            index
        } else {
            self.room();
            self.children.push(Child {
                reference: reference.clone(),
                parent: None,
                title: "Child task".into(),
                harness: "claude-code".into(),
                model: None,
                status: Status::Running,
                summary: String::new(),
                transcript: false,
                incomplete: gap,
            });
            self.children.len() - 1
        };
        let child = &mut self.children[index];
        match &observation.body {
            Body::Child {
                parent,
                title,
                harness,
                model,
                status,
            } => {
                if !title.is_empty() {
                    child.title = clip(title, 512);
                }
                if !harness.is_empty() {
                    child.harness = clip(harness, 128);
                }
                if model.is_some() {
                    child.model = model.as_ref().map(|model| clip(model, 256));
                }
                if let Some(parent) = parent
                    .as_ref()
                    .filter(|parent| valid_id(parent) && *parent != id)
                {
                    if child.parent.as_ref().is_some_and(|known| known != parent) {
                        child.incomplete = true;
                    } else {
                        child.parent = Some(parent.clone());
                    }
                }
                if child.status == Status::Running
                    || child.status == Status::Unknown
                    || *status != Status::Running
                {
                    child.status = *status;
                }
            }
            Body::Session { model, .. } => {
                if model.is_some() {
                    child.model = model.as_ref().map(|model| clip(model, 256));
                }
            }
            Body::Started { provider, model } => {
                child.harness = clip(provider, 128);
                child.model = Some(clip(model, 256));
            }
            Body::Text {
                channel: Channel::Text,
                mode,
                text,
                truncated,
                ..
            } => {
                let item = observation
                    .item
                    .clone()
                    .unwrap_or_else(|| format!("text/{}", observation.seq));
                let lifecycle = item.starts_with("lifecycle/");
                if !lifecycle {
                    child.transcript = true;
                }
                child.incomplete |= *truncated;
                if lifecycle && child.status != Status::Running && *mode == TextMode::Delta {
                    return false;
                }
                match mode {
                    TextMode::Snapshot => {
                        child.summary = clip(text, 2048);
                        self.items.insert(reference.id, (item, true));
                    }
                    TextMode::Delta => {
                        if self
                            .items
                            .get(&reference.id)
                            .is_some_and(|(known, history)| known == &item && *history)
                        {
                            child.summary = clip(&format!("{}{}", child.summary, text), 2048);
                        } else {
                            child.incomplete = true;
                        }
                    }
                }
            }
            Body::ToolStarted { .. }
            | Body::ToolResult { .. }
            | Body::Plan { .. }
            | Body::Request { .. } => child.transcript = true,
            Body::Ended { ending, message } => {
                child.status = match ending {
                    Ending::Completed => Status::Completed,
                    Ending::Failed => Status::Failed,
                    Ending::Interrupted => Status::Interrupted,
                };
                if let Some(message) = message.as_ref() {
                    child.summary = clip(message, 2048);
                }
            }
            Body::Gap { .. } | Body::Unknown => child.incomplete = true,
            _ => {}
        }
        self.revision += 1;
        true
    }
    pub fn merge_sdk(&mut self, snapshot: &Snapshot) -> bool {
        if !snapshot.valid()
            || snapshot.task != self.task
            || snapshot.children.iter().any(|child| {
                child.reference.source != Source::Delegation
                    || child.reference.scope.host != self.host
            })
            || self
                .sdk_revision
                .is_some_and(|revision| snapshot.revision <= revision)
        {
            return false;
        }
        self.sdk_revision = Some(snapshot.revision);
        self.children
            .retain(|child| child.reference.source != Source::Delegation);
        for child in &snapshot.children {
            self.room();
            self.children.push(child.clone());
        }
        self.truncated |= snapshot.truncated;
        self.revision += 1;
        true
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            schema: SCHEMA.into(),
            task: self.task.clone(),
            revision: self.revision,
            children: self.children.clone(),
            truncated: self.truncated,
        }
    }
}
pub fn clip(value: &str, max: usize) -> String {
    let mut end = value.len().min(max);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}

/// Format only one recorded native child's content. Item snapshots replace
/// earlier text; other children and the parent's output never enter this view.
pub fn transcript<'a>(
    observations: impl IntoIterator<Item = &'a Observation>,
    reference: &Reference,
) -> Option<(String, bool, Status)> {
    if !reference.valid() || reference.source != Source::Native {
        return None;
    }
    let mut seq = 0;
    let mut found = false;
    let mut incomplete = false;
    let mut status = Status::Running;
    let mut entries: Vec<(String, String, bool)> = Vec::new();
    for observation in observations {
        if observation.scope != reference.scope
            || !observation.applies_to(
                &reference.scope.host,
                &reference.scope.task,
                reference.scope.turn,
            )
            || observation.seq <= seq
        {
            continue;
        }
        if observation.seq != seq + 1 || matches!(observation.body, Body::Gap { .. }) {
            incomplete = true;
            for (_, _, history) in &mut entries {
                *history = false;
            }
        }
        seq = observation.seq;
        if observation.child.is_none() && matches!(observation.body, Body::Ended { .. }) {
            if status == Status::Running {
                status = Status::Unknown;
                incomplete = true;
            }
            break;
        }
        if observation.child.as_ref() != Some(&reference.child) {
            continue;
        }
        found = true;
        let id = observation
            .item
            .clone()
            .unwrap_or_else(|| format!("event/{}", observation.seq));
        let (key, text, mode) = match &observation.body {
            Body::Child {
                title,
                status: next,
                ..
            } => {
                if status == Status::Running || *next != Status::Running {
                    status = *next;
                }
                if title.is_empty() {
                    continue;
                }
                ("title".into(), format!("Task: {title}"), TextMode::Snapshot)
            }
            Body::Text {
                channel,
                mode,
                text,
                truncated,
                ..
            } => {
                incomplete |= *truncated;
                (
                    format!("text/{id}"),
                    format!(
                        "{}{}",
                        if *mode == TextMode::Snapshot {
                            if *channel == Channel::Reasoning {
                                "Notes:\n"
                            } else {
                                ""
                            }
                        } else {
                            ""
                        },
                        text
                    ),
                    *mode,
                )
            }
            Body::ToolStarted { name, input } => (
                format!("tool/{id}"),
                format!("Tool: {name}\n{input}"),
                TextMode::Snapshot,
            ),
            Body::ToolResult {
                text,
                failed,
                truncated,
            } => {
                incomplete |= *truncated;
                (
                    format!("result/{id}"),
                    format!(
                        "{}:\n{text}",
                        if *failed {
                            "Tool failed"
                        } else {
                            "Tool result"
                        }
                    ),
                    TextMode::Snapshot,
                )
            }
            Body::Plan { items } => (
                format!("plan/{id}"),
                items
                    .iter()
                    .map(|item| format!("{:?}: {}", item.status, item.text))
                    .collect::<Vec<_>>()
                    .join("\n"),
                TextMode::Snapshot,
            ),
            Body::Request { text, .. } => (
                format!("question/{id}"),
                format!("Question: {text}"),
                TextMode::Snapshot,
            ),
            Body::Ended { ending, message } => {
                status = match ending {
                    Ending::Completed => Status::Completed,
                    Ending::Failed => Status::Failed,
                    Ending::Interrupted => Status::Interrupted,
                };
                (
                    "ending".into(),
                    message.clone().unwrap_or_default(),
                    TextMode::Snapshot,
                )
            }
            Body::Gap { .. } | Body::Unknown => {
                incomplete = true;
                continue;
            }
            _ => continue,
        };
        incomplete |= text.len() > 32 * 1024;
        if let Some((_, old, history)) = entries.iter_mut().find(|(known, _, _)| known == &key) {
            if mode == TextMode::Snapshot {
                *old = clip(&text, 32 * 1024);
                *history = true;
            } else if *history {
                incomplete |= old.len() + text.len() > 32 * 1024;
                *old = clip(&format!("{old}{text}"), 32 * 1024);
            } else {
                incomplete = true;
            }
        } else if mode == TextMode::Delta {
            incomplete = true;
        } else if entries.len() < 256 {
            entries.push((key, clip(&text, 32 * 1024), true));
        } else {
            incomplete = true;
        }
    }
    if !found {
        return None;
    }
    let mut text = String::new();
    for (_, entry, _) in entries {
        if text.len() + entry.len() + 2 > TRANSCRIPT_BYTES {
            let left = TRANSCRIPT_BYTES.saturating_sub(text.len());
            text.push_str(&clip(&entry, left));
            incomplete = true;
            break;
        }
        if !entry.is_empty() {
            text.push_str(&entry);
            text.push_str("\n\n");
        }
    }
    Some((text, incomplete, status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use route_contract::observation::{SCHEMA as OBS_SCHEMA, Scope};
    fn event(seq: u64, child: Option<&str>, body: Body) -> Observation {
        Observation {
            schema: OBS_SCHEMA.into(),
            scope: Scope {
                host: "local".into(),
                task: "task".into(),
                turn: 1,
                generation: 1,
            },
            seq,
            item: Some("message".into()),
            child: child.map(str::to_owned),
            native_session: None,
            body,
        }
    }
    fn text(mode: TextMode, text: &str) -> Body {
        Body::Text {
            channel: Channel::Text,
            mode,
            text: text.into(),
            complete: false,
            truncated: false,
        }
    }
    #[test]
    fn interleaved_children_replay_snapshots_and_terminal_parent_are_separate() {
        let mut events = vec![
            event(1, None, text(TextMode::Snapshot, "parent")),
            event(2, Some("a"), text(TextMode::Snapshot, "one")),
            event(3, Some("b"), text(TextMode::Snapshot, "other")),
            event(4, Some("a"), text(TextMode::Delta, " two")),
            event(5, Some("a"), text(TextMode::Snapshot, "one two")),
            event(
                6,
                Some("a"),
                Body::Ended {
                    ending: Ending::Completed,
                    message: None,
                },
            ),
            event(
                7,
                None,
                Body::Ended {
                    ending: Ending::Completed,
                    message: None,
                },
            ),
        ];
        let mut projection = Projection::new("local", "task");
        for event in &events {
            projection.consume(event);
        }
        assert_eq!(projection.children.len(), 2);
        let reference = projection.children[0].reference.clone();
        assert_eq!(projection.children[0].status, Status::Completed);
        assert_eq!(projection.children[1].status, Status::Unknown);
        assert!(!projection.consume(&events[1]));
        let late = event(8, Some("a"), text(TextMode::Snapshot, "late"));
        assert!(!projection.consume(&late));
        events.push(late);
        let (text, incomplete, status) = transcript(&events, &reference).unwrap();
        assert_eq!(text.trim(), "one two");
        assert!(!incomplete);
        assert_eq!(status, Status::Completed);
    }
    #[test]
    fn missing_prefix_and_old_attempt_cannot_append_to_a_new_child() {
        let mut projection = Projection::new("local", "task");
        let first = event(2, Some("a"), text(TextMode::Delta, "unbound suffix"));
        projection.consume(&first);
        assert!(projection.children[0].incomplete);
        assert!(projection.children[0].summary.is_empty());
        let mut fresh = event(1, Some("a"), text(TextMode::Snapshot, "new attempt"));
        fresh.scope.generation = 2;
        projection.consume(&fresh);
        assert_eq!(projection.children.len(), 2);
        assert!(!projection.consume(&first));
        assert_ne!(
            projection.children[0].reference,
            projection.children[1].reference
        );
    }
    #[test]
    fn sdk_catalog_rejects_wrong_hosts_tasks_sources_and_old_revisions() {
        let mut projection = Projection::new("local", "task");
        projection.consume(&event(1, Some("a"), text(TextMode::Snapshot, "native")));
        let mut child = projection.children[0].clone();
        child.reference = Reference::new(
            child.reference.scope.clone(),
            Source::Delegation,
            "sdk.d1".into(),
        );
        let mut snapshot = Snapshot {
            schema: SCHEMA.into(),
            task: "task".into(),
            revision: 1,
            children: vec![child],
            truncated: false,
        };
        assert!(projection.merge_sdk(&snapshot));
        assert_eq!(projection.children.len(), 2);
        assert!(!projection.merge_sdk(&snapshot));
        snapshot.revision = 2;
        snapshot.task = "other".into();
        assert!(!projection.merge_sdk(&snapshot));
        snapshot.task = "task".into();
        snapshot.children[0].reference = Reference::new(
            route_contract::observation::Scope {
                host: "other".into(),
                ..snapshot.children[0].reference.scope.clone()
            },
            Source::Delegation,
            "sdk.d1".into(),
        );
        assert!(!projection.merge_sdk(&snapshot));
    }
}
