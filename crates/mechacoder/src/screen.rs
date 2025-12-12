//! Main MechaCoder screen component.

use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use std::path::PathBuf;
use theme_oa::{bg, border, text, FONT_FAMILY};
use ui_oa::{Button, ButtonVariant};

use crate::actions::*;
use crate::panels::GymPanel;
use crate::sdk_thread::SdkThread;
use crate::ui::thread_view::ThreadView;

/// Main screen for MechaCoder.
pub struct MechaCoderScreen {
    /// Focus handle.
    focus_handle: FocusHandle,
    /// Current project root.
    project_root: PathBuf,
    /// Current thread view.
    thread_view: Option<Entity<ThreadView>>,
    /// Connection status.
    connection_status: ConnectionStatus,
    /// Error message if any.
    error_message: Option<String>,
    /// Whether we need to focus the input on next render.
    needs_focus: bool,
    /// Gym panel entity.
    gym_panel: Entity<GymPanel>,
    /// Whether gym panel is visible.
    gym_panel_visible: bool,
}

/// Connection status.
#[derive(Clone, Debug, Default)]
pub enum ConnectionStatus {
    #[default]
    Connecting,
    Connected,
    Error(String),
}

impl MechaCoderScreen {
    /// Create a new MechaCoder screen.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        // Default to current directory
        let project_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Create gym panel
        let gym_panel = cx.new(|cx| GymPanel::new(cx));

        let mut screen = Self {
            focus_handle,
            project_root,
            thread_view: None,
            connection_status: ConnectionStatus::Connecting,
            error_message: None,
            needs_focus: false,
            gym_panel,
            gym_panel_visible: false,
        };

        // Auto-connect immediately
        screen.connect(cx);

        screen
    }

    /// Set the project root directory.
    #[allow(dead_code)]
    pub fn set_project_root(&mut self, path: impl Into<PathBuf>) {
        self.project_root = path.into();
    }

    /// Connect to Claude Code via SDK and start a new thread.
    fn connect(&mut self, cx: &mut Context<Self>) {
        self.connection_status = ConnectionStatus::Connecting;
        self.error_message = None;
        cx.notify();

        let project_root = self.project_root.clone();

        // Create SDK thread directly - no async connection needed
        let thread = cx.new(|cx| SdkThread::new(project_root, cx));

        // Create thread view
        let thread_view = cx.new(|cx| ThreadView::new(thread, cx));
        self.thread_view = Some(thread_view);
        self.connection_status = ConnectionStatus::Connected;
        self.needs_focus = true;
        cx.notify();
    }

    /// Handle the Quit action.
    fn quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Toggle the gym panel visibility.
    fn toggle_gym_panel(&mut self, _: &ToggleGymPanel, _window: &mut Window, cx: &mut Context<Self>) {
        self.gym_panel_visible = !self.gym_panel_visible;
        cx.notify();
    }

    /// Render the connecting state with disabled input.
    fn render_connecting(&self) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            // Main content area (empty while loading)
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .max_w(px(768.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(text::SECONDARY)
                            .child("Connecting to Claude Code..."),
                    ),
            )
            // Disabled input area
            .child(
                div()
                    .w_full()
                    .max_w(px(768.0))
                    .p(px(16.0))
                    .border_t_1()
                    .border_color(border::DEFAULT)
                    .flex()
                    .flex_row()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .px(px(12.0))
                            .py(px(8.0))
                            .rounded(px(4.0))
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .text_color(text::SECONDARY)
                            .child("Connecting..."),
                    )
                    .child(
                        Button::new("Send")
                            .variant(ButtonVariant::Secondary)
                            .disabled(true),
                    ),
            )
            // Status bar
            .child(
                div()
                    .w_full()
                    .max_w(px(768.0))
                    .px(px(16.0))
                    .py(px(8.0))
                    .border_t_1()
                    .border_color(border::DEFAULT)
                    .child(
                        div()
                            .text_sm()
                            .text_color(text::SECONDARY)
                            .child("Connecting..."),
                    ),
            )
    }

    /// Render the error state.
    fn render_error(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(16.0))
            .child(
                div()
                    .text_xl()
                    .text_color(text::PRIMARY)
                    .child("Connection Failed"),
            )
            .when_some(self.error_message.as_ref(), |el, error| {
                el.child(
                    div()
                        .px(px(16.0))
                        .py(px(8.0))
                        .rounded(px(4.0))
                        .bg(bg::CARD)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .text_color(text::PRIMARY)
                        .max_w(px(500.0))
                        .child(error.clone()),
                )
            })
            .child(
                div().mt(px(16.0)).child(
                    Button::new("Retry")
                        .variant(ButtonVariant::Default)
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.connect(cx);
                        })),
                ),
            )
    }

    /// Render the connected state with thread view.
    fn render_connected(&self) -> impl IntoElement {
        if let Some(thread_view) = &self.thread_view {
            div().size_full().child(thread_view.clone())
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(div().text_color(text::SECONDARY).child("No active thread"))
        }
    }
}

impl Focusable for MechaCoderScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MechaCoderScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Focus the message input when we just connected
        if self.needs_focus {
            self.needs_focus = false;
            if let Some(thread_view) = &self.thread_view {
                let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
                focus_handle.focus(window);
            }
        }

        let gym_panel_visible = self.gym_panel_visible;
        let gym_panel = self.gym_panel.clone();

        div()
            .id("mechacoder-root")
            .key_context("MechaCoder")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(bg::APP)
            .font_family(FONT_FAMILY)
            .text_color(text::PRIMARY)
            .on_action(cx.listener(Self::quit))
            .on_action(cx.listener(Self::toggle_gym_panel))
            .flex()
            .flex_row()
            // Main content area
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .overflow_hidden()
                    .child(match &self.connection_status {
                        ConnectionStatus::Connecting => self.render_connecting().into_any_element(),
                        ConnectionStatus::Connected => self.render_connected().into_any_element(),
                        ConnectionStatus::Error(_) => self.render_error(cx).into_any_element(),
                    })
            )
            // Right panel (Gym) - 320px wide when visible
            .when(gym_panel_visible, |el| {
                el.child(
                    div()
                        .w(px(320.0))
                        .h_full()
                        .border_l_1()
                        .border_color(border::DEFAULT)
                        .bg(bg::SURFACE)
                        .overflow_hidden()
                        .child(gym_panel)
                )
            })
    }
}
