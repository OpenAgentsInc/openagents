//! A Coder task's plan: the to-do items an engine records and where each
//! stands.
//!
//! Engines that plan write the whole list on every update: an Agent Client
//! Protocol engine (Devin, OpenCode, Grok Build) sends a `plan` session
//! update, which Coder's adapter records as a `<engine>_plan` step
//! extension; Claude Code calls `TodoWrite`, and Codex calls
//! `update_plan`. [`recorded`] reads any of those from one ATIF step, so
//! the current plan is simply the latest one a turn recorded ([`latest`]).
//! An empty list clears the plan.
//!
//! The model follows Zeron's checklist (public MIT zeronsh/zeron at
//! `9e1a1115`, `crates/proto/src/agent.rs` `TodoItem` and
//! `crates/ui/src/todo_panel.rs` `TodoSummary`), reimplemented here: an
//! item is pending, in progress, or completed, and an unknown status reads
//! as pending. The panel that draws it is
//! `openagents_chat_app::plan_panel`, shared by the chat and the studio.

use crate::coder_events::CoderEvent;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The most items one plan keeps; later items are dropped.
pub const MAX_ITEMS: usize = 64;
/// The most characters of one item's text.
pub const MAX_TEXT: usize = 300;

/// The tool names engines plan with.
const PLAN_TOOLS: [&str; 4] = ["TodoWrite", "todowrite", "todo_write", "update_plan"];

/// Where one plan item stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Pending,
    InProgress,
    Completed,
}

impl Status {
    /// An engine's status word. Anything else, including `cancelled`, is
    /// pending: not finished and not being worked on.
    #[must_use]
    pub fn parse(word: &str) -> Self {
        match word {
            "completed" | "complete" | "done" => Self::Completed,
            "in_progress" | "inProgress" | "in-progress" | "active" => Self::InProgress,
            _ => Self::Pending,
        }
    }
}

/// One plan item.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Item {
    pub text: String,
    pub status: Status,
}

impl Item {
    #[must_use]
    pub fn new(text: impl Into<String>, status: Status) -> Self {
        Self {
            text: text.into(),
            status,
        }
    }
}

/// The plan one ATIF step records, in the document or the log form:
/// `Some` with the whole list (empty when the engine cleared it), or
/// `None` when the step records no plan.
#[must_use]
pub fn recorded(step: &Value) -> Option<Vec<Item>> {
    let extra = step
        .get("extra")
        .filter(|extra| extra.is_object())
        .or_else(|| step.get("extensions"))
        .unwrap_or(&Value::Null);
    noted(extra).or_else(|| {
        step["tool_calls"]
            .as_array()
            .into_iter()
            .flatten()
            .rev()
            .find_map(called)
    })
}

/// The plan a step's extensions note under `<engine>_plan`.
#[must_use]
pub fn noted(extra: &Value) -> Option<Vec<Item>> {
    extra
        .as_object()
        .into_iter()
        .flatten()
        .find(|(key, value)| key.ends_with("_plan") && value.is_array())
        .map(|(_, entries)| items(entries))
}

/// The plan one tool call writes, when it is a planning tool.
#[must_use]
pub fn called(call: &Value) -> Option<Vec<Item>> {
    let name = call["function_name"].as_str()?;
    if !PLAN_TOOLS.contains(&name) {
        return None;
    }
    let arguments = &call["arguments"];
    let arguments = match arguments {
        Value::String(text) => serde_json::from_str(text).unwrap_or(Value::Null),
        other => other.clone(),
    };
    let entries = ["todos", "plan", "entries", "items"]
        .iter()
        .find_map(|key| arguments.get(*key).filter(|value| value.is_array()))?;
    Some(items(entries))
}

/// The items of an engine's list: each entry's words from `content`,
/// `step`, `text`, or `title`, and its status from `status`, else from a
/// `completed` or `done` flag.
fn items(entries: &Value) -> Vec<Item> {
    entries
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let text = ["content", "step", "text", "title"]
                .iter()
                .find_map(|key| entry[*key].as_str())
                .or_else(|| entry.as_str())?
                .trim();
            if text.is_empty() {
                return None;
            }
            let status = match entry["status"].as_str() {
                Some(word) => Status::parse(word),
                None if entry["completed"] == true || entry["done"] == true => Status::Completed,
                None => Status::Pending,
            };
            Some(Item::new(clip(text), status))
        })
        .take(MAX_ITEMS)
        .collect()
}

fn clip(text: &str) -> String {
    let text: String = text.chars().filter(|ch| !ch.is_control()).collect();
    match text.char_indices().nth(MAX_TEXT) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text,
    }
}

/// The plan as it stands after `events`: the latest one recorded, or
/// `None` when there is none or the latest cleared it.
pub fn latest<'a, I>(events: I) -> Option<&'a [Item]>
where
    I: IntoIterator<Item = &'a CoderEvent>,
    I::IntoIter: DoubleEndedIterator,
{
    let items = events.into_iter().rev().find_map(|event| match event {
        CoderEvent::Step(step) => step.plan.as_deref(),
        _ => None,
    })?;
    (!items.is_empty()).then_some(items)
}

/// What a plan's header reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub total: usize,
    pub done: usize,
    /// The first item in progress.
    pub active: Option<usize>,
    /// The first item not yet completed.
    pub next: Option<usize>,
}

