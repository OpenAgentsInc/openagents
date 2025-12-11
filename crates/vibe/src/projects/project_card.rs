//! Project card component - Displays a single project

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::{Project, ProjectStatus};

/// Render a single project card
pub fn render_project_card(project: &Project) -> impl IntoElement {
    let status_color = match project.status {
        ProjectStatus::Active => text::PRIMARY,
        ProjectStatus::Building => Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }, // Yellow
        ProjectStatus::Error => status::ERROR,
        ProjectStatus::Deployed => status::SUCCESS,
        ProjectStatus::Archived => text::MUTED,
    };

    div()
        .id(SharedString::from(format!("project-{}", project.id)))
        .flex()
        .flex_col()
        .p(px(16.0))
        .bg(bg::ELEVATED)
        .border_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER).border_color(border::SELECTED))
        // Header: name and status
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(8.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_family(FONT_FAMILY)
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(text::PRIMARY)
                        .child(project.name.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(status_color)
                                .child(project.status.indicator()),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(status_color)
                                .child(project.status.label()),
                        ),
                ),
        )
        // Description
        .child(
            div()
                .text_size(px(11.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .mb(px(12.0))
                .child(project.description.clone()),
        )
        // Framework badge
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .mb(px(12.0))
                .child(
                    div()
                        .px(px(8.0))
                        .py(px(2.0))
                        .bg(bg::SURFACE)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(project.framework.label()),
                        ),
                )
                // Feature badges
                .when(project.has_database, |el| {
                    el.child(render_feature_badge("DB"))
                })
                .when(project.has_auth, |el| {
                    el.child(render_feature_badge("AUTH"))
                })
                .when(project.has_payments, |el| {
                    el.child(render_feature_badge("PAY"))
                }),
        )
        // Stats row
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(16.0))
                .pt(px(8.0))
                .border_t_1()
                .border_color(border::DEFAULT)
                // Files
                .child(render_stat_item("FILES", &project.file_count.to_string()))
                // Collaborators
                .child(render_stat_item("COLLAB", &project.collaborator_count.to_string()))
                // Credits used
                .child(render_stat_item("CREDITS", &project.agent_credits_used.to_string()))
                // Updated
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child(format!("Updated {}", project.updated_at)),
                        ),
                ),
        )
        // Deployed URL if live
        .when(project.deployed_url.is_some(), |el| {
            let url = project.deployed_url.as_ref().unwrap();
            el.child(
                div()
                    .mt(px(8.0))
                    .pt(px(8.0))
                    .border_t_1()
                    .border_color(border::DEFAULT)
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(status::SUCCESS)
                            .child("▲"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .font_family(FONT_FAMILY)
                            .text_color(text::LINK)
                            .child(url.clone()),
                    ),
            )
        })
}

/// Render a feature badge (DB, AUTH, PAY)
fn render_feature_badge(label: &str) -> impl IntoElement {
    let label = label.to_string();
    div()
        .px(px(6.0))
        .py(px(2.0))
        .bg(Hsla { h: 0.6, s: 0.5, l: 0.2, a: 1.0 })
        .border_1()
        .border_color(Hsla { h: 0.6, s: 0.5, l: 0.3, a: 1.0 })
        .child(
            div()
                .text_size(px(8.0))
                .font_family(FONT_FAMILY)
                .text_color(Hsla { h: 0.6, s: 0.5, l: 0.7, a: 1.0 })
                .child(label),
        )
}

/// Render a stat item (label + value)
fn render_stat_item(label: &str, value: &str) -> impl IntoElement {
    let label = label.to_string();
    let value = value.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(label),
        )
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(text::PRIMARY)
                .child(value),
        )
}
