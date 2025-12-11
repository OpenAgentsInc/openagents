//! DVM list component - Data Vending Machine listings
//!
//! A GPUI Entity that displays NIP-90 DVMs with clickable USE buttons.

use gpui::prelude::*;
use gpui::*;
use std::sync::Arc;
use theme::{bg, border, text, accent, FONT_FAMILY};

use crate::types::{DVMListing, PricingUnit};

/// Callback type for when USE button is clicked on a DVM
pub type OnUseCallback = Arc<dyn Fn(&DVMListing, &mut Window, &mut App) + Send + Sync>;

/// DVM list component - displays DVMs with USE button click handling
pub struct DVMList {
    dvms: Vec<DVMListing>,
    selected_id: Option<String>,
    focus_handle: FocusHandle,
    on_use: Option<OnUseCallback>,
}

impl DVMList {
    /// Create a new DVM list with mock data
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            dvms: mock_dvms(),
            selected_id: None,
            focus_handle: cx.focus_handle(),
            on_use: None,
        }
    }

    /// Create a new DVM list with specific DVMs
    pub fn with_dvms(cx: &mut Context<Self>, dvms: Vec<DVMListing>) -> Self {
        Self {
            dvms,
            selected_id: None,
            focus_handle: cx.focus_handle(),
            on_use: None,
        }
    }

    /// Set the DVMs to display
    pub fn set_dvms(&mut self, dvms: Vec<DVMListing>) {
        self.dvms = dvms;
    }

    /// Add a DVM to the list
    pub fn add_dvm(&mut self, dvm: DVMListing) {
        self.dvms.push(dvm);
    }

    /// Set the callback for when USE button is clicked
    pub fn set_on_use(&mut self, callback: OnUseCallback) {
        self.on_use = Some(callback);
    }

    /// Get the list of DVMs
    pub fn dvms(&self) -> &[DVMListing] {
        &self.dvms
    }

    /// Merge discovered DVMs with existing ones (mock fallback)
    pub fn merge_discovered(&mut self, discovered: Vec<DVMListing>) {
        use std::collections::HashSet;
        let discovered_kinds: HashSet<u16> = discovered.iter().map(|d| d.kind).collect();
        // Keep mocks for kinds not yet discovered
        self.dvms.retain(|d| !discovered_kinds.contains(&d.kind));
        self.dvms.extend(discovered);
    }

    /// Render a clickable DVM row with USE button
    fn render_dvm_row(&self, dvm: &DVMListing, idx: usize, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let usage_percent = (dvm.request_count as f32 / 10_000.0).min(1.0);
        let is_selected = self.selected_id.as_deref() == Some(&dvm.id);
        let dvm_id = dvm.id.clone();
        let dvm_clone = dvm.clone();

        div()
            .id(ElementId::Name(format!("dvm-row-{}", idx).into()))
            .w_full()
            .flex()
            .flex_col()
            .gap(px(8.0))
            .px(px(16.0))
            .py(px(12.0))
            .border_b_1()
            .border_color(border::SUBTLE)
            .bg(if is_selected { bg::SELECTED } else { bg::CARD })
            .cursor_pointer()
            .hover(|s| s.bg(if is_selected { bg::SELECTED } else { bg::ROW }))
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.selected_id = Some(dvm_id.clone());
                cx.notify();
            }))
            // Top row: icon, name, pricing
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .text_size(px(16.0))
                                    .child(get_dvm_icon(dvm.kind)),
                            )
                            .child(
                                div()
                                    .text_size(px(14.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::PRIMARY)
                                    .child(dvm.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .font_family(FONT_FAMILY)
                            .text_color(accent::PRIMARY)
                            .child(format!("{} sats{}", dvm.sats_per_unit, dvm.pricing_unit.label())),
                    ),
            )
            // Middle row: usage bar
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        div()
                            .flex_1()
                            .h(px(6.0))
                            .bg(bg::ELEVATED)
                            .rounded(px(3.0))
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(usage_percent))
                                    .bg(accent::PRIMARY)
                                    .rounded(px(3.0)),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(format!("{} req", format_k(dvm.request_count))),
                    ),
            )
            // Bottom row: provider, rating, USE button
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::MUTED)
                            .child(dvm.provider_name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .child("⭐"),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(theme::status::WARNING)
                                            .child(format!("{:.1}", dvm.rating)),
                                    ),
                            )
                            .child(self.render_use_button(dvm_clone, cx)),
                    ),
            )
    }

    /// Render the USE button with click handler
    fn render_use_button(&self, dvm: DVMListing, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        div()
            .id(ElementId::Name(format!("use-btn-{}", dvm.id).into()))
            .px(px(10.0))
            .py(px(4.0))
            .bg(accent::PRIMARY_MUTED)
            .rounded(px(4.0))
            .cursor_pointer()
            .hover(|s| s.bg(accent::PRIMARY))
            .on_click(cx.listener(move |this, _event, window, cx| {
                // Call the on_use callback if set
                if let Some(callback) = &this.on_use {
                    callback(&dvm, window, cx);
                }
            }))
            .child(
                div()
                    .text_size(px(10.0))
                    .font_family(FONT_FAMILY)
                    .text_color(accent::PRIMARY)
                    .child("USE"),
            )
    }
}

