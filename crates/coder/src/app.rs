//! Main application state and event handling.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;
use tokio::sync::{mpsc, oneshot};
use tokio::time::timeout;
use async_trait::async_trait;
use arboard::Clipboard;
use web_time::Instant;
use wgpui::input::{Key as UiKey, Modifiers as UiModifiers, NamedKey as UiNamedKey};
use wgpui::components::{Component, EventContext, EventResult, PaintContext};
use wgpui::components::hud::{Command as PaletteCommand, CommandPalette};
use wgpui::markdown::{MarkdownBlock, MarkdownConfig, MarkdownDocument, StyledLine};
use wgpui::renderer::Renderer;
use wgpui::{
    copy_to_clipboard, Bounds, ContextMenu, Hsla, InputEvent, MenuItem, Point, Quad, Scene, Size,
    TextInput, TextSystem,
};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey as WinitNamedKey};
use winit::window::{CursorIcon, Window, WindowId};

use claude_agent_sdk::error::Result as SdkResult;
use claude_agent_sdk::permissions::{CallbackPermissionHandler, PermissionRequest};
use claude_agent_sdk::protocol::{PermissionMode, PermissionResult};
use claude_agent_sdk::{
    query_with_permissions, AgentDefinition, AgentModel, BaseHookInput, HookCallback, HookDecision,
    HookEvent, HookInput, HookOutput, HookSpecificOutput, McpServerConfig, QueryOptions, SdkMessage,
    SettingSource, SyncHookOutput, UserPromptSubmitSpecificOutput, SessionStartSpecificOutput,
    PostToolUseSpecificOutput,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use claude_agent_sdk::protocol::McpServerStatus;

// Autopilot/Adjutant
use crate::autopilot_loop::DspyStage;
use wgpui::components::atoms::{ToolStatus, ToolType};
use wgpui::components::molecules::{CheckpointRestore, SessionAction};
use wgpui::components::organisms::{
    ChildTool, DiffLine, DiffLineKind, DiffToolCall, EventData, EventInspector, InspectorView, PermissionDialog,
    SearchMatch, SearchToolCall, TagData, TerminalToolCall, ToolCallCard,
};

use crate::commands::{parse_command, Command};
use crate::keybindings::{default_keybindings, match_action, Action as KeyAction, Keybinding};
use crate::panels::PanelLayout;

use crate::app::autopilot::AutopilotState;
use crate::app::catalog::CatalogState;
use crate::app::chat::ChatState;
use crate::app::config::SettingsState;
use crate::app::permissions::{
    coder_mode_default_allow, coder_mode_label, extract_bash_command, is_read_only_tool,
    load_permission_config, parse_coder_mode, pattern_matches, PermissionPending, PermissionState,
};
use crate::app::session::SessionState;
use crate::app::tools::ToolsState;
use crate::app::chat::{
    ChatLayout, ChatLineLayout, ChatMessage, ChatSelection, ChatSelectionPoint, InlineToolsLayout,
    MessageLayout, MessageLayoutBuilder, MessageMetadata, MessageRole,
};
use crate::app::catalog::{
    build_hook_map, describe_mcp_config, expand_env_vars_in_value, load_agent_entries,
    load_hook_config, load_hook_scripts, load_mcp_project_servers, load_skill_entries,
    parse_mcp_server_config, save_hook_config, AgentEntry, AgentSource, HookConfig,
    HookRuntimeConfig, HookScriptEntry, HookScriptSource, McpServerEntry, McpServerSource,
    SkillEntry, SkillSource,
};
use crate::app::config::{
    config_dir, config_file, keybindings_file, mcp_project_file, session_messages_dir,
    sessions_dir, CoderSettings, SettingsItem, SettingsTab, StoredKeybinding,
    StoredModifiers,
};
use crate::app::events::CoderMode;
use crate::app::events::{
    convert_key_for_binding, convert_key_for_input, convert_modifiers, convert_mouse_button,
    keybinding_labels, key_from_string, key_to_string, CommandAction, ModalState, QueryControl,
    ResponseEvent,
};
use crate::app::parsing::{build_context_injection, build_todo_context, expand_prompt_text};
use crate::app::session::{
    apply_session_history_limit, load_session_index, save_session_index, RateLimitInfo, RateLimits,
    SessionEntry, SessionInfo, SessionUsageStats,
};
use crate::app::tools::{tool_result_output, DspyStageLayout, ToolPanelBlock};
use crate::app::ui::{palette_for, split_into_words_for_layout, wrap_text, ThemeSetting, UiPalette};
use crate::app::AppState;
use crate::app::{
    build_input, build_markdown_config, build_markdown_renderer, format_relative_time,
    now_timestamp, selection_point_cmp, settings_rows, truncate_bytes, truncate_preview,
    HookLogEntry, HookModalView, ModelOption, SettingsInputMode, SettingsSnapshot,
};

fn byte_offset_for_char_index(text: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    text.char_indices()
        .nth(char_index)
        .map(|(idx, _)| idx)
        .unwrap_or_else(|| text.len())
}

fn char_index_for_byte_offset(text: &str, byte_offset: usize) -> usize {
    let clamped = byte_offset.min(text.len());
    text[..clamped].chars().count()
}

fn prefix_text_for_line(prefix: &LinePrefix, text_system: &mut TextSystem) -> (String, f32) {
    let font_style = wgpui::text::FontStyle::default();
    let prefix_width = text_system.measure_styled_mono(&prefix.text, prefix.font_size, font_style);
    let space_width = text_system.measure_styled_mono(" ", prefix.font_size, font_style).max(1.0);
    let gap_px = (prefix.content_x - prefix.x - prefix_width).max(space_width);
    let space_count = (gap_px / space_width).round().max(1.0) as usize;
    let mut text = prefix.text.clone();
    text.push_str(&" ".repeat(space_count));
    let total_width = prefix_width + space_width * space_count as f32;
    (text, total_width)
}

fn line_text_from_styled(lines: &[StyledLine]) -> String {
    let mut out = String::new();
    for (line_idx, line) in lines.iter().enumerate() {
        if line_idx > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            out.push_str(&span.text);
        }
    }
    out
}

fn layout_styled_lines(
    lines: &[StyledLine],
    origin: Point,
    max_width: f32,
    base_indent: u32,
    text_system: &mut TextSystem,
    builder: &mut MessageLayoutBuilder,
    prefix: &mut Option<LinePrefix>,
) -> f32 {
    let mut y = origin.y;
    let mut first_visual_line = true;

    for line in lines {
        y += line.margin_top;
        let indent = (base_indent + line.indent) as f32 * wgpui::theme::spacing::LG;
        let line_start_x = origin.x + indent;
        let right_edge = origin.x + max_width;

        let base_font_size = line
            .spans
            .first()
            .map(|s| s.style.font_size)
            .unwrap_or(wgpui::theme::font_size::BASE);
        let line_height = base_font_size * line.line_height;

        let mut current_x = line_start_x;
        let mut line_x = line_start_x;
        let mut current_line_text = String::new();

        if first_visual_line {
            if let Some(prefix_line) = prefix.take() {
                let (prefix_text, prefix_width) = prefix_text_for_line(&prefix_line, text_system);
                line_x = prefix_line.x;
                current_x = prefix_line.x + prefix_width;
                current_line_text.push_str(&prefix_text);
            }
        }

        for span in &line.spans {
            let font_style = wgpui::text::FontStyle {
                bold: span.style.bold,
                italic: span.style.italic,
            };
            let words = split_into_words_for_layout(&span.text);

            for word in words {
                if word.is_empty() {
                    continue;
                }

                let word_width = text_system.measure_styled_mono(word, span.style.font_size, font_style);
                if current_x + word_width > right_edge && current_x > line_start_x {
                    builder.push_line(current_line_text, line_x, y, line_height, base_font_size);
                    y += line_height;
                    current_line_text = String::new();
                    current_x = line_start_x;
                    line_x = line_start_x;
                }

                current_line_text.push_str(word);
                current_x += word_width;
            }
        }

        builder.push_line(
            current_line_text,
            line_x,
            y,
            line_height,
            base_font_size,
        );
        y += line_height;
        first_visual_line = false;
    }

    y - origin.y
}

fn layout_markdown_block(
    block: &MarkdownBlock,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    config: &MarkdownConfig,
    builder: &mut MessageLayoutBuilder,
    prefix: &mut Option<LinePrefix>,
) -> f32 {
    match block {
        MarkdownBlock::Paragraph(lines) => {
            layout_styled_lines(lines, origin, max_width, 0, text_system, builder, prefix)
        }
        MarkdownBlock::Header { level, lines } => {
            let margin_top = match level {
                1 => wgpui::theme::spacing::XL,
                2 => wgpui::theme::spacing::LG,
                _ => wgpui::theme::spacing::MD,
            };
            margin_top
                + layout_styled_lines(
                    lines,
                    Point::new(origin.x, origin.y + margin_top),
                    max_width,
                    0,
                    text_system,
                    builder,
                    prefix,
                )
        }
        MarkdownBlock::CodeBlock { lines, .. } => {
            let margin = wgpui::theme::spacing::SM;
            let padding = wgpui::theme::spacing::SM;
            let header_height = wgpui::theme::font_size::XS + wgpui::theme::spacing::XS;

            let content_origin = Point::new(
                origin.x + padding,
                origin.y + margin + header_height + padding,
            );

            let content_height = layout_styled_lines(
                lines,
                content_origin,
                max_width - padding * 2.0,
                0,
                text_system,
                builder,
                prefix,
            );

            content_height + padding * 2.0 + header_height + margin * 2.0
        }
        MarkdownBlock::Blockquote(blocks) => {
            let bar_width = 4.0;
            let gap = wgpui::theme::spacing::MD;
            let indent = bar_width + gap;
            let margin = wgpui::theme::spacing::SM;
            let start_y = origin.y + margin;
            let mut y = start_y;

            for block in blocks {
                y += layout_markdown_block(
                    block,
                    Point::new(origin.x + indent, y),
                    max_width - indent,
                    text_system,
                    config,
                    builder,
                    prefix,
                );
            }

            y - start_y + margin * 2.0
        }
        MarkdownBlock::UnorderedList(items) => {
            let indent = wgpui::theme::spacing::XL;
            let bullet_x = origin.x + wgpui::theme::spacing::SM;
            let margin = wgpui::theme::spacing::XS;
            let mut y = origin.y + margin;

            for item in items {
                let mut item_prefix = Some(LinePrefix {
                    text: "\u{2022}".to_string(),
                    x: bullet_x,
                    content_x: origin.x + indent,
                    font_size: config.base_font_size,
                });
                for block in item {
                    y += layout_markdown_block(
                        block,
                        Point::new(origin.x + indent, y),
                        max_width - indent,
                        text_system,
                        config,
                        builder,
                        &mut item_prefix,
                    );
                }
            }

            y - origin.y + margin
        }
        MarkdownBlock::OrderedList { start, items } => {
            let indent = wgpui::theme::spacing::XL * 2.0;
            let margin = wgpui::theme::spacing::XS;
            let mut y = origin.y + margin;

            for (idx, item) in items.iter().enumerate() {
                let number = start + idx as u64;
                let mut item_prefix = Some(LinePrefix {
                    text: format!("{}.", number),
                    x: origin.x,
                    content_x: origin.x + indent,
                    font_size: config.base_font_size,
                });
                for block in item {
                    y += layout_markdown_block(
                        block,
                        Point::new(origin.x + indent, y),
                        max_width - indent,
                        text_system,
                        config,
                        builder,
                        &mut item_prefix,
                    );
                }
            }

            y - origin.y + margin
        }
        MarkdownBlock::HorizontalRule => {
            let margin = wgpui::theme::spacing::LG;
            margin * 2.0 + 1.0
        }
        MarkdownBlock::Table { headers, rows } => {
            if headers.is_empty() {
                return 0.0;
            }

            let cell_padding = wgpui::theme::spacing::SM;
            let mut y = origin.y + cell_padding;
            let header_text = headers
                .iter()
                .map(|cell| line_text_from_styled(cell))
                .collect::<Vec<_>>()
                .join(" | ");
            builder.push_line(
                header_text,
                origin.x + cell_padding,
                y,
                32.0,
                config.base_font_size,
            );
            y += 32.0 + 1.0;

            for row in rows {
                let row_text = row
                    .iter()
                    .map(|cell| line_text_from_styled(cell))
                    .collect::<Vec<_>>()
                    .join(" | ");
                builder.push_line(
                    row_text,
                    origin.x + cell_padding,
                    y + cell_padding,
                    28.0,
                    config.base_font_size,
                );
                y += 28.0;
            }

            y - origin.y
        }
    }
}

fn layout_markdown_document(
    document: &MarkdownDocument,
    origin: Point,
    max_width: f32,
    text_system: &mut TextSystem,
    config: &MarkdownConfig,
    builder: &mut MessageLayoutBuilder,
) -> f32 {
    let mut y = origin.y;

    for (i, block) in document.blocks.iter().enumerate() {
        if i > 0 {
            y += wgpui::theme::spacing::MD;
            builder.push_gap();
        }
        let mut prefix = None;
        y += layout_markdown_block(
            block,
            Point::new(origin.x, y),
            max_width,
            text_system,
            config,
            builder,
            &mut prefix,
        );
    }

    y - origin.y
}

const INPUT_HEIGHT: f32 = 40.0;
const INPUT_PADDING: f32 = 12.0;
const OUTPUT_PADDING: f32 = 12.0;
const STATUS_BAR_HEIGHT: f32 = 20.0;
const STATUS_BAR_FONT_SIZE: f32 = 13.0;
/// Height of input area (input + padding + status bar) for modal positioning
const INPUT_AREA_HEIGHT: f32 = INPUT_HEIGHT + INPUT_PADDING + STATUS_BAR_HEIGHT;

/// Calculate modal Y position centered in main content area (above input area)
fn modal_y_in_content(logical_height: f32, modal_height: f32) -> f32 {
    let content_height = logical_height - INPUT_AREA_HEIGHT;
    (content_height - modal_height) / 2.0
}
const BUG_REPORT_URL: &str = "https://github.com/OpenAgentsInc/openagents/issues/new";
const SIDEBAR_WIDTH: f32 = 220.0;
const SIDEBAR_MIN_MAIN: f32 = 320.0;

mod command_palette_ids {
    pub const HELP: &str = "help.open";
    pub const SETTINGS: &str = "settings.open";
    pub const MODEL_PICKER: &str = "model.open";
    pub const SESSION_LIST: &str = "session.list";
    pub const SESSION_FORK: &str = "session.fork";
    pub const SESSION_EXPORT: &str = "session.export";
    pub const CLEAR_CONVERSATION: &str = "session.clear";
    pub const UNDO_LAST: &str = "session.undo";
    pub const COMPACT_CONTEXT: &str = "context.compact";
    pub const INTERRUPT_REQUEST: &str = "request.interrupt";
    pub const PERMISSION_RULES: &str = "permissions.rules";
    pub const MODE_CYCLE: &str = "mode.cycle";
    pub const MODE_BYPASS: &str = "mode.bypass";
    pub const MODE_PLAN: &str = "mode.plan";
    pub const MODE_AUTOPILOT: &str = "mode.autopilot";
    pub const TOOLS_LIST: &str = "tools.list";
    pub const MCP_CONFIG: &str = "mcp.open";
    pub const MCP_RELOAD: &str = "mcp.reload";
    pub const MCP_STATUS: &str = "mcp.status";
    pub const AGENTS_LIST: &str = "agents.list";
    pub const AGENT_CLEAR: &str = "agents.clear";
    pub const AGENT_RELOAD: &str = "agents.reload";
    pub const SKILLS_LIST: &str = "skills.list";
    pub const SKILLS_RELOAD: &str = "skills.reload";
    pub const HOOKS_OPEN: &str = "hooks.open";
    pub const HOOKS_RELOAD: &str = "hooks.reload";
    pub const SIDEBAR_LEFT: &str = "sidebar.left";
    pub const SIDEBAR_RIGHT: &str = "sidebar.right";
    pub const SIDEBAR_TOGGLE: &str = "sidebar.toggle";
    pub const BUG_REPORT: &str = "bug.report";
    pub const KITCHEN_SINK: &str = "dev.kitchen_sink";
}

fn clamp_font_size(size: f32) -> f32 {
    size.clamp(12.0, 18.0)
}

fn normalize_settings(settings: &mut CoderSettings) {
    settings.font_size = clamp_font_size(settings.font_size);
}

struct SidebarLayout {
    left: Option<Bounds>,
    right: Option<Bounds>,
    main: Bounds,
}

fn sidebar_layout(
    logical_width: f32,
    logical_height: f32,
    left_open: bool,
    right_open: bool,
) -> SidebarLayout {
    let mut left_width = if left_open { SIDEBAR_WIDTH } else { 0.0 };
    let mut right_width = if right_open { SIDEBAR_WIDTH } else { 0.0 };
    let available_main = logical_width - left_width - right_width;
    if available_main < SIDEBAR_MIN_MAIN {
        let overflow = SIDEBAR_MIN_MAIN - available_main;
        if left_width > 0.0 && right_width > 0.0 {
            let reduce = overflow / 2.0;
            left_width = (left_width - reduce).max(120.0);
            right_width = (right_width - reduce).max(120.0);
        } else if left_width > 0.0 {
            left_width = (left_width - overflow).max(120.0);
        } else if right_width > 0.0 {
            right_width = (right_width - overflow).max(120.0);
        }
    }
    let main_width = (logical_width - left_width - right_width).max(1.0);
    let main = Bounds::new(left_width, 0.0, main_width, logical_height);
    let left = if left_width > 0.0 {
        Some(Bounds::new(0.0, 0.0, left_width, logical_height))
    } else {
        None
    };
    let right = if right_width > 0.0 {
        Some(Bounds::new(
            logical_width - right_width,
            0.0,
            right_width,
            logical_height,
        ))
    } else {
        None
    };

    SidebarLayout { left, right, main }
}

/// Calculate bounds for the "New Session" button in the left sidebar
fn new_session_button_bounds(sidebar_bounds: Bounds) -> Bounds {
    Bounds::new(
        sidebar_bounds.origin.x + 12.0,
        sidebar_bounds.origin.y + 12.0,
        sidebar_bounds.size.width - 24.0,
        32.0,
    )
}

/// Format token count with K/M suffixes
fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Format duration in ms/s/m notation
fn format_duration_ms(ms: u64) -> String {
    if ms < 1_000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1_000.0)
    } else {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1_000;
        format!("{}m{}s", mins, secs)
    }
}

/// Format a reset timestamp as relative time (e.g., "3d", "5h", "30m")
fn format_reset_time(timestamp: i64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let diff = timestamp - now;

    if diff <= 0 {
        return "soon".to_string();
    }
    if diff < 3600 {
        return format!("{}m", diff / 60);
    }
    if diff < 86400 {
        return format!("{}h", diff / 3600);
    }
    format!("{}d", diff / 86400)
}

/// Parse rate limit headers from Anthropic API response
/// Supports multiple header formats:
/// - anthropic-ratelimit-unified-* (Claude Code format)
/// - x-ratelimit-* (standard Anthropic API)
fn parse_rate_limit_headers(headers: &reqwest::header::HeaderMap) -> Option<RateLimits> {
    let mut limits = RateLimits::default();

    // Helper to get header as string
    let get_header = |name: &str| -> Option<&str> {
        headers.get(name)?.to_str().ok()
    };

    // Try unified format first: anthropic-ratelimit-unified-7d-utilization (0-1 range)
    let unified_claims = [
        ("7d", "weekly"),
        ("7ds", "sonnet"),
        ("7do", "opus"),
        ("5h", "session"),
    ];

    for (claim, name) in unified_claims {
        let util_header = format!("anthropic-ratelimit-unified-{}-utilization", claim);
        let reset_header = format!("anthropic-ratelimit-unified-{}-reset", claim);

        if let Some(util_str) = get_header(&util_header) {
            if let Ok(util_val) = util_str.parse::<f64>() {
                let reset = get_header(&reset_header)
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(format_reset_time)
                    .unwrap_or_default();

                let info = RateLimitInfo {
                    name: name.to_string(),
                    percent_used: util_val * 100.0,
                    resets_at: reset,
                };

                if limits.primary.is_none() {
                    limits.primary = Some(info);
                } else if limits.secondary.is_none() {
                    limits.secondary = Some(info);
                    break;
                }
            }
        }
    }

    // Try standard x-ratelimit headers (public Anthropic API)
    if limits.primary.is_none() {
        if let (Some(limit_str), Some(remaining_str)) = (
            get_header("x-ratelimit-limit-requests"),
            get_header("x-ratelimit-remaining-requests"),
        ) {
            if let (Ok(limit), Ok(remaining)) = (
                limit_str.parse::<i64>(),
                remaining_str.parse::<i64>(),
            ) {
                if limit > 0 {
                    let used = limit - remaining;
                    let percent = (used as f64 / limit as f64) * 100.0;

                    // Parse reset time
                    let reset = get_header("x-ratelimit-reset-requests")
                        .map(|s| {
                            // Format might be "60s" or ISO timestamp
                            if s.ends_with('s') {
                                s.trim_end_matches('s')
                                    .parse::<i64>()
                                    .ok()
                                    .map(|secs| {
                                        let now = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0);
                                        format_reset_time(now + secs)
                                    })
                                    .unwrap_or_default()
                            } else {
                                s.to_string()
                            }
                        })
                        .unwrap_or_default();

                    limits.primary = Some(RateLimitInfo {
                        name: "requests".to_string(),
                        percent_used: percent,
                        resets_at: reset,
                    });
                }
            }
        }

        // Also try token limits
        if let (Some(limit_str), Some(remaining_str)) = (
            get_header("x-ratelimit-limit-tokens"),
            get_header("x-ratelimit-remaining-tokens"),
        ) {
            if let (Ok(limit), Ok(remaining)) = (
                limit_str.parse::<i64>(),
                remaining_str.parse::<i64>(),
            ) {
                if limit > 0 {
                    let used = limit - remaining;
                    let percent = (used as f64 / limit as f64) * 100.0;

                    let reset = get_header("x-ratelimit-reset-tokens")
                        .map(|s| {
                            if s.ends_with('s') {
                                s.trim_end_matches('s')
                                    .parse::<i64>()
                                    .ok()
                                    .map(|secs| {
                                        let now = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0);
                                        format_reset_time(now + secs)
                                    })
                                    .unwrap_or_default()
                            } else {
                                s.to_string()
                            }
                        })
                        .unwrap_or_default();

                    let info = RateLimitInfo {
                        name: "tokens".to_string(),
                        percent_used: percent,
                        resets_at: reset,
                    };

                    if limits.primary.is_none() {
                        limits.primary = Some(info);
                    } else if limits.secondary.is_none() {
                        limits.secondary = Some(info);
                    }
                }
            }
        }
    }

    if limits.primary.is_some() || limits.secondary.is_some() {
        Some(limits)
    } else {
        None
    }
}

