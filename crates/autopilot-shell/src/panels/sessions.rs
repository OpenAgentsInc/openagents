//! Sessions panel for the left sidebar
//!
//! Displays recent sessions, model selector, and interrupt button.

use chrono::{DateTime, Local};
use wgpui::{
    Bounds, Component, EventContext, EventResult, Hsla, InputEvent, MouseButton, PaintContext,
    Point, Quad,
    components::atoms::Model,
    components::hud::Frame,
    components::molecules::ModelSelector,
};

use crate::dock::{DockPosition, Panel};

/// Actions that can be triggered by the sessions panel
#[derive(Debug, Clone)]
pub enum SessionAction {
    ResumeSession(String),
    SetModel(Model),
    Interrupt,
    NewSession,
}

/// Session info for display
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub timestamp: DateTime<Local>,
    pub model: String,
    pub is_current: bool,
}

impl SessionInfo {
    /// Format timestamp for display
    fn format_time(&self) -> String {
        let now = Local::now();
        let date = self.timestamp.date_naive();
        let today = now.date_naive();

        if date == today {
            format!("Today {}", self.timestamp.format("%H:%M"))
        } else if date == today.pred_opt().unwrap_or(today) {
            format!("Yesterday {}", self.timestamp.format("%H:%M"))
        } else {
            self.timestamp.format("%b %d %H:%M").to_string()
        }
    }

    /// Short model name for display
    fn short_model(&self) -> &str {
        if self.model.contains("opus") {
            "opus"
        } else if self.model.contains("haiku") {
            "haiku"
        } else {
            "sonnet"
        }
    }
}

/// Left sidebar panel with session list and controls
pub struct SessionsPanel {
    sessions: Vec<SessionInfo>,
    hovered_index: Option<usize>,
    model_selector: ModelSelector,
    is_running: bool,
    new_session_hovered: bool,
    interrupt_hovered: bool,

    /// Pending actions to be polled by the shell
    pending_actions: Vec<SessionAction>,
}

