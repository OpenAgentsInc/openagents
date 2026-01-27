use std::path::PathBuf;

use autopilot_app::{AppEvent, SessionId, UserAction, WorkspaceId};
use wgpui::components::Text;
use wgpui::{Bounds, Component, PaintContext, Quad, theme};

const STATUS_HEIGHT: f32 = 120.0;
const HEADER_HEIGHT: f32 = 48.0;
const LIST_HEADER_HEIGHT: f32 = 24.0;
const LIST_OFFSET: f32 = 140.0;
const LIST_ROW_HEIGHT: f32 = 28.0;
const LIST_ROW_HEIGHT_LARGE: f32 = 32.0;
const PANEL_INSET: f32 = 24.0;
const COLUMN_GAP: f32 = 32.0;

#[derive(Default, Clone)]
pub struct AppViewModel {
    workspace_id: Option<WorkspaceId>,
    workspace_path: Option<PathBuf>,
    session_id: Option<SessionId>,
    session_label: Option<String>,
    last_event: Option<String>,
    event_count: usize,
    sessions: Vec<SessionSummary>,
    event_log: Vec<String>,
}

impl AppViewModel {
    pub fn apply_event(&mut self, event: &AppEvent) {
        self.event_count += 1;
        let formatted = format_event(event);
        self.last_event = Some(formatted.clone());
        self.event_log.push(formatted);
        if self.event_log.len() > 8 {
            let keep = self.event_log.len().saturating_sub(8);
            let _ = self.event_log.drain(0..keep);
        }

        match event {
            AppEvent::WorkspaceOpened { workspace_id, path } => {
                self.workspace_id = Some(*workspace_id);
                self.workspace_path = Some(path.clone());
            }
            AppEvent::SessionStarted { session_id, label, .. } => {
                self.session_id = Some(*session_id);
                self.session_label = label.clone();
                self.sessions.push(SessionSummary {
                    session_id: *session_id,
                    label: label.clone(),
                });
            }
            AppEvent::UserActionDispatched { .. } => {}
        }
    }

    pub fn workspace_path(&self) -> Option<&PathBuf> {
        self.workspace_path.as_ref()
    }

    pub fn session_id(&self) -> Option<SessionId> {
        self.session_id
    }

    pub fn event_count(&self) -> usize {
        self.event_count
    }
}

pub struct DesktopRoot {
    view_model: AppViewModel,
    header: Text,
    status: Text,
    session_title: Text,
    event_title: Text,
    session_items: Vec<Text>,
    event_items: Vec<Text>,
}

impl DesktopRoot {
    pub fn new() -> Self {
        let mut root = Self {
            view_model: AppViewModel::default(),
            header: Text::new("Autopilot Desktop (WGPUI)")
                .font_size(30.0)
                .bold()
                .color(theme::text::PRIMARY),
            status: Text::new("Waiting for events...")
                .font_size(16.0)
                .color(theme::text::MUTED),
            session_title: Text::new("Sessions")
                .font_size(18.0)
                .bold()
                .color(theme::text::PRIMARY),
            event_title: Text::new("Event Log")
                .font_size(18.0)
                .bold()
                .color(theme::text::PRIMARY),
            session_items: Vec::new(),
            event_items: Vec::new(),
        };
        root.refresh_text();
        root
    }

    pub fn apply_event(&mut self, event: AppEvent) {
        self.view_model.apply_event(&event);
        self.refresh_text();
    }

    pub fn view_model(&self) -> &AppViewModel {
        &self.view_model
    }

