//! Main MechaCoder screen component.

use gpui::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, Focusable,
    InteractiveElement, IntoElement, ParentElement, Render, Styled, Subscription, Window,
};
use std::path::PathBuf;
use terminalbench::TBRunStatus;
use theme_oa::{bg, border, text, FONT_FAMILY};
use ui_oa::{Button, ButtonVariant};

use crate::actions::*;
use crate::panels::{GymPanel, GymPanelEvent, TBenchRunner, TBenchRunnerEvent, TBRunOptions};
use crate::sdk_thread::{SdkThread, TBenchRunEntry, TBenchStreamEntry};
use crate::ui::thread_view::ThreadView;

/// Main screen for MechaCoder.
pub struct MechaCoderScreen {
    /// Focus handle.
    focus_handle: FocusHandle,
    /// Current project root.
    project_root: PathBuf,
    /// SDK thread for conversation.
    sdk_thread: Option<Entity<SdkThread>>,
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
    /// TBench runner for TB2 execution.
    tbench_runner: TBenchRunner,
    /// Subscription to gym panel events.
    _gym_panel_subscription: Subscription,
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

        // Subscribe to gym panel events
        let gym_panel_subscription = cx.subscribe(&gym_panel, |this, _panel, event, cx| {
            this.handle_gym_panel_event(event, cx);
        });

        // Create TBench runner
        let tbench_runner = TBenchRunner::new(project_root.clone());

        let mut screen = Self {
            focus_handle,
            project_root,
            sdk_thread: None,
            thread_view: None,
            connection_status: ConnectionStatus::Connecting,
            error_message: None,
            needs_focus: false,
            gym_panel,
            gym_panel_visible: false,
            tbench_runner,
            _gym_panel_subscription: gym_panel_subscription,
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
        self.sdk_thread = Some(thread.clone());

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
    fn toggle_gym_panel(&mut self, _: &ToggleGymPanel, window: &mut Window, cx: &mut Context<Self>) {
        self.gym_panel_visible = !self.gym_panel_visible;

        // When closing the panel, refocus the message input so keybindings keep working
        if !self.gym_panel_visible {
            if let Some(thread_view) = &self.thread_view {
                let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
                focus_handle.focus(window);
            }
        }

        cx.notify();
    }

    /// Focus the message input.
    fn focus_message_input(&mut self, _: &FocusMessageInput, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(thread_view) = &self.thread_view {
            let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
            focus_handle.focus(window);
        }
    }

    /// Handle gym panel events
    fn handle_gym_panel_event(&mut self, event: &GymPanelEvent, cx: &mut Context<Self>) {
        match event {
            GymPanelEvent::StartTB2Run { run_id, task, model } => {
                log::info!("Starting TB2 run: {} for task {}", run_id, task.id);

                // Add TB2 run entry to thread
                if let Some(sdk_thread) = &self.sdk_thread {
                    sdk_thread.update(cx, |thread, cx| {
                        thread.add_tbench_run_entry(TBenchRunEntry {
                            run_id: run_id.clone(),
                            task_id: task.id.clone(),
                            task_name: task.name.clone(),
                            status: TBRunStatus::Running,
                            turns: 0,
                            max_turns: task.max_turns,
                            cost: None,
                            error: None,
                        }, cx);
                    });
                }

                // Start the TBench runner
                let options = TBRunOptions {
                    task: task.clone(),
                    model: Some(model.id().to_string()),
                    timeout_secs: (task.timeout_ms / 1000) as u64,
                    max_turns: task.max_turns,
                };

                let (_runner_run_id, mut rx) = self.tbench_runner.start_run(options, cx);
                let run_id = run_id.clone();
                let gym_panel = self.gym_panel.clone();
                let sdk_thread = self.sdk_thread.clone();

                // Spawn task to process runner events
                cx.spawn(async move |_this, cx| {
                    while let Some(event) = rx.recv().await {
                        match &event {
                            TBenchRunnerEvent::StreamEvent(stream_event) => {
                                // Add to thread
                                if let Some(sdk_thread) = &sdk_thread {
                                    let _ = sdk_thread.update(cx, |thread, cx| {
                                        thread.add_tbench_stream_entry(TBenchStreamEntry {
                                            run_id: run_id.clone(),
                                            event: stream_event.clone(),
                                        }, cx);
                                    });
                                }

                                // Update gym panel
                                let _ = gym_panel.update(cx, |panel, cx| {
                                    panel.handle_tb2_event(&run_id, stream_event, cx);
                                });
                            }
                            TBenchRunnerEvent::RunComplete { run_id, success, turns, cost, error } => {
                                // Update gym panel
                                let _ = gym_panel.update(cx, |panel, cx| {
                                    panel.handle_tb2_complete(
                                        run_id,
                                        *success,
                                        *turns,
                                        *cost,
                                        error.clone(),
                                        cx,
                                    );
                                });
                            }
                            TBenchRunnerEvent::Error(err) => {
                                log::error!("TBench runner error: {}", err);
                            }
                            TBenchRunnerEvent::RunStart { .. } => {
                                // Already handled above
                            }
                        }
                    }
                }).detach();
            }
            GymPanelEvent::TB2StreamEvent { .. } | GymPanelEvent::TB2RunComplete { .. } => {
                // These are forwarded events, ignore in screen handler
            }
        }
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
        // Defer focus to next frame so elements are in the tree
        if self.needs_focus {
            self.needs_focus = false;
            if let Some(thread_view) = &self.thread_view {
                let focus_handle = thread_view.read(cx).message_input_focus_handle(cx);
                cx.defer_in(window, move |_this, window, _cx| {
                    focus_handle.focus(window);
                });
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
            .on_action(cx.listener(Self::focus_message_input))
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