impl SessionsPanel {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            hovered_index: None,
            model_selector: ModelSelector::new(Model::ClaudeSonnet)
                .models(vec![Model::ClaudeSonnet, Model::ClaudeOpus, Model::ClaudeHaiku]),
            is_running: false,
            new_session_hovered: false,
            interrupt_hovered: false,
            pending_actions: Vec::new(),
        }
    }

    /// Set the list of recent sessions
    pub fn set_sessions(&mut self, sessions: Vec<SessionInfo>) {
        self.sessions = sessions;
    }

    /// Set the current model
    pub fn set_model(&mut self, model: Model) {
        self.model_selector.set_model(model);
    }

    /// Set whether a query is currently running
    pub fn set_running(&mut self, running: bool) {
        self.is_running = running;
    }

    /// Take all pending actions
    pub fn take_actions(&mut self) -> Vec<SessionAction> {
        std::mem::take(&mut self.pending_actions)
    }

    fn paint_hotkey_legend(&self, bounds: Bounds, cx: &mut PaintContext) {
        let hotkeys = [
            ("cmd-a", "Toggle Full Auto"),
            ("cmd-[", "Toggle sidebar"),
            ("esc", "Exit"),
        ];

        let line_height = 16.0;
        let padding = 16.0;
        let x = bounds.origin.x + padding;
        let mut y = bounds.origin.y + bounds.size.height - (hotkeys.len() as f32 * line_height) - padding;

        let text_color = Hsla::new(0.0, 0.0, 0.4, 0.9);
        let key_color = Hsla::new(180.0, 0.4, 0.4, 0.9);
        let font_size = 9.0;

        for (key, desc) in &hotkeys {
            let key_run = cx.text.layout(key, Point::new(x, y), font_size, key_color);
            cx.scene.draw_text(key_run);

            let desc_run = cx.text.layout(desc, Point::new(x + 60.0, y), font_size, text_color);
            cx.scene.draw_text(desc_run);

            y += line_height;
        }
    }

    fn paint_section_header(&self, label: &str, y: f32, bounds: Bounds, cx: &mut PaintContext) {
        let header_color = Hsla::new(0.0, 0.0, 0.5, 0.9);
        let font_size = 9.0;
        let text = cx.text.layout(label, Point::new(bounds.origin.x + 16.0, y), font_size, header_color);
        cx.scene.draw_text(text);
    }

    fn paint_session_item(&self, session: &SessionInfo, index: usize, y: f32, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 16.0;
        let x = bounds.origin.x + padding;
        let w = bounds.size.width - padding * 2.0;
        let item_h = 28.0;

        let is_hovered = self.hovered_index == Some(index);

        // Background on hover
        if is_hovered {
            let hover_bg = Hsla::new(0.0, 0.0, 0.15, 1.0);
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y, w, item_h)).with_background(hover_bg));
        }

        // Current session accent bar
        if session.is_current {
            let accent = Hsla::new(180.0, 0.5, 0.5, 1.0); // cyan
            cx.scene.draw_quad(Quad::new(Bounds::new(x, y, 3.0, item_h)).with_background(accent));
        }

        // Session text: "Today 14:32 (sonnet)"
        let label = format!("{} ({})", session.format_time(), session.short_model());
        let text_color = if session.is_current {
            Hsla::new(0.0, 0.0, 0.9, 1.0)
        } else {
            Hsla::new(0.0, 0.0, 0.6, 1.0)
        };
        let font_size = 11.0;
        let indicator = if session.is_current { "▸ " } else { "  " };
        let text = cx.text.layout(
            &format!("{}{}", indicator, label),
            Point::new(x + 8.0, y + 6.0),
            font_size,
            text_color,
        );
        cx.scene.draw_text(text);
    }

    fn paint_button(&self, label: &str, y: f32, bounds: Bounds, hovered: bool, cx: &mut PaintContext) {
        let padding = 16.0;
        let x = bounds.origin.x + padding;
        let w = bounds.size.width - padding * 2.0;
        let h = 24.0;

        let bg = if hovered {
            Hsla::new(0.0, 0.0, 0.2, 1.0)
        } else {
            Hsla::new(0.0, 0.0, 0.1, 1.0)
        };
        let border = Hsla::new(0.0, 0.0, 0.3, 1.0);

        cx.scene.draw_quad(
            Quad::new(Bounds::new(x, y, w, h))
                .with_background(bg)
                .with_border(border, 1.0)
        );

        let text_color = Hsla::new(0.0, 0.0, 0.7, 1.0);
        let text = cx.text.layout(label, Point::new(x + 8.0, y + 5.0), 10.0, text_color);
        cx.scene.draw_text(text);
    }

    fn session_item_bounds(&self, index: usize, bounds: Bounds) -> Bounds {
        let padding = 16.0;
        let item_h = 28.0;
        let header_h = 20.0;
        let y = bounds.origin.y + padding + header_h + (index as f32 * item_h);
        Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, item_h)
    }

    fn new_session_button_bounds(&self, bounds: Bounds) -> Bounds {
        let padding = 16.0;
        let header_h = 20.0;
        let item_h = 28.0;
        let session_count = self.sessions.len().min(5);
        let y = bounds.origin.y + padding + header_h + (session_count as f32 * item_h) + 8.0;
        Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 24.0)
    }

    fn model_selector_bounds(&self, bounds: Bounds) -> Bounds {
        let padding = 16.0;
        let header_h = 20.0;
        let item_h = 28.0;
        let session_count = self.sessions.len().min(5);
        let new_btn_h = 32.0;
        let section_gap = 16.0;
        let y = bounds.origin.y + padding + header_h + (session_count as f32 * item_h) + new_btn_h + section_gap + header_h;
        Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 24.0)
    }

    fn interrupt_button_bounds(&self, bounds: Bounds) -> Bounds {
        let model_bounds = self.model_selector_bounds(bounds);
        let y = model_bounds.origin.y + model_bounds.size.height + 16.0;
        Bounds::new(bounds.origin.x + 16.0, y, bounds.size.width - 32.0, 24.0)
    }
}

