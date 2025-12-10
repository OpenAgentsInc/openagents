//! TBCC Settings Tab - Configuration for TB execution and logging

use gpui::prelude::*;
use gpui::*;
use theme::{bg, border, status, text, FONT_FAMILY};

use super::types::{TBModelOption, ExecutionSettings, ContainerSettings};
use crate::services::{SettingsStore, TBCCSettings};

pub struct SettingsView {
    execution: ExecutionSettings,
    container: ContainerSettings,
    saved: bool,
    settings_store: Option<SettingsStore>,
    focus_handle: FocusHandle,
}

impl SettingsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            execution: ExecutionSettings::default(),
            container: ContainerSettings::default(),
            saved: false,
            settings_store: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Set the settings store and load persisted settings
    pub fn set_settings_store(&mut self, store: SettingsStore, cx: &mut Context<Self>) {
        let settings = store.load();
        self.execution = settings.execution;
        self.container = settings.container;
        self.settings_store = Some(store);
        cx.notify();
    }

    /// Save current settings to disk
    pub fn save(&mut self, cx: &mut Context<Self>) {
        if let Some(ref store) = self.settings_store {
            let settings = TBCCSettings {
                execution: self.execution.clone(),
                container: self.container.clone(),
            };
            if store.save(&settings).is_ok() {
                self.saved = true;
                cx.notify();
            }
        }
    }

    /// Reset to defaults
    pub fn reset(&mut self, cx: &mut Context<Self>) {
        self.execution = ExecutionSettings::default();
        self.container = ContainerSettings::default();
        self.saved = false;
        if let Some(ref store) = self.settings_store {
            let _ = store.reset();
        }
        cx.notify();
    }

    /// Get current execution settings
    pub fn get_execution_settings(&self) -> &ExecutionSettings {
        &self.execution
    }

    /// Get current container settings
    pub fn get_container_settings(&self) -> &ContainerSettings {
        &self.container
    }

    fn render_section_header(&self, icon: &str, title: &str) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(8.0))
            .mb(px(16.0))
            .child(
                div()
                    .text_size(px(16.0))
                    .child(icon.to_string())
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(title.to_string())
            )
    }

    fn render_model_card(&self, model: TBModelOption, is_selected: bool, title: &str, subtitle: &str) -> impl IntoElement {
        let border_color = if is_selected {
            status::SUCCESS.opacity(0.5)
        } else {
            border::DEFAULT
        };
        let bg_color = if is_selected {
            status::SUCCESS_BG
        } else {
            bg::CARD
        };
        let text_color = if is_selected {
            status::SUCCESS
        } else {
            text::MUTED
        };

        div()
            .flex_1()
            .p(px(16.0))
            .rounded(px(8.0))
            .bg(bg_color)
            .border_1()
            .border_color(border_color)
            .cursor_pointer()
            .hover(|el| el.border_color(if is_selected { border_color } else { border::STRONG }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.0))
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(if is_selected { text::BRIGHT } else { text::PRIMARY })
                            .font_weight(FontWeight::MEDIUM)
                            .child(title.to_string())
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text_color)
                            .child(subtitle.to_string())
                    )
            )
    }

    fn render_input_field(&self, label: &str, value: String, hint: Option<&str>) -> impl IntoElement {
        let label = label.to_string();
        let hint = hint.map(|s| s.to_string());

        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                div()
                    .text_size(px(11.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::MUTED)
                    .font_weight(FontWeight::MEDIUM)
                    .child(label)
            )
            .child(
                div()
                    .px(px(12.0))
                    .py(px(8.0))
                    .bg(bg::ELEVATED)
                    .border_1()
                    .border_color(border::DEFAULT)
                    .rounded(px(6.0))
                    .text_size(px(13.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .child(value)
            )
            .when_some(hint, |el, hint| {
                el.child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::DISABLED)
                        .child(hint)
                )
            })
    }

    fn render_checkbox(&self, label: &str, checked: bool) -> impl IntoElement {
        let label = label.to_string();

        div()
            .flex()
            .items_center()
            .gap(px(12.0))
            .cursor_pointer()
            .child(
                div()
                    .w(px(18.0))
                    .h(px(18.0))
                    .rounded(px(4.0))
                    .border_1()
                    .border_color(if checked { status::SUCCESS.opacity(0.5) } else { border::DEFAULT })
                    .bg(if checked { status::SUCCESS_BG } else { bg::ELEVATED })
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(checked, |el| {
                        el.child(
                            div()
                                .text_size(px(12.0))
                                .text_color(status::SUCCESS)
                                .child("✓")
                        )
                    })
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .font_family(FONT_FAMILY)
                    .text_color(text::PRIMARY)
                    .child(label)
            )
    }
}