/// Load OAuth access token from Claude credentials
fn load_claude_oauth_token() -> Option<String> {
    // Try Linux keyring via secret-tool first
    #[cfg(target_os = "linux")]
    {
        if let Ok(output) = std::process::Command::new("secret-tool")
            .args(["lookup", "service", "Claude Code-credentials"])
            .output()
        {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    if let Ok(json) = serde_json::from_str::<Value>(&json_str) {
                        if let Some(token) = json
                            .get("claudeAiOauth")
                            .and_then(|o| o.get("accessToken"))
                            .and_then(|v| v.as_str())
                        {
                            tracing::info!("Loaded OAuth token from Linux keyring");
                            return Some(token.to_string());
                        }
                    }
                }
            }
        }
    }

    // Try macOS keychain
    #[cfg(target_os = "macos")]
    {
        let username = std::env::var("USER").ok()?;
        if let Ok(output) = std::process::Command::new("security")
            .args([
                "find-generic-password",
                "-s",
                "Claude Code-credentials",
                "-a",
                &username,
                "-w",
            ])
            .output()
        {
            if output.status.success() {
                if let Ok(json_str) = String::from_utf8(output.stdout) {
                    if let Ok(json) = serde_json::from_str::<Value>(&json_str.trim()) {
                        if let Some(token) = json
                            .get("claudeAiOauth")
                            .and_then(|o| o.get("accessToken"))
                            .and_then(|v| v.as_str())
                        {
                            tracing::info!("Loaded OAuth token from macOS keychain");
                            return Some(token.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fall back to file-based credentials
    let home = std::env::var("HOME").ok()?;
    let paths = [
        format!("{}/.claude/.credentials.json", home),
        format!("{}/.claude/.credentials", home),
    ];

    for path in &paths {
        if let Ok(contents) = std::fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<Value>(&contents) {
                if let Some(token) = json
                    .get("claudeAiOauth")
                    .and_then(|o| o.get("accessToken"))
                    .and_then(|v| v.as_str())
                {
                    tracing::info!("Loaded OAuth token from {}", path);
                    return Some(token.to_string());
                }
            }
        }
    }

    tracing::warn!("No Claude OAuth credentials found");
    None
}

/// Fetch rate limits by making a minimal API call using OAuth
async fn fetch_rate_limits() -> Option<RateLimits> {
    // Try OAuth first, fall back to API key
    let (auth_header, auth_value) = if let Some(token) = load_claude_oauth_token() {
        ("authorization", format!("Bearer {}", token))
    } else if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
        ("x-api-key", api_key)
    } else {
        tracing::warn!("No OAuth token or API key available for rate limit fetch");
        return None;
    };

    tracing::info!("Fetching rate limits...");

    let client = reqwest::Client::new();
    let mut request = client
        .post("https://api.anthropic.com/v1/messages")
        .header("content-type", "application/json")
        .header("anthropic-version", "2023-06-01")
        .header(auth_header, &auth_value);

    // Add OAuth beta header if using OAuth
    if auth_header == "authorization" {
        request = request.header("anthropic-beta", "oauth-2025-04-20");
    }

    let response = match request
        .body(r#"{"model":"claude-3-haiku-20240307","max_tokens":1,"messages":[{"role":"user","content":"x"}]}"#)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Rate limit fetch failed: {}", e);
            return None;
        }
    };

    // Log response headers for debugging
    for (name, value) in response.headers() {
        if name.as_str().contains("ratelimit") || name.as_str().contains("limit") {
            tracing::info!("Rate limit header: {} = {:?}", name, value);
        }
    }

    let limits = parse_rate_limit_headers(response.headers());
    if let Some(ref l) = limits {
        if let Some(ref p) = l.primary {
            tracing::info!("Rate limit: {} {:.1}% used, resets {}", p.name, p.percent_used, p.resets_at);
        }
    } else {
        tracing::warn!("No rate limit data found in response headers");
    }
    limits
}

const SESSION_MODAL_WIDTH: f32 = 760.0;
const SESSION_MODAL_HEIGHT: f32 = 520.0;
const SESSION_CARD_HEIGHT: f32 = 100.0;
const SESSION_CARD_GAP: f32 = 12.0;
const SESSION_MODAL_PADDING: f32 = 16.0;
const SKILL_CARD_HEIGHT: f32 = 110.0;
const SETTINGS_MODAL_WIDTH: f32 = 760.0;
const SETTINGS_MODAL_HEIGHT: f32 = 480.0;
const SETTINGS_ROW_HEIGHT: f32 = 24.0;
const SETTINGS_TAB_HEIGHT: f32 = 22.0;
const HELP_MODAL_WIDTH: f32 = 760.0;
const HELP_MODAL_HEIGHT: f32 = 520.0;
const HOOK_MODAL_WIDTH: f32 = 860.0;
const HOOK_MODAL_HEIGHT: f32 = 520.0;
const HOOK_EVENT_ROW_HEIGHT: f32 = 20.0;
const HOOK_LOG_LIMIT: usize = 200;
const HOOK_SCRIPT_TIMEOUT_SECS: u64 = 12;
const HOOK_OUTPUT_TRUNCATE: usize = 2000;
const HOOK_BLOCK_PATTERNS: [&str; 3] = ["rm -rf /", "sudo", "> /dev/"];
const TOOL_PANEL_GAP: f32 = 8.0;
const TOOL_HISTORY_LIMIT: usize = 100;

#[derive(Clone, Debug)]
struct LinePrefix {
    text: String,
    x: f32,
    content_x: f32,
    font_size: f32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HookSetting {
    ToolBlocker,
    ToolLogger,
    OutputTruncator,
    ContextInjection,
    TodoEnforcer,
}

#[derive(Clone, Debug)]
pub(crate) enum HookCallbackKind {
    ToolBlocker,
    ToolLogger,
    OutputTruncator,
    ContextEnforcer,
    Script(HookScriptEntry),
}

pub(crate) struct CoderHookCallback {
    kind: HookCallbackKind,
    runtime: Arc<HookRuntimeConfig>,
}

impl CoderHookCallback {
    pub(crate) fn new(kind: HookCallbackKind, runtime: Arc<HookRuntimeConfig>) -> Self {
        Self { kind, runtime }
    }
}

#[async_trait]
impl HookCallback for CoderHookCallback {
    async fn call(&self, input: HookInput, tool_use_id: Option<String>) -> SdkResult<HookOutput> {
        let event = hook_event_from_input(&input);
        let tool_name = hook_tool_name(&input);
        let matcher = match &self.kind {
            HookCallbackKind::Script(entry) => entry.matcher.clone(),
            _ => None,
        };

        let summary: String;
        let mut error = None;
        let mut sources = Vec::new();
        let mut output = HookOutput::Sync(SyncHookOutput::continue_execution());
        let mut log_output = true;

        match &self.kind {
            HookCallbackKind::ToolBlocker => {
                sources.push("builtin:tool_blocker".to_string());
                let (next_output, next_summary) = hook_tool_blocker(&input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::ToolLogger => {
                sources.push("builtin:tool_logger".to_string());
                summary = hook_tool_logger_summary(&input);
            }
            HookCallbackKind::OutputTruncator => {
                sources.push("builtin:output_truncator".to_string());
                let (next_output, next_summary) = hook_output_truncator(&input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::ContextEnforcer => {
                sources.extend(hook_context_sources(&self.runtime.config));
                let (next_output, next_summary) =
                    hook_context_enforcer(&self.runtime, &input);
                output = next_output;
                summary = next_summary;
            }
            HookCallbackKind::Script(entry) => {
                sources.push(hook_script_source_label(entry));
                match run_hook_script(entry, &input, tool_use_id.as_deref(), &self.runtime).await {
                    Ok(next_output) => {
                        output = next_output;
                        summary = format!("Script {} completed.", entry.path.display());
                    }
                    Err(err) => {
                        summary = format!("Script {} failed.", entry.path.display());
                        error = Some(err);
                        log_output = false;
                    }
                }
            }
        }

        let output_ref = if log_output { Some(&output) } else { None };
        log_hook_event(
            &self.runtime,
            event,
            summary,
            tool_name,
            matcher,
            &input,
            output_ref,
            error,
            sources,
        );

        Ok(output)
    }
}

fn parse_legacy_model_setting(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(model_id) = line.strip_prefix("model = \"").and_then(|s| s.strip_suffix("\"")) {
            return Some(model_id.to_string());
        }
    }
    None
}

fn load_settings() -> CoderSettings {
    let path = config_file();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(mut settings) = toml::from_str::<CoderSettings>(&content) {
            normalize_settings(&mut settings);
            return settings;
        }
        let mut settings = CoderSettings::default();
        settings.model = parse_legacy_model_setting(&content);
        normalize_settings(&mut settings);
        return settings;
    }
    CoderSettings::default()
}

fn save_settings(settings: &CoderSettings) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        if let Ok(content) = toml::to_string_pretty(settings) {
            let _ = fs::write(config_file(), content);
        }
    }
}

/// Auto-start llama-server if not already running.
///
/// Returns the child process handle if started, None if already running or unable to start.
fn auto_start_llama_server() -> Option<Child> {
    // Check if already running on port 8000 or 8080
    if adjutant::dspy::lm_config::check_llamacpp_available() {
        tracing::info!("llama-server already running, skipping auto-start");
        return None;
    }

    // Find llama-server binary
    let binary = find_llama_server_binary()?;
    tracing::info!("Found llama-server at: {}", binary.display());

    // Find a usable model
    let model = find_gguf_model()?;
    tracing::info!("Found GGUF model at: {}", model.display());

    // Start llama-server on port 8000
    let port = 8000;
    tracing::info!("Starting llama-server on port {}...", port);

    match ProcessCommand::new(&binary)
        .arg("-m")
        .arg(&model)
        .arg("--port")
        .arg(port.to_string())
        .arg("--ctx-size")
        .arg("8192")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => {
            tracing::info!("llama-server started with PID {}", child.id());
            // Give it a moment to bind the port
            std::thread::sleep(std::time::Duration::from_millis(500));
            Some(child)
        }
        Err(e) => {
            tracing::warn!("Failed to start llama-server: {}", e);
            None
        }
    }
}

/// Find llama-server binary in common locations.
fn find_llama_server_binary() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Check common locations
    let candidates = [
        home.join("code/llama.cpp/build/bin/llama-server"),
        home.join("code/llama.cpp/llama-server"),
        home.join("llama.cpp/build/bin/llama-server"),
        home.join("llama.cpp/llama-server"),
        home.join(".local/bin/llama-server"),
        PathBuf::from("/usr/local/bin/llama-server"),
        PathBuf::from("/usr/bin/llama-server"),
    ];

    for path in &candidates {
        if path.exists() && path.is_file() {
            return Some(path.clone());
        }
    }

    // Try which command
    if let Ok(output) = ProcessCommand::new("which")
        .arg("llama-server")
        .output()
    {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    None
}

/// Find a usable GGUF model file.
fn find_gguf_model() -> Option<PathBuf> {
    let home = dirs::home_dir()?;

    // Check llama.cpp cache first (where downloaded models go)
    let cache_dir = home.join(".cache/llama.cpp");
    if cache_dir.exists() {
        if let Ok(entries) = fs::read_dir(&cache_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "gguf") {
                    // Skip vocab-only files (usually small)
                    if let Ok(meta) = fs::metadata(&path) {
                        // Real models are at least 100MB
                        if meta.len() > 100_000_000 {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    // Check models directory
    let models_dir = home.join("code/llama.cpp/models");
    if models_dir.exists() {
        if let Ok(entries) = fs::read_dir(&models_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "gguf") {
                    if let Ok(meta) = fs::metadata(&path) {
                        if meta.len() > 100_000_000 {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }

    None
}

fn settings_model_option(settings: &CoderSettings) -> ModelOption {
    settings
        .model
        .as_deref()
        .map(ModelOption::from_id)
        .unwrap_or(ModelOption::Opus)
}

fn update_settings_model(settings: &mut CoderSettings, model: ModelOption) {
    settings.model = Some(model.model_id().to_string());
}

fn load_keybindings() -> Vec<Keybinding> {
    let path = keybindings_file();
    let Ok(content) = fs::read_to_string(&path) else {
        return default_keybindings();
    };
    let Ok(entries) = serde_json::from_str::<Vec<StoredKeybinding>>(&content) else {
        return default_keybindings();
    };
    let mut bindings = Vec::new();
    for entry in entries {
        let Some(action) = KeyAction::from_id(&entry.action) else {
            continue;
        };
        let Some(key) = key_from_string(&entry.key) else {
            continue;
        };
        let modifiers = UiModifiers {
            shift: entry.modifiers.shift,
            ctrl: entry.modifiers.ctrl,
            alt: entry.modifiers.alt,
            meta: entry.modifiers.meta,
        };
        bindings.push(Keybinding {
            key,
            modifiers,
            action,
        });
    }
    if bindings.is_empty() {
        default_keybindings()
    } else {
        bindings
    }
}

fn save_keybindings(bindings: &[Keybinding]) {
    let dir = config_dir();
    if fs::create_dir_all(&dir).is_ok() {
        let entries: Vec<StoredKeybinding> = bindings
            .iter()
            .map(|binding| StoredKeybinding {
                action: binding.action.id().to_string(),
                key: key_to_string(&binding.key),
                modifiers: StoredModifiers {
                    shift: binding.modifiers.shift,
                    ctrl: binding.modifiers.ctrl,
                    alt: binding.modifiers.alt,
                    meta: binding.modifiers.meta,
                },
            })
            .collect();
        if let Ok(content) = serde_json::to_string_pretty(&entries) {
            let _ = fs::write(keybindings_file(), content);
        }
    }
}

fn parse_mcp_status(value: &Value) -> Result<Vec<McpServerStatus>, String> {
    if let Some(servers_value) = value
        .get("mcp_servers")
        .or_else(|| value.get("servers"))
    {
        serde_json::from_value(servers_value.clone())
            .map_err(|err| format!("Failed to parse MCP status: {}", err))
    } else if value.is_array() {
        serde_json::from_value(value.clone())
            .map_err(|err| format!("Failed to parse MCP status: {}", err))
    } else {
        Err("Unexpected MCP status response".to_string())
    }
}

/// Main application
pub struct CoderApp {
    state: Option<AppState>,
    runtime_handle: tokio::runtime::Handle,
}

impl CoderApp {
    pub fn new(runtime_handle: tokio::runtime::Handle) -> Self {
        Self {
            state: None,
            runtime_handle,
        }
    }
}

impl ApplicationHandler for CoderApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        let window_attrs = Window::default_attributes()
            .with_title("Coder")
            .with_inner_size(winit::dpi::LogicalSize::new(900, 600));

        let window = Arc::new(
            event_loop
                .create_window(window_attrs)
                .expect("Failed to create window"),
        );

        let state = pollster::block_on(async {
            let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });

            let surface = instance
                .create_surface(window.clone())
                .expect("Failed to create surface");

            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: Some(&surface),
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to find adapter");

            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor::default(), None)
                .await
                .expect("Failed to create device");

            let size = window.inner_size();
            let surface_caps = surface.get_capabilities(&adapter);
            let surface_format = surface_caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(surface_caps.formats[0]);

            let config = wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: size.width.max(1),
                height: size.height.max(1),
                present_mode: wgpu::PresentMode::AutoVsync,
                alpha_mode: surface_caps.alpha_modes[0],
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            surface.configure(&device, &config);

            let renderer = Renderer::new(&device, surface_format);
            let scale_factor = window.scale_factor() as f32;
            let text_system = TextSystem::new(scale_factor);
            let clipboard = Rc::new(RefCell::new(Clipboard::new().ok()));
            let mut event_context = EventContext::new();
            let read_clip = clipboard.clone();
            let write_clip = clipboard.clone();
            event_context.set_clipboard(
                move || read_clip.borrow_mut().as_mut()?.get_text().ok(),
                move |text| {
                    if let Some(clip) = write_clip.borrow_mut().as_mut() {
                        let _ = clip.set_text(text);
                    }
                },
            );
            let (command_palette_tx, command_palette_rx) = mpsc::unbounded_channel();
            let command_palette = CommandPalette::new()
                .max_visible_items(8)
                .mono(true)
                .on_select(move |command| {
                    let _ = command_palette_tx.send(command.id.clone());
                });
            let settings = load_settings();
            let input = build_input(&settings);

            let selected_model = settings_model_option(&settings);
            let mut session_index = load_session_index();
            let removed_sessions =
                apply_session_history_limit(&mut session_index, settings.session_history_limit);
            if !removed_sessions.is_empty() {
                let _ = save_session_index(&session_index);
            }
            let permission_config = load_permission_config();
            let coder_mode = permission_config.coder_mode;
            let permission_default_allow =
                coder_mode_default_allow(coder_mode, permission_config.default_allow);
            let coder_mode_label_str = coder_mode_label(coder_mode).to_string();
            let cwd = std::env::current_dir().unwrap_or_default();
            let (mcp_project_servers, mcp_project_error) = load_mcp_project_servers(&cwd);
            let mcp_project_path = Some(mcp_project_file(&cwd));
            let agent_catalog = load_agent_entries(&cwd);
            let skill_catalog = load_skill_entries(&cwd);
            let hook_config = load_hook_config();
            let hook_catalog = load_hook_scripts(&cwd);

            // Auto-start llama-server if available but not running
            let llama_server_process = auto_start_llama_server();

            // Detect available LM providers on startup (after potential auto-start)
            let available_providers = adjutant::dspy::lm_config::detect_all_providers();
            tracing::info!("Available LM providers: {:?}", available_providers);

            // Boot OANIX on startup (async, will be cached when ready)
            tracing::info!("Booting OANIX runtime...");
            let (oanix_tx, oanix_rx) = mpsc::unbounded_channel();
            let oanix_manifest_rx = Some(oanix_rx);
            tokio::spawn(async move {
                match oanix::boot().await {
                    Ok(manifest) => {
                        tracing::info!("OANIX booted on startup, workspace: {:?}",
                            manifest.workspace.as_ref().map(|w| &w.root));
                        let _ = oanix_tx.send(manifest);
                    }
                    Err(e) => {
                        tracing::warn!("OANIX boot failed on startup: {}", e);
                    }
                }
            });

            // Fetch rate limits on startup
            let (rate_limit_tx, rate_limit_rx) = mpsc::unbounded_channel();
            tokio::spawn(async move {
                if let Some(limits) = fetch_rate_limits().await {
                    let _ = rate_limit_tx.send(limits);
                }
            });

            AppState {
                window,
                surface,
                device,
                queue,
                config,
                renderer,
                text_system,
                event_context,
                clipboard,
                command_palette,
                command_palette_action_rx: Some(command_palette_rx),
                input,
                mouse_pos: (0.0, 0.0),
                modifiers: ModifiersState::default(),
                last_tick: Instant::now(),
                modal_state: ModalState::None,
                panel_layout: PanelLayout::Single,
                left_sidebar_open: false,
                right_sidebar_open: false,
                new_session_button_hovered: false,
                chat: ChatState::new(&settings),
                tools: ToolsState::new(),
                session: SessionState::new(
                    selected_model,
                    coder_mode_label_str,
                    session_index,
                    Some(rate_limit_rx),
                ),
                catalogs: CatalogState::new(
                    agent_catalog,
                    skill_catalog,
                    hook_config,
                    hook_catalog,
                    mcp_project_servers,
                    mcp_project_error,
                    mcp_project_path,
                ),
                settings: SettingsState::new(settings, load_keybindings(), selected_model),
                permissions: PermissionState::new(
                    coder_mode,
                    permission_default_allow,
                    permission_config.allow_tools,
                    permission_config.deny_tools,
                    permission_config.bash_allow_patterns,
                    permission_config.bash_deny_patterns,
                ),
                autopilot: AutopilotState::new(oanix_manifest_rx, available_providers),
                llama_server_process,
                show_kitchen_sink: false,
                kitchen_sink_scroll: 0.0,
            }
        });

        let window_clone = state.window.clone();
        self.state = Some(state);
        tracing::info!("Window initialized");

        // Request initial redraw
        window_clone.request_redraw();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        // Poll for SDK responses first
        self.poll_responses();
        self.poll_permissions();
        self.poll_command_palette_actions();
        self.poll_session_actions();
        self.poll_agent_actions();
        self.poll_skill_actions();
        self.poll_hook_inspector_actions();
        self.poll_oanix_manifest();
        self.poll_autopilot_history();
        self.poll_rate_limits();

        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        let sidebar_layout = sidebar_layout(
            logical_width,
            logical_height,
            state.left_sidebar_open,
            state.right_sidebar_open,
        );
        let content_x = sidebar_layout.main.origin.x + OUTPUT_PADDING;
        // Input bounds above status bar (max width 768px, centered)
        let max_input_width = 768.0_f32;
        let available_input_width = sidebar_layout.main.size.width - INPUT_PADDING * 2.0;
        let input_width = available_input_width.min(max_input_width);
        let input_x = sidebar_layout.main.origin.x + (sidebar_layout.main.size.width - input_width) / 2.0;
        // Set max width for text wrapping, then calculate dynamic height
        state.input.set_max_width(input_width);
        let input_height = state.input.current_height().max(40.0);
        let input_bounds = Bounds::new(
            input_x,
            logical_height - input_height - INPUT_PADDING - STATUS_BAR_HEIGHT,
            input_width,
            input_height,
        );
        let permission_open = state.permissions.permission_dialog
            .as_ref()
            .map(|dialog| dialog.is_open())
            .unwrap_or(false);
        let permission_bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(size) => {
                state.config.width = size.width.max(1);
                state.config.height = size.height.max(1);
                state.surface.configure(&state.device, &state.config);
                state.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                state.modifiers = modifiers.state();
            }
            WindowEvent::RedrawRequested => {
                self.render();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let x = position.x as f32 / scale_factor;
                let y = position.y as f32 / scale_factor;
                state.mouse_pos = (x, y);
                if permission_open {
                    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
                        let input_event = InputEvent::MouseMove { x, y };
                        let _ = dialog.event(&input_event, permission_bounds, &mut state.event_context);
                    }
                    state.window.request_redraw();
                    return;
                }
                if state.command_palette.is_open() {
                    return;
                }
                if matches!(state.modal_state, ModalState::SessionList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SessionList { selected } => *selected,
                        _ => 0,
                    };
                    if state.session.session_cards.len() != state.session.session_index.len() {
                        state.session.refresh_session_cards(state.chat.is_thinking);
                    }
                    let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                        0.0
                    } else {
                        state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
                    };
                    let layout = session_list_layout(
                        logical_width,
                        logical_height,
                        state.session.session_cards.len(),
                        selected,
                        checkpoint_height,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.session.session_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if let Some(bounds) = layout.checkpoint_bounds {
                        if matches!(
                            state.session.checkpoint_restore
                                .event(&input_event, bounds, &mut state.event_context),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::AgentList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::AgentList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len() {
                        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = agent_modal_content_top(modal_y, state);
                    let layout = agent_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.agent_cards.len(),
                        selected,
                        content_top,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.agent_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::SkillList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SkillList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len() {
                        state.catalogs.refresh_skill_cards();
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = skill_modal_content_top(modal_y, state);
                    let layout = skill_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.skill_cards.len(),
                        selected,
                        content_top,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.skill_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected,
                    );
                    let input_event = InputEvent::MouseMove { x, y };
                    let mut handled = false;
                    if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                        if matches!(
                            inspector.event(
                                &input_event,
                                layout.inspector_bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }

                // Track hover state for left sidebar button
                if state.left_sidebar_open {
                    if let Some(left_bounds) = sidebar_layout.left {
                        let btn_bounds = new_session_button_bounds(left_bounds);
                        let was_hovered = state.new_session_button_hovered;
                        state.new_session_button_hovered = btn_bounds.contains(Point::new(x, y));
                        if was_hovered != state.new_session_button_hovered {
                            // Change cursor to pointer when hovering button
                            let cursor = if state.new_session_button_hovered {
                                CursorIcon::Pointer
                            } else {
                                CursorIcon::Default
                            };
                            state.window.set_cursor(cursor);
                            state.window.request_redraw();
                        }
                    }
                } else if state.new_session_button_hovered {
                    // Reset cursor when sidebar closes
                    state.new_session_button_hovered = false;
                    state.window.set_cursor(CursorIcon::Default);
                }

                let input_event = InputEvent::MouseMove { x, y };
                let chat_layout = state.build_chat_layout(&sidebar_layout, logical_height);
                if state.chat.chat_context_menu.is_open() {
                    if matches!(
                        state.chat.chat_context_menu.event(
                            &input_event,
                            Bounds::new(0.0, 0.0, logical_width, logical_height),
                            &mut state.event_context,
                        ),
                        EventResult::Handled
                    ) {
                        state.window.request_redraw();
                        return;
                    }
                }
                if state.chat.chat_selection_dragging {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if let Some(selection) = &mut state.chat.chat_selection {
                            if selection.focus.message_index != point.message_index
                                || selection.focus.offset != point.offset
                            {
                                selection.focus = point;
                                state.window.request_redraw();
                            }
                        }
                    }
                }
                // Handle events for inline tools
                let mut tools_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                            if matches!(
                                tool.card
                                    .event(&input_event, block.card_bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail
                                        .event(&input_event, detail_bounds, &mut state.event_context),
                                    EventResult::Handled
                                ) {
                                    tools_handled = true;
                                }
                            }
                        }
                    }
                }
                if tools_handled {
                    state.window.request_redraw();
                }
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let (x, y) = state.mouse_pos;
                let modifiers = wgpui::Modifiers::default();
                let input_event = if button_state == ElementState::Pressed {
                    InputEvent::MouseDown {
                        button: convert_mouse_button(button),
                        x,
                        y,
                        modifiers,
                    }
                } else {
                    InputEvent::MouseUp {
                        button: convert_mouse_button(button),
                        x,
                        y,
                    }
                };
                if permission_open {
                    if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
                        let _ =
                            dialog.event(&input_event, permission_bounds, &mut state.event_context);
                    }
                    state.window.request_redraw();
                    return;
                }
                if state.command_palette.is_open() {
                    let palette_bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
                    let _ = state
                        .command_palette
                        .event(&input_event, palette_bounds, &mut state.event_context);
                    state.window.request_redraw();
                    return;
                }
                if matches!(state.modal_state, ModalState::SessionList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SessionList { selected } => *selected,
                        _ => 0,
                    };
                    if state.session.session_cards.len() != state.session.session_index.len() {
                        state.session.refresh_session_cards(state.chat.is_thinking);
                    }
                    let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                        0.0
                    } else {
                        state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
                    };
                    let layout = session_list_layout(
                        logical_width,
                        logical_height,
                        state.session.session_cards.len(),
                        selected,
                        checkpoint_height,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.session.session_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if let Some(bounds) = layout.checkpoint_bounds {
                        if matches!(
                            state.session.checkpoint_restore
                                .event(&input_event, bounds, &mut state.event_context),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::AgentList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::AgentList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len() {
                        state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = agent_modal_content_top(modal_y, state);
                    let layout = agent_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.agent_cards.len(),
                        selected,
                        content_top,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.agent_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(state.modal_state, ModalState::SkillList { .. }) {
                    let selected = match &state.modal_state {
                        ModalState::SkillList { selected } => *selected,
                        _ => 0,
                    };
                    if state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len() {
                        state.catalogs.refresh_skill_cards();
                    }
                    let modal_y = modal_y_in_content(logical_height, SESSION_MODAL_HEIGHT);
                    let content_top = skill_modal_content_top(modal_y, state);
                    let layout = skill_list_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.skill_cards.len(),
                        selected,
                        content_top,
                    );
                    let mut handled = false;
                    for (index, bounds) in layout.card_bounds {
                        if let Some(card) = state.catalogs.skill_cards.get_mut(index) {
                            if matches!(
                                card.event(&input_event, bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                handled = true;
                            }
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected_index = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected_index,
                    );
                    let mut handled = false;
                    if button_state == ElementState::Released {
                        if layout.list_bounds.contains(Point::new(x, y)) {
                            for (index, bounds) in &layout.row_bounds {
                                if bounds.contains(Point::new(x, y)) {
                                    state.modal_state = ModalState::Hooks {
                                        view: HookModalView::Events,
                                        selected: *index,
                                    };
                                    state.sync_hook_inspector(*index);
                                    handled = true;
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                        if matches!(
                            inspector.event(
                                &input_event,
                                layout.inspector_bounds,
                                &mut state.event_context
                            ),
                            EventResult::Handled
                        ) {
                            handled = true;
                        }
                    }
                    if handled {
                        state.window.request_redraw();
                    }
                    return;
                }

                // Handle click on left sidebar "New Session" button
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                    && state.left_sidebar_open
                {
                    if let Some(left_bounds) = sidebar_layout.left {
                        let btn_bounds = new_session_button_bounds(left_bounds);
                        if btn_bounds.contains(Point::new(x, y)) {
                            state.start_new_session();
                            state.input.focus();
                            state.window.request_redraw();
                            return;
                        }
                    }
                }

                let chat_layout = state.build_chat_layout(
                    &sidebar_layout,
                    logical_height,
                );
                if state.chat.chat_context_menu.is_open() {
                    if matches!(
                        state.chat.chat_context_menu.event(
                            &input_event,
                            Bounds::new(0.0, 0.0, logical_width, logical_height),
                            &mut state.event_context,
                        ),
                        EventResult::Handled
                    ) {
                        if let Some(action) = state.chat.chat_context_menu.take_selected() {
                            state.handle_chat_menu_action(&action, &chat_layout);
                            state.chat.chat_context_menu_target = None;
                        }
                        state.window.request_redraw();
                        return;
                    }
                }
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if state.modifiers.shift_key() {
                            if let Some(selection) = &mut state.chat.chat_selection {
                                selection.focus = point;
                            } else {
                                state.chat.chat_selection = Some(ChatSelection {
                                    anchor: point,
                                    focus: point,
                                });
                            }
                        } else {
                            state.chat.chat_selection = Some(ChatSelection {
                                anchor: point,
                                focus: point,
                            });
                        }
                        state.chat.chat_selection_dragging = true;
                        state.window.request_redraw();
                    } else {
                        state.chat.chat_selection = None;
                    }
                }
                if button_state == ElementState::Released
                    && matches!(button, winit::event::MouseButton::Left)
                {
                    state.chat.chat_selection_dragging = false;
                }
                if button_state == ElementState::Pressed
                    && matches!(button, winit::event::MouseButton::Right)
                {
                    if let Some(point) = state.chat_selection_point_at(&chat_layout, x, y) {
                        if !state.chat_selection_contains(point) {
                            state.chat.chat_selection = Some(ChatSelection {
                                anchor: point,
                                focus: point,
                            });
                        }
                        state.chat.chat_selection_dragging = false;
                        let copy_enabled = state.chat.chat_selection
                            .as_ref()
                            .is_some_and(|sel| !sel.is_empty())
                            || chat_layout.message_layouts.get(point.message_index).is_some();
                        state.open_chat_context_menu(
                            Point::new(x, y),
                            Some(point.message_index),
                            copy_enabled,
                        );
                        state.window.request_redraw();
                        return;
                    }
                }
                // Handle mouse events for inline tools
                let mut tools_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                            if matches!(
                                tool.card
                                    .event(&input_event, block.card_bounds, &mut state.event_context),
                                EventResult::Handled
                            ) {
                                tools_handled = true;
                            }
                            if tool.sync_expanded_from_card() {
                                tools_handled = true;
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                if matches!(
                                    tool.detail
                                        .event(&input_event, detail_bounds, &mut state.event_context),
                                    EventResult::Handled
                                ) {
                                    tools_handled = true;
                                }
                            }
                        }
                    }
                }
                if tools_handled {
                    state.window.request_redraw();
                }
                if button_state == ElementState::Released
                    && !state.session.session_info.permission_mode.is_empty()
                {
                    let status_y = logical_height - STATUS_BAR_HEIGHT - 2.0;
                    let mode_text = format!("[{}]", state.session.session_info.permission_mode);
                    let mode_width = mode_text.len() as f32 * 6.6;
                    let mode_bounds = Bounds::new(
                        content_x,
                        status_y - 4.0,
                        mode_width,
                        STATUS_BAR_HEIGHT + 8.0,
                    );
                    if mode_bounds.contains(Point::new(x, y)) {
                        state
                            .permissions
                            .cycle_coder_mode(&mut state.session.session_info);
                        state.window.request_redraw();
                        return;
                    }
                }
                state
                    .input
                    .event(&input_event, input_bounds, &mut state.event_context);
                state.window.request_redraw();
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if permission_open {
                    return;
                }
                if state.command_palette.is_open() {
                    return;
                }
                let dy = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => y,
                    winit::event::MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 20.0,
                };
                // Kitchen sink scroll handling
                if state.show_kitchen_sink {
                    state.kitchen_sink_scroll = (state.kitchen_sink_scroll - dy * 40.0).max(0.0);
                    state.window.request_redraw();
                    return;
                }
                if matches!(
                    state.modal_state,
                    ModalState::Hooks {
                        view: HookModalView::Events,
                        ..
                    }
                ) {
                    let selected = match &state.modal_state {
                        ModalState::Hooks { selected, .. } => *selected,
                        _ => 0,
                    };
                    let layout = hook_event_layout(
                        logical_width,
                        logical_height,
                        state.catalogs.hook_event_log.len(),
                        selected,
                    );
                    let mouse_point = Point::new(state.mouse_pos.0, state.mouse_pos.1);
                    if layout.inspector_bounds.contains(mouse_point) {
                        let input_event = InputEvent::Scroll { dx: 0.0, dy: dy * 40.0 };
                        if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                            if matches!(
                                inspector.event(
                                    &input_event,
                                    layout.inspector_bounds,
                                    &mut state.event_context
                                ),
                                EventResult::Handled
                            ) {
                                state.window.request_redraw();
                                return;
                            }
                        }
                    } else if layout.list_bounds.contains(mouse_point) {
                        let mut next_selected = selected;
                        if dy > 0.0 {
                            next_selected = next_selected.saturating_add(1);
                        } else if dy < 0.0 {
                            next_selected = next_selected.saturating_sub(1);
                        }
                        if !state.catalogs.hook_event_log.is_empty() {
                            next_selected = next_selected.min(state.catalogs.hook_event_log.len() - 1);
                        } else {
                            next_selected = 0;
                        }
                        if next_selected != selected {
                            state.modal_state = ModalState::Hooks {
                                view: HookModalView::Events,
                                selected: next_selected,
                            };
                            state.sync_hook_inspector(next_selected);
                            state.window.request_redraw();
                        }
                        return;
                    }
                }
                let chat_layout = state.build_chat_layout(&sidebar_layout, logical_height);
                // Handle scroll events for inline tools
                let mouse_point = Point::new(state.mouse_pos.0, state.mouse_pos.1);
                let scroll_input_event = InputEvent::Scroll { dx: 0.0, dy: dy * 40.0 };
                let mut scroll_handled = false;
                for inline_layout in &chat_layout.inline_tools {
                    for block in &inline_layout.blocks {
                        if block.card_bounds.contains(mouse_point) {
                            if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                                if matches!(
                                    tool.card
                                        .event(&scroll_input_event, block.card_bounds, &mut state.event_context),
                                    EventResult::Handled
                                ) {
                                    scroll_handled = true;
                                }
                                if let Some(detail_bounds) = block.detail_bounds {
                                    if matches!(
                                        tool.detail
                                            .event(&scroll_input_event, detail_bounds, &mut state.event_context),
                                        EventResult::Handled
                                    ) {
                                        scroll_handled = true;
                                    }
                                }
                            }
                        }
                    }
                }
                if scroll_handled {
                    state.window.request_redraw();
                    return;
                }
                // Scroll the message area (positive dy = scroll up, negative = scroll down)
                state.chat.scroll_offset = (state.chat.scroll_offset - dy * 40.0).max(0.0);
                state.window.request_redraw();
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state == ElementState::Pressed {
                    if permission_open {
                        return;
                    }

                    if state.command_palette.is_open() {
                        if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                            let modifiers = convert_modifiers(&state.modifiers);
                            let input_event = InputEvent::KeyDown { key, modifiers };
                            let palette_bounds =
                                Bounds::new(0.0, 0.0, logical_width, logical_height);
                            let _ = state.command_palette.event(
                                &input_event,
                                palette_bounds,
                                &mut state.event_context,
                            );
                            state.window.request_redraw();
                        }
                        return;
                    }

                    // Kitchen sink overlay - handle Escape to close
                    if state.show_kitchen_sink {
                        if let WinitKey::Named(WinitNamedKey::Escape) = &key_event.logical_key {
                            state.show_kitchen_sink = false;
                            state.window.request_redraw();
                            return;
                        }
                        // Consume all other keys while kitchen sink is open
                        return;
                    }

                    // Autopilot loop interrupt - Escape stops autonomous execution
                    if matches!(state.permissions.coder_mode, CoderMode::Autopilot) {
                        if let WinitKey::Named(WinitNamedKey::Escape) = &key_event.logical_key {
                            if state.chat.is_thinking {
                                // Signal interrupt to the autopilot loop
                                state.autopilot.autopilot_interrupt_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                                tracing::info!("Autopilot: interrupt requested by user");
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }

                    if let WinitKey::Named(WinitNamedKey::F1) = &key_event.logical_key {
                        if matches!(state.modal_state, ModalState::Help) {
                            state.modal_state = ModalState::None;
                        } else {
                            state.open_help();
                        }
                        state.window.request_redraw();
                        return;
                    }
                    if handle_modal_input(state, &key_event.logical_key) {
                        return;
                    }

                    let modifiers = convert_modifiers(&state.modifiers);

                    if state.chat.chat_context_menu.is_open() {
                        if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                            let input_event = InputEvent::KeyDown { key, modifiers };
                            if matches!(
                                state.chat.chat_context_menu.event(
                                    &input_event,
                                    Bounds::new(0.0, 0.0, logical_width, logical_height),
                                    &mut state.event_context,
                                ),
                                EventResult::Handled
                            ) {
                                if let Some(action) = state.chat.chat_context_menu.take_selected() {
                                    let chat_layout = state.build_chat_layout(
                                        &sidebar_layout,
                                        logical_height,
                                    );
                                    state.handle_chat_menu_action(&action, &chat_layout);
                                    state.chat.chat_context_menu_target = None;
                                }
                                state.window.request_redraw();
                                return;
                            }
                        }
                    }

                    if state.handle_chat_shortcut(
                        &key_event.logical_key,
                        modifiers,
                        &sidebar_layout,
                        logical_height,
                    ) {
                        state.window.request_redraw();
                        return;
                    }

                    if let Some(key) = convert_key_for_binding(&key_event.logical_key) {
                        if let Some(action) = match_action(&key, modifiers, &state.settings.keybindings) {
                            match action {
                                KeyAction::Interrupt => state.interrupt_query(),
                                KeyAction::OpenCommandPalette => {
                                    state.open_command_palette();
                                }
                                KeyAction::OpenSettings => state.open_config(),
                                KeyAction::ToggleLeftSidebar => state.toggle_left_sidebar(),
                                KeyAction::ToggleRightSidebar => state.toggle_right_sidebar(),
                                KeyAction::ToggleSidebars => state.toggle_sidebars(),
                            }
                            state.window.request_redraw();
                            return;
                        }
                    }

                    if let WinitKey::Named(WinitNamedKey::Tab) = &key_event.logical_key {
                        if state.modifiers.shift_key() {
                            state
                                .permissions
                                .cycle_coder_mode(&mut state.session.session_info);
                            state.window.request_redraw();
                            return;
                        }
                    }

                    // Check for Enter key to submit (but not Shift+Enter, which inserts newline)
                    if let WinitKey::Named(WinitNamedKey::Enter) = &key_event.logical_key {
                        if !state.modifiers.shift_key() {
                            let mut action = CommandAction::None;
                            let mut submit_prompt = None;

                            {
                                let prompt = state.input.get_value().to_string();
                                if prompt.trim().is_empty() {
                                    return;
                                }

                                if let Some(command) = parse_command(&prompt) {
                                    state.settings.command_history.push(prompt);
                                    state.input.set_value("");
                                    action = handle_command(state, command);
                                } else if !state.chat.is_thinking {
                                    state.settings.command_history.push(prompt.clone());
                                    state.input.set_value("");
                                    submit_prompt = Some(prompt);
                                } else {
                                    return;
                                }
                            }

                            if let CommandAction::SubmitPrompt(prompt) = action {
                                self.submit_prompt(prompt);
                            } else if let Some(prompt) = submit_prompt {
                                self.submit_prompt(prompt);
                            }

                            if let Some(s) = &self.state {
                                s.window.request_redraw();
                            }
                            return;
                        }
                        // Shift+Enter falls through to input handler below
                    }

                    if let Some(key) = convert_key_for_input(&key_event.logical_key) {
                        let input_event = InputEvent::KeyDown { key, modifiers };
                        state
                            .input
                            .event(&input_event, input_bounds, &mut state.event_context);
                        state.window.request_redraw();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Continuously request redraws when input is focused for cursor blinking
        if let Some(state) = &self.state {
            if state.input.is_focused() {
                state.window.request_redraw();
            }
        }
    }
}

impl AppState {
    fn build_command_palette_commands(&self) -> Vec<PaletteCommand> {
        let mut commands = Vec::new();
        let mut push_command = |id: &str,
                                label: &str,
                                description: &str,
                                category: &str,
                                keybinding: Option<String>| {
            let mut command = PaletteCommand::new(id, label)
                .description(description)
                .category(category);
            if let Some(keys) = keybinding {
                command = command.keybinding(keys);
            }
            commands.push(command);
        };

        let interrupt_keys = keybinding_labels(&self.settings.keybindings, KeyAction::Interrupt, "Ctrl+C");
        push_command(
            command_palette_ids::INTERRUPT_REQUEST,
            "Interrupt Request",
            "Stop the active response stream",
            "Request",
            Some(interrupt_keys),
        );

        push_command(
            command_palette_ids::HELP,
            "Open Help",
            "Show hotkeys and feature overview",
            "Navigation",
            Some("F1".to_string()),
        );

        let settings_keys = keybinding_labels(&self.settings.keybindings, KeyAction::OpenSettings, "Ctrl+,");
        push_command(
            command_palette_ids::SETTINGS,
            "Open Settings",
            "Configure Coder preferences",
            "Navigation",
            Some(settings_keys),
        );

        push_command(
            command_palette_ids::MODEL_PICKER,
            "Select Model",
            "Choose the model for this session",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::SESSION_LIST,
            "Open Session List",
            "Resume or fork previous sessions",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::AGENTS_LIST,
            "Open Agents",
            "Browse available agents",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::SKILLS_LIST,
            "Open Skills",
            "Browse available skills",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::HOOKS_OPEN,
            "Open Hooks",
            "Manage hook configuration",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::TOOLS_LIST,
            "Open Tool List",
            "Review available tools",
            "Navigation",
            None,
        );
        push_command(
            command_palette_ids::MCP_CONFIG,
            "Open MCP Servers",
            "Manage MCP configuration",
            "Navigation",
            None,
        );

        push_command(
            command_palette_ids::CLEAR_CONVERSATION,
            "Clear Conversation",
            "Reset the current chat history",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::UNDO_LAST,
            "Undo Last Exchange",
            "Remove the most recent exchange",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::COMPACT_CONTEXT,
            "Compact Context",
            "Summarize older context into a shorter prompt",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::SESSION_FORK,
            "Fork Session",
            "Create a new branch of this session",
            "Session",
            None,
        );
        push_command(
            command_palette_ids::SESSION_EXPORT,
            "Export Session",
            "Export conversation to markdown",
            "Session",
            None,
        );

        push_command(
            command_palette_ids::MODE_CYCLE,
            "Cycle Mode",
            "Rotate through modes (Bypass/Plan/Autopilot)",
            "Mode",
            Some("Shift+Tab".to_string()),
        );
        push_command(
            command_palette_ids::MODE_BYPASS,
            "Mode: Bypass Permissions",
            "Auto-approve all tool use",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::MODE_PLAN,
            "Mode: Plan",
            "Read-only mode, deny write operations",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::MODE_AUTOPILOT,
            "Mode: Autopilot",
            "Use DSPy/Adjutant for autonomous execution",
            "Mode",
            None,
        );
        push_command(
            command_palette_ids::PERMISSION_RULES,
            "Open Permission Rules",
            "Manage tool allow/deny rules",
            "Permissions",
            None,
        );

        let left_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleLeftSidebar, "Ctrl+[");
        let right_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleRightSidebar, "Ctrl+]");
        let toggle_keys =
            keybinding_labels(&self.settings.keybindings, KeyAction::ToggleSidebars, "Ctrl+\\");
        push_command(
            command_palette_ids::SIDEBAR_LEFT,
            "Open Left Sidebar",
            "Show the left sidebar",
            "Layout",
            Some(left_keys),
        );
        push_command(
            command_palette_ids::SIDEBAR_RIGHT,
            "Open Right Sidebar",
            "Show the right sidebar",
            "Layout",
            Some(right_keys),
        );
        push_command(
            command_palette_ids::SIDEBAR_TOGGLE,
            "Toggle Sidebars",
            "Show or hide both sidebars",
            "Layout",
            Some(toggle_keys),
        );

        push_command(
            command_palette_ids::MCP_RELOAD,
            "Reload MCP Config",
            "Reload MCP servers from project config",
            "MCP",
            None,
        );
        push_command(
            command_palette_ids::MCP_STATUS,
            "Refresh MCP Status",
            "Fetch MCP server status",
            "MCP",
            None,
        );

        push_command(
            command_palette_ids::AGENT_CLEAR,
            "Clear Active Agent",
            "Stop using the active agent",
            "Agents",
            None,
        );
        push_command(
            command_palette_ids::AGENT_RELOAD,
            "Reload Agents",
            "Reload agent definitions from disk",
            "Agents",
            None,
        );

        push_command(
            command_palette_ids::SKILLS_RELOAD,
            "Reload Skills",
            "Reload skills from disk",
            "Skills",
            None,
        );

        push_command(
            command_palette_ids::HOOKS_RELOAD,
            "Reload Hooks",
            "Reload hook scripts from disk",
            "Hooks",
            None,
        );

        push_command(
            command_palette_ids::BUG_REPORT,
            "Report a Bug",
            "Open the issue tracker",
            "Diagnostics",
            None,
        );

        push_command(
            command_palette_ids::KITCHEN_SINK,
            "Kitchen Sink",
            "Show all UI component variations",
            "Developer",
            None,
        );

        commands
    }

    fn open_command_palette(&mut self) {
        self.modal_state = ModalState::None;
        if self.chat.chat_context_menu.is_open() {
            self.chat.chat_context_menu.close();
            self.chat.chat_context_menu_target = None;
        }
        self.command_palette.set_commands(self.build_command_palette_commands());
        self.command_palette.open();
    }

    fn open_model_picker(&mut self) {
        let current_idx = ModelOption::all()
            .iter()
            .position(|m| *m == self.settings.selected_model)
            .unwrap_or(0);
        self.modal_state = ModalState::ModelPicker { selected: current_idx };
    }

    fn open_session_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (checkpoint_tx, checkpoint_rx) = mpsc::unbounded_channel();
        self.session.session_action_tx = Some(action_tx);
        self.session.session_action_rx = Some(action_rx);
        self.session.checkpoint_action_tx = Some(checkpoint_tx);
        self.session.checkpoint_action_rx = Some(checkpoint_rx);
        self.session.refresh_session_cards(self.chat.is_thinking);
        self.session.refresh_checkpoint_restore(&self.chat.messages);
        let selected = self.session.session_index
            .iter()
            .position(|entry| entry.id == self.session.session_info.session_id)
            .unwrap_or(0);
        self.modal_state = ModalState::SessionList { selected };
    }

    fn open_agent_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.agent_action_tx = Some(action_tx);
        self.catalogs.agent_action_rx = Some(action_rx);
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
        let selected = self.catalogs.active_agent
            .as_ref()
            .and_then(|name| {
                self.catalogs.agent_entries
                    .iter()
                    .position(|entry| entry.name == *name)
            })
            .unwrap_or(0);
        self.modal_state = ModalState::AgentList { selected };
    }

    fn open_skill_list(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.skill_action_tx = Some(action_tx);
        self.catalogs.skill_action_rx = Some(action_rx);
        self.catalogs.refresh_skill_cards();
        self.modal_state = ModalState::SkillList { selected: 0 };
    }

    fn open_tool_list(&mut self) {
        self.modal_state = ModalState::ToolList { selected: 0 };
    }

    fn open_permission_rules(&mut self) {
        self.modal_state = ModalState::PermissionRules;
    }

    fn open_config(&mut self) {
        self.modal_state = ModalState::Config {
            tab: SettingsTab::General,
            selected: 0,
            search: String::new(),
            input_mode: SettingsInputMode::Normal,
        };
    }

    fn persist_settings(&self) {
        save_settings(&self.settings.coder_settings);
    }

    fn apply_settings(&mut self) {
        normalize_settings(&mut self.settings.coder_settings);
        let current_value = self.input.get_value().to_string();
        let focused = self.input.is_focused();
        self.input = build_input(&self.settings.coder_settings);
        self.input.set_value(current_value);
        if focused {
            self.input.focus();
        }
        self.chat.markdown_renderer = build_markdown_renderer(&self.settings.coder_settings);
        self.chat.streaming_markdown.set_markdown_config(build_markdown_config(&self.settings.coder_settings));
    }

    fn update_selected_model(&mut self, model: ModelOption) {
        self.settings.selected_model = model;
        self.session.session_info.model = self.settings.selected_model.model_id().to_string();
        update_settings_model(&mut self.settings.coder_settings, self.settings.selected_model);
        self.persist_settings();
    }

    fn toggle_left_sidebar(&mut self) {
        self.left_sidebar_open = !self.left_sidebar_open;
    }

    fn toggle_right_sidebar(&mut self) {
        self.right_sidebar_open = !self.right_sidebar_open;
    }

    fn toggle_sidebars(&mut self) {
        let should_open = !(self.left_sidebar_open && self.right_sidebar_open);
        self.left_sidebar_open = should_open;
        self.right_sidebar_open = should_open;
    }

    fn apply_session_history_limit(&mut self) {
        let removed =
            apply_session_history_limit(&mut self.session.session_index, self.settings.coder_settings.session_history_limit);
        if !removed.is_empty() {
            let _ = save_session_index(&self.session.session_index);
            for removed_id in removed {
                let _ = fs::remove_dir_all(session_messages_dir(&removed_id));
            }
            self.session.refresh_session_cards(self.chat.is_thinking);
        }
    }

    fn open_mcp_config(&mut self) {
        self.modal_state = ModalState::McpConfig { selected: 0 };
    }

    fn open_help(&mut self) {
        self.modal_state = ModalState::Help;
    }

    fn open_hooks(&mut self) {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        self.catalogs.hook_inspector_action_tx = Some(action_tx);
        self.catalogs.hook_inspector_action_rx = Some(action_rx);
        self.modal_state = ModalState::Hooks {
            view: HookModalView::Config,
            selected: 0,
        };
    }

    fn reload_hooks(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = load_hook_scripts(&cwd);
        self.catalogs.hook_scripts = catalog.entries;
        self.catalogs.hook_project_path = catalog.project_path;
        self.catalogs.hook_user_path = catalog.user_path;
        self.catalogs.hook_load_error = catalog.error;
    }

    fn toggle_hook_setting(&mut self, setting: HookSetting) {
        match setting {
            HookSetting::ToolBlocker => {
                self.catalogs.hook_config.tool_blocker = !self.catalogs.hook_config.tool_blocker;
            }
            HookSetting::ToolLogger => {
                self.catalogs.hook_config.tool_logger = !self.catalogs.hook_config.tool_logger;
            }
            HookSetting::OutputTruncator => {
                self.catalogs.hook_config.output_truncator = !self.catalogs.hook_config.output_truncator;
            }
            HookSetting::ContextInjection => {
                self.catalogs.hook_config.context_injection = !self.catalogs.hook_config.context_injection;
            }
            HookSetting::TodoEnforcer => {
                self.catalogs.hook_config.todo_enforcer = !self.catalogs.hook_config.todo_enforcer;
            }
        }
        save_hook_config(&self.catalogs.hook_config);
    }

    fn clear_hook_log(&mut self) {
        self.catalogs.hook_event_log.clear();
        self.catalogs.hook_inspector = None;
        if let ModalState::Hooks { view, selected } = &mut self.modal_state {
            if *view == HookModalView::Events {
                *selected = 0;
            }
        }
    }

    fn reload_agents(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = load_agent_entries(&cwd);
        self.catalogs.agent_entries = catalog.entries;
        self.catalogs.agent_project_path = catalog.project_path;
        self.catalogs.agent_user_path = catalog.user_path;
        self.catalogs.agent_load_error = catalog.error;
        if let Some(active) = self.catalogs.active_agent.clone() {
            if !self.catalogs.agent_entries.iter().any(|entry| entry.name == active) {
                self.catalogs.active_agent = None;
                self.push_system_message(format!(
                    "Active agent {} no longer available.",
                    active
                ));
            }
        }
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
    }

    fn reload_skills(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let catalog = load_skill_entries(&cwd);
        self.catalogs.skill_entries = catalog.entries;
        self.catalogs.skill_project_path = catalog.project_path;
        self.catalogs.skill_user_path = catalog.user_path;
        self.catalogs.skill_load_error = catalog.error;
        self.catalogs.refresh_skill_cards();
    }

    fn reload_mcp_project_servers(&mut self) {
        let cwd = std::env::current_dir().unwrap_or_default();
        let (servers, error) = load_mcp_project_servers(&cwd);
        self.catalogs.mcp_project_servers = servers;
        self.catalogs.mcp_project_error = error;
        self.catalogs.mcp_project_path = Some(mcp_project_file(&cwd));
    }

    fn request_mcp_status(&mut self) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::FetchMcpStatus);
        } else {
            self.push_system_message("No active session for MCP status.".to_string());
        }
    }

    fn handle_session_card_action(&mut self, action: SessionAction, session_id: String) {
        match action {
            SessionAction::Select | SessionAction::Resume => {
                self.begin_session_resume(session_id);
                self.modal_state = ModalState::None;
            }
            SessionAction::Fork => {
                self.begin_session_fork_from(session_id);
                self.modal_state = ModalState::None;
            }
            SessionAction::Delete => {
                self.push_system_message("Session delete not implemented yet.".to_string());
            }
        }
    }

    fn handle_agent_card_action(&mut self, action: AgentCardAction, agent_id: String) {
        match action {
            AgentCardAction::Select => {
                self.set_active_agent_by_name(&agent_id);
                self.modal_state = ModalState::None;
            }
            AgentCardAction::ToggleActive => {
                if self.catalogs.active_agent.as_deref() == Some(agent_id.as_str()) {
                    self.clear_active_agent();
                } else {
                    self.set_active_agent_by_name(&agent_id);
                }
            }
        }
    }

    fn handle_skill_card_action(&mut self, action: SkillCardAction, skill_id: String) {
        match action {
            SkillCardAction::View => {
                if let Some(index) = self.catalogs.skill_entries
                    .iter()
                    .position(|entry| entry.info.id == skill_id)
                {
                    if matches!(self.modal_state, ModalState::SkillList { .. }) {
                        self.modal_state = ModalState::SkillList { selected: index };
                    }
                }
            }
            SkillCardAction::Install => {
                if let Some(entry) = self.catalogs.skill_entries
                    .iter()
                    .find(|entry| entry.info.id == skill_id)
                {
                    self.push_system_message(format!(
                        "Skill {} is already installed at {}.",
                        entry.info.name,
                        entry.path.display()
                    ));
                }
            }
        }
    }

    fn set_active_agent_by_name(&mut self, name: &str) {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            self.push_system_message("Agent name is required.".to_string());
            return;
        }
        if let Some(entry) = self.catalogs.agent_entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(trimmed))
        {
            self.set_active_agent(Some(entry.name.clone()));
        } else {
            self.push_system_message(format!("Unknown agent: {}.", trimmed));
        }
    }

    fn clear_active_agent(&mut self) {
        self.set_active_agent(None);
    }

    fn set_active_agent(&mut self, agent: Option<String>) {
        let next = agent.and_then(|name| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        if next == self.catalogs.active_agent {
            return;
        }
        self.catalogs.active_agent = next.clone();
        if let Some(name) = next {
            self.push_system_message(format!("Active agent set to {}.", name));
        } else {
            self.push_system_message("Active agent cleared.".to_string());
        }
        self.catalogs.refresh_agent_cards(self.chat.is_thinking);
    }

    fn agent_definitions_for_query(&self) -> HashMap<String, AgentDefinition> {
        let mut agents = HashMap::new();
        for entry in &self.catalogs.agent_entries {
            agents.insert(entry.name.clone(), entry.definition.clone());
        }
        agents
    }

    fn setting_sources_for_query(&self) -> Vec<SettingSource> {
        let mut sources = Vec::new();
        if self.catalogs.skill_entries
            .iter()
            .any(|entry| entry.source == SkillSource::Project)
        {
            sources.push(SettingSource::Project);
        }
        if self.catalogs.skill_entries
            .iter()
            .any(|entry| entry.source == SkillSource::User)
        {
            sources.push(SettingSource::User);
        }
        sources
    }

    fn push_hook_log(&mut self, entry: HookLogEntry) {
        self.catalogs.hook_event_log.insert(0, entry);
        if self.catalogs.hook_event_log.len() > HOOK_LOG_LIMIT {
            self.catalogs.hook_event_log.truncate(HOOK_LOG_LIMIT);
        }
        if let ModalState::Hooks {
            view: HookModalView::Events,
            selected,
        } = &mut self.modal_state
        {
            *selected = 0;
            self.sync_hook_inspector(0);
        }
    }

    fn sync_hook_inspector(&mut self, selected: usize) {
        let Some(entry) = self.catalogs.hook_event_log.get(selected) else {
            self.catalogs.hook_inspector = None;
            return;
        };

        let event = hook_log_event_data(entry);
        let view = self.catalogs.hook_inspector_view;
        let mut inspector = EventInspector::new(event).view(view);
        if let Some(tx) = self.catalogs.hook_inspector_action_tx.clone() {
            inspector = inspector.on_view_change(move |view| {
                let _ = tx.send(view);
            });
        }
        self.catalogs.hook_inspector = Some(inspector);
    }

    fn handle_checkpoint_restore(&mut self, index: usize) {
        if let Some(entry) = self.session.checkpoint_entries.get(index) {
            self.request_rewind_files(entry.user_message_id.clone());
        }
    }

    fn begin_session_fork_from(&mut self, session_id: String) {
        let session_id = session_id.trim().to_string();
        if session_id.is_empty() {
            self.push_system_message("Session id is required to fork.".to_string());
            return;
        }
        self.session.pending_resume_session = Some(session_id.clone());
        self.session.pending_fork_session = true;
        self.session.session_info.session_id = session_id.clone();
        if let Some(entry) = self.session.session_index.iter().find(|entry| entry.id == session_id) {
            self.session.session_info.model = entry.model.clone();
        }
        match self
            .session
            .restore_session(&session_id, &mut self.chat, &mut self.tools)
        {
            Ok(()) => self.push_system_message(format!(
                "Loaded cached history for session {}.",
                session_id
            )),
            Err(_) => {
                self.chat.messages.clear();
                self.push_system_message(format!(
                    "No local history for session {} yet.",
                    session_id
                ));
            }
        }
        self.push_system_message(format!(
            "Next message will fork session {}.",
            session_id
        ));
        self.session.refresh_session_cards(self.chat.is_thinking);
    }

    fn attach_user_message_id(&mut self, uuid: String) {
        if let Some(message) = self.chat.messages
            .iter_mut()
            .rev()
            .find(|msg| matches!(msg.role, MessageRole::User) && msg.uuid.is_none())
        {
            message.uuid = Some(uuid);
            self.session.refresh_checkpoint_restore(&self.chat.messages);
        }
    }

    fn request_rewind_files(&mut self, user_message_id: String) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::RewindFiles { user_message_id: user_message_id.clone() });
            self.push_system_message(format!(
                "Requested checkpoint restore for message {}.",
                truncate_preview(&user_message_id, 12)
            ));
        } else {
            self.push_system_message("No active request to rewind.".to_string());
        }
    }

    fn clear_conversation(&mut self) {
        if self.chat.is_thinking {
            self.push_system_message(
                "Cannot clear while a response is in progress.".to_string(),
            );
            return;
        }
        self.chat.messages.clear();
        self.chat.streaming_markdown.reset();
        self.chat.scroll_offset = 0.0;
        self.tools.current_tool_name = None;
        self.tools.current_tool_input.clear();
        self.tools.current_tool_use_id = None;
        self.tools.tool_history.clear();
        self.session.session_info.session_id.clear();
        self.session.session_info.tool_count = 0;
        self.session.session_info.tools.clear();
        self.session.pending_resume_session = None;
        self.session.pending_fork_session = false;
        self.session.checkpoint_entries.clear();
        self.session.checkpoint_restore = CheckpointRestore::new();
        self.session.refresh_session_cards(self.chat.is_thinking);
    }

    fn start_new_session(&mut self) {
        if self.chat.is_thinking {
            self.push_system_message("Cannot start new session while processing.".to_string());
            return;
        }
        self.chat.messages.clear();
        self.chat.streaming_markdown.reset();
        self.chat.scroll_offset = 0.0;
        self.tools.current_tool_name = None;
        self.tools.current_tool_input.clear();
        self.tools.current_tool_use_id = None;
        self.tools.tool_history.clear();
        self.session.session_usage = SessionUsageStats::default();
        self.session.session_info.session_id.clear();
        self.session.session_info.tool_count = 0;
        self.session.session_info.tools.clear();
        self.session.pending_resume_session = None;
        self.session.pending_fork_session = false;
        self.session.checkpoint_entries.clear();
        self.session.checkpoint_restore = CheckpointRestore::new();
        self.session.refresh_session_cards(self.chat.is_thinking);
        self.push_system_message("Started new session.".to_string());
    }

    fn undo_last_exchange(&mut self) {
        if self.chat.is_thinking {
            self.push_system_message(
                "Cannot undo while a response is in progress.".to_string(),
            );
            return;
        }

        let mut removed = 0;
        while matches!(self.chat.messages.last(), Some(ChatMessage { role: MessageRole::Assistant, .. })) {
            self.chat.messages.pop();
            removed += 1;
        }
        if matches!(self.chat.messages.last(), Some(ChatMessage { role: MessageRole::User, .. })) {
            self.chat.messages.pop();
            removed += 1;
        }

        if removed == 0 {
            self.push_system_message("Nothing to undo.".to_string());
        } else {
            self.session.refresh_checkpoint_restore(&self.chat.messages);
        }
    }

    fn interrupt_query(&mut self) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::Interrupt);
        } else {
            self.push_system_message("No active request to interrupt.".to_string());
        }
    }

    #[allow(dead_code)]
    fn abort_query(&mut self) {
        if let Some(tx) = &self.chat.query_control_tx {
            let _ = tx.send(QueryControl::Abort);
        } else {
            self.push_system_message("No active request to cancel.".to_string());
        }
    }

    fn begin_session_resume(&mut self, session_id: String) {
        let session_id = session_id.trim().to_string();
        if session_id.is_empty() {
            self.push_system_message("Session id is required to resume.".to_string());
            return;
        }
        self.session.pending_resume_session = Some(session_id.clone());
        self.session.pending_fork_session = false;
        self.session.session_info.session_id = session_id.clone();
        if let Some(entry) = self.session.session_index.iter().find(|entry| entry.id == session_id) {
            self.session.session_info.model = entry.model.clone();
        }
        match self
            .session
            .restore_session(&session_id, &mut self.chat, &mut self.tools)
        {
            Ok(()) => self.push_system_message(format!(
                "Loaded cached history for session {}.",
                session_id
            )),
            Err(_) => {
                self.chat.messages.clear();
                self.push_system_message(format!(
                    "No local history for session {} yet.",
                    session_id
                ));
            }
        }
        self.session.refresh_session_cards(self.chat.is_thinking);
    }

    fn begin_session_fork(&mut self) {
        if self.session.session_info.session_id.trim().is_empty() {
            self.push_system_message("No active session to fork.".to_string());
            return;
        }
        self.session.pending_resume_session = Some(self.session.session_info.session_id.clone());
        self.session.pending_fork_session = true;
        self.push_system_message("Next message will fork the current session.".to_string());
    }

    fn export_session(&mut self) {
        if self.chat.messages.is_empty() {
            self.push_system_message("No messages to export yet.".to_string());
            return;
        }
        match export_session_markdown(self) {
            Ok(path) => self.push_system_message(format!(
                "Exported session to {}.",
                path.display()
            )),
            Err(err) => self.push_system_message(format!(
                "Failed to export session: {}.",
                err
            )),
        }
    }

    fn push_system_message(&mut self, message: String) {
        self.chat.messages.push(ChatMessage {
            role: MessageRole::Assistant,
            content: message,
            document: None,
            uuid: None,
            metadata: None,
        });
    }
}

