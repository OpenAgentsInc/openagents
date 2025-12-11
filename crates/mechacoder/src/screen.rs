//! Main MechaCoder screen component.

use acp::{ClaudeCode, Project};
use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Window,
};
use std::path::PathBuf;
use theme::{bg, border, text, FONT_FAMILY};
use ui::{Button, ButtonVariant};

use crate::actions::*;
use crate::ui::thread_view::ThreadView;

/// Main screen for MechaCoder.
pub struct MechaCoderScreen {
    /// Focus handle.
    focus_handle: FocusHandle,
    /// Current project root.
    project_root: PathBuf,
    /// Claude Code connection.
    claude_code: ClaudeCode,
    /// Current thread view.
    thread_view: Option<Entity<ThreadView>>,
    /// Connection status.
    connection_status: ConnectionStatus,
    /// Error message if any.
    error_message: Option<String>,
    /// Whether we need to focus the input on next render.
    needs_focus: bool,
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

        let mut screen = Self {
            focus_handle,
            project_root,
            claude_code: ClaudeCode::new(),
            thread_view: None,
            connection_status: ConnectionStatus::Connecting,
            error_message: None,
            needs_focus: false,
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

    /// Connect to Claude Code and start a new thread.
    fn connect(&mut self, cx: &mut Context<Self>) {
        self.connection_status = ConnectionStatus::Connecting;
        self.error_message = None;
        cx.notify();

        let project_root = self.project_root.clone();

        // Get the connect task before spawning (requires &mut App context)
        let connect_task = self.claude_code.connect(&project_root, cx);

        cx.spawn::<_, anyhow::Result<()>>(async move |this, cx| {
            // Connect to Claude Code
            let connection = match connect_task.await {
                Ok(conn) => conn,
                Err(e) => {
                    this.update(cx, |this, cx| {
                        this.connection_status = ConnectionStatus::Error(e.to_string());
                        this.error_message = Some(e.to_string());
                        cx.notify();
                    })
                    .ok();
                    return Ok(());
                }
            };

            // Create a new thread - need to do this inside update() to get &mut App context
            let project = Project::local(&project_root);
            let thread_result = this
                .update(cx, |_this, cx| {
                    ClaudeCode::new_thread(connection, project, cx)
                })
                .ok();

            let thread = match thread_result {
                Some(task) => match task.await {
                    Ok(thread) => thread,
                    Err(e) => {
                        this.update(cx, |this, cx| {
                            this.connection_status = ConnectionStatus::Error(e.to_string());
                            this.error_message = Some(e.to_string());
                            cx.notify();
                        })
                        .ok();
                        return Ok(());
                    }
                },
                None => return Ok(()),
            };

            // Create thread view and set flag to focus the input
            this.update(cx, |this, cx| {
                let thread_view = cx.new(|cx| ThreadView::new(thread.clone(), cx));
                this.thread_view = Some(thread_view);
                this.connection_status = ConnectionStatus::Connected;
                this.needs_focus = true;
                cx.notify();
            })
            .ok();
            Ok(())
        })
        .detach();
    }

    /// Handle the Quit action.
    fn quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
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

        div()
            .id("mechacoder-root")
            .key_context("MechaCoder")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(bg::APP)
            .font_family(FONT_FAMILY)
            .text_color(text::PRIMARY)
            .on_action(cx.listener(Self::quit))
            .child(match &self.connection_status {
                ConnectionStatus::Connecting => self.render_connecting().into_any_element(),
                ConnectionStatus::Connected => self.render_connected().into_any_element(),
                ConnectionStatus::Error(_) => self.render_error(cx).into_any_element(),
            })
    }
}