    fn refresh_text(&mut self) {
        let workspace_line = self
            .view_model
            .workspace_path
            .as_ref()
            .map(|path| format!("Workspace: {}", path.display()))
            .unwrap_or_else(|| "Workspace: --".to_string());

        let session_line = match (self.view_model.session_id, &self.view_model.session_label) {
            (Some(id), Some(label)) => format!("Session: {:?} ({})", id, label),
            (Some(id), None) => format!("Session: {:?}", id),
            _ => "Session: --".to_string(),
        };

        let last_event_line = self
            .view_model
            .last_event
            .clone()
            .unwrap_or_else(|| "Last event: --".to_string());

        let count_line = format!("Event count: {}", self.view_model.event_count);

        let body = format!(
            "Immediate-mode view model (Zed/GPUI-style)\n{workspace}\n{session}\n{last_event}\n{count}",
            workspace = workspace_line,
            session = session_line,
            last_event = last_event_line,
            count = count_line
        );

        self.status.set_content(body);

        self.session_items = self
            .view_model
            .sessions
            .iter()
            .enumerate()
            .map(|(index, session)| {
                let label = session
                    .label
                    .as_ref()
                    .map(|label| format!("{label} ({:?})", session.session_id))
                    .unwrap_or_else(|| format!("Session {:?}", session.session_id));
                Text::new(format!("{}. {}", index + 1, label))
                    .font_size(14.0)
                    .color(theme::text::PRIMARY)
            })
            .collect();

        self.event_items = self
            .view_model
            .event_log
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                Text::new(format!("{}. {}", index + 1, entry))
                    .font_size(13.0)
                    .color(theme::text::MUTED)
            })
            .collect();
    }
}

impl Default for DesktopRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for DesktopRoot {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let header_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y,
            bounds.size.width,
            HEADER_HEIGHT,
        );
        self.header.paint(header_bounds, cx);

        let body_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + HEADER_HEIGHT + 16.0,
            bounds.size.width,
            bounds.size.height - HEADER_HEIGHT - 16.0,
        );
        let card = Quad::new(body_bounds)
            .with_background(theme::bg::SURFACE)
            .with_corner_radius(14.0);
        cx.scene.draw_quad(card);

        let inner = Bounds::new(
            body_bounds.origin.x + PANEL_INSET,
            body_bounds.origin.y + PANEL_INSET,
            (body_bounds.size.width - PANEL_INSET * 2.0).max(0.0),
            (body_bounds.size.height - PANEL_INSET * 2.0).max(0.0),
        );

        let status_bounds = Bounds::new(inner.origin.x, inner.origin.y, inner.size.width, STATUS_HEIGHT);
        self.status.paint(status_bounds, cx);

        let column_width = (inner.size.width - COLUMN_GAP) * 0.5;
        let list_top = inner.origin.y + LIST_OFFSET;

        let sessions_bounds = Bounds::new(
            inner.origin.x,
            list_top,
            column_width.max(0.0),
            inner.size.height - LIST_OFFSET,
        );
        let events_bounds = Bounds::new(
            inner.origin.x + column_width + COLUMN_GAP,
            list_top,
            column_width.max(0.0),
            inner.size.height - LIST_OFFSET,
        );

        self.session_title.paint(
            Bounds::new(
                sessions_bounds.origin.x,
                sessions_bounds.origin.y,
                sessions_bounds.size.width,
                LIST_HEADER_HEIGHT,
            ),
            cx,
        );
        self.event_title.paint(
            Bounds::new(
                events_bounds.origin.x,
                events_bounds.origin.y,
                events_bounds.size.width,
                LIST_HEADER_HEIGHT,
            ),
            cx,
        );

        paint_list(cx, &mut self.session_items, sessions_bounds, LIST_ROW_HEIGHT_LARGE);
        paint_list(cx, &mut self.event_items, events_bounds, LIST_ROW_HEIGHT);
    }
}

fn paint_list(cx: &mut PaintContext, items: &mut [Text], bounds: Bounds, row_height: f32) {
    let mut y = bounds.origin.y + 32.0;
    for item in items {
        let row_bounds = Bounds::new(bounds.origin.x, y, bounds.size.width, row_height);
        item.paint(row_bounds, cx);
        y += row_height;
        if y > bounds.origin.y + bounds.size.height {
            break;
        }
    }
}

#[derive(Clone, Debug)]
struct SessionSummary {
    session_id: SessionId,
    label: Option<String>,
}

fn format_event(event: &AppEvent) -> String {
    match event {
        AppEvent::WorkspaceOpened { path, .. } => {
            format!("Last event: WorkspaceOpened ({})", path.display())
        }
        AppEvent::SessionStarted { session_id, .. } => {
            format!("Last event: SessionStarted ({:?})", session_id)
        }
        AppEvent::UserActionDispatched { action, .. } => match action {
            UserAction::Message { text, .. } => format!("Last event: Message ({})", text),
            UserAction::Command { name, .. } => format!("Last event: Command ({})", name),
        },
    }
}