impl Focusable for SettingsView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("settings-scroll")
            .h_full()
            .overflow_y_scroll()
            .p(px(32.0))
            .bg(bg::APP)
            .child(
                div()
                    .max_w(px(640.0))
                    .mx_auto()
                    // Header
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mb(px(32.0))
                            .child(
                                div()
                                    .text_size(px(20.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::BRIGHT)
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Settings")
                            )
                            .when(self.saved, |el| {
                                el.child(
                                    div()
                                        .text_size(px(13.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(status::SUCCESS)
                                        .child("Settings saved!")
                                )
                            })
                    )
                    // Model Section
                    .child(
                        div()
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .rounded(px(12.0))
                            .p(px(24.0))
                            .mb(px(24.0))
                            .child(self.render_section_header("🤖", "Model"))
                            .child(
                                div()
                                    .flex()
                                    .gap(px(16.0))
                                    .child(self.render_model_card(
                                        TBModelOption::AppleFM,
                                        self.execution.model == TBModelOption::AppleFM,
                                        "Foundation Model",
                                        "Apple on-device (default)"
                                    ))
                                    .child(self.render_model_card(
                                        TBModelOption::ClaudeSonnet,
                                        self.execution.model == TBModelOption::ClaudeSonnet,
                                        "Claude Sonnet",
                                        "Cloud-based"
                                    ))
                            )
                    )
                    // Execution Section
                    .child(
                        div()
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .rounded(px(12.0))
                            .p(px(24.0))
                            .mb(px(24.0))
                            .child(self.render_section_header("⚡", "Execution"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.0))
                                    // Grid of inputs
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(16.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .child(self.render_input_field(
                                                        "Max Attempts",
                                                        format!("{}", self.execution.max_attempts),
                                                        Some("Retries per task")
                                                    ))
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .child(self.render_input_field(
                                                        "Max Tokens",
                                                        format!("{}", self.execution.max_tokens),
                                                        Some("Token limit per turn")
                                                    ))
                                            )
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(16.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .child(self.render_input_field(
                                                        "Timeout (ms)",
                                                        format!("{}", self.execution.timeout_ms),
                                                        Some("Max execution time")
                                                    ))
                                            )
                                    )
                                    // Checkboxes
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(12.0))
                                            .mt(px(8.0))
                                            .child(self.render_checkbox("Save trajectories (JSON)", self.execution.save_trajectories))
                                    )
                            )
                    )
                    // Container Section
                    .child(
                        div()
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .rounded(px(12.0))
                            .p(px(24.0))
                            .mb(px(24.0))
                            .child(self.render_section_header("📦", "Container"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(16.0))
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(16.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .child(self.render_input_field(
                                                        "Image",
                                                        self.container.image.clone(),
                                                        Some("Docker/Container image to use")
                                                    ))
                                            )
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap(px(16.0))
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .child(self.render_input_field(
                                                        "Memory Limit",
                                                        self.container.memory_limit.clone(),
                                                        None
                                                    ))
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .child(self.render_input_field(
                                                        "CPU Limit",
                                                        format!("{}", self.container.cpu_limit),
                                                        None
                                                    ))
                                            )
                                    )
                                    .child(self.render_checkbox("Auto-remove container after run", self.container.auto_remove))
                            )
                    )
                    // Logging Section
                    .child(
                        div()
                            .bg(bg::CARD)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .rounded(px(12.0))
                            .p(px(24.0))
                            .mb(px(32.0))
                            .child(self.render_section_header("📝", "Logging & Storage"))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(12.0))
                                    .child(self.render_checkbox("Save full trajectories (JSON)", true))
                                    .child(self.render_checkbox("Save terminal output logs", true))
                                    .child(self.render_checkbox("Save ATIF traces", false))
                            )
                    )
                    // Actions
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(16.0))
                            .child(
                                div()
                                    .px(px(16.0))
                                    .py(px(10.0))
                                    .text_size(px(13.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .cursor_pointer()
                                    .hover(|el| el.text_color(text::PRIMARY))
                                    .child("Reset to Defaults")
                            )
                            .child(
                                div()
                                    .px(px(24.0))
                                    .py(px(10.0))
                                    .bg(status::SUCCESS)
                                    .text_size(px(13.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::BRIGHT)
                                    .font_weight(FontWeight::MEDIUM)
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .hover(|el| el.bg(status::SUCCESS.opacity(0.8)))
                                    .shadow_lg()
                                    .child("Save Changes")
                            )
                    )
            )
    }
}