impl Summary {
    #[must_use]
    pub fn of(items: &[Item]) -> Self {
        let mut summary = Self {
            total: items.len(),
            done: 0,
            active: None,
            next: None,
        };
        for (index, item) in items.iter().enumerate() {
            match item.status {
                Status::Completed => summary.done += 1,
                Status::InProgress => {
                    summary.active.get_or_insert(index);
                    summary.next.get_or_insert(index);
                }
                Status::Pending => {
                    summary.next.get_or_insert(index);
                }
            }
        }
        summary
    }

    /// Every item is completed.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.total > 0 && self.done == self.total
    }

    /// The item the header names: the one in progress, else the next one.
    #[must_use]
    pub fn headline(&self) -> Option<usize> {
        self.active.or(self.next)
    }

    /// The step line a plan update shows: `Updated the plan: 2 of 5 done.`
    #[must_use]
    pub fn line(&self) -> String {
        if self.total == 0 {
            "Cleared the plan.".to_owned()
        } else {
            format!("Updated the plan: {} of {} done.", self.done, self.total)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn statuses_read_every_engine_spelling_and_unknown_is_pending() {
        assert_eq!(Status::parse("completed"), Status::Completed);
        assert_eq!(Status::parse("done"), Status::Completed);
        assert_eq!(Status::parse("in_progress"), Status::InProgress);
        assert_eq!(Status::parse("inProgress"), Status::InProgress);
        assert_eq!(Status::parse("pending"), Status::Pending);
        assert_eq!(Status::parse("cancelled"), Status::Pending);
        assert_eq!(Status::parse("blocked"), Status::Pending);
    }

    #[test]
    fn an_acp_plan_note_is_read_from_either_step_form() {
        let entries = json!([
            {"content": "Read the parser", "status": "completed"},
            {"content": "Fix the bug", "status": "in_progress"},
            {"content": "Add a test"}
        ]);
        let document = json!({"source": "system", "extra": {"devin_plan": entries}});
        let log = json!({"source": "system", "extensions": {"opencode_plan": entries}});
        let want = vec![
            Item::new("Read the parser", Status::Completed),
            Item::new("Fix the bug", Status::InProgress),
            Item::new("Add a test", Status::Pending),
        ];
        assert_eq!(recorded(&document), Some(want.clone()));
        assert_eq!(recorded(&log), Some(want));
        let cleared = json!({"extensions": {"grok_plan": []}});
        assert_eq!(recorded(&cleared), Some(vec![]));
        let other = json!({"extensions": {"devin_tool": {"kind": "edit"}}});
        assert_eq!(recorded(&other), None);
    }

    #[test]
    fn todo_write_and_update_plan_calls_are_plans() {
        let claude = json!({"source": "agent", "tool_calls": [{
        "tool_call_id": "1", "function_name": "TodoWrite",
        "arguments": {"todos": [
            {"content": "One", "status": "completed", "activeForm": "Doing one"},
            {"content": "Two", "status": "in_progress"}
        ]}}]});
        assert_eq!(
            recorded(&claude),
            Some(vec![
                Item::new("One", Status::Completed),
                Item::new("Two", Status::InProgress)
            ])
        );
        let codex = json!({"function_name": "update_plan",
            "arguments": "{\"plan\":[{\"step\":\"Plan\",\"status\":\"pending\"}]}"});
        assert_eq!(
            called(&codex),
            Some(vec![Item::new("Plan", Status::Pending)])
        );
        let flagged = json!({"function_name": "todo_write",
            "arguments": {"items": [{"text": "a", "done": true}, {"text": " "}]}});
        assert_eq!(
            called(&flagged),
            Some(vec![Item::new("a", Status::Completed)])
        );
        let shell = json!({"function_name": "Bash", "arguments": {"command": "ls"}});
        assert_eq!(called(&shell), None);
    }

    #[test]
    fn a_plan_is_bounded() {
        let entries: Vec<Value> = (0..MAX_ITEMS + 5)
            .map(|n| json!({"content": format!("{n}{}", "x".repeat(MAX_TEXT))}))
            .collect();
        let items = recorded(&json!({"extensions": {"devin_plan": entries}})).unwrap();
        assert_eq!(items.len(), MAX_ITEMS);
        assert!(items[0].text.ends_with('…'));
        assert_eq!(items[0].text.chars().count(), MAX_TEXT + 1);
    }

    #[test]
    fn the_summary_names_the_item_in_progress_else_the_next() {
        let mut items = vec![
            Item::new("a", Status::Completed),
            Item::new("b", Status::Pending),
            Item::new("c", Status::InProgress),
        ];
        let summary = Summary::of(&items);
        assert_eq!((summary.total, summary.done), (3, 1));
        assert_eq!(summary.headline(), Some(2));
        assert!(!summary.finished());
        assert_eq!(summary.line(), "Updated the plan: 1 of 3 done.");
        items[2].status = Status::Pending;
        assert_eq!(Summary::of(&items).headline(), Some(1));
        for item in &mut items {
            item.status = Status::Completed;
        }
        let done = Summary::of(&items);
        assert!(done.finished());
        assert_eq!(done.headline(), None);
        assert!(!Summary::of(&[]).finished());
        assert_eq!(Summary::of(&[]).line(), "Cleared the plan.");
    }
}