impl AppState {
    fn build_chat_layout(
        &mut self,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
    ) -> ChatLayout {
        let viewport_top = OUTPUT_PADDING;
        // Calculate input width for wrapping
        let max_input_width = 768.0_f32;
        let available_input_width = sidebar_layout.main.size.width - INPUT_PADDING * 2.0;
        let input_width = available_input_width.min(max_input_width);
        // Set max width for wrapping, then calculate dynamic height
        self.input.set_max_width(input_width);
        let input_height = self.input.current_height().max(40.0);
        let viewport_bottom =
            logical_height - input_height - INPUT_PADDING * 2.0 - STATUS_BAR_HEIGHT - 16.0;
        let viewport_height = (viewport_bottom - viewport_top).max(0.0);

        // Apply max width 768px and center content (matching input container)
        let max_content_width = 768.0_f32;
        let full_available_width = sidebar_layout.main.size.width - OUTPUT_PADDING * 2.0;
        let available_width = full_available_width.min(max_content_width);
        let content_x = sidebar_layout.main.origin.x + (sidebar_layout.main.size.width - available_width) / 2.0;

        let chat_font_size = self.settings.coder_settings.font_size;
        let chat_line_height = (chat_font_size * 1.4).round();
        let char_width = chat_font_size * 0.6;
        let max_chars = (available_width / char_width).max(1.0) as usize;

        let mut message_layouts = Vec::with_capacity(self.chat.messages.len());
        let mut inline_tools_layouts: Vec<InlineToolsLayout> = Vec::new();
        let mut dspy_stage_layouts: Vec<DspyStageLayout> = Vec::new();
        let mut total_content_height = 0.0_f32;

        // Group tools by message_index
        let mut tools_by_message: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for (tool_idx, tool) in self.tools.tool_history.iter().enumerate() {
            tools_by_message
                .entry(tool.message_index)
                .or_default()
                .push(tool_idx);
        }

        // Group DSPy stages by message_index
        let mut dspy_by_message: std::collections::HashMap<usize, Vec<usize>> =
            std::collections::HashMap::new();
        for (stage_idx, stage) in self.tools.dspy_stages.iter().enumerate() {
            dspy_by_message
                .entry(stage.message_index)
                .or_default()
                .push(stage_idx);
        }

        for index in 0..self.chat.messages.len() {
            let (role, content, document) = {
                let msg = &self.chat.messages[index];
                (msg.role, msg.content.clone(), msg.document.clone())
            };
            let layout = match role {
                MessageRole::User => self.layout_user_message(
                    index,
                    &content,
                    content_x,
                    chat_font_size,
                    chat_line_height,
                    max_chars,
                ),
                MessageRole::Assistant => self.layout_assistant_message(
                    index,
                    &content,
                    document.as_ref(),
                    content_x,
                    available_width,
                    chat_line_height,
                    max_chars,
                ),
            };
            total_content_height += layout.height;
            message_layouts.push(layout);

            // Add DSPy stage cards for this message (before tools)
            if let Some(stage_indices) = dspy_by_message.get(&index) {
                for &stage_idx in stage_indices {
                    let stage_height = self.measure_dspy_stage_height(stage_idx, available_width);
                    dspy_stage_layouts.push(DspyStageLayout {
                        message_index: index,
                        y_offset: total_content_height,
                        height: stage_height,
                        stage_index: stage_idx,
                    });
                    total_content_height += stage_height + TOOL_PANEL_GAP;
                }
            }

            // Add inline tools for this message
            if let Some(tool_indices) = tools_by_message.get(&index) {
                if !tool_indices.is_empty() {
                    let inline_layout = self.build_inline_tools_layout(
                        index,
                        tool_indices,
                        content_x,
                        available_width,
                        total_content_height,
                    );
                    total_content_height += inline_layout.height + TOOL_PANEL_GAP;
                    inline_tools_layouts.push(inline_layout);
                }
            }
        }

        let streaming_height = if !self.chat.streaming_markdown.source().is_empty() {
            let doc = self.chat.streaming_markdown.document();
            let size = self.chat.markdown_renderer
                .measure(doc, available_width, &mut self.text_system);
            size.height + chat_line_height
        } else if self.chat.is_thinking {
            chat_line_height
        } else {
            0.0
        };
        total_content_height += streaming_height;

        // Add inline tools for streaming/current message (last message index or beyond)
        let streaming_msg_index = self.chat.messages.len().saturating_sub(1);
        if let Some(tool_indices) = tools_by_message.get(&streaming_msg_index) {
            // Check if we already added tools for this message above
            let already_added = inline_tools_layouts
                .iter()
                .any(|l| l.message_index == streaming_msg_index);
            if !already_added && !tool_indices.is_empty() {
                let inline_layout = self.build_inline_tools_layout(
                    streaming_msg_index,
                    tool_indices,
                    content_x,
                    available_width,
                    total_content_height,
                );
                total_content_height += inline_layout.height + TOOL_PANEL_GAP;
                inline_tools_layouts.push(inline_layout);
            }
        }

        let max_scroll = (total_content_height - viewport_height).max(0.0);
        self.chat.scroll_offset = self.chat.scroll_offset.clamp(0.0, max_scroll);
        let was_near_bottom = self.chat.scroll_offset >= max_scroll - chat_line_height * 2.0;
        if self.settings.coder_settings.auto_scroll && self.tools.has_running() && was_near_bottom {
            self.chat.scroll_offset = max_scroll;
        }

        if let Some(selection) = self.chat.chat_selection {
            if selection.anchor.message_index >= message_layouts.len()
                || selection.focus.message_index >= message_layouts.len()
            {
                self.chat.chat_selection = None;
            }
        }

        // Apply scroll offset to message Y positions
        let scroll_adjust = viewport_top - self.chat.scroll_offset;
        let mut y = scroll_adjust;
        let mut inline_tools_idx = 0;
        let mut dspy_stages_idx = 0;
        for (msg_idx, layout) in message_layouts.iter_mut().enumerate() {
            for line in &mut layout.lines {
                line.y += y;
            }
            y += layout.height;

            // Adjust DSPy stage Y positions for this message
            while dspy_stages_idx < dspy_stage_layouts.len()
                && dspy_stage_layouts[dspy_stages_idx].message_index == msg_idx
            {
                let dsl = &mut dspy_stage_layouts[dspy_stages_idx];
                dsl.y_offset += scroll_adjust;
                y += dsl.height + TOOL_PANEL_GAP;
                dspy_stages_idx += 1;
            }

            // Adjust inline tools Y positions for this message
            if inline_tools_idx < inline_tools_layouts.len()
                && inline_tools_layouts[inline_tools_idx].message_index == msg_idx
            {
                let itl = &mut inline_tools_layouts[inline_tools_idx];
                itl.y_offset += scroll_adjust;
                for block in &mut itl.blocks {
                    block.card_bounds.origin.y += scroll_adjust;
                    if let Some(ref mut db) = block.detail_bounds {
                        db.origin.y += scroll_adjust;
                    }
                }
                y += itl.height + TOOL_PANEL_GAP;
                inline_tools_idx += 1;
            }
        }

        // Handle any remaining DSPy stages (for streaming message)
        while dspy_stages_idx < dspy_stage_layouts.len() {
            let dsl = &mut dspy_stage_layouts[dspy_stages_idx];
            dsl.y_offset += scroll_adjust;
            dspy_stages_idx += 1;
        }

        // Handle any remaining inline tools (for streaming message)
        while inline_tools_idx < inline_tools_layouts.len() {
            let itl = &mut inline_tools_layouts[inline_tools_idx];
            itl.y_offset += scroll_adjust;
            for block in &mut itl.blocks {
                block.card_bounds.origin.y += scroll_adjust;
                if let Some(ref mut db) = block.detail_bounds {
                    db.origin.y += scroll_adjust;
                }
            }
            inline_tools_idx += 1;
        }

        ChatLayout {
            viewport_top,
            viewport_bottom,
            content_x,
            available_width,
            chat_font_size,
            chat_line_height,
            message_layouts,
            streaming_height,
            inline_tools: inline_tools_layouts,
            dspy_stages: dspy_stage_layouts,
        }
    }

    fn build_inline_tools_layout(
        &self,
        message_index: usize,
        tool_indices: &[usize],
        content_x: f32,
        available_width: f32,
        y_offset: f32,
    ) -> InlineToolsLayout {
        let panel_x = content_x;
        let panel_width = available_width;

        let mut blocks = Vec::new();
        let mut block_y = y_offset + TOOL_PANEL_GAP;
        let mut total_height = TOOL_PANEL_GAP;

        for (i, &tool_idx) in tool_indices.iter().enumerate() {
            let tool = &self.tools.tool_history[tool_idx];
            let card_height = tool.card.size_hint().1.unwrap_or(22.0);
            let card_bounds = Bounds::new(
                panel_x,
                block_y,
                panel_width,
                card_height,
            );
            block_y += card_height;
            total_height += card_height;

            let detail_height = tool.detail.height();
            let detail_bounds = if detail_height > 0.0 {
                block_y += TOOL_PANEL_GAP;
                total_height += TOOL_PANEL_GAP;
                let db = Bounds::new(
                    panel_x,
                    block_y,
                    panel_width,
                    detail_height,
                );
                block_y += detail_height;
                total_height += detail_height;
                Some(db)
            } else {
                None
            };

            blocks.push(ToolPanelBlock {
                index: tool_idx,
                card_bounds,
                detail_bounds,
            });

            if i + 1 < tool_indices.len() {
                block_y += TOOL_PANEL_GAP;
                total_height += TOOL_PANEL_GAP;
            }
        }

        InlineToolsLayout {
            message_index,
            y_offset,
            height: total_height,
            blocks,
        }
    }

