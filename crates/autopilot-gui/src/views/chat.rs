use std::cell::RefCell;
use std::rc::Rc;

use autopilot::metrics::SessionMetrics;
use wgpui::components::atoms::{Model, StreamingIndicator};
use wgpui::components::molecules::{MessageHeader, ThinkingBlock};
use wgpui::components::organisms::ToolCallCard;
use wgpui::components::sections::MessageEditor;
use wgpui::components::{Component, Dropdown, DropdownOption, Tab, Tabs};
use wgpui::scroll::{calculate_scrollbar_thumb, ScrollContainer};
use wgpui::{Bounds, EventContext, EventResult, InputEvent, Point, Quad, Size, Text, theme};

use crate::backend::BackendCommand;
use crate::state::{AppState, ChatEntry, ToolCallData};
use crate::views::fit_text;

const ENTRY_GAP: f32 = 12.0;
const HEADER_LINE_HEIGHT: f32 = 18.0;
const THINKING_TOGGLE_HEIGHT: f32 = 24.0;
const THINKING_MAX_COLLAPSED_LINES: usize = 3;
const SESSION_TAB_HEIGHT: f32 = 28.0;
const SESSION_TAB_GAP: f32 = 8.0;
const SESSION_TAB_FONT_SIZE: f32 = theme::font_size::XS;
const SESSION_TAB_PADDING_H: f32 = 12.0;
const SESSION_TAB_PADDING_V: f32 = 6.0;
const SESSION_TAB_LIMIT: usize = 6;
const SESSION_TAB_LABEL_MAX: usize = 24;
const AGENT_LABEL: &str = "Agent";
const AGENT_LABEL_WIDTH: f32 = 60.0;
const AGENT_ROW_GAP: f32 = 8.0;

pub struct ChatView {
    state: Rc<RefCell<AppState>>,
    command_tx: std::sync::mpsc::Sender<BackendCommand>,
    editor: MessageEditor,
    scroll: ScrollContainer,
    dragging_scrollbar: bool,
    auto_scroll: bool,
    last_revision: u64,
    last_width: f32,
    layout_entries: Vec<ChatLayoutEntry>,
    content_height: f32,
    streaming_indicator: StreamingIndicator,
    session_tabs: Tabs,
    session_tab_ids: Vec<String>,
    session_tab_active_id: Option<String>,
    agent_dropdown: Dropdown,
    agent_values: Vec<String>,
}

struct ChatLayoutEntry {
    entry: ChatEntry,
    lines: Vec<String>,
    height: f32,
    /// Index in the original chat entries list (for thinking toggle state)
    index: usize,
    thinking_block: Option<ThinkingBlock>,
    thinking_height: f32,
}

struct AgentLayout {
    row: Bounds,
    label: Bounds,
    dropdown: Bounds,
}

impl ChatView {
    pub fn new(
        state: Rc<RefCell<AppState>>,
        command_tx: std::sync::mpsc::Sender<BackendCommand>,
    ) -> Self {
        let tx = command_tx.clone();
        let editor = MessageEditor::new()
            .placeholder("Send a new task prompt...")
            .show_mode_badge(false)
            .show_keybinding_hint(true)
            .on_send(move |prompt| {
                let _ = tx.send(BackendCommand::RunPrompt { prompt });
            })
            .on_cancel({
                let tx = command_tx.clone();
                move || {
                    let _ = tx.send(BackendCommand::AbortPrompt);
                }
            });

        let agent_values = vec![
            "claude".to_string(),
            "codex".to_string(),
            "gpt-oss".to_string(),
            "fm-bridge".to_string(),
        ];
        let agent_options = vec![
            DropdownOption::new("Claude", "claude"),
            DropdownOption::new("Codex", "codex"),
            DropdownOption::new("GPT-OSS", "gpt-oss"),
            DropdownOption::new("FM-Bridge", "fm-bridge"),
        ];
        let selected_index = agent_values
            .iter()
            .position(|value| value == &state.borrow().agent)
            .unwrap_or(0);
        let agent_state = state.clone();
        let agent_tx = command_tx.clone();
        let agent_dropdown = Dropdown::new(agent_options)
            .selected(selected_index)
            .font_size(theme::font_size::XS)
            .padding(theme::spacing::SM, theme::spacing::XS)
            .on_change(move |_index, value| {
                let agent = value.to_string();
                agent_state.borrow_mut().agent = agent.clone();
                let _ = agent_tx.send(BackendCommand::SetAgent { agent });
            });

        Self {
            state,
            command_tx,
            editor,
            scroll: ScrollContainer::vertical(Bounds::ZERO),
            dragging_scrollbar: false,
            auto_scroll: true,
            last_revision: 0,
            last_width: 0.0,
            layout_entries: Vec::new(),
            content_height: 0.0,
            streaming_indicator: StreamingIndicator::new(),
            session_tabs: Tabs::new(Vec::new()),
            session_tab_ids: Vec::new(),
            session_tab_active_id: None,
            agent_dropdown,
            agent_values,
        }
    }

