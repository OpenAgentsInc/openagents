//! Model list component - Table of active models

use gpui::*;
use theme::{bg, border, text, status, FONT_FAMILY};

use crate::types::{ActiveModel, ModelStatus};

/// Render the active models table
pub fn render_model_list(models: &[ActiveModel]) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .flex_col()
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(8.0))
        .overflow_hidden()
        // Header row
        .child(render_table_header())
        // Model rows
        .children(models.iter().map(|model| {
            render_model_row(model)
        }))
}

/// Render the table header
fn render_table_header() -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .px(px(16.0))
        .py(px(10.0))
        .bg(bg::HEADER)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Model column
        .child(
            div()
                .flex_1()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("MODEL"),
        )
        // Device column
        .child(
            div()
                .w(px(60.0))
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("DEVICE"),
        )
        // Requests column
        .child(
            div()
                .w(px(70.0))
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("REQ/HR"),
        )
        // Earnings column
        .child(
            div()
                .w(px(80.0))
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("SATS/HR"),
        )
        // Status column
        .child(
            div()
                .w(px(100.0))
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child("STATUS"),
        )
}

/// Render a model row
fn render_model_row(model: &ActiveModel) -> impl IntoElement {
    let status_color = match model.status {
        ModelStatus::Ready => status::SUCCESS,
        ModelStatus::Processing => status::WARNING,
        ModelStatus::Loading => status::INFO,
        ModelStatus::Error => status::ERROR,
    };

    div()
        .w_full()
        .flex()
        .items_center()
        .px(px(16.0))
        .py(px(12.0))
        .border_b_1()
        .border_color(border::SUBTLE)
        .hover(|s| s.bg(bg::ROW))
        // Model name
        .child(
            div()
                .flex_1()
                .text_size(px(13.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(model.name.clone()),
        )
        // Device
        .child(
            div()
                .w(px(60.0))
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .child(model.device.clone()),
        )
        // Requests per hour
        .child(
            div()
                .w(px(70.0))
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::SECONDARY)
                .child(format!("{:.0}", model.requests_per_hour)),
        )
        // Earnings per hour
        .child(
            div()
                .w(px(80.0))
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(status::SUCCESS)
                .child(format!("{}", model.earnings_per_hour_sats)),
        )
        // Status
        .child(
            div()
                .w(px(100.0))
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(status_color)
                        .child(model.status.icon().to_string()),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status_color)
                        .child(model.status.label().to_string()),
                ),
        )
}

/// Render network stats panel
pub fn render_network_stats(
    connected_relays: u32,
    pending_jobs: u32,
    completed_today: u64,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .py(px(12.0))
        .bg(bg::CARD)
        .border_1()
        .border_color(border::DEFAULT)
        .rounded(px(8.0))
        // Connected relays
        .child(render_stat_item("Connected", &format!("{} relays", connected_relays)))
        // Pending jobs
        .child(render_stat_item("Queue", &format!("{} pending", pending_jobs)))
        // Completed today
        .child(render_stat_item("Today", &format!("{} completed", completed_today)))
}

/// Render a stat item
fn render_stat_item(label: &str, value: &str) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(format!("{}:", label)),
        )
        .child(
            div()
                .text_size(px(12.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(value.to_string()),
        )
}

/// Generate mock models for UI development
pub fn mock_models() -> Vec<ActiveModel> {
    vec![
        ActiveModel::mock("gemma:7b", "CPU", ModelStatus::Ready),
        ActiveModel::mock("mistral:7b", "GPU", ModelStatus::Processing),
        ActiveModel::mock("devstral:24b", "GPU", ModelStatus::Ready),
    ]
}