    /// Measure the height needed for a DSPy stage card
    fn measure_dspy_stage_height(&self, stage_idx: usize, _available_width: f32) -> f32 {
        let stage_viz = &self.tools.dspy_stages[stage_idx];
        // Calculate height based on stage type
        match &stage_viz.stage {
            DspyStage::EnvironmentAssessment { .. } => 160.0,
            DspyStage::Planning { implementation_steps, .. } => {
                80.0 + (implementation_steps.len() as f32 * 20.0).min(200.0)
            }
            DspyStage::TodoList { tasks } => {
                60.0 + (tasks.len() as f32 * 24.0).min(240.0)
            }
            DspyStage::ExecutingTask { .. } => 60.0,
            DspyStage::TaskComplete { .. } => 40.0,
            DspyStage::Complete { .. } => 80.0,
        }
    }

    fn layout_user_message(
        &mut self,
        message_index: usize,
        content: &str,
        content_x: f32,
        chat_font_size: f32,
        chat_line_height: f32,
        max_chars: usize,
    ) -> MessageLayout {
        let content_with_prefix = format!("> {}", content);
        let wrapped_lines = wrap_text(&content_with_prefix, max_chars);
        let line_count = wrapped_lines.len();
        let mut builder = MessageLayoutBuilder::new(message_index);
        let mut y = chat_line_height * 0.5;
        for line in wrapped_lines {
            builder.push_line(line, content_x, y, chat_line_height, chat_font_size);
            y += chat_line_height;
        }
        let height = chat_line_height * 0.5 + line_count as f32 * chat_line_height
            + chat_line_height * 0.5;
        builder.build(height)
    }

    fn layout_assistant_message(
        &mut self,
        message_index: usize,
        content: &str,
        document: Option<&MarkdownDocument>,
        content_x: f32,
        available_width: f32,
        chat_line_height: f32,
        max_chars: usize,
    ) -> MessageLayout {
        if let Some(doc) = document {
            let config = build_markdown_config(&self.settings.coder_settings);
            let mut builder = MessageLayoutBuilder::new(message_index);
            let height = layout_markdown_document(
                doc,
                Point::new(content_x, 0.0),
                available_width,
                &mut self.text_system,
                &config,
                &mut builder,
            );
            builder.build(height + chat_line_height)
        } else {
            let wrapped_lines = wrap_text(content, max_chars);
            let line_count = wrapped_lines.len();
            let mut builder = MessageLayoutBuilder::new(message_index);
            let mut y = 0.0;
            for line in wrapped_lines {
                builder.push_line(line, content_x, y, chat_line_height, self.settings.coder_settings.font_size);
                y += chat_line_height;
            }
            let height = line_count as f32 * chat_line_height;
            builder.build(height)
        }
    }

    fn chat_selection_point_at(
        &mut self,
        layout: &ChatLayout,
        x: f32,
        y: f32,
    ) -> Option<ChatSelectionPoint> {
        if y < layout.viewport_top || y > layout.viewport_bottom {
            return None;
        }
        let mut lines = layout
            .message_layouts
            .iter()
            .flat_map(|layout| layout.lines.iter());

        let first_line = lines.next()?;
        let mut closest = first_line;
        if y < first_line.y {
            return Some(ChatSelectionPoint {
                message_index: first_line.message_index,
                offset: first_line.display_range.start,
            });
        }

        if y >= first_line.y && y <= first_line.y + first_line.line_height {
            return Some(self.chat_point_for_line(first_line, x));
        }

        for line in lines {
            if y >= line.y && y <= line.y + line.line_height {
                return Some(self.chat_point_for_line(line, x));
            }
            closest = line;
        }

        if y > closest.y + closest.line_height {
            return Some(ChatSelectionPoint {
                message_index: closest.message_index,
                offset: closest.display_range.end,
            });
        }

        Some(self.chat_point_for_line(closest, x))
    }

    fn chat_point_for_line(&mut self, line: &ChatLineLayout, x: f32) -> ChatSelectionPoint {
        let char_width = self
            .text_system
            .measure_styled_mono("M", line.font_size, wgpui::text::FontStyle::default())
            .max(1.0);
        let char_count = line.text.chars().count();
        let rel_x = (x - line.x).max(0.0);
        let mut char_index = (rel_x / char_width).floor() as usize;
        if char_index > char_count {
            char_index = char_count;
        }
        let byte_offset = byte_offset_for_char_index(&line.text, char_index);
        ChatSelectionPoint {
            message_index: line.message_index,
            offset: line.display_range.start + byte_offset,
        }
    }

    fn chat_selection_contains(&self, point: ChatSelectionPoint) -> bool {
        let Some(selection) = self.chat.chat_selection else {
            return false;
        };
        let (start, end) = selection.normalized();
        selection_point_cmp(&point, &start).is_ge() && selection_point_cmp(&point, &end).is_le()
    }

    fn chat_selection_text(&self, layout: &ChatLayout) -> Option<String> {
        let selection = self.chat.chat_selection?;
        if selection.is_empty() {
            return None;
        }
        let (start, end) = selection.normalized();
        let mut out = String::new();
        for idx in start.message_index..=end.message_index {
            let Some(message) = layout.message_layouts.get(idx) else {
                continue;
            };
            let text = &message.display_text;
            let start_offset = if idx == start.message_index {
                start.offset.min(text.len())
            } else {
                0
            };
            let end_offset = if idx == end.message_index {
                end.offset.min(text.len())
            } else {
                text.len()
            };
            if start_offset <= end_offset {
                if let Some(slice) = text.get(start_offset..end_offset) {
                    out.push_str(slice);
                }
            }
            if idx != end.message_index {
                out.push('\n');
            }
        }
        if out.is_empty() {
            None
        } else {
            Some(out)
        }
    }

    fn select_all_chat(&mut self, layout: &ChatLayout) {
        if layout.message_layouts.is_empty() {
            return;
        }
        let last_idx = layout.message_layouts.len() - 1;
        let end_offset = layout.message_layouts[last_idx].display_text.len();
        self.chat.chat_selection = Some(ChatSelection {
            anchor: ChatSelectionPoint {
                message_index: 0,
                offset: 0,
            },
            focus: ChatSelectionPoint {
                message_index: last_idx,
                offset: end_offset,
            },
        });
    }

    fn open_chat_context_menu(
        &mut self,
        position: Point,
        target_message: Option<usize>,
        copy_enabled: bool,
    ) {
        // Use Ctrl on Linux/Windows, Cmd on macOS
        let mod_key = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
        let copy_item = MenuItem::new("copy", "Copy")
            .shortcut(format!("{}+C", mod_key))
            .disabled(!copy_enabled);
        let items = vec![
            copy_item,
            MenuItem::separator(),
            MenuItem::new("select_all", "Select All").shortcut(format!("{}+A", mod_key)),
        ];
        self.chat.chat_context_menu = ContextMenu::new().items(items);
        self.chat.chat_context_menu_target = target_message;
        self.chat.chat_context_menu.open(position);
    }

    fn handle_chat_menu_action(&mut self, action: &str, layout: &ChatLayout) {
        match action {
            "copy" => {
                if let Some(text) = self.chat_selection_text(layout) {
                    self.write_chat_clipboard(&text);
                } else if let Some(target) = self.chat.chat_context_menu_target {
                    if let Some(message) = layout.message_layouts.get(target) {
                        self.write_chat_clipboard(&message.display_text);
                    }
                }
            }
            "select_all" => {
                self.select_all_chat(layout);
            }
            _ => {}
        }
    }

    fn handle_chat_shortcut(
        &mut self,
        key: &WinitKey,
        modifiers: UiModifiers,
        sidebar_layout: &SidebarLayout,
        logical_height: f32,
    ) -> bool {
        if self.input.is_focused() {
            return false;
        }
        let ctrl_or_meta = modifiers.ctrl || modifiers.meta;
        if !ctrl_or_meta {
            return false;
        }
        match key {
            WinitKey::Character(c) if c.eq_ignore_ascii_case("c") => {
                if self.chat.chat_selection
                    .as_ref()
                    .is_some_and(|sel| !sel.is_empty())
                {
                    let chat_layout =
                        self.build_chat_layout(sidebar_layout, logical_height);
                    if let Some(text) = self.chat_selection_text(&chat_layout) {
                        self.write_chat_clipboard(&text);
                        return true;
                    }
                }
            }
            WinitKey::Character(c) if c.eq_ignore_ascii_case("a") => {
                let chat_layout =
                    self.build_chat_layout(sidebar_layout, logical_height);
                self.select_all_chat(&chat_layout);
                return true;
            }
            _ => {}
        }
        false
    }

    fn write_chat_clipboard(&mut self, text: &str) {
        // Always use system clipboard command (wl-copy on Wayland) for reliability
        let _ = copy_to_clipboard(text);
    }
}