    pub fn tick(&mut self, _delta: std::time::Duration) {
        if self
            .layout_entries
            .iter()
            .any(|entry| matches!(entry.entry, ChatEntry::Assistant { streaming: true, .. }))
        {
            self.streaming_indicator.tick();
        }
    }

    fn refresh_session_tabs(&mut self) {
        let state = self.state.borrow();
        let tab_sessions = session_tabs_for_state(&state);
        let mut session_ids = Vec::with_capacity(tab_sessions.len());
        let tabs: Vec<Tab> = tab_sessions
            .iter()
            .map(|session| {
                session_ids.push(session.id.clone());
                Tab::new(session_tab_label(session))
            })
            .collect();

        let active_id = state.log_session_id.clone();
        if session_ids == self.session_tab_ids && active_id == self.session_tab_active_id {
            return;
        }

        let active_index = active_id
            .as_ref()
            .and_then(|id| session_ids.iter().position(|tab_id| tab_id == id))
            .unwrap_or(0);

        let tx = self.command_tx.clone();
        let ids_for_callback = session_ids.clone();
        let tabs = Tabs::new(tabs)
            .active(active_index)
            .tab_height(SESSION_TAB_HEIGHT)
            .tab_padding(SESSION_TAB_PADDING_H, SESSION_TAB_PADDING_V)
            .font_size(SESSION_TAB_FONT_SIZE)
            .on_change(move |index| {
                if let Some(session_id) = ids_for_callback.get(index) {
                    let _ = tx.send(BackendCommand::SelectSession {
                        session_id: session_id.clone(),
                    });
                }
            });

        self.session_tabs = tabs;
        self.session_tab_ids = session_ids;
        self.session_tab_active_id = active_id;
    }

    fn sync_agent_dropdown(&mut self) {
        let agent = self.state.borrow().agent.clone();
        if self.agent_dropdown.selected_value() == Some(agent.as_str()) {
            return;
        }
        let index = self.agent_values.iter().position(|value| value == &agent);
        self.agent_dropdown.set_selected(index);
    }

    fn agent_layout(&self, bounds: Bounds, editor_height: f32) -> AgentLayout {
        let padding = theme::spacing::MD;
        let dropdown_height = self
            .agent_dropdown
            .size_hint()
            .1
            .unwrap_or(theme::font_size::XS * 1.4 + theme::spacing::XS * 2.0);
        let row_y = bounds.origin.y + bounds.size.height - editor_height - dropdown_height - AGENT_ROW_GAP;
        let row = Bounds::new(bounds.origin.x, row_y, bounds.size.width, dropdown_height);

        let label_width = AGENT_LABEL_WIDTH.min(row.size.width - padding * 2.0).max(0.0);
        let label = Bounds::new(
            row.origin.x + padding,
            row.origin.y,
            label_width,
            row.size.height,
        );

        let dropdown_x = label.origin.x + label_width + theme::spacing::SM;
        let dropdown_width = (row.size.width - padding * 2.0 - label_width - theme::spacing::SM)
            .max(0.0);
        let dropdown = Bounds::new(dropdown_x, row.origin.y, dropdown_width, row.size.height);

        AgentLayout { row, label, dropdown }
    }

    fn header_layout(&self, bounds: Bounds) -> (Bounds, Option<Bounds>, f32) {
        let padding = theme::spacing::SM;
        let has_tabs = self.session_tabs.tab_count() > 0;
        let tabs_height = if has_tabs { SESSION_TAB_HEIGHT } else { 0.0 };
        let tabs_gap = if has_tabs { SESSION_TAB_GAP } else { 0.0 };
        let header_height = padding * 2.0 + tabs_height + tabs_gap + HEADER_LINE_HEIGHT * 2.0;
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            header_height,
        );
        let tabs_bounds = if has_tabs {
            Some(Bounds::new(
                bounds.origin.x,
                bounds.origin.y + padding,
                bounds.size.width,
                tabs_height,
            ))
        } else {
            None
        };
        let info_origin_y = if let Some(tabs_bounds) = tabs_bounds {
            tabs_bounds.origin.y + tabs_bounds.size.height + tabs_gap
        } else {
            bounds.origin.y + padding
        };

