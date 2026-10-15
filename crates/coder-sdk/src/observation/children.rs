//! Claude task events stay separate from the parent provider conversation.

use super::*;
use route_contract::{
    children::{Status, valid_id},
    digest_of,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Default)]
pub(super) struct NativeChildren {
    aliases: HashMap<String, String>,
    ambiguous: HashSet<String>,
    runs: HashMap<(String, Option<String>), String>,
}
impl NativeChildren {
    pub fn resolve(&mut self, id: &str) -> Option<String> {
        if !valid_id(id) || self.ambiguous.contains(id) {
            return None;
        }
        if let Some(known) = self.aliases.get(id) {
            return Some(known.clone());
        }
        let key = format!(
            "native-{}",
            digest_of(&id).as_str().trim_start_matches("sha256:")
        );
        if self.aliases.len() < 256 {
            self.aliases.insert(id.into(), key.clone());
        }
        Some(key)
    }
    fn bind(&mut self, data: &Value) -> Option<String> {
        let task = data["task_id"].as_str().filter(|task| valid_id(task))?;
        let run = data["run_id"]
            .as_str()
            .filter(|run| valid_id(run))
            .map(str::to_owned);
        if run.is_none() && self.ambiguous.contains(task) {
            return None;
        }
        if run.is_none()
            && let Some(known) = self.aliases.get(task)
        {
            return Some(known.clone());
        }
        let pair = (task.to_owned(), run);
        if let Some(known) = self.runs.get(&pair) {
            return Some(known.clone());
        }
        let tool = data["tool_use_id"].as_str().filter(|tool| valid_id(tool));
        // Messages can arrive before task_started. Preserve their identity when
        // their tool ID has not already belonged to an earlier task run.
        let early = tool
            .and_then(|tool| self.aliases.get(tool))
            .or_else(|| self.aliases.get(task))
            .filter(|key| !self.runs.values().any(|known| known == *key))
            .cloned();
        let key = early.unwrap_or_else(|| {
            format!(
                "native-{}",
                digest_of(&pair).as_str().trim_start_matches("sha256:")
            )
        });
        if self.runs.len() < 64 {
            self.runs.insert(pair, key.clone());
        }
        for alias in [Some(task), tool].into_iter().flatten() {
            if self.aliases.get(alias).is_some_and(|known| known != &key)
                && self.ambiguous.len() < 256
            {
                self.ambiguous.insert(alias.into());
            }
            if self.aliases.len() < 256 || self.aliases.contains_key(alias) {
                self.aliases.insert(alias.into(), key.clone());
            }
        }
        Some(key)
    }
    pub fn system(
        &mut self,
        recorder: &mut Recorder,
        message: &claude_agent_sdk::SdkSystemMessage,
    ) -> Vec<Observation> {
        use claude_agent_sdk::SdkSystemMessage as System;
        let (value, kind) = match message {
            System::TaskStarted(value) => (serde_json::to_value(value).ok(), "start"),
            System::TaskProgress(value) => (serde_json::to_value(value).ok(), "progress"),
            System::TaskNotification(value) => (serde_json::to_value(value).ok(), "end"),
            System::TaskUpdated(value) => (serde_json::to_value(value).ok(), "update"),
            _ => return vec![],
        };
        let Some(value) = value else {
            return vec![];
        };
        let Some(child) = self.bind(&value) else {
            return vec![];
        };
        let parent = value["parent_task_id"]
            .as_str()
            .and_then(|parent| self.resolve(parent))
            .filter(|parent| parent != &child);
        let status = match kind {
            "start" | "progress" => Status::Running,
            _ => status(
                value["status"]
                    .as_str()
                    .or_else(|| value["patch"]["status"].as_str())
                    .unwrap_or(if kind == "update" {
                        "running"
                    } else {
                        "unknown"
                    }),
            ),
        };
        let title = if kind == "start" {
            value["description"].as_str().unwrap_or("Child task")
        } else {
            value["patch"]["description"].as_str().unwrap_or("")
        };
        let mut out = vec![tag(
            recorder.next(
                None,
                Body::Child {
                    parent,
                    title: clip(title, 512),
                    harness: "claude-code".into(),
                    model: None,
                    status,
                },
            ),
            &child,
        )];
        let progress = value["summary"]
            .as_str()
            .or_else(|| value["patch"]["error"].as_str())
            .or_else(|| value["description"].as_str());
        if let Some(text) = progress.filter(|text| !text.is_empty()) {
            out.push(tag(
                recorder.next(
                    Some("lifecycle/progress".into()),
                    Body::Text {
                        channel: Channel::Text,
                        mode: TextMode::Snapshot,
                        text: bounded(text),
                        complete: kind == "end",
                        truncated: text.len() > MAX_TEXT,
                    },
                ),
                &child,
            ));
        }
        if kind == "end"
            || (kind == "update" && status != Status::Running && status != Status::Unknown)
        {
            let ending = match status {
                Status::Completed => Some(Ending::Completed),
                Status::Failed => Some(Ending::Failed),
                Status::Interrupted => Some(Ending::Interrupted),
                _ => None,
            };
            if let Some(ending) = ending {
                out.push(tag(
                    recorder.next(
                        None,
                        Body::Ended {
                            ending,
                            message: progress.map(bounded),
                        },
                    ),
                    &child,
                ));
            }
        }
        // output_file and resource_links are provider metadata, not file grants.
        out
    }
}
pub(super) fn tag(mut observation: Observation, child: &str) -> Observation {
    observation.child = Some(child.into());
    observation
}
pub(super) fn status(value: &str) -> Status {
    match value {
        "running" | "in_progress" | "pending" => Status::Running,
        "completed" | "done" => Status::Completed,
        "failed" | "error" => Status::Failed,
        "stopped" | "cancelled" | "canceled" | "interrupted" => Status::Interrupted,
        _ => Status::Unknown,
    }
}
pub(super) fn clip(value: &str, limit: usize) -> String {
    let mut end = value.len().min(limit);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn normalizer() -> Claude {
        Claude::new(Recorder::new(Scope {
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            generation: 1,
        }))
    }
    fn task(kind: &str, run: &str) -> claude_agent_sdk::SdkSystemMessage {
        serde_json::from_value(json!({"subtype":kind,"task_id":"task-a","run_id":run,"tool_use_id":"tool-a","description":"Inspect code","summary":"Found the file","status":"completed","output_file":"/private/never-read","resource_links":[{"uri":"file:///private/never-read"}],"uuid":"uuid","session_id":"child-session"})).unwrap()
    }
    #[test]
    fn native_child_aliases_keep_early_messages_and_separate_resumed_runs() {
        let mut claude = normalizer();
        let early = claude.assistant_for(&json!({"id":"child-message","model":"child-model","content":[{"type":"text","text":"Child answer"}]}), Some("tool-a"));
        let first = claude.system(&task("task_started", "run-1"));
        assert_eq!(early[0].child, first[0].child);
        let completed = claude.system(&task("task_notification", "run-1"));
        assert_eq!(first[0].child, completed[0].child);
        assert!(completed.iter().any(|event| matches!(
            event.body,
            Body::Ended {
                ending: Ending::Completed,
                ..
            }
        )));
        let second = claude.system(&task("task_started", "run-2"));
        assert_ne!(first[0].child, second[0].child);
        assert!(
            claude
                .assistant_for(
                    &json!({"content":[{"type":"text","text":"late old run"}]}),
                    Some("tool-a")
                )
                .is_empty()
        );
        assert!(
            first
                .iter()
                .chain(&completed)
                .all(|event| event.child.is_some() && event.native_session.is_none())
        );
        assert!(
            !serde_json::to_string(&completed)
                .unwrap()
                .contains("/private/never-read")
        );
        assert!(
            claude
                .assistant_for(&json!({}), Some("bad\nchild"))
                .is_empty()
        );
        let root = claude.session("root-session");
        assert!(root.child.is_none());
        assert_eq!(root.native_session.as_deref(), Some("root-session"));
    }
    #[test]
    fn native_nested_parent_identity_survives_out_of_order_start_messages() {
        let mut claude = normalizer();
        let nested=serde_json::from_value(json!({"subtype":"task_started","task_id":"nested","run_id":"nested-run","parent_task_id":"parent","description":"Nested","uuid":"nested","session_id":"root"})).unwrap();
        let child = claude.system(&nested);
        let Body::Child {
            parent: Some(parent),
            ..
        } = &child[0].body
        else {
            panic!("parent")
        };
        let started=serde_json::from_value(json!({"subtype":"task_started","task_id":"parent","run_id":"parent-run","description":"Parent","uuid":"parent","session_id":"root"})).unwrap();
        assert_eq!(claude.system(&started)[0].child.as_ref(), Some(parent));
    }
    #[test]
    fn native_child_failure_and_interrupt_are_typed_and_bounded() {
        let mut claude = normalizer();
        for (status, ending) in [("failed", Ending::Failed), ("stopped", Ending::Interrupted)] {
            let message = serde_json::from_value(json!({"subtype":"task_notification","task_id":status,"status":status,"summary":"é".repeat(MAX_TEXT),"output_file":"/not-a-capability","uuid":"id","session_id":"id"})).unwrap();
            let events = claude.system(&message);
            assert!(events.iter().any(
                |event| matches!(event.body, Body::Ended { ending: actual, .. } if actual == ending)
            ));
            assert!(
                events
                    .iter()
                    .filter_map(|event| if let Body::Text { text, .. } = &event.body {
                        Some(text)
                    } else {
                        None
                    })
                    .all(|text| text.len() <= MAX_TEXT)
            );
        }
    }
}