impl CoderApp {
    fn submit_prompt(&mut self, prompt: String) {
        let Some(state) = &mut self.state else {
            return;
        };

        tracing::info!("Submitted prompt: {}", prompt);

        // Add user message to history
        state.chat.messages.push(ChatMessage {
            role: MessageRole::User,
            content: prompt.clone(),
            document: None,
            uuid: None,
            metadata: None,
        });

        if matches!(state.permissions.coder_mode, CoderMode::Autopilot) {
            crate::app::autopilot::submit_autopilot_prompt(&self.runtime_handle, state, prompt);
            return;
        }

        let cwd = std::env::current_dir().unwrap_or_default();
        let active_agent = state.catalogs.active_agent.clone();
        let expanded_prompt = match expand_prompt_text(&prompt, &cwd) {
            Ok(result) => result,
            Err(err) => {
                state.push_system_message(err);
                state.window.request_redraw();
                return;
            }
        };
        let expanded_prompt = if let Some(agent) = active_agent.as_ref() {
            format!("Use the {} subagent for this request.\n\n{}", agent, expanded_prompt)
        } else {
            expanded_prompt
        };

        // Create channel for receiving responses
        let (tx, rx) = mpsc::unbounded_channel();
        let (control_tx, mut control_rx) = mpsc::unbounded_channel();
        let (permission_tx, permission_rx) = mpsc::unbounded_channel();
        let (permission_action_tx, permission_action_rx) = mpsc::unbounded_channel();
        state.chat.response_rx = Some(rx);
        state.chat.query_control_tx = Some(control_tx);
        state.permissions.permission_requests_rx = Some(permission_rx);
        state.permissions.permission_action_tx = Some(permission_action_tx.clone());
        state.permissions.permission_action_rx = Some(permission_action_rx);
        state.permissions.permission_queue.clear();
        state.permissions.permission_pending = None;
        state.permissions.permission_dialog = None;
        state.chat.is_thinking = true;
        state.chat.streaming_markdown.reset();
        state.catalogs.refresh_agent_cards(state.chat.is_thinking);

        // Get window handle for triggering redraws from async task
        let window = state.window.clone();
        let model_id = state.settings.selected_model.model_id().to_string();
        let resume_session = state.session.pending_resume_session
            .take()
            .or_else(|| {
                if state.session.session_info.session_id.trim().is_empty() {
                    None
                } else {
                    Some(state.session.session_info.session_id.clone())
                }
            });
        let fork_session = state.session.pending_fork_session;
        state.session.pending_fork_session = false;
        let permission_mode = Some(state.permissions.coder_mode.to_sdk_permission_mode());
        let output_style = state.permissions.output_style.clone();
        let allowed_tools = state.permissions.tools_allowed.clone();
        let disallowed_tools = state.permissions.tools_disallowed.clone();
        let permission_allow_tools = state.permissions.permission_allow_tools.clone();
        let permission_deny_tools = state.permissions.permission_deny_tools.clone();
        let permission_allow_bash_patterns = state.permissions.permission_allow_bash_patterns.clone();
        let permission_deny_bash_patterns = state.permissions.permission_deny_bash_patterns.clone();
        let permission_default_allow = state.permissions.permission_default_allow;
        let mcp_servers = state.catalogs.merged_mcp_servers();
        let agent_definitions = state.agent_definitions_for_query();
        let setting_sources = state.setting_sources_for_query();
        let hook_config = state.catalogs.hook_config.clone();
        let hook_scripts = state.catalogs.hook_scripts.clone();
        let max_thinking_tokens = state.settings.coder_settings.max_thinking_tokens;
        let persist_session = state.settings.coder_settings.session_auto_save;

        // Spawn async query task
        let handle = self.runtime_handle.clone();
        handle.spawn(async move {
            let hook_cwd = cwd.clone();
            let mut options = QueryOptions::new()
                .cwd(cwd)
                .include_partial_messages(true) // Enable streaming deltas
                .model(&model_id);

            options.max_thinking_tokens = max_thinking_tokens;
            options.persist_session = persist_session;

            if let Some(mode) = permission_mode.clone() {
                options = options.permission_mode(mode);
            }
            if let Some(resume_id) = resume_session {
                options = options.resume(resume_id);
            }
            if fork_session {
                options = options.fork_session(true);
            }
            if !allowed_tools.is_empty() {
                options.allowed_tools = Some(allowed_tools);
            }
            if !disallowed_tools.is_empty() {
                options.disallowed_tools = Some(disallowed_tools);
            }
            if let Some(style) = output_style {
                options
                    .extra_args
                    .insert("output-style".to_string(), Some(style));
            }
            if !mcp_servers.is_empty() {
                options.mcp_servers = mcp_servers;
            }
            if !agent_definitions.is_empty() {
                options.agents = agent_definitions;
            }
            if !setting_sources.is_empty() {
                options.setting_sources = setting_sources;
            }
            if let Some(hooks) = build_hook_map(hook_cwd, hook_config, hook_scripts, tx.clone()) {
                options = options.hooks(hooks);
            }

            let permission_window = window.clone();
            let permissions = Arc::new(CallbackPermissionHandler::new(move |request: PermissionRequest| {
                let permission_tx = permission_tx.clone();
                let permission_window = permission_window.clone();
                let permission_mode = permission_mode.clone();
                let permission_allow_tools = permission_allow_tools.clone();
                let permission_deny_tools = permission_deny_tools.clone();
                let permission_allow_bash_patterns = permission_allow_bash_patterns.clone();
                let permission_deny_bash_patterns = permission_deny_bash_patterns.clone();
                async move {
                    let tool_name = request.tool_name.clone();

                    if tool_name == "Bash" {
                        if let Some(command) = extract_bash_command(&request.input) {
                            if permission_deny_bash_patterns
                                .iter()
                                .any(|pattern| pattern_matches(pattern, &command))
                            {
                                return Ok(PermissionResult::Deny {
                                    message: format!("Bash command denied by rule: {}", command),
                                    interrupt: None,
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            if permission_allow_bash_patterns
                                .iter()
                                .any(|pattern| pattern_matches(pattern, &command))
                            {
                                return Ok(PermissionResult::Allow {
                                    updated_input: request.input.clone(),
                                    updated_permissions: request.suggestions.clone(),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                        }
                    }

                    if permission_deny_tools.iter().any(|tool| tool == &tool_name) {
                        return Ok(PermissionResult::Deny {
                            message: format!("Tool {} is denied by rule.", tool_name),
                            interrupt: None,
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }
                    if permission_allow_tools.iter().any(|tool| tool == &tool_name) {
                        return Ok(PermissionResult::Allow {
                            updated_input: request.input.clone(),
                            updated_permissions: request.suggestions.clone(),
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }

                    if let Some(mode) = permission_mode.as_ref() {
                        match mode {
                            PermissionMode::BypassPermissions | PermissionMode::AcceptEdits => {
                                return Ok(PermissionResult::Allow {
                                    updated_input: request.input.clone(),
                                    updated_permissions: request.suggestions.clone(),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::DontAsk => {
                                return Ok(PermissionResult::Deny {
                                    message: format!("Permission denied for tool {}.", tool_name),
                                    interrupt: Some(true),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::Plan => {
                                if is_read_only_tool(&tool_name) {
                                    return Ok(PermissionResult::Allow {
                                        updated_input: request.input.clone(),
                                        updated_permissions: None,
                                        tool_use_id: Some(request.tool_use_id.clone()),
                                    });
                                }
                                return Ok(PermissionResult::Deny {
                                    message: format!("Plan mode denies tool {}.", tool_name),
                                    interrupt: Some(true),
                                    tool_use_id: Some(request.tool_use_id.clone()),
                                });
                            }
                            PermissionMode::Default => {}
                        }
                    }

                    if permission_default_allow {
                        return Ok(PermissionResult::Allow {
                            updated_input: request.input.clone(),
                            updated_permissions: request.suggestions.clone(),
                            tool_use_id: Some(request.tool_use_id.clone()),
                        });
                    }

                    let (respond_to, response_rx) = oneshot::channel();
                    let pending = PermissionPending { request, respond_to };
                    if permission_tx.send(pending).is_err() {
                        return Ok(PermissionResult::deny_and_interrupt(
                            "Permission prompt unavailable.",
                        ));
                    }
                    permission_window.request_redraw();
                    match response_rx.await {
                        Ok(result) => Ok(result),
                        Err(_) => Ok(PermissionResult::deny_and_interrupt(
                            "Permission prompt interrupted.",
                        )),
                    }
                }
            }));

            tracing::info!("Starting query...");

            match query_with_permissions(&expanded_prompt, options, permissions).await {
                Ok(mut stream) => {
                    tracing::info!("Query stream started");
                    let mut interrupt_requested = false;

                    loop {
                        if interrupt_requested {
                            if let Err(e) = stream.interrupt().await {
                                tracing::error!("Interrupt failed: {}", e);
                                let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                window.request_redraw();
                                break;
                            }
                            interrupt_requested = false;
                        }

                        tokio::select! {
                            Some(control) = control_rx.recv() => {
                                match control {
                                    QueryControl::Interrupt => {
                                        interrupt_requested = true;
                                    }
                                    QueryControl::RewindFiles { user_message_id } => {
                                        match stream.rewind_files(&user_message_id).await {
                                            Ok(()) => {
                                                let _ = tx.send(ResponseEvent::SystemMessage(
                                                    "Checkpoint restore requested.".to_string(),
                                                ));
                                            }
                                            Err(err) => {
                                                let _ = tx.send(ResponseEvent::SystemMessage(
                                                    format!("Checkpoint restore failed: {}", err),
                                                ));
                                            }
                                        }
                                        window.request_redraw();
                                    }
                                    QueryControl::Abort => {
                                        if let Err(e) = stream.abort().await {
                                            tracing::error!("Abort failed: {}", e);
                                            let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                        } else {
                                            let _ = tx.send(ResponseEvent::Error("Request aborted.".to_string()));
                                        }
                                        window.request_redraw();
                                        break;
                                    }
                                    QueryControl::FetchMcpStatus => {
                                        match stream.mcp_server_status().await {
                                            Ok(value) => {
                                                match parse_mcp_status(&value) {
                                                    Ok(servers) => {
                                                        let _ = tx.send(ResponseEvent::McpStatus {
                                                            servers,
                                                            error: None,
                                                        });
                                                    }
                                                    Err(err) => {
                                                        let _ = tx.send(ResponseEvent::McpStatus {
                                                            servers: Vec::new(),
                                                            error: Some(err),
                                                        });
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                let _ = tx.send(ResponseEvent::McpStatus {
                                                    servers: Vec::new(),
                                                    error: Some(err.to_string()),
                                                });
                                            }
                                        }
                                        window.request_redraw();
                                    }
                                }
                            }
                            msg = stream.next() => {
                                match msg {
                                    Some(Ok(SdkMessage::Assistant(m))) => {
                                        // Don't extract text here - we get it from STREAM_EVENT deltas
                                        // The ASSISTANT message contains the full text which would duplicate
                                        tracing::trace!("ASSISTANT: (skipping text extraction, using stream events)");
                                        tracing::trace!("  full message: {:?}", m.message);
                                    }
                                    Some(Ok(SdkMessage::StreamEvent(e))) => {
                                        tracing::trace!("STREAM_EVENT: {:?}", e.event);
                                        // Check for tool call start
                                        if let Some((tool_name, tool_id)) = extract_tool_call_start(&e.event) {
                                            tracing::debug!("  -> tool call start: {}", tool_name);
                                            let _ = tx.send(ResponseEvent::ToolCallStart {
                                                name: tool_name,
                                                tool_use_id: tool_id,
                                            });
                                            window.request_redraw();
                                        }
                                        // Check for tool input delta
                                        else if let Some(json) = extract_tool_input_delta(&e.event) {
                                            let _ = tx.send(ResponseEvent::ToolCallInput { json });
                                            window.request_redraw();
                                        }
                                        // Check for content_block_stop (tool call end)
                                        else if e.event.get("type").and_then(|t| t.as_str()) == Some("content_block_stop") {
                                            let _ = tx.send(ResponseEvent::ToolCallEnd);
                                            window.request_redraw();
                                        }
                                        // Extract streaming text delta
                                        else if let Some(text) = extract_stream_text(&e.event) {
                                            tracing::trace!("  -> stream text: {}", text);
                                            if tx.send(ResponseEvent::Chunk(text)).is_err() {
                                                break;
                                            }
                                            window.request_redraw();
                                        }
                                    }
                                    Some(Ok(SdkMessage::System(s))) => {
                                        tracing::debug!("SYSTEM: {:?}", s);
                                        // Extract init info
                                        if let claude_agent_sdk::SdkSystemMessage::Init(init) = s {
                                    let _ = tx.send(ResponseEvent::SystemInit {
                                        model: init.model.clone(),
                                        permission_mode: init.permission_mode.clone(),
                                        session_id: init.session_id.clone(),
                                        tool_count: init.tools.len(),
                                        tools: init.tools.clone(),
                                        output_style: init.output_style.clone(),
                                        slash_commands: init.slash_commands.clone(),
                                        mcp_servers: init.mcp_servers.clone(),
                                    });
                                    window.request_redraw();
                                }
                                    }
                                    Some(Ok(SdkMessage::User(u))) => {
                                        tracing::trace!("USER message received (tool result)");
                                        if let Some(uuid) = u.uuid.clone() {
                                            let _ = tx.send(ResponseEvent::UserMessageId { uuid });
                                            window.request_redraw();
                                        }
                                        // Extract tool results from USER messages
                                        if let Some(content) = u.message.get("content").and_then(|c| c.as_array()) {
                                            for item in content {
                                                if item.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                                    let tool_use_id = item
                                                        .get("tool_use_id")
                                                        .or_else(|| item.get("toolUseId"))
                                                        .or_else(|| item.get("toolUseID"))
                                                        .and_then(|v| v.as_str())
                                                        .map(|v| v.to_string())
                                                        .or_else(|| u.parent_tool_use_id.clone());
                                                    let is_error = item
                                                        .get("is_error")
                                                        .or_else(|| item.get("isError"))
                                                        .and_then(|e| e.as_bool())
                                                        .unwrap_or(false);
                                                    let content_value = item.get("content").cloned().unwrap_or(Value::Null);
                                                    let (result_content, exit_code, output_value) =
                                                        tool_result_output(&content_value, u.tool_use_result.as_ref());
                                                    let _ = tx.send(ResponseEvent::ToolResult {
                                                        content: result_content,
                                                        is_error,
                                                        tool_use_id,
                                                        exit_code,
                                                        output_value,
                                                    });
                                                    window.request_redraw();
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(SdkMessage::ToolProgress(tp))) => {
                                        tracing::trace!(
                                            "TOOL_PROGRESS: {} - {:.1}s",
                                            tp.tool_name,
                                            tp.elapsed_time_seconds
                                        );
                                        let _ = tx.send(ResponseEvent::ToolProgress {
                                            tool_use_id: tp.tool_use_id.clone(),
                                            tool_name: tp.tool_name.clone(),
                                            elapsed_secs: tp.elapsed_time_seconds,
                                        });
                                        window.request_redraw();
                                    }
                                    Some(Ok(SdkMessage::AuthStatus(a))) => {
                                        tracing::debug!("AUTH_STATUS: {:?}", a);
                                    }
                                    Some(Ok(SdkMessage::Result(_r))) => {
                                        tracing::debug!("RESULT received");
                                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                                        window.request_redraw();
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        tracing::error!("ERROR: {}", e);
                                        let _ = tx.send(ResponseEvent::Error(e.to_string()));
                                        window.request_redraw();
                                        break;
                                    }
                                    None => {
                                        let _ = tx.send(ResponseEvent::Complete { metadata: None });
                                        window.request_redraw();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    tracing::info!("Query stream ended");
                }
                Err(e) => {
                    tracing::error!("Query failed to start: {}", e);
                    let _ = tx.send(ResponseEvent::Error(e.to_string()));
                    window.request_redraw();
                }
            }
        });
    }

    fn poll_responses(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut events = Vec::new();
        if let Some(rx) = &mut state.chat.response_rx {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        } else {
            return;
        }

        let mut needs_redraw = false;

        for event in events {
            match event {
                ResponseEvent::Chunk(text) => {
                    state.chat.streaming_markdown.append(&text);
                    state.chat.streaming_markdown.tick();
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallStart { name, tool_use_id } => {
                    let message_index = state.chat.messages.len().saturating_sub(1);
                    state
                        .tools
                        .start_tool_call(name, tool_use_id, message_index);
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallInput { json } => {
                    state.tools.current_tool_input.push_str(&json);
                    needs_redraw = true;
                }
                ResponseEvent::ToolCallEnd => {
                    state.tools.finalize_tool_input();
                    needs_redraw = true;
                }
                ResponseEvent::ToolResult {
                    content,
                    is_error,
                    tool_use_id,
                    exit_code,
                    output_value,
                } => {
                    state.tools.apply_tool_result(
                        tool_use_id,
                        content,
                        is_error,
                        exit_code,
                        output_value,
                    );
                    needs_redraw = true;
                }
                ResponseEvent::ToolProgress {
                    tool_use_id,
                    tool_name,
                    elapsed_secs,
                } => {
                    let message_index = state.chat.messages.len().saturating_sub(1);
                    state.tools.update_tool_progress(
                        tool_use_id,
                        tool_name,
                        elapsed_secs,
                        message_index,
                    );
                    needs_redraw = true;
                }
                ResponseEvent::UserMessageId { uuid } => {
                    state.attach_user_message_id(uuid);
                    needs_redraw = true;
                }
                ResponseEvent::SystemMessage(message) => {
                    state.push_system_message(message);
                    needs_redraw = true;
                }
                ResponseEvent::Complete { metadata } => {
                    // Complete and move to messages
                    state.chat.streaming_markdown.complete();
                    let source = state.chat.streaming_markdown.source().to_string();
                    if !source.is_empty() {
                        // Aggregate into session usage
                        if let Some(ref meta) = metadata {
                            if let Some(input) = meta.input_tokens {
                                state.session.session_usage.input_tokens += input;
                            }
                            if let Some(output) = meta.output_tokens {
                                state.session.session_usage.output_tokens += output;
                            }
                            if let Some(ms) = meta.duration_ms {
                                state.session.session_usage.duration_ms += ms;
                            }
                            // Cost estimation: ~$3/M input, ~$15/M output for Claude Opus
                            let cost = (meta.input_tokens.unwrap_or(0) as f64 * 3.0 / 1_000_000.0)
                                     + (meta.output_tokens.unwrap_or(0) as f64 * 15.0 / 1_000_000.0);
                            state.session.session_usage.total_cost_usd += cost;
                        }
                        state.session.session_usage.num_turns += 1;

                        let doc = state.chat.streaming_markdown.document().clone();
                        state.chat.messages.push(ChatMessage {
                            role: MessageRole::Assistant,
                            content: source,
                            document: Some(doc),
                            uuid: None,
                            metadata,
                        });
                    }
                    state.chat.streaming_markdown.reset();
                    state.session.record_session(
                        &state.settings.coder_settings,
                        &state.chat.messages,
                        state.chat.is_thinking,
                    );
                    state.tools.cancel_running_tools();
                    state.chat.is_thinking = false;
                    state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    state.chat.response_rx = None;
                    state.chat.query_control_tx = None;
                    state.permissions.permission_requests_rx = None;
                    state.permissions.permission_action_tx = None;
                    state.permissions.permission_action_rx = None;
                    state.permissions.permission_dialog = None;
                    state.permissions.permission_pending = None;
                    state.permissions.permission_queue.clear();
                    state.tools.current_tool_name = None;
                    state.tools.current_tool_input.clear();
                    state.tools.current_tool_use_id = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::Error(e) => {
                    state.chat.messages.push(ChatMessage {
                        role: MessageRole::Assistant,
                        content: format!("Error: {}", e),
                        document: None,
                        uuid: None,
                        metadata: None,
                    });
                    state.chat.streaming_markdown.reset();
                    state.session.record_session(
                        &state.settings.coder_settings,
                        &state.chat.messages,
                        state.chat.is_thinking,
                    );
                    state.tools.cancel_running_tools();
                    state.chat.is_thinking = false;
                    state.catalogs.refresh_agent_cards(state.chat.is_thinking);
                    state.chat.response_rx = None;
                    state.chat.query_control_tx = None;
                    state.permissions.permission_requests_rx = None;
                    state.permissions.permission_action_tx = None;
                    state.permissions.permission_action_rx = None;
                    state.permissions.permission_dialog = None;
                    state.permissions.permission_pending = None;
                    state.permissions.permission_queue.clear();
                    state.tools.current_tool_name = None;
                    state.tools.current_tool_input.clear();
                    state.tools.current_tool_use_id = None;
                    needs_redraw = true;
                    break;
                }
                ResponseEvent::SystemInit {
                    model,
                    permission_mode,
                    session_id,
                    tool_count,
                    tools,
                    output_style,
                    slash_commands,
                    mcp_servers,
                } => {
                    state.session.session_info = SessionInfo {
                        model,
                        permission_mode,
                        session_id,
                        tool_count,
                        tools,
                        output_style,
                        slash_commands,
                    };
                    state.catalogs.update_mcp_status(mcp_servers, None);
                    if let Some(parsed_mode) = parse_coder_mode(&state.session.session_info.permission_mode)
                    {
                        state.permissions.coder_mode = parsed_mode;
                        state.permissions.permission_default_allow =
                            coder_mode_default_allow(parsed_mode, state.permissions.permission_default_allow);
                    }
                    state.session.refresh_session_cards(state.chat.is_thinking);
                    needs_redraw = true;
                }
                ResponseEvent::McpStatus { servers, error } => {
                    state.catalogs.update_mcp_status(servers, error);
                    needs_redraw = true;
                }
                ResponseEvent::HookLog(entry) => {
                    state.push_hook_log(entry);
                    needs_redraw = true;
                }
                ResponseEvent::DspyStage(stage) => {
                    let message_index = state.chat.messages.len().saturating_sub(1);
                    state.tools.push_dspy_stage(stage, message_index);
                    needs_redraw = true;
                }
            }
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_permissions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;

        let mut pending_requests = Vec::new();
        if let Some(rx) = &mut state.permissions.permission_requests_rx {
            while let Ok(pending) = rx.try_recv() {
                pending_requests.push(pending);
            }
        }
        for pending in pending_requests {
            state.permissions.enqueue_permission_prompt(pending);
            needs_redraw = true;
        }

        let mut pending_actions = Vec::new();
        if let Some(rx) = &mut state.permissions.permission_action_rx {
            while let Ok(action) = rx.try_recv() {
                pending_actions.push(action);
            }
        }
        for action in pending_actions {
            state.permissions.handle_permission_action(action);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_command_palette_actions(&mut self) {
        let actions = {
            let Some(state) = &mut self.state else {
                return;
            };
            let mut actions = Vec::new();
            if let Some(rx) = &mut state.command_palette_action_rx {
                while let Ok(action) = rx.try_recv() {
                    actions.push(action);
                }
            }
            actions
        };

        if actions.is_empty() {
            return;
        }

        for action in actions {
            if let Some(prompt) = self.execute_command_palette_action(&action) {
                self.submit_prompt(prompt);
            }
        }

        if let Some(state) = &mut self.state {
            state.window.request_redraw();
        }
    }

    fn poll_session_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;

        let mut session_events = Vec::new();
        if let Some(rx) = &mut state.session.session_action_rx {
            while let Ok(event) = rx.try_recv() {
                session_events.push(event);
            }
        }
        for event in session_events {
            state.handle_session_card_action(event.action, event.session_id);
            needs_redraw = true;
        }

        let mut checkpoint_events = Vec::new();
        if let Some(rx) = &mut state.session.checkpoint_action_rx {
            while let Ok(index) = rx.try_recv() {
                checkpoint_events.push(index);
            }
        }
        for index in checkpoint_events {
            state.handle_checkpoint_restore(index);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_agent_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut agent_events = Vec::new();
        if let Some(rx) = &mut state.catalogs.agent_action_rx {
            while let Ok(event) = rx.try_recv() {
                agent_events.push(event);
            }
        }
        for event in agent_events {
            state.handle_agent_card_action(event.action, event.agent_id);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_skill_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut skill_events = Vec::new();
        if let Some(rx) = &mut state.catalogs.skill_action_rx {
            while let Ok(event) = rx.try_recv() {
                skill_events.push(event);
            }
        }
        for event in skill_events {
            state.handle_skill_card_action(event.action, event.skill_id);
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_hook_inspector_actions(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let mut needs_redraw = false;
        let mut views = Vec::new();
        if let Some(rx) = &mut state.catalogs.hook_inspector_action_rx {
            while let Ok(view) = rx.try_recv() {
                views.push(view);
            }
        }
        for view in views {
            state.catalogs.hook_inspector_view = view;
            needs_redraw = true;
        }

        if needs_redraw {
            state.window.request_redraw();
        }
    }

    fn poll_oanix_manifest(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received a manifest to cache
        if let Some(rx) = &mut state.autopilot.oanix_manifest_rx {
            if let Ok(manifest) = rx.try_recv() {
                tracing::info!("Autopilot: cached OANIX manifest");
                state.autopilot.oanix_manifest = Some(manifest);
                state.autopilot.oanix_manifest_rx = None; // Done receiving
            }
        }
    }

    fn poll_autopilot_history(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received updated conversation history from Adjutant
        if let Some(rx) = &mut state.autopilot.autopilot_history_rx {
            if let Ok(updated_history) = rx.try_recv() {
                tracing::info!("Autopilot: updated conversation history ({} turns)", updated_history.len());
                state.autopilot.autopilot_history = updated_history;
                state.autopilot.autopilot_history_rx = None; // Done receiving
            }
        }
    }

    fn poll_rate_limits(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        // Check if we received rate limits
        if let Some(rx) = &mut state.session.rate_limit_rx {
            if let Ok(limits) = rx.try_recv() {
                state.session.rate_limits = limits;
                state.session.rate_limit_rx = None; // Done receiving (one-shot)
                state.window.request_redraw();
            }
        }
    }

    fn execute_command_palette_action(&mut self, command_id: &str) -> Option<String> {
        let Some(state) = &mut self.state else {
            return None;
        };

        let command_action = match command_id {
            command_palette_ids::HELP => {
                state.open_help();
                None
            }
            command_palette_ids::SETTINGS => Some(handle_command(state, Command::Config)),
            command_palette_ids::MODEL_PICKER => Some(handle_command(state, Command::Model)),
            command_palette_ids::SESSION_LIST => Some(handle_command(state, Command::SessionList)),
            command_palette_ids::SESSION_FORK => Some(handle_command(state, Command::SessionFork)),
            command_palette_ids::SESSION_EXPORT => Some(handle_command(state, Command::SessionExport)),
            command_palette_ids::CLEAR_CONVERSATION => Some(handle_command(state, Command::Clear)),
            command_palette_ids::UNDO_LAST => Some(handle_command(state, Command::Undo)),
            command_palette_ids::COMPACT_CONTEXT => Some(handle_command(state, Command::Compact)),
            command_palette_ids::INTERRUPT_REQUEST => {
                state.interrupt_query();
                None
            }
            command_palette_ids::PERMISSION_RULES => Some(handle_command(state, Command::PermissionRules)),
            command_palette_ids::MODE_CYCLE => {
                state
                    .permissions
                    .cycle_coder_mode(&mut state.session.session_info);
                None
            }
            command_palette_ids::MODE_BYPASS => {
                state.permissions.set_coder_mode(
                    CoderMode::BypassPermissions,
                    &mut state.session.session_info,
                );
                None
            }
            command_palette_ids::MODE_PLAN => {
                state
                    .permissions
                    .set_coder_mode(CoderMode::Plan, &mut state.session.session_info);
                None
            }
            command_palette_ids::MODE_AUTOPILOT => {
                state.permissions.set_coder_mode(
                    CoderMode::Autopilot,
                    &mut state.session.session_info,
                );
                None
            }
            command_palette_ids::TOOLS_LIST => Some(handle_command(state, Command::ToolsList)),
            command_palette_ids::MCP_CONFIG => Some(handle_command(state, Command::Mcp)),
            command_palette_ids::MCP_RELOAD => Some(handle_command(state, Command::McpReload)),
            command_palette_ids::MCP_STATUS => Some(handle_command(state, Command::McpStatus)),
            command_palette_ids::AGENTS_LIST => Some(handle_command(state, Command::Agents)),
            command_palette_ids::AGENT_CLEAR => Some(handle_command(state, Command::AgentClear)),
            command_palette_ids::AGENT_RELOAD => Some(handle_command(state, Command::AgentReload)),
            command_palette_ids::SKILLS_LIST => Some(handle_command(state, Command::Skills)),
            command_palette_ids::SKILLS_RELOAD => Some(handle_command(state, Command::SkillsReload)),
            command_palette_ids::HOOKS_OPEN => Some(handle_command(state, Command::Hooks)),
            command_palette_ids::HOOKS_RELOAD => Some(handle_command(state, Command::HooksReload)),
            command_palette_ids::SIDEBAR_LEFT => {
                state.toggle_left_sidebar();
                None
            }
            command_palette_ids::SIDEBAR_RIGHT => {
                state.toggle_right_sidebar();
                None
            }
            command_palette_ids::SIDEBAR_TOGGLE => {
                state.toggle_sidebars();
                None
            }
            command_palette_ids::BUG_REPORT => Some(handle_command(state, Command::Bug)),
            command_palette_ids::KITCHEN_SINK => {
                state.show_kitchen_sink = true;
                None
            }
            _ => None,
        };

        match command_action {
            Some(CommandAction::SubmitPrompt(prompt)) => Some(prompt),
            _ => None,
        }
    }

    fn render(&mut self) {
        let Some(state) = &mut self.state else {
            return;
        };

        let scale_factor = state.window.scale_factor() as f32;
        let logical_width = state.config.width as f32 / scale_factor;
        let logical_height = state.config.height as f32 / scale_factor;

        // Get surface texture
        let output = match state.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost) => {
                state.surface.configure(&state.device, &state.config);
                return;
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                tracing::error!("Out of memory");
                return;
            }
            Err(_) => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Build scene
        let mut scene = Scene::new();
        let bounds = Bounds::new(0.0, 0.0, logical_width, logical_height);
        let palette = palette_for(state.settings.coder_settings.theme);
        let sidebar_layout = sidebar_layout(
            logical_width,
            logical_height,
            state.left_sidebar_open,
            state.right_sidebar_open,
        );

        // Dark terminal background
        scene.draw_quad(Quad::new(bounds).with_background(palette.background));

        // Sidebar background color #0a0a0a
        let sidebar_bg = Hsla::new(0.0, 0.0, 0.039, 1.0);

        if let Some(left_bounds) = sidebar_layout.left {
            scene.draw_quad(
                Quad::new(left_bounds)
                    .with_background(sidebar_bg)
                    .with_border(palette.panel_border, 1.0),
            );

            // New Session button
            let btn_bounds = new_session_button_bounds(left_bounds);
            let btn_bg = if state.new_session_button_hovered {
                Hsla::new(0.0, 0.0, 0.15, 1.0)
            } else {
                Hsla::new(0.0, 0.0, 0.1, 1.0)
            };
            scene.draw_quad(
                Quad::new(btn_bounds)
                    .with_background(btn_bg)
                    .with_corner_radius(4.0),
            );
            let btn_text_y = btn_bounds.origin.y + (btn_bounds.size.height - 12.0) / 2.0;
            let btn_run = state.text_system.layout_styled_mono(
                "+ New Session",
                Point::new(btn_bounds.origin.x + 12.0, btn_text_y),
                12.0,
                palette.text_primary,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(btn_run);
        }

        if let Some(right_bounds) = sidebar_layout.right {
            scene.draw_quad(
                Quad::new(right_bounds)
                    .with_background(sidebar_bg)
                    .with_border(palette.panel_border, 1.0),
            );

            // Usage display
            let padding = 12.0;
            let mut y = right_bounds.origin.y + padding;
            let x = right_bounds.origin.x + padding;
            let w = right_bounds.size.width - padding * 2.0;

            let label_color = Hsla::new(0.0, 0.0, 0.5, 1.0);
            let value_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
            let muted_color = Hsla::new(0.0, 0.0, 0.4, 1.0);
            let font_size = 10.0;
            let line_height = 14.0;

            // Header
            let header = state.text_system.layout_styled_mono(
                "SESSION USAGE",
                Point::new(x, y),
                font_size,
                label_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(header);
            y += line_height + 8.0;

            // Model
            let model_text = &state.session.session_info.model;
            if !model_text.is_empty() {
                let model_run = state.text_system.layout_styled_mono(
                    model_text,
                    Point::new(x, y),
                    11.0,
                    value_color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(model_run);
                y += line_height + 8.0;
            }

            // Cost and turns
            let cost_text = format!("${:.4}", state.session.session_usage.total_cost_usd);
            let cost_run = state.text_system.layout_styled_mono(
                &cost_text,
                Point::new(x, y),
                11.0,
                value_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(cost_run);

            let turns_text = format!("{} turns", state.session.session_usage.num_turns);
            let turns_run = state.text_system.layout_styled_mono(
                &turns_text,
                Point::new(x + 70.0, y),
                font_size,
                muted_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(turns_run);
            y += line_height + 8.0;

            // Tokens
            let in_text = format!("{} in", format_tokens(state.session.session_usage.input_tokens));
            let in_run = state.text_system.layout_styled_mono(
                &in_text,
                Point::new(x, y),
                font_size,
                muted_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(in_run);

            let out_text = format!("{} out", format_tokens(state.session.session_usage.output_tokens));
            let out_run = state.text_system.layout_styled_mono(
                &out_text,
                Point::new(x + w / 2.0, y),
                font_size,
                muted_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(out_run);
            y += line_height + 4.0;

            // Duration
            let dur_text = format_duration_ms(state.session.session_usage.duration_ms);
            let dur_run = state.text_system.layout_styled_mono(
                &dur_text,
                Point::new(x, y),
                font_size,
                muted_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(dur_run);
            y += line_height + 16.0;

            // Rate limits section
            let green_color = Hsla::new(0.389, 0.7, 0.5, 1.0);

            // Render each rate limit
            let rate_limits_to_render: Vec<_> = [
                state.session.rate_limits.primary.clone(),
                state.session.rate_limits.secondary.clone(),
            ]
            .into_iter()
            .flatten()
            .collect();

            if !rate_limits_to_render.is_empty() {
                let header = state.text_system.layout_styled_mono(
                    "RATE LIMITS",
                    Point::new(x, y),
                    font_size,
                    label_color,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(header);
                y += line_height + 4.0;

                for limit in rate_limits_to_render {
                    // Limit name and percentage
                    let limit_text = format!("{} {:.0}%", limit.name, limit.percent_used);
                    let limit_run = state.text_system.layout_styled_mono(
                        &limit_text,
                        Point::new(x, y),
                        font_size,
                        muted_color,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(limit_run);
                    y += line_height;

                    // Progress bar
                    let bar_h = 4.0;
                    scene.draw_quad(
                        Quad::new(Bounds::new(x, y, w, bar_h))
                            .with_background(Hsla::new(0.0, 0.0, 0.2, 1.0)),
                    );
                    let bar_color = if limit.percent_used < 50.0 {
                        green_color
                    } else if limit.percent_used < 75.0 {
                        Hsla::new(0.125, 0.8, 0.5, 1.0) // yellow
                    } else {
                        Hsla::new(0.0, 0.8, 0.5, 1.0) // red
                    };
                    let fill_w = (w * limit.percent_used as f32 / 100.0).min(w);
                    scene.draw_quad(
                        Quad::new(Bounds::new(x, y, fill_w, bar_h)).with_background(bar_color),
                    );
                    y += bar_h + 2.0;

                    // Reset time
                    if !limit.resets_at.is_empty() {
                        let reset_text = format!("resets {}", limit.resets_at);
                        let reset_run = state.text_system.layout_styled_mono(
                            &reset_text,
                            Point::new(x, y),
                            9.0,
                            muted_color,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(reset_run);
                        y += line_height + 4.0;
                    }
                }
            }
        }

        let chat_layout =
            state.build_chat_layout(&sidebar_layout, logical_height);
        let viewport_top = chat_layout.viewport_top;
        let viewport_bottom = chat_layout.viewport_bottom;
        let content_x = chat_layout.content_x;
        let available_width = chat_layout.available_width;
        let chat_font_size = chat_layout.chat_font_size;
        let chat_line_height = chat_layout.chat_line_height;
        let streaming_height = chat_layout.streaming_height;

        let chat_clip_height = (viewport_bottom - viewport_top).max(0.0);
        let chat_clip_bounds = Bounds::new(
            sidebar_layout.main.origin.x,
            viewport_top,
            sidebar_layout.main.size.width,
            chat_clip_height,
        );
        let chat_clip_active = chat_clip_height > 0.0;
        if chat_clip_active {
            scene.push_clip(chat_clip_bounds);
        }

        if let Some(selection) = state.chat.chat_selection {
            if !selection.is_empty() {
                let (start, end) = selection.normalized();
                for layout in &chat_layout.message_layouts {
                    for line in &layout.lines {
                        if line.y + line.line_height < viewport_top || line.y > viewport_bottom {
                            continue;
                        }
                        if line.message_index < start.message_index
                            || line.message_index > end.message_index
                        {
                            continue;
                        }
                        let mut sel_start = if line.message_index == start.message_index {
                            start.offset
                        } else {
                            line.display_range.start
                        };
                        let mut sel_end = if line.message_index == end.message_index {
                            end.offset
                        } else {
                            line.display_range.end
                        };
                        sel_start = sel_start.clamp(line.display_range.start, line.display_range.end);
                        sel_end = sel_end.clamp(line.display_range.start, line.display_range.end);
                        if sel_end <= sel_start {
                            continue;
                        }
                        let start_char =
                            char_index_for_byte_offset(&line.text, sel_start - line.display_range.start);
                        let end_char =
                            char_index_for_byte_offset(&line.text, sel_end - line.display_range.start);
                        if end_char <= start_char {
                            continue;
                        }
                        let char_width = state
                            .text_system
                            .measure_styled_mono(
                                "M",
                                line.font_size,
                                wgpui::text::FontStyle::default(),
                            )
                            .max(1.0);
                        let highlight_x = line.x + start_char as f32 * char_width;
                        let highlight_w = (end_char - start_char) as f32 * char_width;
                        let bounds = Bounds::new(highlight_x, line.y, highlight_w, line.line_height);
                        scene.draw_quad(Quad::new(bounds).with_background(palette.selection_bg));
                    }
                }
            }
        }

        let mut y = viewport_top - state.chat.scroll_offset;
        let mut inline_tools_render_idx = 0;
        for (i, msg) in state.chat.messages.iter().enumerate() {
            let layout = &chat_layout.message_layouts[i];
            let msg_height = layout.height;

            if y + msg_height < viewport_top || y > viewport_bottom {
                y += msg_height;
                // Account for inline tools even when skipping off-screen messages
                if inline_tools_render_idx < chat_layout.inline_tools.len()
                    && chat_layout.inline_tools[inline_tools_render_idx].message_index == i
                {
                    y += chat_layout.inline_tools[inline_tools_render_idx].height + TOOL_PANEL_GAP;
                    inline_tools_render_idx += 1;
                }
                continue;
            }

            match msg.role {
                MessageRole::User => {
                    for line in &layout.lines {
                        // Check if line overlaps with viewport (standard range overlap test)
                        if line.y < viewport_bottom && line.y + line.line_height > viewport_top {
                            let text_run = state.text_system.layout_styled_mono(
                                &line.text,
                                Point::new(line.x, line.y),
                                line.font_size,
                                palette.user_text,
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(text_run);
                        }
                    }
                }
                MessageRole::Assistant => {
                    if let Some(doc) = &msg.document {
                        let content_visible = y + msg_height > viewport_top && y < viewport_bottom;
                        if content_visible {
                            state.chat.markdown_renderer.render(
                                doc,
                                Point::new(content_x, y),
                                available_width,
                                &mut state.text_system,
                                &mut scene,
                            );
                        }
                    } else {
                        for line in &layout.lines {
                            // Check if line overlaps with viewport (standard range overlap test)
                            if line.y < viewport_bottom && line.y + line.line_height > viewport_top {
                                let text_run = state.text_system.layout_styled_mono(
                                    &line.text,
                                    Point::new(line.x, line.y),
                                    line.font_size,
                                    palette.assistant_text,
                                    wgpui::text::FontStyle::default(),
                                );
                                scene.draw_text(text_run);
                            }
                        }
                    }

                    // Render metadata under assistant messages
                    if let Some(meta) = &msg.metadata {
                        let meta_y = y + msg_height - chat_line_height * 0.5;
                        if meta_y > viewport_top && meta_y < viewport_bottom {
                            let mut parts = Vec::new();
                            if let Some(model) = &meta.model {
                                parts.push(model.clone());
                            }
                            if let Some(input) = meta.input_tokens {
                                if let Some(output) = meta.output_tokens {
                                    parts.push(format!("{}+{} tokens", input, output));
                                }
                            }
                            if let Some(ms) = meta.duration_ms {
                                if ms >= 1000 {
                                    parts.push(format!("{:.1}s", ms as f64 / 1000.0));
                                } else {
                                    parts.push(format!("{}ms", ms));
                                }
                            }
                            if let Some(cost) = meta.cost_msats {
                                if cost > 0 {
                                    parts.push(format!("{} msats", cost));
                                }
                            }
                            if !parts.is_empty() {
                                let meta_text = parts.join(" · ");
                                let meta_color = Hsla::new(0.0, 0.0, 0.35, 1.0); // dark gray
                                let meta_run = state.text_system.layout_styled_mono(
                                    &meta_text,
                                    Point::new(content_x, meta_y),
                                    11.0, // smaller font
                                    meta_color,
                                    wgpui::text::FontStyle::default(),
                                );
                                scene.draw_text(meta_run);
                            }
                        }
                    }
                }
            }
            y += msg_height;

            // Account for inline tools after this message
            if inline_tools_render_idx < chat_layout.inline_tools.len()
                && chat_layout.inline_tools[inline_tools_render_idx].message_index == i
            {
                y += chat_layout.inline_tools[inline_tools_render_idx].height + TOOL_PANEL_GAP;
                inline_tools_render_idx += 1;
            }
        }

        if !state.chat.streaming_markdown.source().is_empty() {
            let doc = state.chat.streaming_markdown.document();
            let content_visible = y + streaming_height > viewport_top && y < viewport_bottom;
            if content_visible {
                state.chat.markdown_renderer.render(
                    doc,
                    Point::new(content_x, y),
                    available_width,
                    &mut state.text_system,
                    &mut scene,
                );
            }
        } else if state.chat.is_thinking {
            // Check if thinking indicator overlaps with viewport (standard range overlap test)
            if y < viewport_bottom && y + chat_line_height > viewport_top {
                let text_run = state.text_system.layout_styled_mono(
                    "...",
                    Point::new(content_x, y),
                    chat_font_size,
                    palette.thinking_text,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(text_run);
            }
        }

        // Render inline tools (scrolls with messages, no panel background)
        {
            let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
            for inline_layout in &chat_layout.inline_tools {
                for block in &inline_layout.blocks {
                    // Check if the tool block is visible in the viewport
                    let block_top = block.card_bounds.origin.y;
                    let block_bottom = block.detail_bounds
                        .as_ref()
                        .map(|db| db.origin.y + db.size.height)
                        .unwrap_or(block.card_bounds.origin.y + block.card_bounds.size.height);

                    if block_bottom > viewport_top && block_top < viewport_bottom {
                        if let Some(tool) = state.tools.tool_history.get_mut(block.index) {
                            tool.card.paint(block.card_bounds, &mut paint_cx);
                            if tool.status == ToolStatus::Running {
                                let ratio = tool
                                    .elapsed_secs
                                    .map(|elapsed| (elapsed / 6.0).min(1.0).max(0.1) as f32)
                                    .unwrap_or(0.2_f32);
                                let bar_height = 2.0;
                                let bar_bounds = Bounds::new(
                                    block.card_bounds.origin.x,
                                    block.card_bounds.origin.y + block.card_bounds.size.height - bar_height,
                                    block.card_bounds.size.width,
                                    bar_height,
                                );
                                paint_cx.scene.draw_quad(
                                    Quad::new(bar_bounds)
                                        .with_background(palette.tool_progress_bg),
                                );
                                paint_cx.scene.draw_quad(
                                    Quad::new(Bounds::new(
                                        bar_bounds.origin.x,
                                        bar_bounds.origin.y,
                                        bar_bounds.size.width * ratio,
                                        bar_bounds.size.height,
                                    ))
                                    .with_background(palette.tool_progress_fg),
                                );
                            }
                            if let Some(detail_bounds) = block.detail_bounds {
                                tool.detail.paint(detail_bounds, &mut paint_cx);
                            }
                        }
                    }
                }
            }
        }

        // Render DSPy stage cards
        {
            let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
            for dspy_layout in &chat_layout.dspy_stages {
                let stage_top = dspy_layout.y_offset;
                let stage_bottom = stage_top + dspy_layout.height;

                // Check if the stage is visible in the viewport
                if stage_bottom > viewport_top && stage_top < viewport_bottom {
                    if let Some(stage_viz) = state.tools.dspy_stages.get(dspy_layout.stage_index) {
                        Self::render_dspy_stage_card(
                            &stage_viz.stage,
                            Bounds::new(content_x, stage_top, available_width, dspy_layout.height),
                            &mut paint_cx,
                            &palette,
                        );
                    }
                }
            }
        }

        if chat_clip_active {
            scene.pop_clip();
        }

        // Input box (max width 768px, centered)
        let max_input_width = 768.0_f32;
        let available_input_width = sidebar_layout.main.size.width - INPUT_PADDING * 2.0;
        let input_width = available_input_width.min(max_input_width);
        let input_x = sidebar_layout.main.origin.x + (sidebar_layout.main.size.width - input_width) / 2.0;
        // Set max width for wrapping, then calculate dynamic height
        state.input.set_max_width(input_width);
        let input_height = state.input.current_height().max(40.0);

        // Input area background - flush with top of input box
        let input_area_y = logical_height - input_height - INPUT_PADDING - STATUS_BAR_HEIGHT;
        let input_area_bounds = Bounds::new(
            sidebar_layout.main.origin.x,
            input_area_y,
            sidebar_layout.main.size.width,
            logical_height - input_area_y,
        );
        scene.draw_quad(Quad::new(input_area_bounds).with_background(palette.background));

        let input_bounds = Bounds::new(
            input_x,
            logical_height - input_height - INPUT_PADDING - STATUS_BAR_HEIGHT,
            input_width,
            input_height,
        );

        let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
        state.input.paint(input_bounds, &mut paint_cx);

        // Draw ">" prompt inside input, aligned with the cursor line (bottom)
        let prompt_font = state.settings.coder_settings.font_size;
        let line_height = prompt_font * 1.4;
        let cursor_line = state.input.cursor_line();
        let prompt_y = input_bounds.origin.y + 8.0 + line_height * cursor_line as f32 + prompt_font * 0.15;
        let prompt_run = state.text_system.layout_styled_mono(
            ">",
            Point::new(input_bounds.origin.x + 12.0, prompt_y),
            prompt_font,
            palette.prompt,
            wgpui::text::FontStyle::default(),
        );
        scene.draw_text(prompt_run);

        let mode_label = coder_mode_display(state.permissions.coder_mode);
        let mode_color = coder_mode_color(state.permissions.coder_mode, &palette);
        if state.session.session_info.permission_mode.is_empty() {
            let mode_text = format!("Mode: {}", mode_label);
            let mode_run = state.text_system.layout_styled_mono(
                &mode_text,
                Point::new(
                    input_bounds.origin.x,
                    input_bounds.origin.y + input_bounds.size.height + 2.0,
                ),
                10.0,
                mode_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(mode_run);
        }

        // Draw status bar at very bottom (centered vertically)
        let status_y = logical_height - STATUS_BAR_HEIGHT - 3.0;

        // Left side: mode (colored) + hint (gray), flush with left edge of 768px container
        if !state.session.session_info.permission_mode.is_empty() {
            let mode_x = input_x;
            let mode_text = coder_mode_display(state.permissions.coder_mode);
            let mode_run = state.text_system.layout_styled_mono(
                mode_text,
                Point::new(mode_x, status_y),
                STATUS_BAR_FONT_SIZE,
                mode_color,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(mode_run);

            // Draw hint in gray after the mode text
            let hint_text = " (shift+tab to cycle)";
            let mode_width = mode_text.len() as f32 * 7.8; // Approx char width at 13pt
            let hint_run = state.text_system.layout_styled_mono(
                hint_text,
                Point::new(mode_x + mode_width, status_y),
                STATUS_BAR_FONT_SIZE,
                palette.status_right,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(hint_run);
        }

        // Right side: model, available open models, tools, session
        if !state.session.session_info.model.is_empty() {
            // Format: "haiku | gptoss | 18 tools | abc123"
            let model_short = state.session.session_info.model
                .replace("claude-", "")
                .replace("-20251101", "")
                .replace("-20250929", "")
                .replace("-20251001", "");
            let session_short = if state.session.session_info.session_id.len() > 8 {
                &state.session.session_info.session_id[..8]
            } else {
                &state.session.session_info.session_id
            };
            let mut parts = Vec::new();
            parts.push(model_short);
            // Add available open models (not Claude SDK since that's already shown)
            for provider in &state.autopilot.available_providers {
                if !matches!(provider, adjutant::dspy::lm_config::LmProvider::ClaudeSdk) {
                    parts.push(provider.short_name().to_string());
                }
            }
            if let Some(summary) = state.catalogs.mcp_status_summary() {
                parts.push(summary);
            }
            if let Some(active_agent) = &state.catalogs.active_agent {
                parts.push(format!(
                    "agent {}",
                    truncate_preview(active_agent, 12)
                ));
            }
            // Only show session if we have an actual session ID
            if !state.session.session_info.session_id.is_empty() {
                parts.push(format!("session {}", session_short));
            }
            let right_text = parts.join(" | ");
            // Measure and right-align within the 768px container
            let text_width = right_text.len() as f32 * 7.8; // Approx char width at 13pt
            let right_x = input_x + input_width - text_width;
            let right_run = state.text_system.layout_styled_mono(
                &right_text,
                Point::new(right_x, status_y),
                STATUS_BAR_FONT_SIZE,
                palette.status_right,
                wgpui::text::FontStyle::default(),
            );
            scene.draw_text(right_run);
        }

        // Draw modal if active
        let should_refresh_sessions = matches!(state.modal_state, ModalState::SessionList { .. })
            && state.session.session_cards.len() != state.session.session_index.len();
        if should_refresh_sessions {
            state.session.refresh_session_cards(state.chat.is_thinking);
        }
        let should_refresh_agents = matches!(state.modal_state, ModalState::AgentList { .. })
            && state.catalogs.agent_cards.len() != state.catalogs.agent_entries.len();
        if should_refresh_agents {
            state.catalogs.refresh_agent_cards(state.chat.is_thinking);
        }
        let should_refresh_skills = matches!(state.modal_state, ModalState::SkillList { .. })
            && state.catalogs.skill_cards.len() != state.catalogs.skill_entries.len();
        if should_refresh_skills {
            state.catalogs.refresh_skill_cards();
        }
        match &state.modal_state {
            ModalState::None => {}
            ModalState::ModelPicker { selected } => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                // Semi-transparent overlay
                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                // Modal box
                let modal_width = 700.0;
                let modal_height = 200.0;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                // Modal background
                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;

                // Title
                let title_run = state.text_system.layout_styled_mono(
                    "Select model",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0), // White
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                // Description
                let desc_run = state.text_system.layout_styled_mono(
                    "Switch between Claude models. Applies to this session and future Claude Code sessions.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(desc_run);
                y += 30.0;

                // Model options
                let models = ModelOption::all();
                for (i, model) in models.iter().enumerate() {
                    let is_selected = i == *selected;
                    let is_current = *model == state.settings.selected_model;

                    // Selection indicator
                    let indicator = if is_selected { ">" } else { " " };
                    let indicator_run = state.text_system.layout_styled_mono(
                        indicator,
                        Point::new(modal_x + 16.0, y),
                        14.0,
                        Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(indicator_run);

                    // Number
                    let num_text = format!("{}.", i + 1);
                    let num_run = state.text_system.layout_styled_mono(
                        &num_text,
                        Point::new(modal_x + 32.0, y),
                        14.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(num_run);

                    // Name
                    let name_color = if is_selected {
                        Hsla::new(120.0, 0.6, 0.6, 1.0) // Green for selected
                    } else {
                        Hsla::new(0.0, 0.0, 0.7, 1.0) // White-ish
                    };
                    let name_run = state.text_system.layout_styled_mono(
                        model.name(),
                        Point::new(modal_x + 56.0, y),
                        14.0,
                        name_color,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(name_run);

                    // Checkmark if current
                    if is_current {
                        let check_run = state.text_system.layout_styled_mono(
                            "✓",
                            Point::new(modal_x + 220.0, y),
                            14.0,
                            Hsla::new(120.0, 0.6, 0.5, 1.0), // Green
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(check_run);
                    }

                    // Description
                    let desc_run = state.text_system.layout_styled_mono(
                        model.description(),
                        Point::new(modal_x + 240.0, y),
                        14.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0), // Gray
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(desc_run);

                    y += 24.0;
                }

                // Footer
                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Enter to confirm · Esc to exit",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.4, 1.0), // Dim gray
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::SessionList { selected } => {
                let sessions = &state.session.session_index;
                // Semi-transparent overlay
                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                let selected = (*selected).min(sessions.len().saturating_sub(1));
                let checkpoint_height = if state.session.checkpoint_entries.is_empty() {
                    0.0
                } else {
                    state.session.checkpoint_restore.size_hint().1.unwrap_or(0.0)
                };
                let layout = session_list_layout(
                    logical_width,
                    logical_height,
                    sessions.len(),
                    selected,
                    checkpoint_height,
                );
                let modal_bounds = layout.modal_bounds;
                let modal_x = modal_bounds.origin.x;
                let modal_y = modal_bounds.origin.y;
                let _modal_width = modal_bounds.size.width;
                let modal_height = modal_bounds.size.height;

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + SESSION_MODAL_PADDING;
                let title_run = state.text_system.layout_styled_mono(
                    "Sessions",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let desc_run = state.text_system.layout_styled_mono(
                    "Click a card to resume, or fork from a previous session.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(desc_run);

                if sessions.is_empty() {
                    y += 26.0;
                    let empty_run = state.text_system.layout_styled_mono(
                        "No sessions recorded yet.",
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                } else {
                    let mut paint_cx =
                        PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
                    for (index, bounds) in &layout.card_bounds {
                        if let Some(card) = state.session.session_cards.get_mut(*index) {
                            card.paint(*bounds, &mut paint_cx);
                        }
                        if *index == selected {
                            let outline = Quad::new(*bounds)
                                .with_border(Hsla::new(120.0, 0.6, 0.5, 1.0), 1.0);
                            paint_cx.scene.draw_quad(outline);
                        }
                    }
                }

                if let Some(bounds) = layout.checkpoint_bounds {
                    let mut paint_cx =
                        PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
                    state.session.checkpoint_restore.paint(bounds, &mut paint_cx);
                }

                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Enter to resume · Esc to exit · Fork with button",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.4, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::AgentList { selected } => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                // Semi-transparent overlay
                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                let modal_width = SESSION_MODAL_WIDTH;
                let modal_height = SESSION_MODAL_HEIGHT;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "Agents",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let desc_run = state.text_system.layout_styled_mono(
                    "Select an agent to focus the next request.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(desc_run);
                y += 18.0;

                if let Some(active) = &state.catalogs.active_agent {
                    let active_line = format!("Active agent: {}", active);
                    let active_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&active_line, 90),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(120.0, 0.6, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(active_run);
                    y += 18.0;
                }

                let project_path = state.catalogs.agent_project_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let project_line = format!("Project agents: {}", project_path);
                let project_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&project_line, 90),
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(project_run);
                y += 18.0;

                if let Some(user_path) = &state.catalogs.agent_user_path {
                    let user_line = format!("User agents: {}", user_path.display());
                    let user_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&user_line, 90),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(user_run);
                    y += 18.0;
                }

                if let Some(error) = &state.catalogs.agent_load_error {
                    let error_line = format!("Load warning: {}", error);
                    let error_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&error_line, 100),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(15.0, 0.7, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(error_run);
                    y += 18.0;
                }

                let project_count = state.catalogs.agent_entries
                    .iter()
                    .filter(|entry| entry.source == AgentSource::Project)
                    .count();
                let user_count = state.catalogs.agent_entries
                    .iter()
                    .filter(|entry| entry.source == AgentSource::User)
                    .count();
                let counts_line = format!(
                    "Agents: {} project · {} user",
                    project_count, user_count
                );
                let counts_run = state.text_system.layout_styled_mono(
                    &counts_line,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(counts_run);

                let list_top = agent_modal_content_top(modal_y, state);
                let layout = agent_list_layout(
                    logical_width,
                    logical_height,
                    state.catalogs.agent_entries.len(),
                    *selected,
                    list_top,
                );

                if state.catalogs.agent_entries.is_empty() {
                    let empty_run = state.text_system.layout_styled_mono(
                        "No agents found.",
                        Point::new(modal_x + 16.0, list_top),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                } else {
                    let selected = (*selected).min(state.catalogs.agent_entries.len().saturating_sub(1));
                    let mut paint_cx =
                        PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
                    for (index, bounds) in &layout.card_bounds {
                        if let Some(card) = state.catalogs.agent_cards.get_mut(*index) {
                            card.paint(*bounds, &mut paint_cx);
                        }
                        if *index == selected {
                            let outline = Quad::new(*bounds)
                                .with_border(Hsla::new(120.0, 0.6, 0.5, 1.0), 1.0);
                            paint_cx.scene.draw_quad(outline);
                        }
                    }
                }

                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Enter to activate · R to reload · Esc to exit",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.4, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::SkillList { selected } => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                let modal_width = SESSION_MODAL_WIDTH;
                let modal_height = SESSION_MODAL_HEIGHT;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "Skills",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let desc_run = state.text_system.layout_styled_mono(
                    "Filesystem skills available to Claude.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(desc_run);
                y += 18.0;

                let project_path = state.catalogs.skill_project_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let project_line = format!("Project skills: {}", project_path);
                let project_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&project_line, 90),
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(project_run);
                y += 18.0;

                if let Some(user_path) = &state.catalogs.skill_user_path {
                    let user_line = format!("User skills: {}", user_path.display());
                    let user_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&user_line, 90),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(user_run);
                    y += 18.0;
                }

                if let Some(error) = &state.catalogs.skill_load_error {
                    let error_line = format!("Load warning: {}", error);
                    let error_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&error_line, 100),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(15.0, 0.7, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(error_run);
                    y += 18.0;
                }

                let project_count = state.catalogs.skill_entries
                    .iter()
                    .filter(|entry| entry.source == SkillSource::Project)
                    .count();
                let user_count = state.catalogs.skill_entries
                    .iter()
                    .filter(|entry| entry.source == SkillSource::User)
                    .count();
                let counts_line = format!(
                    "Skills: {} project · {} user",
                    project_count, user_count
                );
                let counts_run = state.text_system.layout_styled_mono(
                    &counts_line,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(counts_run);

                let list_top = skill_modal_content_top(modal_y, state);
                let layout = skill_list_layout(
                    logical_width,
                    logical_height,
                    state.catalogs.skill_entries.len(),
                    *selected,
                    list_top,
                );

                if state.catalogs.skill_entries.is_empty() {
                    let empty_run = state.text_system.layout_styled_mono(
                        "No skills found.",
                        Point::new(modal_x + 16.0, list_top),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                } else {
                    let selected = (*selected).min(state.catalogs.skill_entries.len().saturating_sub(1));
                    let mut paint_cx =
                        PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
                    for (index, bounds) in &layout.card_bounds {
                        if let Some(card) = state.catalogs.skill_cards.get_mut(*index) {
                            card.paint(*bounds, &mut paint_cx);
                        }
                        if *index == selected {
                            let outline = Quad::new(*bounds)
                                .with_border(Hsla::new(120.0, 0.6, 0.5, 1.0), 1.0);
                            paint_cx.scene.draw_quad(outline);
                        }
                    }
                }

                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Enter to close · R to reload · Esc to exit",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.4, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::Hooks { view, selected } => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                let modal_width = HOOK_MODAL_WIDTH;
                let modal_height = HOOK_MODAL_HEIGHT;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "Hooks",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let view_label = match view {
                    HookModalView::Config => "Config",
                    HookModalView::Events => "Events",
                };
                let view_line = format!("View: {} (Tab to switch)", view_label);
                let view_run = state.text_system.layout_styled_mono(
                    &view_line,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(view_run);
                y += 18.0;

                match view {
                    HookModalView::Config => {
                        let desc_run = state.text_system.layout_styled_mono(
                            "Configure built-in hooks and review loaded scripts.",
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.5, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(desc_run);
                        y += 20.0;

                        let config_lines = [
                            (HookSetting::ToolBlocker, "ToolBlocker", state.catalogs.hook_config.tool_blocker),
                            (HookSetting::ToolLogger, "ToolLogger", state.catalogs.hook_config.tool_logger),
                            (
                                HookSetting::OutputTruncator,
                                "OutputTruncator",
                                state.catalogs.hook_config.output_truncator,
                            ),
                            (
                                HookSetting::ContextInjection,
                                "ContextInjection",
                                state.catalogs.hook_config.context_injection,
                            ),
                            (HookSetting::TodoEnforcer, "TodoEnforcer", state.catalogs.hook_config.todo_enforcer),
                        ];

                        for (idx, (_setting, label, enabled)) in config_lines.iter().enumerate() {
                            let marker = if *enabled { "[x]" } else { "[ ]" };
                            let line = format!("{}. {} {}", idx + 1, marker, label);
                            let line_run = state.text_system.layout_styled_mono(
                                &line,
                                Point::new(modal_x + 16.0, y),
                                12.0,
                                if *enabled {
                                    Hsla::new(120.0, 0.6, 0.6, 1.0)
                                } else {
                                    Hsla::new(0.0, 0.0, 0.6, 1.0)
                                },
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(line_run);
                            y += 18.0;
                        }

                        y += 4.0;
                        let project_path = state.catalogs.hook_project_path
                            .as_ref()
                            .map(|path| path.display().to_string())
                            .unwrap_or_else(|| "unknown".to_string());
                        let project_line = format!("Project hooks: {}", project_path);
                        let project_run = state.text_system.layout_styled_mono(
                            &truncate_preview(&project_line, 90),
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.6, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(project_run);
                        y += 18.0;

                        if let Some(user_path) = &state.catalogs.hook_user_path {
                            let user_line = format!("User hooks: {}", user_path.display());
                            let user_run = state.text_system.layout_styled_mono(
                                &truncate_preview(&user_line, 90),
                                Point::new(modal_x + 16.0, y),
                                12.0,
                                Hsla::new(0.0, 0.0, 0.6, 1.0),
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(user_run);
                            y += 18.0;
                        }

                        if let Some(error) = &state.catalogs.hook_load_error {
                            let error_line = format!("Load warning: {}", error);
                            let error_run = state.text_system.layout_styled_mono(
                                &truncate_preview(&error_line, 100),
                                Point::new(modal_x + 16.0, y),
                                12.0,
                                Hsla::new(15.0, 0.7, 0.6, 1.0),
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(error_run);
                            y += 18.0;
                        }

                        let script_count = state.catalogs.hook_scripts.len();
                        let scripts_line = format!("Scripts: {}", script_count);
                        let scripts_run = state.text_system.layout_styled_mono(
                            &scripts_line,
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.6, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(scripts_run);
                        y += 18.0;

                        let list_top = y;
                        let list_bottom = modal_y + modal_height - 48.0;
                        let row_height = 18.0;
                        let max_rows =
                            ((list_bottom - list_top) / row_height).floor().max(0.0) as usize;
                        if script_count == 0 {
                            let empty_run = state.text_system.layout_styled_mono(
                                "No hook scripts found.",
                                Point::new(modal_x + 16.0, list_top),
                                12.0,
                                Hsla::new(0.0, 0.0, 0.5, 1.0),
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(empty_run);
                        } else {
                            for (idx, script) in state.catalogs.hook_scripts.iter().take(max_rows).enumerate() {
                                let source_label = match script.source {
                                    HookScriptSource::Project => "project",
                                    HookScriptSource::User => "user",
                                };
                                let matcher = script
                                    .matcher
                                    .as_ref()
                                    .map(|matcher| format!(" ({})", matcher))
                                    .unwrap_or_default();
                                let line = format!(
                                    "- {}{} · {} · {}",
                                    hook_event_label(script.event),
                                    matcher,
                                    source_label,
                                    script.path.display()
                                );
                                let line_run = state.text_system.layout_styled_mono(
                                    &truncate_preview(&line, 120),
                                    Point::new(modal_x + 16.0, list_top + idx as f32 * row_height),
                                    12.0,
                                    Hsla::new(0.0, 0.0, 0.55, 1.0),
                                    wgpui::text::FontStyle::default(),
                                );
                                scene.draw_text(line_run);
                            }
                        }

                        y = modal_y + modal_height - 24.0;
                        let footer_run = state.text_system.layout_styled_mono(
                            "1-5 toggle · Tab for events · R to reload · Esc to exit",
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.4, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(footer_run);
                    }
                    HookModalView::Events => {
                        let desc_run = state.text_system.layout_styled_mono(
                            "Hook callbacks executed during the current session.",
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.5, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(desc_run);

                        let layout = hook_event_layout(
                            logical_width,
                            logical_height,
                            state.catalogs.hook_event_log.len(),
                            *selected,
                        );

                        if state.catalogs.hook_event_log.is_empty() {
                            let empty_run = state.text_system.layout_styled_mono(
                                "No hook events logged yet.",
                                Point::new(modal_x + 16.0, layout.list_bounds.origin.y),
                                12.0,
                                Hsla::new(0.0, 0.0, 0.5, 1.0),
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(empty_run);
                        } else {
                            let selected = (*selected).min(state.catalogs.hook_event_log.len().saturating_sub(1));
                            for (index, bounds) in &layout.row_bounds {
                                if let Some(entry) = state.catalogs.hook_event_log.get(*index) {
                                    if *index == selected {
                                        let highlight = Quad::new(*bounds)
                                            .with_background(Hsla::new(220.0, 0.2, 0.18, 1.0));
                                        scene.draw_quad(highlight);
                                    }
                                    let timestamp = format_relative_time(entry.timestamp);
                                    let mut label = format!(
                                        "{} · {}",
                                        timestamp,
                                        hook_event_label(entry.event)
                                    );
                                    if let Some(tool) = &entry.tool_name {
                                        label.push_str(" · ");
                                        label.push_str(tool);
                                    }
                                    let label_run = state.text_system.layout_styled_mono(
                                        &truncate_preview(&label, 42),
                                        Point::new(bounds.origin.x + 6.0, bounds.origin.y + 4.0),
                                        11.0,
                                        Hsla::new(0.0, 0.0, 0.7, 1.0),
                                        wgpui::text::FontStyle::default(),
                                    );
                                    scene.draw_text(label_run);
                                }
                            }

                            if state.catalogs.hook_inspector.is_none() {
                                state.sync_hook_inspector(selected);
                            }
                            if let Some(inspector) = state.catalogs.hook_inspector.as_mut() {
                                let mut paint_cx =
                                    PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
                                inspector.paint(layout.inspector_bounds, &mut paint_cx);
                            }
                        }

                        y = modal_y + modal_height - 24.0;
                        let footer_run = state.text_system.layout_styled_mono(
                            "Up/Down to select · Tab for config · C to clear · Esc to exit",
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.4, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(footer_run);
                    }
                }
            }
            ModalState::ToolList { selected } => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                let tools = &state.session.session_info.tools;
                // Semi-transparent overlay
                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                let modal_width = 520.0;
                let modal_height = 320.0;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "Tools",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let desc_run = state.text_system.layout_styled_mono(
                    "Available tools from the active session.",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(desc_run);
                y += 26.0;

                if tools.is_empty() {
                    let empty_run = state.text_system.layout_styled_mono(
                        "No tool data yet.",
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                } else {
                    let selected = (*selected).min(tools.len().saturating_sub(1));
                    for (i, tool) in tools.iter().take(12).enumerate() {
                        let is_selected = i == selected;
                        let indicator = if is_selected { ">" } else { " " };
                        let indicator_run = state.text_system.layout_styled_mono(
                            indicator,
                            Point::new(modal_x + 16.0, y),
                            13.0,
                            Hsla::new(120.0, 0.6, 0.5, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(indicator_run);

                        let mut label = tool.clone();
                        if state.permissions.tools_disallowed.iter().any(|t| t == tool) {
                            label.push_str(" (disabled)");
                        } else if state.permissions.tools_allowed.iter().any(|t| t == tool) {
                            label.push_str(" (enabled)");
                        }

                        let label_run = state.text_system.layout_styled_mono(
                            &label,
                            Point::new(modal_x + 32.0, y),
                            13.0,
                            if is_selected {
                                Hsla::new(120.0, 0.6, 0.6, 1.0)
                            } else {
                                Hsla::new(0.0, 0.0, 0.7, 1.0)
                            },
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(label_run);
                        y += 20.0;
                    }
                }

                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Enter to close · Esc to exit",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.4, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::PermissionRules => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                let modal_width = 560.0;
                let modal_height = 420.0;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "Permission rules",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let default_text = if state.permissions.permission_default_allow {
                    "Default: allow"
                } else {
                    "Default: deny"
                };
                let default_run = state.text_system.layout_styled_mono(
                    default_text,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(default_run);
                y += 22.0;

                let allow_label = state.text_system.layout_styled_mono(
                    "Allow:",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(120.0, 0.6, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(allow_label);
                let allow_text = if state.permissions.permission_allow_tools.is_empty() {
                    "None".to_string()
                } else {
                    state.permissions.permission_allow_tools.join(", ")
                };
                let allow_run = state.text_system.layout_styled_mono(
                    &allow_text,
                    Point::new(modal_x + 80.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(allow_run);
                y += 22.0;

                let deny_label = state.text_system.layout_styled_mono(
                    "Deny:",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.6, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(deny_label);
                let deny_text = if state.permissions.permission_deny_tools.is_empty() {
                    "None".to_string()
                } else {
                    state.permissions.permission_deny_tools.join(", ")
                };
                let deny_run = state.text_system.layout_styled_mono(
                    &deny_text,
                    Point::new(modal_x + 80.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(deny_run);

                y += 22.0;
                let bash_allow_label = state.text_system.layout_styled_mono(
                    "Bash allow:",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(120.0, 0.6, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(bash_allow_label);
                let bash_allow_text = if state.permissions.permission_allow_bash_patterns.is_empty() {
                    "None".to_string()
                } else {
                    state.permissions.permission_allow_bash_patterns.join(", ")
                };
                let bash_allow_run = state.text_system.layout_styled_mono(
                    &bash_allow_text,
                    Point::new(modal_x + 120.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(bash_allow_run);
                y += 22.0;

                let bash_deny_label = state.text_system.layout_styled_mono(
                    "Bash deny:",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.6, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(bash_deny_label);
                let bash_deny_text = if state.permissions.permission_deny_bash_patterns.is_empty() {
                    "None".to_string()
                } else {
                    state.permissions.permission_deny_bash_patterns.join(", ")
                };
                let bash_deny_run = state.text_system.layout_styled_mono(
                    &bash_deny_text,
                    Point::new(modal_x + 120.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(bash_deny_run);
                y += 26.0;

                let history_title = state.text_system.layout_styled_mono(
                    "Recent decisions:",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.85, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(history_title);
                y += 18.0;

                if state.permissions.permission_history.is_empty() {
                    let empty_run = state.text_system.layout_styled_mono(
                        "No recent permission decisions.",
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                } else {
                    for entry in state.permissions.permission_history.iter().rev().take(5) {
                        let mut line = format!(
                            "@{} [{}] {}",
                            entry.timestamp, entry.decision, entry.tool_name
                        );
                        if let Some(detail) = &entry.detail {
                            if !detail.trim().is_empty() {
                                line.push_str(" - ");
                                line.push_str(detail);
                            }
                        }
                        let line = truncate_preview(&line, 120);
                        let entry_run = state.text_system.layout_styled_mono(
                            &line,
                            Point::new(modal_x + 16.0, y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.6, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(entry_run);
                        y += 18.0;
                    }
                }

                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Enter to close · Esc to exit",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.4, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::Config {
                tab,
                selected,
                search,
                input_mode,
            } => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                let overlay = Quad::new(bounds).with_background(palette.overlay);
                scene.draw_quad(overlay);

                let modal_width = SETTINGS_MODAL_WIDTH;
                let modal_height = SETTINGS_MODAL_HEIGHT;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(palette.panel)
                    .with_border(palette.panel_border, 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "Settings",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let search_label = if matches!(input_mode, SettingsInputMode::Search) {
                    format!("Search: {}_", search)
                } else if search.trim().is_empty() {
                    "Search: /".to_string()
                } else {
                    format!("Search: {}", search)
                };
                let search_run = state.text_system.layout_styled_mono(
                    &search_label,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    if matches!(input_mode, SettingsInputMode::Search) {
                        palette.text_primary
                    } else {
                        palette.text_muted
                    },
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(search_run);
                y += 18.0;

                let tabs = SettingsTab::all();
                let mut tab_x = modal_x + 16.0;
                let tab_y = y;
                for entry in tabs {
                    let label = entry.label();
                    let tab_width = (label.len() as f32 * 7.0).max(48.0);
                    if *entry == *tab {
                        let highlight = Quad::new(Bounds::new(
                            tab_x - 6.0,
                            tab_y - 2.0,
                            tab_width + 12.0,
                            SETTINGS_TAB_HEIGHT,
                        ))
                        .with_background(palette.panel_highlight);
                        scene.draw_quad(highlight);
                    }
                    let tab_run = state.text_system.layout_styled_mono(
                        label,
                        Point::new(tab_x, tab_y),
                        12.0,
                        if *entry == *tab {
                            palette.text_primary
                        } else {
                            palette.text_muted
                        },
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(tab_run);
                    tab_x += tab_width + 16.0;
                }
                y += SETTINGS_TAB_HEIGHT + 8.0;

                let snapshot = SettingsSnapshot::from_state(state);
                let rows = settings_rows(&snapshot, *tab, search);
                let list_top = y;
                let list_bottom = modal_y + modal_height - 48.0;
                let max_visible =
                    ((list_bottom - list_top) / SETTINGS_ROW_HEIGHT).floor().max(0.0) as usize;

                if rows.is_empty() {
                    let empty_run = state.text_system.layout_styled_mono(
                        "No settings match this search.",
                        Point::new(modal_x + 16.0, list_top),
                        12.0,
                        palette.text_dim,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                } else {
                    let visible = rows.len().min(max_visible.max(1));
                    let selected = (*selected).min(rows.len().saturating_sub(1));
                    let mut start = selected.saturating_sub(visible / 2);
                    if start + visible > rows.len() {
                        start = rows.len().saturating_sub(visible);
                    }
                    let value_x = modal_x + modal_width * 0.55;
                    let hint_x = modal_x + modal_width * 0.75;
                    let capture_action = match input_mode {
                        SettingsInputMode::Capture(action) => Some(*action),
                        _ => None,
                    };

                    for idx in 0..visible {
                        let index = start + idx;
                        let row = &rows[index];
                        let row_y = list_top + idx as f32 * SETTINGS_ROW_HEIGHT;
                        let is_selected = index == selected;
                        if is_selected {
                            let highlight = Quad::new(Bounds::new(
                                modal_x + 12.0,
                                row_y - 2.0,
                                modal_width - 24.0,
                                SETTINGS_ROW_HEIGHT,
                            ))
                            .with_background(palette.panel_highlight);
                            scene.draw_quad(highlight);
                        }

                        let label_run = state.text_system.layout_styled_mono(
                            &row.label,
                            Point::new(modal_x + 20.0, row_y),
                            12.0,
                            if is_selected {
                                palette.text_primary
                            } else {
                                palette.text_muted
                            },
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(label_run);

                        let value_text = if let Some(action) = capture_action {
                            if is_selected
                                && matches!(row.item, SettingsItem::Keybinding(a) if a == action)
                            {
                                "Press keys...".to_string()
                            } else {
                                row.value.clone()
                            }
                        } else {
                            row.value.clone()
                        };
                        let value_run = state.text_system.layout_styled_mono(
                            &value_text,
                            Point::new(value_x, row_y),
                            12.0,
                            palette.text_secondary,
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(value_run);

                        if let Some(hint) = &row.hint {
                            let hint_run = state.text_system.layout_styled_mono(
                                hint,
                                Point::new(hint_x, row_y),
                                11.0,
                                palette.text_faint,
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(hint_run);
                        }
                    }
                }

                y = modal_y + modal_height - 24.0;
                let footer_text = match input_mode {
                    SettingsInputMode::Search => "Type to search · Enter/Esc to finish",
                    SettingsInputMode::Capture(_) => "Press new shortcut · Esc to cancel",
                    SettingsInputMode::Normal => {
                        "Tab to switch · / to search · Enter/Left/Right to change · Esc to close"
                    }
                };
                let footer_run = state.text_system.layout_styled_mono(
                    footer_text,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::McpConfig { selected } => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                let overlay = Quad::new(bounds).with_background(Hsla::new(0.0, 0.0, 0.0, 0.7));
                scene.draw_quad(overlay);

                let modal_width = 720.0;
                let modal_height = 420.0;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(Hsla::new(220.0, 0.15, 0.12, 1.0))
                    .with_border(Hsla::new(220.0, 0.15, 0.25, 1.0), 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "MCP Servers",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    Hsla::new(0.0, 0.0, 0.9, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let project_path = state.catalogs.mcp_project_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                let project_line = format!("Project config: {}", project_path);
                let project_run = state.text_system.layout_styled_mono(
                    &truncate_preview(&project_line, 90),
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(project_run);
                y += 18.0;

                if let Some(error) = &state.catalogs.mcp_project_error {
                    let error_line = format!("Config warning: {}", error);
                    let error_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&error_line, 100),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(15.0, 0.7, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(error_run);
                    y += 18.0;
                }

                if let Some(error) = &state.catalogs.mcp_status_error {
                    let status_line = format!("Status warning: {}", error);
                    let status_run = state.text_system.layout_styled_mono(
                        &truncate_preview(&status_line, 100),
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        Hsla::new(15.0, 0.7, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(status_run);
                    y += 18.0;
                }

                let entries = state.catalogs.mcp_entries();
                let counts_line = format!(
                    "Servers: {} project · {} runtime · {} disabled",
                    state.catalogs.mcp_project_servers.len(),
                    state.catalogs.mcp_runtime_servers.len(),
                    state.catalogs.mcp_disabled_servers.len()
                );
                let counts_run = state.text_system.layout_styled_mono(
                    &counts_line,
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(counts_run);
                y += 20.0;

                let list_top = y;
                let list_bottom = modal_y + modal_height - 48.0;
                let row_height = 22.0;
                let max_visible = ((list_bottom - list_top) / row_height).floor().max(0.0) as usize;
                if entries.is_empty() {
                    let empty_run = state.text_system.layout_styled_mono(
                        "No MCP servers configured.",
                        Point::new(modal_x + 16.0, list_top),
                        12.0,
                        Hsla::new(0.0, 0.0, 0.5, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(empty_run);
                } else {
                    let visible = entries.len().min(max_visible.max(1));
                    let selected = (*selected).min(entries.len().saturating_sub(1));
                    let mut start = selected.saturating_sub(visible / 2);
                    if start + visible > entries.len() {
                        start = entries.len().saturating_sub(visible);
                    }

                    for idx in 0..visible {
                        let index = start + idx;
                        let entry = &entries[index];
                        let row_y = list_top + idx as f32 * row_height;
                        if index == selected {
                            let highlight = Quad::new(Bounds::new(
                                modal_x + 12.0,
                                row_y - 2.0,
                                modal_width - 24.0,
                                row_height,
                            ))
                            .with_background(Hsla::new(220.0, 0.2, 0.18, 1.0));
                            scene.draw_quad(highlight);
                        }

                        let source_label = match entry.source {
                            Some(McpServerSource::Project) => "project",
                            Some(McpServerSource::Runtime) => "runtime",
                            None => "status",
                        };
                        let mut line = format!("{} [{}]", entry.name, source_label);
                        if let Some(status) = &entry.status {
                            line.push_str(&format!(" · {}", status));
                        }
                        if entry.disabled {
                            line.push_str(" · disabled");
                        }
                        let line = truncate_preview(&line, 120);
                        let line_run = state.text_system.layout_styled_mono(
                            &line,
                            Point::new(modal_x + 20.0, row_y),
                            12.0,
                            Hsla::new(0.0, 0.0, 0.7, 1.0),
                            wgpui::text::FontStyle::default(),
                        );
                        scene.draw_text(line_run);

                        if let Some(config) = &entry.config {
                            let detail = describe_mcp_config(config);
                            let detail = truncate_preview(&detail, 120);
                            let detail_run = state.text_system.layout_styled_mono(
                                &detail,
                                Point::new(modal_x + 260.0, row_y),
                                11.0,
                                Hsla::new(0.0, 0.0, 0.5, 1.0),
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(detail_run);
                        }
                    }
                }

                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Enter/Esc to close · R reload · S status · Del disable",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    Hsla::new(0.0, 0.0, 0.4, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
            ModalState::Help => {
                // Render on layer 1 to be on top of all layer 0 content
                scene.set_layer(1);

                let overlay = Quad::new(bounds).with_background(palette.overlay);
                scene.draw_quad(overlay);

                let modal_width = HELP_MODAL_WIDTH;
                let modal_height = HELP_MODAL_HEIGHT;
                let modal_x = (logical_width - modal_width) / 2.0;
                let modal_y = modal_y_in_content(logical_height, modal_height);
                let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

                let modal_bg = Quad::new(modal_bounds)
                    .with_background(palette.panel)
                    .with_border(palette.panel_border, 1.0);
                scene.draw_quad(modal_bg);

                let mut y = modal_y + 16.0;
                let title_run = state.text_system.layout_styled_mono(
                    "Help",
                    Point::new(modal_x + 16.0, y),
                    14.0,
                    palette.text_primary,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(title_run);
                y += 20.0;

                let max_chars = ((modal_width - 32.0) / 7.0).max(20.0) as usize;
                let line_height = 14.0;
                let section_gap = 6.0;

                let interrupt = keybinding_labels(&state.settings.keybindings, KeyAction::Interrupt, "Ctrl+C");
                let palette_key =
                    keybinding_labels(&state.settings.keybindings, KeyAction::OpenCommandPalette, "Ctrl+K");
                let settings_key =
                    keybinding_labels(&state.settings.keybindings, KeyAction::OpenSettings, "Ctrl+,");
                let left_sidebar =
                    keybinding_labels(&state.settings.keybindings, KeyAction::ToggleLeftSidebar, "Ctrl+[");
                let right_sidebar =
                    keybinding_labels(&state.settings.keybindings, KeyAction::ToggleRightSidebar, "Ctrl+]");
                let toggle_sidebars =
                    keybinding_labels(&state.settings.keybindings, KeyAction::ToggleSidebars, "Ctrl+\\");

                let sections: Vec<(&str, Vec<String>)> = vec![
                    (
                        "Hotkeys",
                        vec![
                            "F1 - Help".to_string(),
                            format!("Enter - Send message"),
                            "Shift+Tab - Cycle permission mode".to_string(),
                            format!("{} - Interrupt request", interrupt),
                            format!("{} - Command palette", palette_key),
                            format!("{} - Settings", settings_key),
                            format!("{} - Toggle left sidebar", left_sidebar),
                            format!("{} - Toggle right sidebar", right_sidebar),
                            format!("{} - Toggle both sidebars", toggle_sidebars),
                        ],
                    ),
                    (
                        "Core",
                        vec![
                            "/model - choose model; /output-style <name> - style output".to_string(),
                            "/clear - reset chat; /compact - compact context; /undo - undo last exchange"
                                .to_string(),
                            "/cancel - cancel active run; /bug - report issue".to_string(),
                        ],
                    ),
                    (
                        "Sessions",
                        vec![
                            "/session list - list sessions; /session resume <id> - resume".to_string(),
                            "/session fork - fork current; /session export - export markdown".to_string(),
                        ],
                    ),
                    (
                        "Permissions",
                        vec![
                            "/permission mode <default|plan|acceptEdits|bypassPermissions|dontAsk>"
                                .to_string(),
                            "/permission rules - manage rules".to_string(),
                            "/permission allow|deny <tool|bash:pattern>".to_string(),
                        ],
                    ),
                    (
                        "Tools, MCP, Hooks",
                        vec![
                            "/tools - list tools; /tools enable|disable <tool>".to_string(),
                            "/mcp - open MCP servers; /mcp add|remove <name> <json>".to_string(),
                            "/hooks - hook panel; /hooks reload - reload scripts".to_string(),
                        ],
                    ),
                    (
                        "Agents, Skills, Prompts",
                        vec![
                            "/agents - manage agents; /agent select <name>; /agent clear".to_string(),
                            "/skills - manage skills; /skills reload".to_string(),
                            "@file - insert file; !command - run bash and insert output".to_string(),
                        ],
                    ),
                ];

                for (title, lines) in sections {
                    let heading = state.text_system.layout_styled_mono(
                        title,
                        Point::new(modal_x + 16.0, y),
                        12.0,
                        palette.text_primary,
                        wgpui::text::FontStyle::default(),
                    );
                    scene.draw_text(heading);
                    y += line_height;

                    for line in lines {
                        for wrapped in wrap_text(&line, max_chars) {
                            let text_run = state.text_system.layout_styled_mono(
                                &wrapped,
                                Point::new(modal_x + 20.0, y),
                                11.0,
                                palette.text_muted,
                                wgpui::text::FontStyle::default(),
                            );
                            scene.draw_text(text_run);
                            y += line_height;
                        }
                    }

                    y += section_gap;
                }

                y = modal_y + modal_height - 24.0;
                let footer_run = state.text_system.layout_styled_mono(
                    "Esc/F1 to close",
                    Point::new(modal_x + 16.0, y),
                    12.0,
                    palette.text_faint,
                    wgpui::text::FontStyle::default(),
                );
                scene.draw_text(footer_run);
            }
        }

        // Kitchen sink storybook overlay (covers full screen)
        if state.show_kitchen_sink {
            // Render on layer 1 to be on top of all layer 0 content
            scene.set_layer(1);

            paint_kitchen_sink(
                bounds,
                &mut scene,
                &mut state.text_system,
                scale_factor,
                state.kitchen_sink_scroll,
                &palette,
            );
        }

        if state.command_palette.is_open() {
            let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
            state.command_palette.paint(bounds, &mut paint_cx);
        }

        if state.chat.chat_context_menu.is_open() {
            scene.set_layer(1);
            let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
            state.chat.chat_context_menu.paint(bounds, &mut paint_cx);
        }

        if let Some(dialog) = state.permissions.permission_dialog.as_mut() {
            if dialog.is_open() {
                let mut paint_cx = PaintContext::new(&mut scene, &mut state.text_system, scale_factor);
                dialog.paint(bounds, &mut paint_cx);
            }
        }

        // Render
        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Coder Render"),
            });

        let physical_width = state.config.width as f32;
        let physical_height = state.config.height as f32;

        state.renderer.resize(
            &state.queue,
            Size::new(physical_width, physical_height),
            1.0,
        );

        if state.text_system.is_dirty() {
            state.renderer.update_atlas(
                &state.queue,
                state.text_system.atlas_data(),
                state.text_system.atlas_size(),
            );
            state.text_system.mark_clean();
        }

        state.renderer.prepare(&state.device, &state.queue, &scene, scale_factor);
        state.renderer.render(&mut encoder, &view);

        state.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// Render a DSPy stage card with visual styling
    fn render_dspy_stage_card(
        stage: &DspyStage,
        bounds: Bounds,
        cx: &mut PaintContext,
        palette: &UiPalette,
    ) {
        let padding = 12.0;
        let font_size = 13.0;
        let small_font_size = 11.0;
        let line_height = font_size * 1.4;
        let small_line_height = small_font_size * 1.4;

        // Background with accent border based on stage type
        let (header_text, accent_color, icon) = match stage {
            DspyStage::EnvironmentAssessment { .. } => (
                "Environment Assessment",
                Hsla::new(200.0 / 360.0, 0.7, 0.5, 1.0), // Blue
                "🔍",
            ),
            DspyStage::Planning { .. } => (
                "Planning",
                Hsla::new(280.0 / 360.0, 0.6, 0.5, 1.0), // Purple
                "📋",
            ),
            DspyStage::TodoList { .. } => (
                "Todo List",
                Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0), // Green
                "✅",
            ),
            DspyStage::ExecutingTask { .. } => (
                "Executing",
                Hsla::new(30.0 / 360.0, 0.8, 0.5, 1.0), // Orange
                "🔧",
            ),
            DspyStage::TaskComplete { .. } => (
                "Task Complete",
                Hsla::new(120.0 / 360.0, 0.6, 0.45, 1.0), // Green
                "✓",
            ),
            DspyStage::Complete { .. } => (
                "Complete",
                Hsla::new(120.0 / 360.0, 0.7, 0.45, 1.0), // Green
                "🎉",
            ),
        };

        // Draw card background
        let card_bg = Quad::new(bounds)
            .with_background(Hsla::new(220.0 / 360.0, 0.15, 0.14, 1.0))
            .with_border(accent_color, 2.0)
            .with_corner_radius(6.0);
        cx.scene.draw_quad(card_bg);

        let content_x = bounds.origin.x + padding;
        let mut y = bounds.origin.y + padding;

        // Header with icon
        let header = format!("{} {}", icon, header_text);
        let header_run = cx.text.layout_styled_mono(
            &header,
            Point::new(content_x, y),
            font_size,
            accent_color,
            wgpui::text::FontStyle::default(),
        );
        cx.scene.draw_text(header_run);
        y += line_height + 8.0;

        // Stage-specific content
        match stage {
            DspyStage::EnvironmentAssessment {
                system_info,
                workspace,
                active_directive,
                open_issues,
                compute_backends,
                priority_action,
                urgency,
                ..
            } => {
                let lines = [
                    format!("System: {}", system_info),
                    format!("Workspace: {}", workspace),
                    format!(
                        "Directive: {}",
                        active_directive.as_deref().unwrap_or("None")
                    ),
                    format!("Open issues: {}", open_issues),
                    format!("Compute: {}", compute_backends.join(", ")),
                    format!("Priority: {} | Urgency: {}", priority_action, urgency),
                ];
                for line in lines {
                    let run = cx.text.layout_styled_mono(
                        &line,
                        Point::new(content_x, y),
                        small_font_size,
                        palette.assistant_text,
                        wgpui::text::FontStyle::default(),
                    );
                    cx.scene.draw_text(run);
                    y += small_line_height;
                }
            }
            DspyStage::Planning {
                analysis,
                files_to_modify,
                complexity,
                confidence,
                ..
            } => {
                // Analysis (truncated)
                let analysis_preview = if analysis.len() > 100 {
                    format!("{}...", &analysis[..100])
                } else {
                    analysis.clone()
                };
                let run = cx.text.layout_styled_mono(
                    &analysis_preview,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.assistant_text,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height * 2.0;

                // Files
                if !files_to_modify.is_empty() {
                    let files_line = format!("Files: {}", files_to_modify.join(", "));
                    let run = cx.text.layout_styled_mono(
                        &files_line,
                        Point::new(content_x, y),
                        small_font_size,
                        Hsla::new(0.0, 0.0, 0.6, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    cx.scene.draw_text(run);
                    y += small_line_height;
                }

                // Complexity and confidence
                let stats = format!(
                    "Complexity: {} | Confidence: {:.0}%",
                    complexity,
                    confidence * 100.0
                );
                let run = cx.text.layout_styled_mono(
                    &stats,
                    Point::new(content_x, y),
                    small_font_size,
                    Hsla::new(0.0, 0.0, 0.5, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
            }
            DspyStage::TodoList { tasks } => {
                for task in tasks.iter().take(10) {
                    let checkbox = match task.status {
                        crate::autopilot_loop::TodoStatus::Pending => "□",
                        crate::autopilot_loop::TodoStatus::InProgress => "◐",
                        crate::autopilot_loop::TodoStatus::Complete => "✓",
                        crate::autopilot_loop::TodoStatus::Failed => "✗",
                    };
                    let status_color = match task.status {
                        crate::autopilot_loop::TodoStatus::Pending => {
                            Hsla::new(0.0, 0.0, 0.5, 1.0)
                        }
                        crate::autopilot_loop::TodoStatus::InProgress => {
                            Hsla::new(30.0 / 360.0, 0.8, 0.5, 1.0)
                        }
                        crate::autopilot_loop::TodoStatus::Complete => {
                            Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0)
                        }
                        crate::autopilot_loop::TodoStatus::Failed => {
                            Hsla::new(0.0, 0.8, 0.5, 1.0)
                        }
                    };
                    let line = format!("{} {}. {}", checkbox, task.index, task.description);
                    let run = cx.text.layout_styled_mono(
                        &line,
                        Point::new(content_x, y),
                        small_font_size,
                        status_color,
                        wgpui::text::FontStyle::default(),
                    );
                    cx.scene.draw_text(run);
                    y += small_line_height;
                }
                if tasks.len() > 10 {
                    let more = format!("... and {} more", tasks.len() - 10);
                    let run = cx.text.layout_styled_mono(
                        &more,
                        Point::new(content_x, y),
                        small_font_size,
                        Hsla::new(0.0, 0.0, 0.4, 1.0),
                        wgpui::text::FontStyle::default(),
                    );
                    cx.scene.draw_text(run);
                }
            }
            DspyStage::ExecutingTask {
                task_index,
                total_tasks,
                task_description,
            } => {
                let status = format!("Working on task {} of {}...", task_index, total_tasks);
                let run = cx.text.layout_styled_mono(
                    &status,
                    Point::new(content_x, y),
                    small_font_size,
                    Hsla::new(30.0 / 360.0, 0.8, 0.6, 1.0),
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
                y += small_line_height;

                let run = cx.text.layout_styled_mono(
                    task_description,
                    Point::new(content_x, y),
                    small_font_size,
                    palette.assistant_text,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
            }
            DspyStage::TaskComplete { task_index, success } => {
                let (status, color) = if *success {
                    (format!("✓ Task {} completed", task_index), Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0))
                } else {
                    (format!("✗ Task {} failed", task_index), Hsla::new(0.0, 0.8, 0.5, 1.0))
                };
                let run = cx.text.layout_styled_mono(
                    &status,
                    Point::new(content_x, y),
                    font_size,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
            }
            DspyStage::Complete {
                total_tasks,
                successful,
                failed,
            } => {
                let summary = format!(
                    "Completed {} tasks: {} successful, {} failed",
                    total_tasks, successful, failed
                );
                let color = if *failed == 0 {
                    Hsla::new(120.0 / 360.0, 0.6, 0.5, 1.0)
                } else {
                    Hsla::new(30.0 / 360.0, 0.7, 0.5, 1.0)
                };
                let run = cx.text.layout_styled_mono(
                    &summary,
                    Point::new(content_x, y),
                    font_size,
                    color,
                    wgpui::text::FontStyle::default(),
                );
                cx.scene.draw_text(run);
            }
        }
    }
}

/// Paint the Kitchen Sink storybook overlay showing all UI component variations
fn paint_kitchen_sink(
    bounds: Bounds,
    scene: &mut Scene,
    text_system: &mut TextSystem,
    scale_factor: f32,
    scroll_offset: f32,
    palette: &UiPalette,
) {
    // Opaque background to cover content behind
    let overlay = Quad::new(bounds).with_background(Hsla::new(220.0, 0.15, 0.08, 1.0));
    scene.draw_quad(overlay);

    // Content area with padding
    let padding = 24.0;
    let content_x = bounds.origin.x + padding;
    let content_width = bounds.size.width - padding * 2.0;
    let card_width = (content_width - 16.0) / 2.0; // Two columns

    let mut y = bounds.origin.y + padding - scroll_offset;
    let font_style = wgpui::text::FontStyle::default();

    // Title
    let title_run = text_system.layout_styled_mono(
        "Kitchen Sink - Component Storybook",
        Point::new(content_x, y),
        18.0,
        Hsla::new(0.0, 0.0, 0.95, 1.0),
        font_style,
    );
    scene.draw_text(title_run);
    y += 28.0;

    let subtitle_run = text_system.layout_styled_mono(
        "Press Escape to close | Scroll to see more",
        Point::new(content_x, y),
        12.0,
        Hsla::new(0.0, 0.0, 0.5, 1.0),
        font_style,
    );
    scene.draw_text(subtitle_run);
    y += 32.0;

    // Section: Tool Types
    let section_run = text_system.layout_styled_mono(
        "TOOL TYPES (with Success status)",
        Point::new(content_x, y),
        14.0,
        Hsla::new(42.0 / 360.0, 0.8, 0.6, 1.0), // Yellow/gold
        font_style,
    );
    scene.draw_text(section_run);
    y += 24.0;

    let tool_types = [
        (ToolType::Read, "Read", "src/main.rs"),
        (ToolType::Write, "Write", "output.txt"),
        (ToolType::Edit, "Edit", "config.toml"),
        (ToolType::Bash, "Bash", "cargo build"),
        (ToolType::Glob, "Glob", "**/*.rs"),
        (ToolType::Grep, "Grep", "fn main"),
        (ToolType::Search, "Search", "error handling"),
        (ToolType::List, "List", "/home/user"),
        (ToolType::Task, "Task", "Analyze codebase"),
        (ToolType::WebFetch, "WebFetch", "https://example.com"),
    ];

    let mut paint_cx = PaintContext::new(scene, text_system, scale_factor);
    let mut col = 0;
    let mut row_y = y;

    for (tool_type, name, input) in &tool_types {
        let x = content_x + (col as f32 * (card_width + 16.0));
        let card_bounds = Bounds::new(x, row_y, card_width, 28.0);

        let mut card = ToolCallCard::new(*tool_type, *name)
            .status(ToolStatus::Success)
            .input(*input)
            .elapsed_secs(0.42);
        card.paint(card_bounds, &mut paint_cx);

        col += 1;
        if col >= 2 {
            col = 0;
            row_y += 36.0;
        }
    }

    if col != 0 {
        row_y += 36.0;
    }
    y = row_y + 16.0;

    // Section: Tool Statuses
    let section_run = paint_cx.text.layout_styled_mono(
        "TOOL STATUSES (Read tool)",
        Point::new(content_x, y),
        14.0,
        Hsla::new(200.0 / 360.0, 0.8, 0.6, 1.0), // Blue
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    let statuses = [
        (ToolStatus::Pending, "Pending"),
        (ToolStatus::Running, "Running"),
        (ToolStatus::Success, "Success"),
        (ToolStatus::Error, "Error"),
        (ToolStatus::Cancelled, "Cancelled"),
    ];

    col = 0;
    row_y = y;

    for (status, label) in &statuses {
        let x = content_x + (col as f32 * (card_width + 16.0));
        let card_bounds = Bounds::new(x, row_y, card_width, 28.0);

        let elapsed = if matches!(status, ToolStatus::Success | ToolStatus::Error) {
            Some(1.23)
        } else {
            None
        };

        let mut card = ToolCallCard::new(ToolType::Read, format!("Read ({})", label))
            .status(*status)
            .input("example.rs");
        if let Some(e) = elapsed {
            card = card.elapsed_secs(e);
        }
        card.paint(card_bounds, &mut paint_cx);

        col += 1;
        if col >= 2 {
            col = 0;
            row_y += 36.0;
        }
    }

    if col != 0 {
        row_y += 36.0;
    }
    y = row_y + 16.0;

    // Section: Task with Children
    let section_run = paint_cx.text.layout_styled_mono(
        "TASK WITH CHILD TOOLS",
        Point::new(content_x, y),
        14.0,
        Hsla::new(280.0 / 360.0, 0.8, 0.6, 1.0), // Purple
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    {
        // Task with children: header(22) + expanded input(18) + 3 children(66) = 106, use 120
        let card_bounds = Bounds::new(content_x, y, content_width, 120.0);
        let mut task_card = ToolCallCard::new(ToolType::Task, "Task")
            .status(ToolStatus::Running)
            .input("Explore the authentication module")
            .expanded(true);

        task_card.add_child(ChildTool {
            tool_type: ToolType::Glob,
            name: "Glob".to_string(),
            params: "**/auth*.rs".to_string(),
            status: ToolStatus::Success,
            elapsed_secs: Some(0.12),
        });
        task_card.add_child(ChildTool {
            tool_type: ToolType::Read,
            name: "Read".to_string(),
            params: "src/auth/mod.rs".to_string(),
            status: ToolStatus::Success,
            elapsed_secs: Some(0.08),
        });
        task_card.add_child(ChildTool {
            tool_type: ToolType::Grep,
            name: "Grep".to_string(),
            params: "verify_token".to_string(),
            status: ToolStatus::Running,
            elapsed_secs: None,
        });

        task_card.paint(card_bounds, &mut paint_cx);
    }
    y += 130.0;

    // Section: Diff Tool
    let section_run = paint_cx.text.layout_styled_mono(
        "DIFF TOOL CALL",
        Point::new(content_x, y),
        14.0,
        Hsla::new(120.0 / 360.0, 0.8, 0.5, 1.0), // Green
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    {
        // Diff with 4 lines: base(44) + 4*18 + 12 = 128, use 140
        let diff_bounds = Bounds::new(content_x, y, content_width, 140.0);
        let mut diff_tool = DiffToolCall::new("src/main.rs")
            .status(ToolStatus::Success)
            .lines(vec![
                DiffLine {
                    kind: DiffLineKind::Context,
                    old_line: Some(10),
                    new_line: Some(10),
                    content: "fn main() {".to_string(),
                },
                DiffLine {
                    kind: DiffLineKind::Deletion,
                    old_line: Some(11),
                    new_line: None,
                    content: "    println!(\"Hello\");".to_string(),
                },
                DiffLine {
                    kind: DiffLineKind::Addition,
                    old_line: None,
                    new_line: Some(11),
                    content: "    println!(\"Hello, World!\");".to_string(),
                },
                DiffLine {
                    kind: DiffLineKind::Context,
                    old_line: Some(12),
                    new_line: Some(12),
                    content: "}".to_string(),
                },
            ]);
        diff_tool.paint(diff_bounds, &mut paint_cx);
    }
    y += 150.0;

    // Section: Search Tool
    let section_run = paint_cx.text.layout_styled_mono(
        "SEARCH TOOL CALL",
        Point::new(content_x, y),
        14.0,
        Hsla::new(30.0 / 360.0, 0.8, 0.6, 1.0), // Orange
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    {
        // Search with 2 matches: base(64) + 2*36 = 136, use 150
        let search_bounds = Bounds::new(content_x, y, content_width, 150.0);
        let mut search_tool = SearchToolCall::new("error")
            .status(ToolStatus::Success)
            .matches(vec![
                SearchMatch {
                    file: "src/lib.rs".to_string(),
                    line: 42,
                    content: "    Err(Error::NotFound)".to_string(),
                },
                SearchMatch {
                    file: "src/main.rs".to_string(),
                    line: 15,
                    content: "    .expect(\"error loading config\")".to_string(),
                },
            ]);
        search_tool.paint(search_bounds, &mut paint_cx);
    }
    y += 160.0;

    // Section: Terminal Tool
    let section_run = paint_cx.text.layout_styled_mono(
        "TERMINAL TOOL CALL",
        Point::new(content_x, y),
        14.0,
        Hsla::new(0.0, 0.0, 0.7, 1.0), // Gray
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    {
        // Terminal: base(44) + output(84) = 128, use 140
        let terminal_bounds = Bounds::new(content_x, y, content_width, 140.0);
        let mut terminal_tool = TerminalToolCall::new("cargo build --release")
            .status(ToolStatus::Success)
            .output("   Compiling myapp v0.1.0\n    Finished release [optimized] target(s) in 2.34s")
            .exit_code(0);
        terminal_tool.paint(terminal_bounds, &mut paint_cx);
    }
    y += 150.0;

    // Section: Messages (placeholder)
    let section_run = paint_cx.text.layout_styled_mono(
        "MESSAGE COMPONENTS",
        Point::new(content_x, y),
        14.0,
        Hsla::new(180.0 / 360.0, 0.6, 0.5, 1.0), // Cyan
        font_style,
    );
    paint_cx.scene.draw_text(section_run);
    y += 24.0;

    // User message
    let user_bg = Quad::new(Bounds::new(content_x, y, content_width, 32.0))
        .with_background(Hsla::new(0.0, 0.0, 0.1, 1.0));
    paint_cx.scene.draw_quad(user_bg);
    let user_run = paint_cx.text.layout_styled_mono(
        "User: What does the main function do?",
        Point::new(content_x + 8.0, y + 8.0),
        12.0,
        palette.user_text,
        font_style,
    );
    paint_cx.scene.draw_text(user_run);
    y += 40.0;

    // Assistant message
    let assist_bg = Quad::new(Bounds::new(content_x, y, content_width, 48.0))
        .with_background(Hsla::new(0.0, 0.0, 0.08, 1.0));
    paint_cx.scene.draw_quad(assist_bg);
    let assist_run = paint_cx.text.layout_styled_mono(
        "Assistant: The main function is the entry point of the program.",
        Point::new(content_x + 8.0, y + 8.0),
        12.0,
        palette.assistant_text,
        font_style,
    );
    paint_cx.scene.draw_text(assist_run);
    let meta_run = paint_cx.text.layout_styled_mono(
        "claude-3-opus · 45+120 tokens · 1.2s",
        Point::new(content_x + 8.0, y + 28.0),
        11.0,
        Hsla::new(0.0, 0.0, 0.35, 1.0),
        font_style,
    );
    paint_cx.scene.draw_text(meta_run);
    y += 56.0;

    // Footer
    y += 16.0;
    let footer_run = paint_cx.text.layout_styled_mono(
        "End of Kitchen Sink",
        Point::new(content_x, y),
        12.0,
        Hsla::new(0.0, 0.0, 0.4, 1.0),
        font_style,
    );
    paint_cx.scene.draw_text(footer_run);
}

/// Extract text from streaming event
fn extract_stream_text(event: &Value) -> Option<String> {
    // Stream events can have various formats depending on event type
    // Common patterns:
    // - content_block_delta with delta.text
    // - message_delta with content

    // Try content_block_delta format
    if let Some(delta) = event.get("delta") {
        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
            return Some(text.to_string());
        }
    }

    // Try direct text field
    if let Some(text) = event.get("text").and_then(|t| t.as_str()) {
        return Some(text.to_string());
    }

    // Try content field
    if let Some(content) = event.get("content").and_then(|c| c.as_str()) {
        return Some(content.to_string());
    }

    None
}

/// Safely truncate a string at a valid UTF-8 char boundary
/// Extract tool call start info from content_block_start event
fn extract_tool_call_start(event: &Value) -> Option<(String, String)> {
    let event_type = event.get("type")?.as_str()?;
    if event_type != "content_block_start" {
        return None;
    }

    let content_block = event.get("content_block")?;
    let block_type = content_block.get("type")?.as_str()?;
    if block_type != "tool_use" {
        return None;
    }

    let tool_name = content_block.get("name")?.as_str()?.to_string();
    let tool_id = content_block.get("id")?.as_str()?.to_string();
    Some((tool_name, tool_id))
}

/// Extract tool input JSON delta
fn extract_tool_input_delta(event: &Value) -> Option<String> {
    let event_type = event.get("type")?.as_str()?;
    if event_type != "content_block_delta" {
        return None;
    }

    let delta = event.get("delta")?;
    let delta_type = delta.get("type")?.as_str()?;
    if delta_type != "input_json_delta" {
        return None;
    }

    delta.get("partial_json")?.as_str().map(|s| s.to_string())
}

struct SessionListLayout {
    modal_bounds: Bounds,
    card_bounds: Vec<(usize, Bounds)>,
    checkpoint_bounds: Option<Bounds>,
}

struct AgentListLayout {
    card_bounds: Vec<(usize, Bounds)>,
}

struct SkillListLayout {
    card_bounds: Vec<(usize, Bounds)>,
}

struct HookEventLayout {
    list_bounds: Bounds,
    inspector_bounds: Bounds,
    row_bounds: Vec<(usize, Bounds)>,
}

fn session_list_layout(
    logical_width: f32,
    logical_height: f32,
    session_count: usize,
    selected: usize,
    checkpoint_height: f32,
) -> SessionListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let modal_bounds = Bounds::new(modal_x, modal_y, modal_width, modal_height);

    let content_top = modal_y + SESSION_MODAL_PADDING + 46.0;
    let footer_y = modal_y + modal_height - 24.0;
    let checkpoint_height = checkpoint_height.max(0.0);
    let checkpoint_bounds = if checkpoint_height > 0.0 {
        let y = footer_y - 12.0 - checkpoint_height;
        Some(Bounds::new(
            modal_x + SESSION_MODAL_PADDING,
            y,
            modal_width - SESSION_MODAL_PADDING * 2.0,
            checkpoint_height,
        ))
    } else {
        None
    };

    let card_area_bottom = checkpoint_bounds
        .as_ref()
        .map(|bounds| bounds.origin.y - 16.0)
        .unwrap_or(footer_y - 16.0);
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SESSION_CARD_HEIGHT + SESSION_CARD_GAP)) as usize
    };

    let visible_count = session_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(session_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > session_count {
            start = session_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SESSION_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SESSION_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    SessionListLayout {
        modal_bounds,
        card_bounds,
        checkpoint_bounds,
    }
}

fn agent_modal_content_top(modal_y: f32, state: &AppState) -> f32 {
    let mut y = modal_y + 16.0;
    y += 20.0;
    y += 18.0;
    if state.catalogs.active_agent.is_some() {
        y += 18.0;
    }
    y += 18.0;
    if state.catalogs.agent_user_path.is_some() {
        y += 18.0;
    }
    if state.catalogs.agent_load_error.is_some() {
        y += 18.0;
    }
    y + 20.0
}

fn skill_modal_content_top(modal_y: f32, state: &AppState) -> f32 {
    let mut y = modal_y + 16.0;
    y += 20.0;
    y += 18.0;
    y += 18.0;
    if state.catalogs.skill_user_path.is_some() {
        y += 18.0;
    }
    if state.catalogs.skill_load_error.is_some() {
        y += 18.0;
    }
    y + 20.0
}

fn agent_list_layout(
    logical_width: f32,
    logical_height: f32,
    agent_count: usize,
    selected: usize,
    content_top: f32,
) -> AgentListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let footer_y = modal_y + modal_height - 24.0;
    let card_area_bottom = footer_y - 16.0;
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SESSION_CARD_HEIGHT + SESSION_CARD_GAP)) as usize
    };

    let visible_count = agent_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(agent_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > agent_count {
            start = agent_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SESSION_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SESSION_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    AgentListLayout { card_bounds }
}

fn skill_list_layout(
    logical_width: f32,
    logical_height: f32,
    skill_count: usize,
    selected: usize,
    content_top: f32,
) -> SkillListLayout {
    let modal_width = SESSION_MODAL_WIDTH;
    let modal_height = SESSION_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);
    let footer_y = modal_y + modal_height - 24.0;
    let card_area_bottom = footer_y - 16.0;
    let available_height = (card_area_bottom - content_top).max(0.0);
    let max_cards = if available_height <= 0.0 {
        0
    } else {
        ((available_height + SESSION_CARD_GAP) / (SKILL_CARD_HEIGHT + SESSION_CARD_GAP)) as usize
    };

    let visible_count = skill_count.min(max_cards);
    let mut card_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(skill_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > skill_count {
            start = skill_count.saturating_sub(visible_count);
        }

        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * (SKILL_CARD_HEIGHT + SESSION_CARD_GAP);
            let bounds = Bounds::new(
                modal_x + SESSION_MODAL_PADDING,
                y,
                modal_width - SESSION_MODAL_PADDING * 2.0,
                SKILL_CARD_HEIGHT,
            );
            card_bounds.push((index, bounds));
        }
    }

    SkillListLayout { card_bounds }
}

fn hook_event_layout(
    logical_width: f32,
    logical_height: f32,
    event_count: usize,
    selected: usize,
) -> HookEventLayout {
    let modal_width = HOOK_MODAL_WIDTH;
    let modal_height = HOOK_MODAL_HEIGHT;
    let modal_x = (logical_width - modal_width) / 2.0;
    let modal_y = modal_y_in_content(logical_height, modal_height);

    let content_top = modal_y + 64.0;
    let content_bottom = modal_y + modal_height - 32.0;
    let content_height = (content_bottom - content_top).max(0.0);
    let list_width = 260.0;
    let list_bounds = Bounds::new(
        modal_x + 16.0,
        content_top,
        list_width,
        content_height,
    );
    let inspector_bounds = Bounds::new(
        list_bounds.origin.x + list_width + 16.0,
        content_top,
        modal_width - list_width - 48.0,
        content_height,
    );

    let max_rows = (content_height / HOOK_EVENT_ROW_HEIGHT).floor().max(0.0) as usize;
    let visible_count = event_count.min(max_rows.max(1));
    let mut row_bounds = Vec::new();
    if visible_count > 0 {
        let selected = selected.min(event_count.saturating_sub(1));
        let mut start = selected.saturating_sub(visible_count / 2);
        if start + visible_count > event_count {
            start = event_count.saturating_sub(visible_count);
        }
        for i in 0..visible_count {
            let index = start + i;
            let y = content_top + i as f32 * HOOK_EVENT_ROW_HEIGHT;
            let bounds = Bounds::new(
                list_bounds.origin.x,
                y,
                list_bounds.size.width,
                HOOK_EVENT_ROW_HEIGHT,
            );
            row_bounds.push((index, bounds));
        }
    }

    HookEventLayout {
        list_bounds,
        inspector_bounds,
        row_bounds,
    }
}

fn handle_command(state: &mut AppState, command: Command) -> CommandAction {
    match command {
        Command::Help => {
            state.open_command_palette();
            CommandAction::None
        }
        Command::Clear => {
            state.clear_conversation();
            CommandAction::None
        }
        Command::Compact => {
            if state.chat.is_thinking {
                state.push_system_message("Cannot compact during an active request.".to_string());
                CommandAction::None
            } else {
                CommandAction::SubmitPrompt("/compact".to_string())
            }
        }
        Command::Model => {
            state.open_model_picker();
            CommandAction::None
        }
        Command::Undo => {
            state.undo_last_exchange();
            CommandAction::None
        }
        Command::Cancel => {
            state.interrupt_query();
            CommandAction::None
        }
        Command::Bug => {
            match open_url(BUG_REPORT_URL) {
                Ok(()) => state.push_system_message("Opened bug report in browser.".to_string()),
                Err(err) => state.push_system_message(format!(
                    "Failed to open browser: {} (URL: {}).",
                    err, BUG_REPORT_URL
                )),
            }
            CommandAction::None
        }
        Command::SessionList => {
            state.open_session_list();
            CommandAction::None
        }
        Command::SessionResume(id) => {
            state.begin_session_resume(id);
            CommandAction::None
        }
        Command::SessionFork => {
            state.begin_session_fork();
            CommandAction::None
        }
        Command::SessionExport => {
            state.export_session();
            CommandAction::None
        }
        Command::PermissionMode(mode) => {
            match parse_coder_mode(&mode) {
                Some(parsed) => state
                    .permissions
                    .set_coder_mode(parsed, &mut state.session.session_info),
                None => state.push_system_message(format!(
                    "Unknown mode: {}. Valid modes: bypass, plan, autopilot",
                    mode
                )),
            }
            CommandAction::None
        }
        Command::PermissionRules => {
            state.open_permission_rules();
            CommandAction::None
        }
        Command::PermissionAllow(tools) => {
            let message = state.permissions.add_permission_allow(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::PermissionDeny(tools) => {
            let message = state.permissions.add_permission_deny(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::ToolsList => {
            state.open_tool_list();
            CommandAction::None
        }
        Command::ToolsEnable(tools) => {
            let message = state.permissions.enable_tools(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::ToolsDisable(tools) => {
            let message = state.permissions.disable_tools(tools);
            state.push_system_message(message);
            CommandAction::None
        }
        Command::Config => {
            state.open_config();
            CommandAction::None
        }
        Command::OutputStyle(style) => {
            let trimmed = style.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
                let message = state.permissions.set_output_style(None);
                state.push_system_message(message);
                return CommandAction::None;
            }

            match resolve_output_style(trimmed) {
                Ok(Some(_path)) => {
                    let message = state
                        .permissions
                        .set_output_style(Some(trimmed.to_string()));
                    state.push_system_message(message);
                }
                Ok(None) => state.push_system_message(format!(
                    "Output style not found: {}.",
                    trimmed
                )),
                Err(err) => state.push_system_message(format!(
                    "Failed to load output style: {}.",
                    err
                )),
            }
            CommandAction::None
        }
        Command::Mcp => {
            state.open_mcp_config();
            CommandAction::None
        }
        Command::McpReload => {
            state.reload_mcp_project_servers();
            if let Some(err) = &state.catalogs.mcp_project_error {
                state.push_system_message(format!("MCP config reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded MCP project config.".to_string());
            }
            CommandAction::None
        }
        Command::McpStatus => {
            state.request_mcp_status();
            CommandAction::None
        }
        Command::McpAdd { name, config } => {
            let trimmed_name = name.trim();
            if trimmed_name.is_empty() {
                state.push_system_message("MCP add requires a server name.".to_string());
                return CommandAction::None;
            }
            let config_text = config.trim();
            if config_text.is_empty() {
                state.push_system_message("MCP add requires a JSON config.".to_string());
                return CommandAction::None;
            }
            match serde_json::from_str::<Value>(config_text) {
                Ok(value) => {
                    let expanded = expand_env_vars_in_value(&value);
                    match parse_mcp_server_config(trimmed_name, &expanded) {
                        Ok(server) => {
                            state.catalogs.add_runtime_mcp_server(trimmed_name.to_string(), server);
                            state.push_system_message(format!(
                                "Added MCP server {} (applies next request).",
                                trimmed_name
                            ));
                        }
                        Err(err) => state.push_system_message(format!(
                            "Failed to add MCP server {}: {}",
                            trimmed_name, err
                        )),
                    }
                }
                Err(err) => state.push_system_message(format!(
                    "Failed to parse MCP server JSON: {}",
                    err
                )),
            }
            CommandAction::None
        }
        Command::McpRemove(name) => {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                state.push_system_message("MCP remove requires a server name.".to_string());
                return CommandAction::None;
            }
            state.catalogs.remove_mcp_server(trimmed);
            state.push_system_message(format!(
                "Disabled MCP server {} (applies next request).",
                trimmed
            ));
            CommandAction::None
        }
        Command::Agents => {
            state.open_agent_list();
            CommandAction::None
        }
        Command::AgentSelect(name) => {
            state.set_active_agent_by_name(&name);
            CommandAction::None
        }
        Command::AgentClear => {
            state.clear_active_agent();
            CommandAction::None
        }
        Command::AgentReload => {
            state.reload_agents();
            if let Some(err) = &state.catalogs.agent_load_error {
                state.push_system_message(format!("Agent reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded agents from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Skills => {
            state.open_skill_list();
            CommandAction::None
        }
        Command::SkillsReload => {
            state.reload_skills();
            if let Some(err) = &state.catalogs.skill_load_error {
                state.push_system_message(format!("Skill reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded skills from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Hooks => {
            state.open_hooks();
            CommandAction::None
        }
        Command::HooksReload => {
            state.reload_hooks();
            if let Some(err) = &state.catalogs.hook_load_error {
                state.push_system_message(format!("Hook reload warning: {}", err));
            } else {
                state.push_system_message("Reloaded hook scripts from disk.".to_string());
            }
            CommandAction::None
        }
        Command::Custom(name, args) => {
            if state.chat.is_thinking {
                state.push_system_message(
                    "Cannot run custom commands during an active request.".to_string(),
                );
                return CommandAction::None;
            }

            match load_custom_command(&name) {
                Ok(Some(template)) => {
                    let prompt = apply_custom_command_args(&template, &args);
                    CommandAction::SubmitPrompt(prompt)
                }
                Ok(None) => {
                    let mut message = format!("Unknown command: /{}", name);
                    if !args.is_empty() {
                        message.push(' ');
                        message.push_str(&args.join(" "));
                    }
                    state.push_system_message(message);
                    CommandAction::None
                }
                Err(err) => {
                    state.push_system_message(format!(
                        "Failed to load custom command /{}: {}.",
                        name, err
                    ));
                    CommandAction::None
                }
            }
        }
    }
}

fn handle_modal_input(state: &mut AppState, key: &WinitKey) -> bool {
    let empty_entries: Vec<McpServerEntry> = Vec::new();
    let mcp_entries = if matches!(state.modal_state, ModalState::McpConfig { .. }) {
        Some(state.catalogs.mcp_entries())
    } else {
        None
    };
    let settings_snapshot = SettingsSnapshot::from_state(state);
    match &mut state.modal_state {
        ModalState::ModelPicker { selected } => {
            let selected = *selected;
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    let models = ModelOption::all();
                    state.update_selected_model(models[selected]);
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if selected > 0 {
                        state.modal_state = ModalState::ModelPicker { selected: selected - 1 };
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if selected + 1 < ModelOption::all().len() {
                        state.modal_state = ModalState::ModelPicker { selected: selected + 1 };
                    }
                }
                WinitKey::Character(c) => {
                    match c.as_str() {
                        "1" => {
                            state.settings.selected_model = ModelOption::Opus;
                        }
                        "2" => {
                            state.settings.selected_model = ModelOption::Sonnet;
                        }
                        "3" => {
                            state.settings.selected_model = ModelOption::Haiku;
                        }
                        _ => {}
                    }
                    if matches!(c.as_str(), "1" | "2" | "3") {
                        state.update_selected_model(state.settings.selected_model);
                        state.modal_state = ModalState::None;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::SessionList { selected } => {
            let session_count = state.session.session_index.len();
            if session_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= session_count {
                *selected = session_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    if let Some(entry) = state.session.session_index.get(*selected).cloned() {
                        state.begin_session_resume(entry.id);
                    }
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < session_count {
                        *selected += 1;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::AgentList { selected } => {
            let agent_count = state.catalogs.agent_entries.len();
            if agent_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_agents();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= agent_count {
                *selected = agent_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Enter) => {
                    let selected_name = state.catalogs.agent_entries
                        .get(*selected)
                        .map(|entry| entry.name.clone());
                    if let Some(name) = selected_name {
                        state.set_active_agent_by_name(&name);
                    }
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < agent_count {
                        *selected += 1;
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_agents();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::SkillList { selected } => {
            let skill_count = state.catalogs.skill_entries.len();
            if skill_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_skills();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= skill_count {
                *selected = skill_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < skill_count {
                        *selected += 1;
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_skills();
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Hooks { view, selected } => {
            let mut sync_index = None;
            match key {
                WinitKey::Named(WinitNamedKey::Escape) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::Tab) => {
                    *view = match *view {
                        HookModalView::Config => HookModalView::Events,
                        HookModalView::Events => HookModalView::Config,
                    };
                    if *view == HookModalView::Events {
                        *selected = 0;
                        sync_index = Some(*selected);
                    }
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                    state.reload_hooks();
                }
                WinitKey::Character(c) if c.eq_ignore_ascii_case("c") => {
                    if *view == HookModalView::Events {
                        state.clear_hook_log();
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *view == HookModalView::Events && !state.catalogs.hook_event_log.is_empty() {
                        if *selected > 0 {
                            *selected -= 1;
                            sync_index = Some(*selected);
                        }
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *view == HookModalView::Events && !state.catalogs.hook_event_log.is_empty() {
                        if *selected + 1 < state.catalogs.hook_event_log.len() {
                            *selected += 1;
                            sync_index = Some(*selected);
                        }
                    }
                }
                WinitKey::Character(c) if *view == HookModalView::Config => match c.as_str() {
                    "1" => state.toggle_hook_setting(HookSetting::ToolBlocker),
                    "2" => state.toggle_hook_setting(HookSetting::ToolLogger),
                    "3" => state.toggle_hook_setting(HookSetting::OutputTruncator),
                    "4" => state.toggle_hook_setting(HookSetting::ContextInjection),
                    "5" => state.toggle_hook_setting(HookSetting::TodoEnforcer),
                    _ => {}
                },
                _ => {}
            }
            if let Some(index) = sync_index {
                state.sync_hook_inspector(index);
            }
            state.window.request_redraw();
            true
        }
        ModalState::ToolList { selected } => {
            let tool_count = state.session.session_info.tools.len();
            if tool_count == 0 {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= tool_count {
                *selected = tool_count - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < tool_count {
                        *selected += 1;
                    }
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::McpConfig { selected } => {
            let entries = mcp_entries.as_ref().unwrap_or(&empty_entries);
            if entries.is_empty() {
                match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Character(c) if c.eq_ignore_ascii_case("r") => {
                        state.reload_mcp_project_servers();
                    }
                    _ => {}
                }
                state.window.request_redraw();
                return true;
            }

            if *selected >= entries.len() {
                *selected = entries.len() - 1;
            }

            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                WinitKey::Named(WinitNamedKey::ArrowUp) => {
                    if *selected > 0 {
                        *selected -= 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::ArrowDown) => {
                    if *selected + 1 < entries.len() {
                        *selected += 1;
                    }
                }
                WinitKey::Named(WinitNamedKey::Delete | WinitNamedKey::Backspace) => {
                    if let Some(entry) = entries.get(*selected) {
                        state.catalogs.remove_mcp_server(&entry.name);
                    }
                }
                WinitKey::Character(c) => match c.as_str() {
                    "r" | "R" => {
                        state.reload_mcp_project_servers();
                    }
                    "s" | "S" => {
                        state.request_mcp_status();
                    }
                    _ => {}
                },
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::PermissionRules => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                    state.modal_state = ModalState::None;
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Help => {
            match key {
                WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter | WinitNamedKey::F1) => {
                    state.modal_state = ModalState::None;
                }
                _ => {}
            }
            state.window.request_redraw();
            true
        }
        ModalState::Config {
            tab,
            selected,
            search,
            input_mode,
        } => {
            let rows = settings_rows(&settings_snapshot, *tab, search);
            if rows.is_empty() {
                *selected = 0;
            } else if *selected >= rows.len() {
                *selected = rows.len().saturating_sub(1);
            }
            let current_item = rows.get(*selected).map(|row| row.item);
            let shift = state.modifiers.shift_key();
            let ctrl = state.modifiers.control_key();

            let mut change_tab = |forward: bool| {
                let tabs = SettingsTab::all();
                let current_index = tabs.iter().position(|entry| entry == tab).unwrap_or(0);
                let next_index = if forward {
                    (current_index + 1) % tabs.len()
                } else {
                    (current_index + tabs.len() - 1) % tabs.len()
                };
                *tab = tabs[next_index];
                *selected = 0;
            };

            match input_mode {
                SettingsInputMode::Search => match key {
                    WinitKey::Named(WinitNamedKey::Escape | WinitNamedKey::Enter) => {
                        *input_mode = SettingsInputMode::Normal;
                    }
                    WinitKey::Named(WinitNamedKey::Backspace) => {
                        search.pop();
                        *selected = 0;
                    }
                    WinitKey::Character(c) => {
                        search.push_str(c.as_str());
                        *selected = 0;
                    }
                    WinitKey::Named(WinitNamedKey::Tab) => {
                        *input_mode = SettingsInputMode::Normal;
                        change_tab(!shift);
                    }
                    _ => {}
                },
                SettingsInputMode::Capture(action) => match key {
                    WinitKey::Named(WinitNamedKey::Escape) => {
                        *input_mode = SettingsInputMode::Normal;
                    }
                    WinitKey::Named(WinitNamedKey::Backspace | WinitNamedKey::Delete) => {
                        state.settings.keybindings.retain(|binding| binding.action != *action);
                        save_keybindings(&state.settings.keybindings);
                        *input_mode = SettingsInputMode::Normal;
                    }
                    _ => {
                        if let Some(binding_key) = convert_key_for_binding(key) {
                            let modifiers = convert_modifiers(&state.modifiers);
                            state.settings.keybindings.retain(|binding| {
                                binding.action != *action
                                    && !(binding.key == binding_key && binding.modifiers == modifiers)
                            });
                            state.settings.keybindings.push(Keybinding {
                                key: binding_key,
                                modifiers,
                                action: *action,
                            });
                            save_keybindings(&state.settings.keybindings);
                        }
                        *input_mode = SettingsInputMode::Normal;
                    }
                },
                SettingsInputMode::Normal => match key {
                    WinitKey::Named(WinitNamedKey::Escape) => {
                        state.modal_state = ModalState::None;
                    }
                    WinitKey::Named(WinitNamedKey::Tab) => {
                        change_tab(!shift);
                    }
                    WinitKey::Named(WinitNamedKey::ArrowUp) => {
                        if *selected > 0 {
                            *selected -= 1;
                        }
                    }
                    WinitKey::Named(WinitNamedKey::ArrowDown) => {
                        if *selected + 1 < rows.len() {
                            *selected += 1;
                        }
                    }
                    WinitKey::Character(c) if (ctrl && c.eq_ignore_ascii_case("f")) || c == "/" => {
                        *input_mode = SettingsInputMode::Search;
                    }
                    WinitKey::Named(WinitNamedKey::ArrowLeft)
                    | WinitKey::Named(WinitNamedKey::ArrowRight)
                    | WinitKey::Named(WinitNamedKey::Enter) => {
                        let forward = !matches!(key, WinitKey::Named(WinitNamedKey::ArrowLeft));
                        if let Some(item) = current_item {
                            match item {
                                SettingsItem::Theme => {
                                    state.settings.coder_settings.theme = if state.settings.coder_settings.theme == ThemeSetting::Dark {
                                        ThemeSetting::Light
                                    } else {
                                        ThemeSetting::Dark
                                    };
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::FontSize => {
                                    let delta = if forward { 1.0 } else { -1.0 };
                                    state.settings.coder_settings.font_size =
                                        clamp_font_size(state.settings.coder_settings.font_size + delta);
                                    state.apply_settings();
                                    state.persist_settings();
                                }
                                SettingsItem::AutoScroll => {
                                    state.settings.coder_settings.auto_scroll = !state.settings.coder_settings.auto_scroll;
                                    state.persist_settings();
                                }
                                SettingsItem::DefaultModel => {
                                    let next = cycle_model(state.settings.selected_model, forward);
                                    state.update_selected_model(next);
                                }
                                SettingsItem::MaxThinkingTokens => {
                                    const THINKING_STEP: u32 = 256;
                                    const THINKING_MAX: u32 = 8192;
                                    let current = state.settings.coder_settings.max_thinking_tokens.unwrap_or(0);
                                    let next = if forward {
                                        let value = current.saturating_add(THINKING_STEP).min(THINKING_MAX);
                                        Some(value)
                                    } else if current <= THINKING_STEP {
                                        None
                                    } else {
                                        Some(current - THINKING_STEP)
                                    };
                                    state.settings.coder_settings.max_thinking_tokens = next;
                                    state.persist_settings();
                                }
                                SettingsItem::PermissionMode => {
                                    let next = cycle_coder_mode_standalone(state.permissions.coder_mode, forward);
                                    state.permissions.coder_mode = next;
                                    state.permissions.permission_default_allow =
                                        coder_mode_default_allow(next, state.permissions.permission_default_allow);
                                    state.session.session_info.permission_mode =
                                        coder_mode_label(next).to_string();
                                    state.permissions.persist_permission_config();
                                }
                                SettingsItem::PermissionDefaultAllow => {
                                    state.permissions.permission_default_allow = !state.permissions.permission_default_allow;
                                    state.permissions.persist_permission_config();
                                }
                                SettingsItem::PermissionRules
                                | SettingsItem::PermissionAllowList
                                | SettingsItem::PermissionDenyList
                                | SettingsItem::PermissionBashAllowList
                                | SettingsItem::PermissionBashDenyList => {
                                    state.open_permission_rules();
                                }
                                SettingsItem::SessionAutoSave => {
                                    state.settings.coder_settings.session_auto_save = !state.settings.coder_settings.session_auto_save;
                                    state.persist_settings();
                                    if state.settings.coder_settings.session_auto_save {
                                        state.apply_session_history_limit();
                                    }
                                }
                                SettingsItem::SessionHistoryLimit => {
                                    const HISTORY_STEP: usize = 10;
                                    const HISTORY_MAX: usize = 500;
                                    let current = state.settings.coder_settings.session_history_limit;
                                    let next = if forward {
                                        if current == 0 {
                                            HISTORY_STEP
                                        } else {
                                            (current + HISTORY_STEP).min(HISTORY_MAX)
                                        }
                                    } else if current <= HISTORY_STEP {
                                        0
                                    } else {
                                        current - HISTORY_STEP
                                    };
                                    state.settings.coder_settings.session_history_limit = next;
                                    state.persist_settings();
                                    state.apply_session_history_limit();
                                }
                                SettingsItem::SessionStoragePath | SettingsItem::McpSummary => {}
                                SettingsItem::McpOpenConfig => {
                                    state.open_mcp_config();
                                }
                                SettingsItem::McpReloadProject => {
                                    state.reload_mcp_project_servers();
                                    if let Some(err) = &state.catalogs.mcp_project_error {
                                        state.push_system_message(format!(
                                            "MCP reload warning: {}",
                                            err
                                        ));
                                    } else {
                                        state.push_system_message(
                                            "Reloaded MCP project config.".to_string(),
                                        );
                                    }
                                }
                                SettingsItem::McpRefreshStatus => {
                                    state.request_mcp_status();
                                }
                                SettingsItem::HookToolBlocker => {
                                    state.toggle_hook_setting(HookSetting::ToolBlocker);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookToolLogger => {
                                    state.toggle_hook_setting(HookSetting::ToolLogger);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookOutputTruncator => {
                                    state.toggle_hook_setting(HookSetting::OutputTruncator);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookContextInjection => {
                                    state.toggle_hook_setting(HookSetting::ContextInjection);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookTodoEnforcer => {
                                    state.toggle_hook_setting(HookSetting::TodoEnforcer);
                                    save_hook_config(&state.catalogs.hook_config);
                                }
                                SettingsItem::HookOpenPanel => {
                                    state.open_hooks();
                                }
                                SettingsItem::Keybinding(action) => {
                                    *input_mode = SettingsInputMode::Capture(action);
                                }
                                SettingsItem::KeybindingReset => {
                                    state.settings.keybindings = default_keybindings();
                                    save_keybindings(&state.settings.keybindings);
                                }
                            }
                        }
                    }
                    _ => {}
                },
            }
            state.window.request_redraw();
            true
        }
        ModalState::None => false,
    }
}

fn cycle_model(current: ModelOption, forward: bool) -> ModelOption {
    let models = ModelOption::all();
    let idx = models
        .iter()
        .position(|model| *model == current)
        .unwrap_or(0);
    let next = if forward {
        (idx + 1) % models.len()
    } else {
        (idx + models.len() - 1) % models.len()
    };
    models[next]
}

fn cycle_coder_mode_standalone(current: CoderMode, forward: bool) -> CoderMode {
    let modes = [
        CoderMode::BypassPermissions,
        CoderMode::Plan,
        CoderMode::Autopilot,
    ];
    let idx = match current {
        CoderMode::BypassPermissions => 0,
        CoderMode::Plan => 1,
        CoderMode::Autopilot => 2,
    };
    let next = if forward {
        (idx + 1) % modes.len()
    } else {
        (idx + modes.len() - 1) % modes.len()
    };
    modes[next]
}

fn coder_mode_display(mode: CoderMode) -> &'static str {
    match mode {
        CoderMode::BypassPermissions => "bypass permissions",
        CoderMode::Plan => "plan mode",
        CoderMode::Autopilot => "autopilot mode",
    }
}

fn coder_mode_color(mode: CoderMode, _palette: &UiPalette) -> Hsla {
    match mode {
        CoderMode::BypassPermissions => Hsla::new(0.0, 0.65, 0.65, 1.0), // lighter red/salmon
        CoderMode::Plan => Hsla::from_hex(0x00D5FF), // vivid teal/cyan
        CoderMode::Autopilot => Hsla::from_hex(0x39FF14), // neon green
    }
}

fn hook_event_from_input(input: &HookInput) -> HookEvent {
    match input {
        HookInput::PreToolUse(_) => HookEvent::PreToolUse,
        HookInput::PostToolUse(_) => HookEvent::PostToolUse,
        HookInput::PostToolUseFailure(_) => HookEvent::PostToolUseFailure,
        HookInput::Notification(_) => HookEvent::Notification,
        HookInput::UserPromptSubmit(_) => HookEvent::UserPromptSubmit,
        HookInput::SessionStart(_) => HookEvent::SessionStart,
        HookInput::SessionEnd(_) => HookEvent::SessionEnd,
        HookInput::Stop(_) => HookEvent::Stop,
        HookInput::SubagentStart(_) => HookEvent::SubagentStart,
        HookInput::SubagentStop(_) => HookEvent::SubagentStop,
        HookInput::PreCompact(_) => HookEvent::PreCompact,
        HookInput::PermissionRequest(_) => HookEvent::PermissionRequest,
    }
}

fn hook_base_input(input: &HookInput) -> &BaseHookInput {
    match input {
        HookInput::PreToolUse(hook) => &hook.base,
        HookInput::PostToolUse(hook) => &hook.base,
        HookInput::PostToolUseFailure(hook) => &hook.base,
        HookInput::Notification(hook) => &hook.base,
        HookInput::UserPromptSubmit(hook) => &hook.base,
        HookInput::SessionStart(hook) => &hook.base,
        HookInput::SessionEnd(hook) => &hook.base,
        HookInput::Stop(hook) => &hook.base,
        HookInput::SubagentStart(hook) => &hook.base,
        HookInput::SubagentStop(hook) => &hook.base,
        HookInput::PreCompact(hook) => &hook.base,
        HookInput::PermissionRequest(hook) => &hook.base,
    }
}

fn hook_tool_name(input: &HookInput) -> Option<String> {
    match input {
        HookInput::PreToolUse(hook) => Some(hook.tool_name.clone()),
        HookInput::PostToolUse(hook) => Some(hook.tool_name.clone()),
        HookInput::PostToolUseFailure(hook) => Some(hook.tool_name.clone()),
        HookInput::PermissionRequest(hook) => Some(hook.tool_name.clone()),
        _ => None,
    }
}

fn hook_tool_input(input: &HookInput) -> Option<&Value> {
    match input {
        HookInput::PreToolUse(hook) => Some(&hook.tool_input),
        HookInput::PostToolUse(hook) => Some(&hook.tool_input),
        HookInput::PostToolUseFailure(hook) => Some(&hook.tool_input),
        HookInput::PermissionRequest(hook) => Some(&hook.tool_input),
        _ => None,
    }
}

fn hook_tool_response(input: &HookInput) -> Option<&Value> {
    match input {
        HookInput::PostToolUse(hook) => Some(&hook.tool_response),
        _ => None,
    }
}

fn hook_tool_error(input: &HookInput) -> Option<&str> {
    match input {
        HookInput::PostToolUseFailure(hook) => Some(hook.error.as_str()),
        _ => None,
    }
}

fn hook_tool_blocker(input: &HookInput) -> (HookOutput, String) {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    let mut summary = format!("ToolBlocker allowed {}.", tool_name);
    let mut sync = SyncHookOutput::continue_execution();

    let is_bash = tool_name.eq_ignore_ascii_case("bash");
    if !is_bash {
        return (HookOutput::Sync(sync), summary);
    }

    let Some(tool_input) = hook_tool_input(input) else {
        return (HookOutput::Sync(sync), summary);
    };
    let Some(command) = extract_bash_command(tool_input) else {
        return (HookOutput::Sync(sync), summary);
    };

    let lowered = command.to_ascii_lowercase();
    for pattern in HOOK_BLOCK_PATTERNS {
        if lowered.contains(&pattern.to_ascii_lowercase()) {
            let reason = format!(
                "Blocked dangerous command: {}",
                truncate_preview(&command, 160)
            );
            sync = SyncHookOutput {
                continue_execution: Some(false),
                decision: Some(HookDecision::Block),
                reason: Some(reason),
                ..Default::default()
            };
            summary = format!("ToolBlocker blocked {}.", tool_name);
            break;
        }
    }

    (HookOutput::Sync(sync), summary)
}

fn hook_tool_logger_summary(input: &HookInput) -> String {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    match hook_event_from_input(input) {
        HookEvent::PreToolUse => format!("ToolLogger pre {}.", tool_name),
        HookEvent::PostToolUse => format!("ToolLogger post {}.", tool_name),
        HookEvent::PostToolUseFailure => {
            if let Some(error) = hook_tool_error(input) {
                format!(
                    "ToolLogger failure {}: {}",
                    tool_name,
                    truncate_preview(error, 120)
                )
            } else {
                format!("ToolLogger failure {}.", tool_name)
            }
        }
        event => format!("ToolLogger {}.", hook_event_label(event)),
    }
}

fn hook_output_truncator(input: &HookInput) -> (HookOutput, String) {
    let tool_name = hook_tool_name(input).unwrap_or_else(|| "unknown".to_string());
    let Some(tool_response) = hook_tool_response(input) else {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            format!("OutputTruncator skipped {}.", tool_name),
        );
    };

    let response_text =
        serde_json::to_string(tool_response).unwrap_or_else(|_| tool_response.to_string());
    let response_len = response_text.len();
    if response_len <= HOOK_OUTPUT_TRUNCATE {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            format!("OutputTruncator ok for {}.", tool_name),
        );
    }

    let truncated = truncate_bytes(response_text, HOOK_OUTPUT_TRUNCATE);
    let mut sync = SyncHookOutput::continue_execution();
    sync.suppress_output = Some(true);
    sync.hook_specific_output = Some(HookSpecificOutput::PostToolUse(
        PostToolUseSpecificOutput {
            hook_event_name: HookEvent::PostToolUse.as_str().to_string(),
            additional_context: Some(format!(
                "Tool output truncated ({} bytes):\n{}",
                response_len, truncated
            )),
            updated_mcp_tool_output: None,
        },
    ));

    (
        HookOutput::Sync(sync),
        format!("OutputTruncator truncated {} output.", tool_name),
    )
}

fn hook_context_sources(config: &HookConfig) -> Vec<String> {
    let mut sources = Vec::new();
    if config.context_injection {
        sources.push("builtin:context_injection".to_string());
    }
    if config.todo_enforcer {
        sources.push("builtin:todo_enforcer".to_string());
    }
    sources
}

fn hook_context_enforcer(
    runtime: &HookRuntimeConfig,
    input: &HookInput,
) -> (HookOutput, String) {
    let event = hook_event_from_input(input);
    let mut sections = Vec::new();

    if runtime.config.context_injection {
        if let Some(context) = build_context_injection(&runtime.cwd) {
            sections.push(context);
        }
    }
    if runtime.config.todo_enforcer {
        if let Some(todo) = build_todo_context(&runtime.cwd) {
            sections.push(todo);
        }
    }

    if sections.is_empty() {
        return (
            HookOutput::Sync(SyncHookOutput::continue_execution()),
            "ContextEnforcer no context.".to_string(),
        );
    }

    let combined = sections.join("\n\n");
    let combined_len = combined.len();
    let hook_specific_output = match event {
        HookEvent::UserPromptSubmit => HookSpecificOutput::UserPromptSubmit(
            UserPromptSubmitSpecificOutput {
                hook_event_name: HookEvent::UserPromptSubmit.as_str().to_string(),
                additional_context: Some(combined),
            },
        ),
        HookEvent::SessionStart => HookSpecificOutput::SessionStart(SessionStartSpecificOutput {
            hook_event_name: HookEvent::SessionStart.as_str().to_string(),
            additional_context: Some(combined),
        }),
        _ => {
            return (
                HookOutput::Sync(SyncHookOutput::continue_execution()),
                "ContextEnforcer skipped.".to_string(),
            )
        }
    };

    let mut sync = SyncHookOutput::continue_execution();
    sync.hook_specific_output = Some(hook_specific_output);
    (
        HookOutput::Sync(sync),
        format!("ContextEnforcer injected {} bytes.", combined_len),
    )
}

fn hook_script_source_label(entry: &HookScriptEntry) -> String {
    let source = match entry.source {
        HookScriptSource::Project => "project",
        HookScriptSource::User => "user",
    };
    format!("script:{}:{}", source, entry.path.display())
}

fn hook_script_env(input: &HookInput, tool_use_id: Option<&str>) -> Vec<(String, String)> {
    let base = hook_base_input(input);
    let event = hook_event_from_input(input);
    let mut envs = vec![
        (
            "CLAUDE_HOOK_EVENT".to_string(),
            hook_event_label(event).to_string(),
        ),
        ("CLAUDE_SESSION_ID".to_string(), base.session_id.clone()),
        (
            "CLAUDE_TRANSCRIPT_PATH".to_string(),
            base.transcript_path.clone(),
        ),
        ("CLAUDE_CWD".to_string(), base.cwd.clone()),
    ];
    if let Some(mode) = &base.permission_mode {
        envs.push(("CLAUDE_PERMISSION_MODE".to_string(), mode.clone()));
    }
    if let Some(tool_name) = hook_tool_name(input) {
        envs.push(("CLAUDE_TOOL_NAME".to_string(), tool_name));
    }
    if let Some(tool_use_id) = tool_use_id {
        envs.push(("CLAUDE_TOOL_USE_ID".to_string(), tool_use_id.to_string()));
    }
    envs
}

async fn run_hook_script(
    entry: &HookScriptEntry,
    input: &HookInput,
    tool_use_id: Option<&str>,
    runtime: &HookRuntimeConfig,
) -> Result<HookOutput, String> {
    let mut command = TokioCommand::new(&entry.path);
    command
        .current_dir(&runtime.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in hook_script_env(input, tool_use_id) {
        command.env(key, value);
    }

    let mut child = command.spawn().map_err(|err| {
        format!(
            "Failed to spawn hook script {}: {}",
            entry.path.display(),
            err
        )
    })?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = serde_json::to_vec(input)
            .map_err(|err| format!("Failed to serialize hook input: {}", err))?;
        stdin
            .write_all(&payload)
            .await
            .map_err(|err| format!("Failed to write hook input: {}", err))?;
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let stdout_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(mut stdout) = stdout {
            let _ = stdout.read_to_end(&mut buffer).await;
        }
        buffer
    });
    let stderr_task = tokio::spawn(async move {
        let mut buffer = Vec::new();
        if let Some(mut stderr) = stderr {
            let _ = stderr.read_to_end(&mut buffer).await;
        }
        buffer
    });

    let status = match timeout(Duration::from_secs(HOOK_SCRIPT_TIMEOUT_SECS), child.wait()).await {
        Ok(status) => status.map_err(|err| format!("Hook script failed: {}", err))?,
        Err(_) => {
            let _ = child.kill().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(format!(
                "Hook script {} timed out after {}s.",
                entry.path.display(),
                HOOK_SCRIPT_TIMEOUT_SECS
            ));
        }
    };

    let stdout_bytes = stdout_task
        .await
        .unwrap_or_default();
    let stderr_bytes = stderr_task
        .await
        .unwrap_or_default();

    let stdout_text = String::from_utf8_lossy(&stdout_bytes).trim().to_string();
    let stderr_text = String::from_utf8_lossy(&stderr_bytes).trim().to_string();

    if !status.success() {
        let mut message = format!("Hook script exited with status {}", status);
        if !stderr_text.is_empty() {
            message.push_str(": ");
            message.push_str(&stderr_text);
        }
        return Err(message);
    }

    if stdout_text.is_empty() {
        return Ok(HookOutput::Sync(SyncHookOutput::continue_execution()));
    }

    serde_json::from_str::<HookOutput>(&stdout_text).map_err(|err| {
        format!(
            "Failed to parse hook output: {} (stdout: {})",
            err,
            truncate_preview(&stdout_text, 160)
        )
    })
}

fn truncate_hook_value(value: Value, max_bytes: usize) -> Value {
    match value {
        Value::String(text) => {
            if text.len() <= max_bytes {
                Value::String(text)
            } else {
                Value::String(truncate_bytes(text, max_bytes))
            }
        }
        other => {
            let raw = serde_json::to_string(&other).unwrap_or_else(|_| other.to_string());
            if raw.len() <= max_bytes {
                other
            } else {
                Value::String(truncate_bytes(raw, max_bytes))
            }
        }
    }
}

fn serialize_hook_value<T: Serialize>(value: &T, max_bytes: usize) -> Value {
    let serialized = serde_json::to_value(value).unwrap_or(Value::Null);
    truncate_hook_value(serialized, max_bytes)
}

fn log_hook_event(
    runtime: &HookRuntimeConfig,
    event: HookEvent,
    summary: String,
    tool_name: Option<String>,
    matcher: Option<String>,
    input: &HookInput,
    output: Option<&HookOutput>,
    error: Option<String>,
    sources: Vec<String>,
) {
    let id = format!(
        "hook-{}-{}",
        hook_event_label(event).to_ascii_lowercase(),
        runtime.counter.fetch_add(1, Ordering::SeqCst)
    );
    let entry = HookLogEntry {
        id,
        event,
        timestamp: now_timestamp(),
        summary,
        tool_name,
        matcher,
        input: serialize_hook_value(input, HOOK_OUTPUT_TRUNCATE),
        output: output.map(|value| serialize_hook_value(value, HOOK_OUTPUT_TRUNCATE)),
        error,
        sources,
    };
    let _ = runtime.log_tx.send(ResponseEvent::HookLog(entry));
}

fn hook_event_label(event: HookEvent) -> &'static str {
    event.as_str()
}

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

fn hook_log_event_data(entry: &HookLogEntry) -> EventData {
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

fn resolve_output_style(name: &str) -> io::Result<Option<PathBuf>> {
    if name.trim().is_empty() {
        return Ok(None);
    }

    let file_name = if name.ends_with(".md") {
        name.to_string()
    } else {
        format!("{}.md", name)
    };

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".claude").join("output-styles").join(&file_name));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude").join("output-styles").join(&file_name));
    }

    for path in candidates {
        if path.is_file() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn resolve_custom_command_path(name: &str) -> io::Result<Option<PathBuf>> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let file_name = if trimmed.ends_with(".md") {
        trimmed.to_string()
    } else {
        format!("{}.md", trimmed)
    };

    let mut candidates = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join(".claude").join("commands").join(&file_name));
    }
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".claude").join("commands").join(&file_name));
    }

    for path in candidates {
        if path.is_file() {
            return Ok(Some(path));
        }
    }

    Ok(None)
}

fn load_custom_command(name: &str) -> io::Result<Option<String>> {
    let Some(path) = resolve_custom_command_path(name)? else {
        return Ok(None);
    };
    let content = fs::read_to_string(path)?;
    Ok(Some(content))
}

fn apply_custom_command_args(template: &str, args: &[String]) -> String {
    if args.is_empty() {
        return template.to_string();
    }
    let joined = args.join(" ");
    if template.contains("{{args}}") {
        template.replace("{{args}}", &joined)
    } else {
        format!("{}\n\n{}", template.trim_end(), joined)
    }
}

fn export_session_markdown(state: &AppState) -> io::Result<PathBuf> {
    let export_dir = config_dir().join("exports");
    fs::create_dir_all(&export_dir)?;
    let session_id = if state.session.session_info.session_id.is_empty() {
        "session".to_string()
    } else {
        state.session.session_info.session_id.clone()
    };
    let filename = format!("{}-{}.md", session_id, now_timestamp());
    let path = export_dir.join(filename);
    let mut file = fs::File::create(&path)?;

    writeln!(file, "# Coder Session {}", session_id)?;
    if !state.session.session_info.model.is_empty() {
        writeln!(file, "- Model: {}", state.session.session_info.model)?;
    }
    writeln!(file, "- Exported: {}", now_timestamp())?;
    writeln!(file)?;

    for message in &state.chat.messages {
        match message.role {
            MessageRole::User => {
                for line in message.content.lines() {
                    writeln!(file, "> {}", line)?;
                }
                writeln!(file)?;
            }
            MessageRole::Assistant => {
                writeln!(file, "{}", message.content)?;
                writeln!(file)?;
            }
        }
    }

    Ok(path)
}

fn open_url(url: &str) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = ProcessCommand::new("open");
        cmd.arg(url);
        cmd
    };

    #[cfg(target_os = "linux")]
    let mut command = {
        let mut cmd = ProcessCommand::new("xdg-open");
        cmd.arg(url);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = ProcessCommand::new("cmd");
        cmd.args(["/C", "start", url]);
        cmd
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}