        (header_bounds, tabs_bounds, info_origin_y)
    }

    fn rebuild_layout(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        self.layout_entries.clear();
        self.content_height = 0.0;

        for (index, entry) in state.chat_entries.iter().enumerate() {
            let (lines, height, thinking_block, thinking_height) = match entry {
                ChatEntry::User { text, .. } => {
                    let lines = wrap_lines(cx, text, theme::font_size::BASE, available_width);
                    let height = text_entry_height(&lines, true, false);
                    (lines, height, None, 0.0)
                }
                ChatEntry::Assistant { text, streaming, .. } => {
                    let (thinking_content, visible_text) = parse_thinking_block(text);
                    let lines = if visible_text.trim().is_empty() {
                        Vec::new()
                    } else {
                        wrap_lines(cx, &visible_text, theme::font_size::BASE, available_width)
                    };

                    let thinking_expanded = state.is_thinking_expanded(index);
                    let (thinking_block, thinking_height) = if let Some(thinking_content) =
                        thinking_content
                    {
                        let trimmed = thinking_content.trim();
                        if trimmed.is_empty() {
                            (None, 0.0)
                        } else {
                            let thinking_lines = wrap_lines(
                                cx,
                                trimmed,
                                theme::font_size::SM,
                                available_width,
                            );
                            let thinking_text = thinking_lines.join("\n");
                            let thinking_block = ThinkingBlock::new(thinking_text)
                                .expanded(thinking_expanded)
                                .max_collapsed_lines(THINKING_MAX_COLLAPSED_LINES);
                            let thinking_height =
                                thinking_block_height(thinking_lines.len(), thinking_expanded);
                            (Some(thinking_block), thinking_height)
                        }
                    } else {
                        (None, 0.0)
                    };

                    let height = assistant_entry_height(lines.len(), thinking_height, *streaming);
                    (lines, height, thinking_block, thinking_height)
                }
                ChatEntry::System { text, .. } => {
                    let lines = wrap_lines(cx, text, theme::font_size::SM, available_width);
                    let height = system_entry_height(&lines);
                    (lines, height, None, 0.0)
                }
                ChatEntry::ToolCall(tool) => {
                    let height = tool_entry_height(tool);
                    (Vec::new(), height, None, 0.0)
                }
            };

            self.layout_entries.push(ChatLayoutEntry {
                entry: entry.clone(),
                lines,
                height,
                index,
                thinking_block,
                thinking_height,
            });

            self.content_height += height + ENTRY_GAP;
        }

        if self.content_height > 0.0 {
            self.content_height -= ENTRY_GAP;
        }
    }

    fn paint_header(
        &mut self,
        bounds: Bounds,
        info_origin_y: f32,
        cx: &mut wgpui::PaintContext,
    ) {
        let state = self.state.borrow();
        let padding = theme::spacing::SM;
        let mut y = info_origin_y;

        let session = state
            .log_session_id
            .as_ref()
            .map(|id| format!("Session: {}", id))
            .unwrap_or_else(|| "Session: none".to_string());
        let session = fit_text(cx, &session, theme::font_size::XS, bounds.size.width);
        let mut session_text = Text::new(session)
            .font_size(theme::font_size::XS)
            .color(theme::text::MUTED);
        session_text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                HEADER_LINE_HEIGHT,
            ),
            cx,
        );
        y += HEADER_LINE_HEIGHT;

        if let Some(prompt) = state.prompt_last.as_ref() {
            let prompt = format!("Prompt: {}", prompt);
            let prompt = fit_text(cx, &prompt, theme::font_size::XS, bounds.size.width);
            let mut prompt_text = Text::new(prompt)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            prompt_text.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    HEADER_LINE_HEIGHT,
                ),
                cx,
            );
        }
    }

    fn handle_thinking_event(
        &mut self,
        event: &InputEvent,
        list_bounds: Bounds,
        cx: &mut EventContext,
    ) -> EventResult {
        let scroll_offset = self.scroll.scroll_offset.y;
        let mut y = list_bounds.origin.y - scroll_offset;

        for entry in &mut self.layout_entries {
            let entry_bounds =
                Bounds::new(list_bounds.origin.x, y, list_bounds.size.width, entry.height);
            if entry_bounds.origin.y + entry_bounds.size.height >= list_bounds.origin.y
                && entry_bounds.origin.y <= list_bounds.origin.y + list_bounds.size.height
            {
                if let Some(thinking_bounds) = thinking_block_bounds(entry, entry_bounds) {
                    if let Some(block) = entry.thinking_block.as_mut() {
                        let before = block.is_expanded();
                        let result = block.event(event, thinking_bounds, cx);
                        if result == EventResult::Handled {
                            let after = block.is_expanded();
                            if before != after {
                                let mut state = self.state.borrow_mut();
                                state.set_thinking_expanded(entry.index, after);
                            }
                            return EventResult::Handled;
                        }
                    }
                }
            }
            y += entry.height + ENTRY_GAP;
        }

        EventResult::Ignored
    }
}