impl Default for SessionsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for SessionsPanel {
    fn panel_id(&self) -> &'static str {
        "sessions"
    }

    fn title(&self) -> &str {
        "Sessions"
    }

    fn preferred_position(&self) -> DockPosition {
        DockPosition::Left
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Draw frame
        let line_color = Hsla::new(0.0, 0.0, 0.3, 0.5);
        let bg_color = Hsla::new(0.0, 0.0, 0.05, 0.9);

        let mut frame = Frame::lines()
            .line_color(line_color)
            .bg_color(bg_color)
            .stroke_width(1.0);
        frame.paint(bounds, cx);

        let padding = 16.0;
        let header_h = 20.0;
        let item_h = 28.0;
        let mut y = bounds.origin.y + padding;

        // SESSIONS section
        self.paint_section_header("SESSIONS", y, bounds, cx);
        y += header_h;

        // Session list (max 5)
        for (i, session) in self.sessions.iter().take(5).enumerate() {
            self.paint_session_item(session, i, y, bounds, cx);
            y += item_h;
        }

        // New Session button
        y += 8.0;
        self.paint_button("+ New Session", y, bounds, self.new_session_hovered, cx);
        y += 32.0;

        // MODEL section
        y += 8.0;
        self.paint_section_header("MODEL", y, bounds, cx);
        y += header_h;

        // Model selector
        let model_bounds = Bounds::new(bounds.origin.x + padding, y, bounds.size.width - padding * 2.0, 24.0);
        self.model_selector.paint(model_bounds, cx);
        y += 32.0;

        // Interrupt button (only when running)
        if self.is_running {
            y += 8.0;
            self.paint_button("Interrupt", y, bounds, self.interrupt_hovered, cx);
        }

        // Hotkey legend at bottom
        self.paint_hotkey_legend(bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Handle model selector events first
        let model_bounds = self.model_selector_bounds(bounds);
        if let EventResult::Handled = self.model_selector.event(event, model_bounds, cx) {
            // Check if selection changed
            let selected = self.model_selector.current_model();
            self.pending_actions.push(SessionAction::SetModel(selected));
            return EventResult::Handled;
        }

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);

                // Check session item hover
                self.hovered_index = None;
                for i in 0..self.sessions.len().min(5) {
                    let item_bounds = self.session_item_bounds(i, bounds);
                    if item_bounds.contains(point) {
                        self.hovered_index = Some(i);
                        break;
                    }
                }

                // Check button hovers
                let new_btn_bounds = self.new_session_button_bounds(bounds);
                self.new_session_hovered = new_btn_bounds.contains(point);

                let interrupt_bounds = self.interrupt_button_bounds(bounds);
                self.interrupt_hovered = self.is_running && interrupt_bounds.contains(point);

                EventResult::Handled
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button != MouseButton::Left {
                    return EventResult::Ignored;
                }

                let point = Point::new(*x, *y);
                tracing::info!("SessionsPanel: MouseDown at ({}, {}), bounds: {:?}", x, y, bounds);

                // Check session clicks
                for i in 0..self.sessions.len().min(5) {
                    let item_bounds = self.session_item_bounds(i, bounds);
                    if item_bounds.contains(point) {
                        let session_id = self.sessions[i].id.clone();
                        self.pending_actions.push(SessionAction::ResumeSession(session_id));
                        return EventResult::Handled;
                    }
                }

                // Check New Session button
                let new_btn_bounds = self.new_session_button_bounds(bounds);
                tracing::info!("SessionsPanel: New Session btn bounds: {:?}, contains: {}", new_btn_bounds, new_btn_bounds.contains(point));
                if new_btn_bounds.contains(point) {
                    tracing::info!("SessionsPanel: New Session clicked!");
                    self.pending_actions.push(SessionAction::NewSession);
                    return EventResult::Handled;
                }

                // Check Interrupt button
                if self.is_running {
                    let interrupt_bounds = self.interrupt_button_bounds(bounds);
                    if interrupt_bounds.contains(point) {
                        self.pending_actions.push(SessionAction::Interrupt);
                        return EventResult::Handled;
                    }
                }

                EventResult::Ignored
            }
            _ => EventResult::Ignored,
        }
    }
}
