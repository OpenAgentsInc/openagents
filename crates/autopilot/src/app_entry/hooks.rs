use serde_json::Value;
use wgpui::components::organisms::{EventData, TagData};

use crate::app::catalog::types::HookEvent;
use crate::app::{hook_event_label, truncate_preview, HookLogEntry};

fn hook_event_kind(event: HookEvent) -> u32 {
    match event {
        HookEvent::PreToolUse => 61001,
        HookEvent::PostToolUse => 61002,
        HookEvent::PostToolUseFailure => 61003,
        HookEvent::Notification => 61004,
        HookEvent::UserPromptSubmit => 61005,
        HookEvent::SessionStart => 61006,
        HookEvent::SessionEnd => 61007,
        HookEvent::Stop => 61008,
        HookEvent::SubagentStart => 61009,
        HookEvent::SubagentStop => 61010,
        HookEvent::PreCompact => 61011,
        HookEvent::PermissionRequest => 61012,
    }
}

fn value_preview(value: &Value, max_chars: usize) -> String {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    truncate_preview(&text, max_chars)
}

pub(super) fn hook_log_event_data(entry: &HookLogEntry) -> EventData {
    let mut tags = Vec::new();
    tags.push(TagData::new(
        "event",
        vec![hook_event_label(entry.event).to_string()],
    ));
    if let Some(tool) = &entry.tool_name {
        tags.push(TagData::new("tool", vec![tool.clone()]));
    }
    if let Some(matcher) = &entry.matcher {
        tags.push(TagData::new("matcher", vec![matcher.clone()]));
    }
    if !entry.sources.is_empty() {
        tags.push(TagData::new("sources", entry.sources.clone()));
    }
    if let Some(error) = &entry.error {
        tags.push(TagData::new("error", vec![error.clone()]));
    }
    tags.push(TagData::new(
        "input",
        vec![value_preview(&entry.input, 180)],
    ));
    if let Some(output) = &entry.output {
        tags.push(TagData::new(
            "output",
            vec![value_preview(output, 180)],
        ));
    }

    let mut content = entry.summary.clone();
    if let Some(error) = &entry.error {
        if !error.trim().is_empty() {
            content.push_str("\n");
            content.push_str(error);
        }
    }

    EventData::new(&entry.id, "hooks", hook_event_kind(entry.event))
        .content(content)
        .created_at(entry.timestamp)
        .tags(tags)
        .sig("")
        .verified(false)
}