impl Focusable for DVMList {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for DVMList {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Pre-render all rows using a for loop (closures can't capture cx)
        let mut dvm_rows = Vec::with_capacity(self.dvms.len());
        for (idx, dvm) in self.dvms.iter().enumerate() {
            dvm_rows.push(self.render_dvm_row(dvm, idx, cx));
        }

        div()
            .track_focus(&self.focus_handle)
            .w_full()
            .flex()
            .flex_col()
            .bg(bg::CARD)
            .border_1()
            .border_color(border::DEFAULT)
            .rounded(px(8.0))
            .overflow_hidden()
            // Header
            .child(render_section_header())
            // DVM rows
            .when(!dvm_rows.is_empty(), |el| el.children(dvm_rows))
    }
}

/// Render the section header
fn render_section_header() -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(12.0))
        .bg(bg::HEADER)
        .border_b_1()
        .border_color(border::DEFAULT)
        .child(
            div()
                .text_size(px(14.0))
                .child("🔥"),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("POPULAR DATA VENDING MACHINES"),
        )
}

/// Get icon for DVM kind (NIP-90 job kinds 5000-5999)
fn get_dvm_icon(kind: u16) -> &'static str {
    match kind {
        // Text extraction / OCR (KIND_JOB_TEXT_EXTRACTION = 5000)
        5000 => "🎙️",
        // Summarization (KIND_JOB_SUMMARIZATION = 5001)
        5001 => "📋",
        // Translation (KIND_JOB_TRANSLATION = 5002)
        5002 => "📝",
        // Text generation / Chat (KIND_JOB_TEXT_GENERATION = 5050)
        5050..=5099 => "🤖",
        // Image generation (KIND_JOB_IMAGE_GENERATION = 5100)
        5100..=5199 => "🖼️",
        // Speech to text (KIND_JOB_SPEECH_TO_TEXT = 5250)
        5250..=5299 => "🎤",
        // Other services
        _ => "🔧",
    }
}

/// Format number with K suffix
fn format_k(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Generate mock DVMs for UI development
pub fn mock_dvms() -> Vec<DVMListing> {
    vec![
        DVMListing::mock("Whisper Transcription", 5000, 50, PricingUnit::PerMinute),
        DVMListing::mock("GPT-4 Vision Analysis", 5100, 200, PricingUnit::PerImage),
        DVMListing::mock("DeepSeek R1 Inference", 5050, 5, PricingUnit::Per1KTokens),
        DVMListing::mock("Claude Translation", 5002, 10, PricingUnit::Per1KTokens),
    ]
}

// Keep the old render function for backward compatibility during transition
pub fn render_dvm_list(dvms: &[DVMListing]) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(8.0))
        .overflow_hidden()
        // Header
        .child(render_section_header())
        // DVM rows (non-interactive for backward compatibility)
        .children(dvms.iter().map(|dvm| {
            render_static_dvm_row(dvm)
        }))
}

/// Render a static DVM row (for backward compatibility)
fn render_static_dvm_row(dvm: &DVMListing) -> impl IntoElement {
    let usage_percent = (dvm.request_count as f32 / 10_000.0).min(1.0);

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(border::SUBTLE)
        .cursor_pointer()
        .hover(|s| s.bg(bg::ROW))
        // Top row: icon, name, pricing
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(16.0))
                                .child(get_dvm_icon(dvm.kind)),
                        )
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .child(dvm.name.clone()),
                        ),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_family(FONT_FAMILY)
                        .text_color(accent::PRIMARY)
                        .child(format!("{} sats{}", dvm.sats_per_unit, dvm.pricing_unit.label())),
                ),
        )
        // Middle row: usage bar
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .flex_1()
                        .h(px(6.0))
                        .bg(bg::ELEVATED)
                        .rounded(px(3.0))
                        .overflow_hidden()
                        .child(
                            div()
                                .h_full()
                                .w(relative(usage_percent))
                                .bg(accent::PRIMARY)
                                .rounded(px(3.0)),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("{} req", format_k(dvm.request_count))),
                ),
        )
        // Bottom row: provider, rating, action
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(dvm.provider_name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .child("⭐"),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(theme::status::WARNING)
                                        .child(format!("{:.1}", dvm.rating)),
                                ),
                        )
                        .child(render_static_use_button()),
                ),
        )
}

/// Render a static USE button (for backward compatibility)
fn render_static_use_button() -> impl IntoElement {
    div()
        .px(px(10.0))
        .py(px(4.0))
        .bg(accent::PRIMARY_MUTED)
        .rounded(px(4.0))
        .cursor_pointer()
        .hover(|s| s.bg(accent::PRIMARY))
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(accent::PRIMARY)
                .child("USE"),
        )
}
