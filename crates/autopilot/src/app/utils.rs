use std::time::{SystemTime, UNIX_EPOCH};

use super::catalog::agents::AgentModel;
use super::catalog::types::HookEvent;
use wgpui::TextInput;
use wgpui::markdown::{
    MarkdownConfig, MarkdownDocument, MarkdownRenderer as MdRenderer, StreamingMarkdown,
};

use super::catalog::AgentEntry;
use super::chat::{ChatMessage, ChatSelectionPoint, MessageRole};
use super::config::CoderSettings;
use super::session::CheckpointEntry;
use super::ui::{ThemeSetting, palette_for};

pub(crate) fn selection_point_cmp(
    a: &ChatSelectionPoint,
    b: &ChatSelectionPoint,
) -> std::cmp::Ordering {
    match a.message_index.cmp(&b.message_index) {
        std::cmp::Ordering::Equal => a.offset.cmp(&b.offset),
        ordering => ordering,
    }
}

pub(crate) fn truncate_preview(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.len() <= max_chars {
        return trimmed;
    }
    let mut result = trimmed
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    result.push_str("...");
    result
}

/// Strip basic markdown formatting markers for plain text display.
/// Handles: **bold**, *italic*, `code`, __bold__, _italic_
pub(crate) fn strip_markdown_markers(text: &str) -> String {
    let mut result = text.to_string();
    // Strip ** (bold)
    result = result.replace("**", "");
    // Strip __ (bold)
    result = result.replace("__", "");
    // Strip ` (inline code) - be careful not to strip code blocks
    // Only strip single backticks, not triple
    let chars: Vec<char> = result.chars().collect();
    let mut i = 0;
    let mut new_chars = Vec::new();
    while i < chars.len() {
        if chars[i] == '`' {
            // Check if it's a triple backtick (code block)
            if i + 2 < chars.len() && chars[i + 1] == '`' && chars[i + 2] == '`' {
                // Keep code blocks as-is
                new_chars.push(chars[i]);
                new_chars.push(chars[i + 1]);
                new_chars.push(chars[i + 2]);
                i += 3;
            } else {
                // Skip single backtick
                i += 1;
            }
        } else {
            new_chars.push(chars[i]);
            i += 1;
        }
    }
    result = new_chars.into_iter().collect();
    // Strip single * or _ only when they appear at word boundaries (simple heuristic)
    // This is tricky - for now just strip pairs
    result
}

pub(crate) fn default_font_size() -> f32 {
    14.0
}

pub(crate) fn default_auto_scroll() -> bool {
    true
}

pub(crate) fn default_session_auto_save() -> bool {
    true
}

pub(crate) fn default_session_history_limit() -> usize {
    50
}

pub(crate) fn default_local_oss_base_url() -> String {
    "http://localhost:8000/v1".to_string()
}

pub(crate) fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn build_markdown_document(source: &str) -> MarkdownDocument {
    let mut parser = StreamingMarkdown::new();
    parser.append(source);
    parser.complete();
    parser.document().clone()
}

pub(crate) fn build_markdown_config(
    settings: &CoderSettings,
    theme: ThemeSetting,
) -> MarkdownConfig {
    let palette = palette_for(theme);
    let mut config = MarkdownConfig::default();
    config.base_font_size = settings.font_size;
    config.text_color = palette.text_primary;
    config.header_color = palette.text_primary;
    config.code_background = palette.code_bg;
    config.inline_code_background = palette.inline_code_bg;
    config.link_color = palette.link;
    config.blockquote_color = palette.blockquote;
    config
}

pub(crate) fn build_markdown_renderer(settings: &CoderSettings, theme: ThemeSetting) -> MdRenderer {
    MdRenderer::with_config(build_markdown_config(settings, theme))
}

pub(crate) fn build_input(settings: &CoderSettings, theme: ThemeSetting) -> TextInput {
    let palette = palette_for(theme);
    let mut input = TextInput::new()
        .with_id(1)
        .font_size(settings.font_size)
        .padding(28.0, 10.0)
        .background(palette.background)
        .border_color(palette.input_border)
        .border_color_focused(palette.input_border_focused)
        .text_color(palette.text_primary)
        .placeholder_color(palette.text_dim)
        .cursor_color(palette.text_primary)
        .mono(true);
    input.focus();
    input
}

pub(crate) fn format_relative_time(timestamp: u64) -> String {
    let now = now_timestamp();
    if timestamp >= now {
        return "just now".to_string();
    }
    let delta = now - timestamp;
    if delta < 60 {
        format!("{}s ago", delta)
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86_400 {
        format!("{}h ago", delta / 3600)
    } else {
        format!("{}d ago", delta / 86_400)
    }
}

pub(crate) fn build_checkpoint_entries(messages: &[ChatMessage]) -> Vec<CheckpointEntry> {
    let mut entries = Vec::new();
    for (idx, message) in messages.iter().enumerate() {
        if matches!(message.role, MessageRole::User) {
            if let Some(uuid) = &message.uuid {
                let label = format!("{}: {}", idx + 1, truncate_preview(&message.content, 32));
                entries.push(CheckpointEntry {
                    user_message_id: uuid.clone(),
                    label,
                });
            }
        }
    }
    entries
}

fn agent_model_label(model: AgentModel) -> &'static str {
    match model {
        AgentModel::Default => "default",
        AgentModel::Mini => "mini",
        AgentModel::Reasoning => "reasoning",
        AgentModel::Inherit => "inherit",
    }
}

pub(crate) fn agent_capabilities(entry: &AgentEntry) -> Vec<String> {
    let mut caps = Vec::new();
    if let Some(model) = entry.definition.model {
        caps.push(format!("model {}", agent_model_label(model)));
    }
    if let Some(tools) = &entry.definition.tools {
        caps.extend(tools.clone());
    } else if let Some(disallowed) = &entry.definition.disallowed_tools {
        caps.extend(disallowed.iter().map(|tool| format!("no {}", tool)));
    }
    if caps.is_empty() {
        caps.push("all tools".to_string());
    }
    caps
}

pub(crate) fn truncate_bytes(input: String, max_bytes: usize) -> String {
    if input.len() <= max_bytes {
        return input;
    }
    let mut truncated = input.as_bytes()[..max_bytes].to_vec();
    while !truncated.is_empty() && std::str::from_utf8(&truncated).is_err() {
        truncated.pop();
    }
    let mut result = String::from_utf8_lossy(&truncated).to_string();
    result.push_str("\n... [truncated]");
    result
}

pub(crate) fn hook_event_label(event: HookEvent) -> &'static str {
    event.as_str()
}
