//! Main MechaCoder screen component.

use acp::{AcpThread, AcpThreadEvent, ClaudeCode, Project, ThreadEntry, ThreadStatus};
use gpui::{
    div, prelude::*, px, App, Context, Entity, EventEmitter, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled, Task, Window,
};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use theme::{bg, border, text, FONT_FAMILY};
use ui::{Button, ButtonVariant, TextInput};

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
}

/// Connection status.
#[derive(Clone, Debug, Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
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

        Self {
            focus_handle,
            project_root,
            claude_code: ClaudeCode::new(),
            thread_view: None,
            connection_status: ConnectionStatus::Disconnected,
            error_message: None,
        }
    }

    /// Set the project root directory.
    pub fn set_project_root(&mut self, path: impl Into<PathBuf>) {
        self.project_root = path.into();
    }

    /// Connect to Claude Code and start a new thread.
    pub fn connect(&mut self, cx: &mut Context<Self>) {
        self.connection_status = ConnectionStatus::Connecting;
        self.error_message = None;
        cx.notify();

        let project_root = self.project_root.clone();
        let claude_code = self.claude_code.clone();

        cx.spawn::<_, ()>(|this: gpui::WeakEntity<Self>, mut cx: gpui::AsyncApp| async move {
            // Connect to Claude Code
            let connection = match claude_code.connect(&project_root, &mut cx).await {
                Ok(conn) => conn,
                Err(e) => {
                    this.update(&mut cx, |this, cx| {
                        this.connection_status = ConnectionStatus::Error(e.to_string());
                        this.error_message = Some(e.to_string());
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            // Create a new thread
            let project = Project::local(&project_root);
            let thread = match ClaudeCode::new_thread(connection, project, &mut cx).await {
                Ok(thread) => thread,
                Err(e) => {
                    this.update(&mut cx, |this, cx| {
                        this.connection_status = ConnectionStatus::Error(e.to_string());
                        this.error_message = Some(e.to_string());
                        cx.notify();
                    })
                    .ok();
                    return;
                }
            };

            // Create thread view
            this.update(&mut cx, |this, cx| {
                let thread_view = cx.new(|cx| ThreadView::new(thread.clone(), cx));
                this.thread_view = Some(thread_view);
                this.connection_status = ConnectionStatus::Connected;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Handle the Quit action.
    fn quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Render the disconnected state.
    fn render_disconnected(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .gap(px(16.0))
            .child(
                div()
                    .text_xl()
                    .text_color(text::PRIMARY)
                    .child("MechaCoder"),
            )
            .child(
                div()
                    .text_color(text::SECONDARY)
                    .child("Claude Code Harness"),
            )
            .child(
                div()
                    .mt(px(24.0))
                    .child(
                        Button::new("Connect to Claude Code")
                            .variant(ButtonVariant::Default)
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.connect(cx);
                            })),
                    ),
            )
            .when_some(self.error_message.as_ref(), |el, error| {
                el.child(
                    div()
                        .mt(px(16.0))
                        .px(px(16.0))
                        .py(px(8.0))
                        .rounded(px(4.0))
                        .bg(bg::CARD)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .text_color(text::PRIMARY)
                        .max_w(px(400.0))
                        .child(error.clone()),
                )
            })
    }

    /// Render the connecting state.
    fn render_connecting(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .size_full()
            .gap(px(16.0))
            .child(
                div()
                    .text_xl()
                    .text_color(text::PRIMARY)
                    .child("Connecting..."),
            )
            .child(
                div()
                    .text_color(text::SECONDARY)
                    .child("Establishing connection to Claude Code"),
            )
    }

    /// Render the connected state with thread view.
    fn render_connected(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(thread_view) = &self.thread_view {
            div()
                .size_full()
                .child(thread_view.clone())
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(text::SECONDARY)
                        .child("No active thread"),
                )
        }
    }
}

impl Focusable for MechaCoderScreen {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for MechaCoderScreen {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                ConnectionStatus::Disconnected => self.render_disconnected(cx).into_any_element(),
                ConnectionStatus::Connecting => self.render_connecting().into_any_element(),
                ConnectionStatus::Connected => self.render_connected(cx).into_any_element(),
                ConnectionStatus::Error(_) => self.render_disconnected(cx).into_any_element(),
            })
    }
}
