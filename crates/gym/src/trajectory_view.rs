//! Trajectory View component
//!
//! Displays trajectory list and detail viewer for browsing ATIF trajectories.

use gpui::prelude::*;
use gpui::*;
use std::sync::{Arc, Mutex};
use atif_store::{TrajectoryStore, TrajectoryMetadata, TrajectoryStatus};
use atif::{Step, StepSource};
use theme::{bg, border, status, text, FONT_FAMILY};

pub struct TrajectoryView {
    /// Trajectory store
    store: Option<Arc<Mutex<TrajectoryStore>>>,
    /// Loaded trajectories
    trajectories: Vec<TrajectoryMetadata>,
    /// Selected trajectory ID
    selected_id: Option<String>,
    /// Steps for selected trajectory
    selected_steps: Vec<Step>,
    /// Loading state
    #[allow(dead_code)]
    loading: bool,
    /// Focus handle
    focus_handle: FocusHandle,
}

impl TrajectoryView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            store: None,
            trajectories: vec![],
            selected_id: None,
            selected_steps: vec![],
            loading: false,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the trajectory store and load trajectories
    pub fn set_store(&mut self, store: Arc<Mutex<TrajectoryStore>>, cx: &mut Context<Self>) {
        self.store = Some(store);
        self.refresh(cx);
    }

    /// Refresh trajectory list from store
    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if let Some(ref store) = self.store {
            if let Ok(guard) = store.lock() {
                if let Ok(trajectories) = guard.list_trajectories(100, 0) {
                    self.trajectories = trajectories;
                }
            }
        }
        cx.notify();
    }

    /// Select a trajectory and load its steps
    fn select_trajectory(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected_id = Some(id.clone());
        self.selected_steps.clear();

        if let Some(ref store) = self.store {
            if let Ok(guard) = store.lock() {
                if let Ok(trajectory) = guard.get_trajectory(&id) {
                    self.selected_steps = trajectory.steps;
                }
            }
        }
        cx.notify();
    }

    fn status_color(&self, status: TrajectoryStatus) -> (Hsla, Hsla) {
        match status {
            TrajectoryStatus::Completed => (status::SUCCESS, status::SUCCESS_BG),
            TrajectoryStatus::Failed => (status::ERROR, status::ERROR_BG),
            TrajectoryStatus::InProgress => (status::WARNING, status::WARNING_BG),
        }
    }

    fn format_time(&self, dt: &chrono::DateTime<chrono::Utc>) -> String {
        dt.format("%m-%d %H:%M").to_string()
    }

    fn render_trajectory_item(&self, traj: &TrajectoryMetadata, cx: &mut Context<Self>) -> impl IntoElement {
        let is_selected = self.selected_id.as_ref() == Some(&traj.session_id);
        let (status_color, status_bg) = self.status_color(traj.status);
        let session_id_for_click = traj.session_id.clone();

        let status_label = match traj.status {
            TrajectoryStatus::Completed => "done",
            TrajectoryStatus::Failed => "failed",
            TrajectoryStatus::InProgress => "running",
        };

        div()
            .id(SharedString::from(traj.session_id.clone()))
            .p(px(12.0))
            .cursor_pointer()
            .border_b_1()
            .border_color(border::SUBTLE)
            .when(is_selected, |el| {
                el.bg(bg::SELECTED)
                    .border_l_2()
                    .border_color(border::SELECTED)
            })
            .when(!is_selected, |el| {
                el.bg(bg::ROW)
                    .hover(|el| el.bg(bg::HOVER))
            })
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.select_trajectory(session_id_for_click.clone(), cx);
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    // Header row
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(13.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(if is_selected { text::BRIGHT } else { text::PRIMARY })
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(traj.agent_name.clone())
                            )
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .rounded(px(4.0))
                                    .text_size(px(9.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(status_color)
                                    .bg(status_bg)
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(status_label.to_uppercase())
                            )
                    )
                    // Details row
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} steps", traj.total_steps))
                            .child(self.format_time(&traj.created_at))
                    )
                    // Model info
                    .when_some(traj.model_name.clone(), |el, model| {
                        el.child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::DISABLED)
                                .child(model)
                        )
                    })
            )
    }

    fn render_trajectory_list(&self, cx: &mut Context<Self>) -> AnyElement {
        let is_empty = self.trajectories.is_empty();
        let count = self.trajectories.len();

        // Pre-render items to avoid borrow issues in closures
        let items: Vec<AnyElement> = if !is_empty {
            self.trajectories.clone().iter()
                .map(|t| self.render_trajectory_item(t, cx).into_any_element())
                .collect()
        } else {
            vec![]
        };

        div()
            .w(px(300.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(bg::SURFACE)
            .border_r_1()
            .border_color(border::DEFAULT)
            // Header
            .child(
                div()
                    .px(px(16.0))
                    .py(px(12.0))
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(14.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::PRIMARY)
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Trajectories")
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{}", count))
                    )
            )
            // List
            .child(
                div()
                    .id("trajectory-list-scroll")
                    .flex_1()
                    .overflow_y_scroll()
                    .when(is_empty, |el| {
                        el.child(
                            div()
                                .p(px(24.0))
                                .text_center()
                                .text_size(px(13.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("No trajectories yet")
                        )
                    })
                    .when(!is_empty, |el| {
                        el.children(items)
                    })
            )
            .into_any_element()
    }

    fn render_step(&self, step: &Step, index: usize) -> impl IntoElement {
        let (source_color, source_label) = match step.source {
            StepSource::User => (status::INFO, "USER"),
            StepSource::Agent => (status::SUCCESS, "AGENT"),
            StepSource::System => (text::MUTED, "SYSTEM"),
        };

        div()
            .mb(px(12.0))
            .bg(bg::CARD)
            .border_1()
            .border_color(border::DEFAULT)
            .rounded(px(8.0))
            .overflow_hidden()
            // Step header
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .bg(bg::SURFACE)
                    .border_b_1()
                    .border_color(border::SUBTLE)
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::DISABLED)
                            .child(format!("#{}", index + 1))
                    )
                    .child(
                        div()
                            .px(px(6.0))
                            .py(px(2.0))
                            .rounded(px(4.0))
                            .bg(source_color.opacity(0.15))
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(source_color)
                            .font_weight(FontWeight::MEDIUM)
                            .child(source_label)
                    )
            )
            // Step content
            .child(
                div()
                    .p(px(12.0))
                    .text_size(px(13.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .child(step.message.clone())
            )
    }

    fn render_detail_view(&self) -> impl IntoElement {
        let selected = self.selected_id.as_ref()
            .and_then(|id| self.trajectories.iter().find(|t| &t.session_id == id));

        match selected {
            Some(traj) => {
                let (status_color, _) = self.status_color(traj.status);

                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .bg(bg::APP)
                    // Header
                    .child(
                        div()
                            .px(px(20.0))
                            .py(px(16.0))
                            .border_b_1()
                            .border_color(border::DEFAULT)
                            .bg(bg::SURFACE)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(4.0))
                                            .child(
                                                div()
                                                    .text_size(px(16.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::BRIGHT)
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(traj.agent_name.clone())
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .font_family(FONT_FAMILY)
                                                    .text_color(text::MUTED)
                                                    .child(format!("Session: {}", &traj.session_id[..8.min(traj.session_id.len())]))
                                            )
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(16.0))
                                            .text_size(px(12.0))
                                            .font_family(FONT_FAMILY)
                                            .child(
                                                div()
                                                    .text_color(text::MUTED)
                                                    .child(format!("{} steps", traj.total_steps))
                                            )
                                            .child(
                                                div()
                                                    .text_color(status_color)
                                                    .child(match traj.status {
                                                        TrajectoryStatus::Completed => "Completed",
                                                        TrajectoryStatus::Failed => "Failed",
                                                        TrajectoryStatus::InProgress => "In Progress",
                                                    })
                                            )
                                    )
                            )
                    )
                    // Steps
                    .child(
                        div()
                            .id("steps-scroll")
                            .flex_1()
                            .overflow_y_scroll()
                            .p(px(20.0))
                            .when(self.selected_steps.is_empty(), |el| {
                                el.child(
                                    div()
                                        .p(px(24.0))
                                        .text_center()
                                        .text_size(px(13.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child("No steps recorded")
                                )
                            })
                            .when(!self.selected_steps.is_empty(), |el| {
                                el.children(
                                    self.selected_steps.iter().enumerate()
                                        .map(|(i, step)| self.render_step(step, i))
                                )
                            })
                    )
                    .into_any_element()
            }
            None => {
                // Empty state
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(bg::APP)
                    .child(
                        div()
                            .text_center()
                            .child(
                                div()
                                    .text_size(px(32.0))
                                    .text_color(text::DISABLED)
                                    .mb(px(12.0))
                                    .child("📊")
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("Select a trajectory to view details")
                            )
                    )
                    .into_any_element()
            }
        }
    }
}

impl Focusable for TrajectoryView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TrajectoryView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Render components
        let list = self.render_trajectory_list(cx);
        let detail = self.render_detail_view();

        div()
            .flex()
            .h_full()
            .w_full()
            .bg(bg::APP)
            // Left panel: Trajectory list
            .child(list)
            // Right panel: Detail view
            .child(detail)
    }
}