impl Component for ChatView {
    fn paint(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        self.refresh_session_tabs();
        self.sync_agent_dropdown();
        let (header_bounds, tabs_bounds, info_origin_y) = self.header_layout(bounds);
        let editor_height = self.editor.size_hint().1.unwrap_or(64.0);

        if let Some(tabs_bounds) = tabs_bounds {
            self.session_tabs.paint(tabs_bounds, cx);
        }
        self.paint_header(header_bounds, info_origin_y, cx);

        let editor_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - editor_height,
            bounds.size.width,
            editor_height,
        );
        let agent_layout = self.agent_layout(bounds, editor_height);

        let list_bounds = Bounds::new(
            bounds.origin.x,
            header_bounds.origin.y + header_bounds.size.height,
            bounds.size.width,
            (agent_layout.row.origin.y - header_bounds.origin.y - header_bounds.size.height)
                .max(0.0),
        );

        let revision = self.state.borrow().chat_revision;
        if (list_bounds.size.width - self.last_width).abs() > 0.5 || revision != self.last_revision {
            self.rebuild_layout(list_bounds, cx);
            self.last_width = list_bounds.size.width;
            self.last_revision = revision;
            if self.auto_scroll {
                self.scroll.scroll_to(Point::new(0.0, self.content_height));
            }
        }

        self.scroll.set_viewport(list_bounds);
        self.scroll
            .set_content_size(Size::new(list_bounds.size.width, self.content_height));

        cx.scene.push_clip(list_bounds);

        let scroll_offset = self.scroll.scroll_offset.y;
        let mut y = list_bounds.origin.y - scroll_offset;
        let indicator = &mut self.streaming_indicator;
        for entry in &mut self.layout_entries {
            let entry_bounds = Bounds::new(list_bounds.origin.x, y, list_bounds.size.width, entry.height);
            if entry_bounds.origin.y + entry_bounds.size.height >= list_bounds.origin.y
                && entry_bounds.origin.y <= list_bounds.origin.y + list_bounds.size.height
            {
                paint_entry(entry, entry_bounds, cx, indicator);
            }
            y += entry.height + ENTRY_GAP;
        }

        cx.scene.pop_clip();

        if self.scroll.can_scroll() {
            paint_scrollbar(&self.scroll, list_bounds, cx);
        }

        if agent_layout.dropdown.size.width > 1.0 && agent_layout.dropdown.size.height > 1.0 {
            let label_height = theme::font_size::XS * 1.2;
            let label_y =
                agent_layout.label.origin.y + (agent_layout.label.size.height - label_height) * 0.5;
            let mut label_text = Text::new(AGENT_LABEL)
                .font_size(theme::font_size::XS)
                .color(theme::text::MUTED);
            label_text.paint(
                Bounds::new(
                    agent_layout.label.origin.x,
                    label_y,
                    agent_layout.label.size.width,
                    label_height,
                ),
                cx,
            );
            self.agent_dropdown.paint(agent_layout.dropdown, cx);
        }

        let prompt_running = self.state.borrow().prompt_running;
        self.editor.set_streaming(prompt_running);
        self.editor.paint(editor_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        self.refresh_session_tabs();
        self.sync_agent_dropdown();
        let (header_bounds, tabs_bounds, _) = self.header_layout(bounds);
        let editor_height = self.editor.size_hint().1.unwrap_or(64.0);

        let editor_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - editor_height,
            bounds.size.width,
            editor_height,
        );
        let agent_layout = self.agent_layout(bounds, editor_height);
        let list_bounds = Bounds::new(
            bounds.origin.x,
            header_bounds.origin.y + header_bounds.size.height,
            bounds.size.width,
            (agent_layout.row.origin.y - header_bounds.origin.y - header_bounds.size.height)
                .max(0.0),
        );

        if let Some(tabs_bounds) = tabs_bounds {
            let result = self.session_tabs.event(event, tabs_bounds, cx);
            if result == EventResult::Handled {
                return result;
            }
        }

        let result = self.agent_dropdown.event(event, agent_layout.dropdown, cx);
        if result == EventResult::Handled {
            return result;
        }

