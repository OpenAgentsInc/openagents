//! Domain manager component - Custom domain configuration

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::{Domain, DomainStatus};

/// Render the domain manager
pub fn render_domain_manager(domains: &[Domain]) -> impl IntoElement {
    let primary_domain = domains.iter().find(|d| d.is_primary);

    div()
        .id("domain-manager")
        .flex()
        .flex_col()
        .bg(bg::APP)
        // Header
        .child(
            div()
                .h(px(36.0))
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .px(px(16.0))
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("DOMAINS"),
                        )
                        .child(
                            div()
                                .px(px(6.0))
                                .py(px(2.0))
                                .bg(bg::ELEVATED)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child(format!("{} configured", domains.len())),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(10.0))
                        .py(px(4.0))
                        .bg(bg::ELEVATED)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .text_size(px(10.0))
                                .text_color(text::MUTED)
                                .child("+"),
                        )
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("ADD DOMAIN"),
                        ),
                ),
        )
        // Primary domain card
        .when(primary_domain.is_some(), |el| {
            let domain = primary_domain.unwrap();
            el.child(
                div()
                    .w_full()
                    .p(px(16.0))
                    .bg(bg::ELEVATED)
                    .border_b_1()
                    .border_color(border::DEFAULT)
                    // Primary badge
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mb(px(8.0))
                            .child(
                                div()
                                    .px(px(6.0))
                                    .py(px(2.0))
                                    .bg(Hsla { h: 0.14, s: 0.3, l: 0.2, a: 1.0 })
                                    .border_1()
                                    .border_color(Hsla { h: 0.14, s: 0.5, l: 0.4, a: 1.0 })
                                    .child(
                                        div()
                                            .text_size(px(8.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
                                            .child("PRIMARY"),
                                    ),
                            )
                            .child(render_status_badge(&domain.status)),
                    )
                    // Domain name
                    .child(
                        div()
                            .text_size(px(16.0))
                            .font_family(FONT_FAMILY)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(text::PRIMARY)
                            .mb(px(8.0))
                            .child(domain.domain.clone()),
                    )
                    // SSL info
                    .when(domain.ssl_expires.is_some(), |el| {
                        el.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.0))
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(status::SUCCESS)
                                        .child("SSL ACTIVE"),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child(format!("Expires {}", domain.ssl_expires.as_ref().unwrap())),
                                ),
                        )
                    }),
            )
        })
        // Other domains list
        .child(
            div()
                .id("domain-list-scroll")
                .flex_1()
                .overflow_y_scroll()
                .children(domains.iter().filter(|d| !d.is_primary).map(|domain| {
                    render_domain_item(domain)
                })),
        )
        // Footer with DNS info
        .child(
            div()
                .h(px(48.0))
                .w_full()
                .flex()
                .items_center()
                .px(px(16.0))
                .bg(bg::SURFACE)
                .border_t_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("Point your domain's CNAME to:"),
                        )
                        .child(
                            div()
                                .text_size(px(10.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::PRIMARY)
                                .child("cname.vibe.dev"),
                        ),
                ),
        )
}

/// Render a domain item
fn render_domain_item(domain: &Domain) -> impl IntoElement {
    div()
        .id(SharedString::from(format!("domain-{}", domain.domain.replace('.', "-"))))
        .w_full()
        .h(px(56.0))
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .border_b_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        // Domain name and status
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(domain.domain.clone()),
                )
                .child(render_status_badge(&domain.status)),
        )
        // Actions
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::LINK)
                        .cursor_pointer()
                        .child("SET PRIMARY"),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(status::ERROR)
                        .cursor_pointer()
                        .child("REMOVE"),
                ),
        )
}

/// Render a status badge
fn render_status_badge(status: &DomainStatus) -> impl IntoElement {
    let (color, label) = match status {
        DomainStatus::Pending => (text::MUTED, "PENDING"),
        DomainStatus::Verifying => (Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }, "VERIFYING"),
        DomainStatus::Active => (status::SUCCESS, "ACTIVE"),
        DomainStatus::Error => (status::ERROR, "ERROR"),
    };

    div()
        .flex()
        .items_center()
        .gap(px(4.0))
        .child(
            div()
                .w(px(6.0))
                .h(px(6.0))
                .rounded_full()
                .bg(color),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(color)
                .child(label),
        )
}
