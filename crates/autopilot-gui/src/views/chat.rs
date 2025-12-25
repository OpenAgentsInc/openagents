use std::cell::RefCell;
use std::rc::Rc;

use wgpui::components::atoms::{Model, StreamingIndicator};
use wgpui::components::molecules::MessageHeader;
use wgpui::components::organisms::ToolCallCard;
use wgpui::components::sections::MessageEditor;
use wgpui::components::Component;
use wgpui::scroll::{calculate_scrollbar_thumb, ScrollContainer};
use wgpui::{
    Bounds, EventContext, EventResult, InputEvent, Point, Quad, Size, Text, theme,
};

use crate::backend::BackendCommand;
use crate::state::{AppState, ChatEntry, ToolCallData};
use crate::views::fit_text;

const ENTRY_GAP: f32 = 12.0;
const HEADER_LINE_HEIGHT: f32 = 18.0;

pub struct ChatView {
    state: Rc<RefCell<AppState>>,
    editor: MessageEditor,
    scroll: ScrollContainer,
    dragging_scrollbar: bool,
    auto_scroll: bool,
    last_revision: u64,
    last_width: f32,
    layout_entries: Vec<ChatLayoutEntry>,
    content_height: f32,
    streaming_indicator: StreamingIndicator,
}

struct ChatLayoutEntry {
    entry: ChatEntry,
    lines: Vec<String>,
    height: f32,
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

        Self {
            state,
            editor,
            scroll: ScrollContainer::vertical(Bounds::ZERO),
            dragging_scrollbar: false,
            auto_scroll: true,
            last_revision: 0,
            last_width: 0.0,
            layout_entries: Vec::new(),
            content_height: 0.0,
            streaming_indicator: StreamingIndicator::new(),
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

    fn rebuild_layout(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::MD;
        let available_width = (bounds.size.width - padding * 2.0).max(0.0);

        self.layout_entries.clear();
        self.content_height = 0.0;

        for entry in &state.chat_entries {
            let (lines, height) = match entry {
                ChatEntry::User { text, .. } => {
                    let lines = wrap_lines(cx, text, theme::font_size::BASE, available_width);
                    let height = text_entry_height(&lines, true, false);
                    (lines, height)
                }
                ChatEntry::Assistant { text, streaming, .. } => {
                    let lines = wrap_lines(cx, text, theme::font_size::BASE, available_width);
                    let height = text_entry_height(&lines, true, *streaming);
                    (lines, height)
                }
                ChatEntry::System { text, .. } => {
                    let lines = wrap_lines(cx, text, theme::font_size::SM, available_width);
                    let height = system_entry_height(&lines);
                    (lines, height)
                }
                ChatEntry::ToolCall(tool) => {
                    let height = tool_entry_height(tool);
                    (Vec::new(), height)
                }
            };

            self.layout_entries.push(ChatLayoutEntry {
                entry: entry.clone(),
                lines,
                height,
            });

            self.content_height += height + ENTRY_GAP;
        }

        if self.content_height > 0.0 {
            self.content_height -= ENTRY_GAP;
        }
    }

    fn paint_header(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let state = self.state.borrow();
        let padding = theme::spacing::SM;
        let mut y = bounds.origin.y + padding;

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

}

impl Component for ChatView {
    fn paint(&mut self, bounds: Bounds, cx: &mut wgpui::PaintContext) {
        let padding = theme::spacing::SM;
        let header_height = HEADER_LINE_HEIGHT * 2.0 + padding * 2.0;
        let editor_height = self.editor.size_hint().1.unwrap_or(64.0);

        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            header_height,
        );
        self.paint_header(header_bounds, cx);

        let editor_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - editor_height,
            bounds.size.width,
            editor_height,
        );

        let list_bounds = Bounds::new(
            bounds.origin.x,
            header_bounds.origin.y + header_bounds.size.height,
            bounds.size.width,
            (editor_bounds.origin.y - header_bounds.origin.y - header_bounds.size.height).max(0.0),
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
        for entry in &self.layout_entries {
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

        let prompt_running = self.state.borrow().prompt_running;
        self.editor.set_streaming(prompt_running);
        self.editor.paint(editor_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = theme::spacing::SM;
        let header_height = HEADER_LINE_HEIGHT * 2.0 + padding * 2.0;
        let editor_height = self.editor.size_hint().1.unwrap_or(64.0);

        let editor_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - editor_height,
            bounds.size.width,
            editor_height,
        );
        let list_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + header_height,
            bounds.size.width,
            (editor_bounds.origin.y - bounds.origin.y - header_height).max(0.0),
        );

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

        let _ = list_bounds;
        EventResult::Ignored
    }
}

fn paint_entry(
    layout: &ChatLayoutEntry,
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
            let indicator = if *streaming { Some(indicator) } else { None };
            paint_text_entry(
                bounds,
                cx,
                MessageHeader::assistant(Model::ClaudeSonnet).timestamp_opt(timestamp.clone()),
                &layout.lines,
                theme::bg::SURFACE,
                theme::text::PRIMARY,
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
    use std::sync::mpsc;
    use wgpui::{EventContext, Modifiers, NamedKey, Scene, TextSystem};

    fn make_context(scale: f32) -> (Scene, TextSystem) {
        (Scene::new(), TextSystem::new(scale))
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
}