        match event {
            InputEvent::Scroll { dx, dy } => {
                self.scroll.scroll_by(Point::new(*dx, *dy));
                self.auto_scroll = false;
                let max_scroll = self.scroll.max_scroll().y;
                if (self.scroll.scroll_offset.y - max_scroll).abs() < 2.0 {
                    self.auto_scroll = true;
                }
                return EventResult::Handled;
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == wgpui::MouseButton::Left
                    && self.scroll.can_scroll()
                    && scrollbar_bounds(list_bounds).contains(Point::new(*x, *y))
                {
                    self.dragging_scrollbar = true;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { button, .. } => {
                if *button == wgpui::MouseButton::Left && self.dragging_scrollbar {
                    self.dragging_scrollbar = false;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseMove { x, y } => {
                if self.dragging_scrollbar {
                    handle_scrollbar_drag(&mut self.scroll, list_bounds, *x, *y);
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        if editor_bounds.contains(match event {
            InputEvent::MouseMove { x, y }
            | InputEvent::MouseDown { x, y, .. }
            | InputEvent::MouseUp { x, y, .. } => Point::new(*x, *y),
            _ => Point::new(-1.0, -1.0),
        }) {
            return self.editor.event(event, editor_bounds, cx);
        }

        if matches!(event, InputEvent::KeyDown { .. } | InputEvent::KeyUp { .. }) {
            return self.editor.event(event, editor_bounds, cx);
        }

        if matches!(
            event,
            InputEvent::MouseMove { .. } | InputEvent::MouseDown { .. } | InputEvent::MouseUp { .. }
        ) {
            let result = self.handle_thinking_event(event, list_bounds, cx);
            if result == EventResult::Handled {
                return result;
            }
        }

        EventResult::Ignored
    }
}

fn session_tabs_for_state(state: &AppState) -> Vec<&SessionMetrics> {
    if state.sessions.is_empty() {
        return Vec::new();
    }

    let mut sessions: Vec<&SessionMetrics> =
        state.sessions.iter().take(SESSION_TAB_LIMIT).collect();

    if let Some(active_id) = state.log_session_id.as_ref() {
        let has_active = sessions.iter().any(|session| &session.id == active_id);
        if !has_active {
            if let Some(active) = state.sessions.iter().find(|session| &session.id == active_id) {
                sessions.push(active);
            }
        }
    }

    sessions
}

fn session_tab_label(session: &SessionMetrics) -> String {
    let id = session.id.chars().take(8).collect::<String>();
    let prompt = session.prompt.replace('\n', " ");
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return id;
    }

    let label = format!("{} {}", id, prompt);
    truncate_label(&label, SESSION_TAB_LABEL_MAX)
}

fn truncate_label(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let ellipsis = "...";
    if max_chars <= ellipsis.len() {
        return ellipsis.chars().take(max_chars).collect();
    }
    let trimmed: String = text.chars().take(max_chars - ellipsis.len()).collect();
    format!("{}{}", trimmed, ellipsis)
}

fn paint_entry(
    layout: &mut ChatLayoutEntry,
    bounds: Bounds,
    cx: &mut wgpui::PaintContext,
    indicator: &mut StreamingIndicator,
) {
    cx.scene.push_clip(bounds);
    match &layout.entry {
        ChatEntry::User { timestamp, .. } => {
            paint_text_entry(
                bounds,
                cx,
                MessageHeader::user().timestamp_opt(timestamp.clone()),
                &layout.lines,
                theme::bg::MUTED,
                theme::text::PRIMARY,
                false,
                None,
            );
        }
        ChatEntry::Assistant {
            timestamp,
            streaming,
            ..
        } => {
            paint_assistant_entry(
                layout,
                bounds,
                cx,
                MessageHeader::assistant(Model::ClaudeSonnet).timestamp_opt(timestamp.clone()),
                *streaming,
                indicator,
            );
        }
        ChatEntry::System { .. } => {
            paint_system_entry(bounds, cx, &layout.lines);
        }
        ChatEntry::ToolCall(tool) => {
            paint_tool_entry(tool, bounds, cx);
        }
    }
    cx.scene.pop_clip();
}

fn wrap_lines(
    cx: &mut wgpui::PaintContext,
    text: &str,
    font_size: f32,
    max_width: f32,
) -> Vec<String> {
    let char_width = cx.text.measure("W", font_size).max(1.0);
    let max_chars = (max_width / char_width).floor().max(1.0) as usize;
    let mut lines = Vec::new();

    for raw_line in text.lines() {
        let mut current = String::new();
        let mut count = 0;
        for ch in raw_line.chars() {
            if count >= max_chars {
                lines.push(current);
                current = String::new();
                count = 0;
            }
            current.push(ch);
            count += 1;
        }
        lines.push(current);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn parse_thinking_block(text: &str) -> (Option<String>, String) {
    const THINKING_START: &str = "<thinking>";
    const THINKING_END: &str = "</thinking>";

    let mut thinking_parts = Vec::new();
    let mut visible = String::new();
    let mut remaining = text;

    loop {
        let Some(start) = remaining.find(THINKING_START) else {
            visible.push_str(remaining);
            break;
        };
        let (before, after_start) = remaining.split_at(start);
        visible.push_str(before);
        let after_start = &after_start[THINKING_START.len()..];
        let Some(end) = after_start.find(THINKING_END) else {
            return (None, text.to_string());
        };
        let (thinking, after) = after_start.split_at(end);
        if !thinking.trim().is_empty() {
            thinking_parts.push(thinking.trim().to_string());
        }
        remaining = &after[THINKING_END.len()..];
    }

    let visible = visible.trim().to_string();
    let thinking = if thinking_parts.is_empty() {
        None
    } else {
        Some(thinking_parts.join("\n\n"))
    };
    (thinking, visible)
}

fn text_entry_height(lines: &[String], with_header: bool, streaming: bool) -> f32 {
    let padding = theme::spacing::MD;
    let header_height = if with_header { 24.0 } else { 0.0 };
    let line_height = theme::font_size::BASE * 1.5;
    let mut height = padding * 2.0 + header_height;
    height += lines.len() as f32 * line_height;
    if streaming {
        height += 16.0;
    }
    height + theme::spacing::SM
}

fn assistant_entry_height(line_count: usize, thinking_height: f32, streaming: bool) -> f32 {
    let padding = theme::spacing::MD;
    let header_height = 24.0;
    let header_gap = theme::spacing::SM;
    let line_height = theme::font_size::BASE * 1.5;
    let mut height =
        padding * 2.0 + header_height + header_gap + line_count as f32 * line_height;
    if thinking_height > 0.0 {
        height += theme::spacing::SM + thinking_height;
    }
    if streaming {
        height += 16.0;
    }
    height
}

fn thinking_block_height(line_count: usize, expanded: bool) -> f32 {
    let padding = theme::spacing::SM;
    let line_height = theme::font_size::SM * 1.4;
    let collapsed = line_count.min(THINKING_MAX_COLLAPSED_LINES);
    let visible_lines = if expanded {
        line_count
    } else if line_count > THINKING_MAX_COLLAPSED_LINES {
        collapsed + 1
    } else {
        collapsed
    };
    padding * 2.0 + THINKING_TOGGLE_HEIGHT + theme::spacing::XS + visible_lines as f32 * line_height
}

fn system_entry_height(lines: &[String]) -> f32 {
    let padding = theme::spacing::SM;
    let line_height = theme::font_size::SM * 1.4;
    padding * 2.0 + lines.len() as f32 * line_height
}

fn tool_entry_height(_tool: &ToolCallData) -> f32 {
    let base = 28.0 + theme::spacing::SM * 2.0;
    let content = 60.0 + theme::spacing::SM;
    base + content
}

fn paint_text_entry(
    bounds: Bounds,
    cx: &mut wgpui::PaintContext,
    mut header: MessageHeader,
    lines: &[String],
    background: wgpui::Hsla,
    color: wgpui::Hsla,
    streaming: bool,
    indicator: Option<&mut StreamingIndicator>,
) {
    let padding = theme::spacing::MD;

    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(background)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let header_height = 24.0;
    header.paint(
        Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            header_height,
        ),
        cx,
    );

    let mut y = bounds.origin.y + padding + header_height + theme::spacing::SM;
    let line_height = theme::font_size::BASE * 1.5;
    for line in lines {
        let mut text = Text::new(line)
            .font_size(theme::font_size::BASE)
            .color(color);
        text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
        y += line_height;
    }

    if streaming {
        if let Some(indicator) = indicator {
            indicator.paint(
                Bounds::new(
                    bounds.origin.x + padding,
                    bounds.origin.y + bounds.size.height - padding - 16.0,
                    100.0,
                    16.0,
                ),
                cx,
            );
        }
    }
}

fn paint_assistant_entry(
    layout: &mut ChatLayoutEntry,
    bounds: Bounds,
    cx: &mut wgpui::PaintContext,
    mut header: MessageHeader,
    streaming: bool,
    indicator: &mut StreamingIndicator,
) {
    let padding = theme::spacing::MD;

    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let header_height = 24.0;
    header.paint(
        Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y + padding,
            bounds.size.width - padding * 2.0,
            header_height,
        ),
        cx,
    );

    let mut y = bounds.origin.y + padding + header_height + theme::spacing::SM;
    let line_height = theme::font_size::BASE * 1.5;
    for line in &layout.lines {
        let mut text = Text::new(line)
            .font_size(theme::font_size::BASE)
            .color(theme::text::PRIMARY);
        text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
        y += line_height;
    }

    if let Some(thinking_bounds) = thinking_block_bounds(layout, bounds) {
        if let Some(block) = layout.thinking_block.as_mut() {
            block.paint(thinking_bounds, cx);
        }
    }

    if streaming {
        indicator.paint(
            Bounds::new(
                bounds.origin.x + padding,
                bounds.origin.y + bounds.size.height - padding - 16.0,
                100.0,
                16.0,
            ),
            cx,
        );
    }
}

fn thinking_block_bounds(layout: &ChatLayoutEntry, bounds: Bounds) -> Option<Bounds> {
    if layout.thinking_height <= 0.0 {
        return None;
    }

    let padding = theme::spacing::MD;
    let header_height = 24.0;
    let line_height = theme::font_size::BASE * 1.5;
    let mut y = bounds.origin.y + padding + header_height + theme::spacing::SM;
    y += layout.lines.len() as f32 * line_height;
    y += theme::spacing::SM;

    Some(Bounds::new(
        bounds.origin.x + padding,
        y,
        bounds.size.width - padding * 2.0,
        layout.thinking_height,
    ))
}

fn paint_system_entry(bounds: Bounds, cx: &mut wgpui::PaintContext, lines: &[String]) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );
    let padding = theme::spacing::SM;
    let line_height = theme::font_size::SM * 1.4;
    let mut y = bounds.origin.y + padding;
    for line in lines {
        let mut text = Text::new(line)
            .font_size(theme::font_size::SM)
            .color(theme::text::MUTED);
        text.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                line_height,
            ),
            cx,
        );
        y += line_height;
    }
}

fn paint_tool_entry(tool: &ToolCallData, bounds: Bounds, cx: &mut wgpui::PaintContext) {
    let padding = theme::spacing::SM;
    let max_width = bounds.size.width - padding * 2.0 - 16.0;
    let input = tool
        .input
        .as_ref()
        .map(|text| fit_text(cx, text, theme::font_size::SM, max_width));
    let output = tool
        .output
        .as_ref()
        .map(|text| fit_text(cx, text, theme::font_size::SM, max_width));

    let mut card = ToolCallCard::new(tool.tool_type, &tool.name)
        .status(tool.status)
        .expanded(true);
    if let Some(input) = input {
        card = card.input(input);
    }
    if let Some(output) = output {
        card = card.output(output);
    }
    card.paint(bounds, cx);
}

fn scrollbar_bounds(bounds: Bounds) -> Bounds {
    let width = 6.0;
    Bounds::new(
        bounds.origin.x + bounds.size.width - width - 2.0,
        bounds.origin.y,
        width + 2.0,
        bounds.size.height,
    )
}

fn paint_scrollbar(scroll: &ScrollContainer, bounds: Bounds, cx: &mut wgpui::PaintContext) {
    if let Some(thumb) = calculate_scrollbar_thumb(scroll, true, scrollbar_bounds(bounds), 30.0) {
        cx.scene.draw_quad(
            Quad::new(thumb)
                .with_background(theme::text::MUTED)
                .with_corner_radius(4.0),
        );
    }
}

fn handle_scrollbar_drag(scroll: &mut ScrollContainer, bounds: Bounds, _x: f32, y: f32) {
    let track = scrollbar_bounds(bounds);
    let progress = ((y - track.origin.y) / track.size.height).clamp(0.0, 1.0);
    let max = scroll.max_scroll();
    scroll.scroll_to(Point::new(scroll.scroll_offset.x, max.y * progress));
}

trait MessageHeaderExt {
    fn timestamp_opt(self, ts: Option<String>) -> Self;
}

impl MessageHeaderExt for MessageHeader {
    fn timestamp_opt(mut self, ts: Option<String>) -> Self {
        if let Some(ts) = ts {
            self = self.timestamp(ts);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use autopilot::metrics::{SessionMetrics, SessionStatus};
    use chrono::Utc;
    use std::sync::mpsc;
    use wgpui::{EventContext, Modifiers, MouseButton, NamedKey, Scene, TextSystem};

    fn make_context(scale: f32) -> (Scene, TextSystem) {
        (Scene::new(), TextSystem::new(scale))
    }

    fn make_session(id: &str, prompt: &str) -> SessionMetrics {
        SessionMetrics {
            id: id.to_string(),
            timestamp: Utc::now(),
            model: "sonnet".to_string(),
            prompt: prompt.to_string(),
            duration_seconds: 60.0,
            tokens_in: 0,
            tokens_out: 0,
            tokens_cached: 0,
            cost_usd: 0.0,
            issues_claimed: 0,
            issues_completed: 0,
            tool_calls: 0,
            tool_errors: 0,
            final_status: SessionStatus::Completed,
            messages: 0,
            apm: None,
            source: "autopilot".to_string(),
            issue_numbers: None,
            directive_id: None,
        }
    }

    #[test]
    fn test_chat_view_builds_layout_from_entries() {
        let state = Rc::new(RefCell::new(AppState::new()));
        state.borrow_mut().set_chat_entries(vec![
            ChatEntry::User {
                text: "Hello".to_string(),
                timestamp: None,
            },
            ChatEntry::Assistant {
                text: "Hi there".to_string(),
                timestamp: None,
                streaming: false,
            },
        ]);

        let (tx, _rx) = mpsc::channel();
        let mut view = ChatView::new(state, tx);
        let bounds = Bounds::new(0.0, 0.0, 640.0, 480.0);
        let (mut scene, mut text) = make_context(1.0);
        let mut cx = wgpui::PaintContext::new(&mut scene, &mut text, 1.0);

        view.paint(bounds, &mut cx);

        assert_eq!(view.layout_entries.len(), 2);
        assert!(view.content_height > 0.0);
    }

    #[test]
    fn test_chat_view_sends_prompt() {
        let state = Rc::new(RefCell::new(AppState::new()));
        let (tx, rx) = mpsc::channel();
        let mut view = ChatView::new(state, tx);
        let bounds = Bounds::new(0.0, 0.0, 640.0, 320.0);

        view.editor.set_value("Ship it");
        view.editor.focus();

        let mut event_cx = EventContext::new();
        let event = InputEvent::KeyDown {
            key: wgpui::Key::Named(NamedKey::Enter),
            modifiers: Modifiers::default(),
        };

        let result = view.event(&event, bounds, &mut event_cx);
        assert!(matches!(result, EventResult::Handled));

        let cmd = rx.try_recv().expect("command");
        assert!(matches!(cmd, BackendCommand::RunPrompt { ref prompt } if prompt == "Ship it"));
    }

    #[test]
    fn test_chat_view_agent_dropdown_updates_backend() {
        let state = Rc::new(RefCell::new(AppState::new()));
        let (tx, rx) = mpsc::channel();
        let mut view = ChatView::new(state.clone(), tx);
        let bounds = Bounds::new(0.0, 0.0, 640.0, 360.0);
        let editor_height = view.editor.size_hint().1.unwrap_or(64.0);
        let layout = view.agent_layout(bounds, editor_height);
        let dropdown_height = view.agent_dropdown.size_hint().1.unwrap_or(24.0);

        let mut event_cx = EventContext::new();
        let open = InputEvent::MouseDown {
            button: MouseButton::Left,
            x: layout.dropdown.origin.x + 2.0,
            y: layout.dropdown.origin.y + 2.0,
        };
        let _ = view.event(&open, bounds, &mut event_cx);

        let gpt_oss_index = 2;
        let dropdown_top = layout.dropdown.origin.y + layout.dropdown.size.height;
        let select = InputEvent::MouseDown {
            button: MouseButton::Left,
            x: layout.dropdown.origin.x + 4.0,
            y: dropdown_top + dropdown_height * (gpt_oss_index as f32 + 0.5),
        };
        let result = view.event(&select, bounds, &mut event_cx);
        assert!(matches!(result, EventResult::Handled));

        let cmd = rx.try_recv().expect("command");
        assert!(matches!(cmd, BackendCommand::SetAgent { ref agent } if agent == "gpt-oss"));
        assert_eq!(state.borrow().agent, "gpt-oss");
    }

    #[test]
    fn test_chat_view_selects_session_tab() {
        let session_one = make_session("session-one", "First run");
        let session_two = make_session("session-two", "Second run");
        let state = Rc::new(RefCell::new(AppState::new()));
        {
            let mut state = state.borrow_mut();
            state.sessions = vec![session_one.clone(), session_two.clone()];
            state.log_session_id = Some(session_one.id.clone());
        }

        let (tx, rx) = mpsc::channel();
        let mut view = ChatView::new(state, tx);
        let bounds = Bounds::new(0.0, 0.0, 720.0, 480.0);
        let (mut scene, mut text) = make_context(1.0);
        let mut cx = wgpui::PaintContext::new(&mut scene, &mut text, 1.0);

        view.paint(bounds, &mut cx);

        let first_label = session_tab_label(&session_one);
        let first_width = first_label.chars().count() as f32 * SESSION_TAB_FONT_SIZE * 0.6
            + SESSION_TAB_PADDING_H * 2.0;
        let tab_x = theme::spacing::SM + first_width + 2.0;
        let tab_y = theme::spacing::SM + 2.0;

        let mut event_cx = EventContext::new();
        let event = InputEvent::MouseDown {
            button: MouseButton::Left,
            x: tab_x,
            y: tab_y,
        };
        let result = view.event(&event, bounds, &mut event_cx);
        assert!(matches!(result, EventResult::Handled));

        let cmd = rx.try_recv().expect("command");
        assert!(matches!(cmd, BackendCommand::SelectSession { ref session_id } if session_id == "session-two"));
    }

    #[test]
    fn test_parse_thinking_block_extracts_content() {
        let text = "Answer line 1.\n<thinking>\nReason A\nReason B\n</thinking>\nAnswer line 2.";
        let (thinking, visible) = parse_thinking_block(text);
        assert_eq!(thinking.as_deref(), Some("Reason A\nReason B"));
        assert_eq!(visible, "Answer line 1.\n\nAnswer line 2.");
    }
}
