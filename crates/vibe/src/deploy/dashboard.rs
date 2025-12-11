//! Deploy dashboard - Combines all deploy components

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::DeployTabState;
use crate::screen::VibeScreen;
use super::{render_deploy_panel, render_domain_manager, render_analytics_view};

/// Render the complete deploy dashboard
pub fn render_deploy_dashboard(state: &DeployTabState, _cx: &mut Context<VibeScreen>) -> impl IntoElement {
    div()
        .id("deploy-dashboard")
        .flex()
        .flex_col()
        .h_full()
        .w_full()
        .bg(bg::APP)
        // Toolbar
        .child(render_deploy_toolbar(state))
        // Main content - three columns
        .child(
            div()
                .flex()
                .flex_1()
                .overflow_hidden()
                // Left: Deploy panel
                .child(
                    div()
                        .w(px(400.0))
                        .h_full()
                        .id("deploy-panel-scroll")
                        .border_r_1()
                        .border_color(border::DEFAULT)
                        .overflow_y_scroll()
                        .child(render_deploy_panel(&state.deployments)),
                )
                // Center: Analytics (if shown) or Domains
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .overflow_hidden()
                        .when(state.show_analytics, |el| {
                            el.child(render_analytics_view(&state.analytics, state.analytics_range))
                        })
                        .when(!state.show_analytics, |el| {
                            el.child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .h_full()
                                    // Domains section
                                    .child(
                                        div()
                                            .h(px(300.0))
                                            .border_b_1()
                                            .border_color(border::DEFAULT)
                                            .child(render_domain_manager(&state.domains))
                                    )
                                    // Environment settings
                                    .child(render_environment_settings())
                            )
                        }),
                )
                // Right: Quick actions sidebar
                .child(render_quick_actions_sidebar()),
        )
}

/// Render the deploy toolbar
fn render_deploy_toolbar(state: &DeployTabState) -> impl IntoElement {
    div()
        .id("deploy-toolbar")
        .h(px(44.0))
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .px(px(16.0))
        .bg(bg::SURFACE)
        .border_b_1()
        .border_color(border::DEFAULT)
        // Left: View toggles
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.0))
                .child(render_view_toggle("DEPLOYMENTS", true))
                .child(render_view_toggle("ANALYTICS", state.show_analytics))
                .child(render_view_toggle("DOMAINS", !state.show_analytics))
                .child(render_view_toggle("SETTINGS", false)),
        )
        // Center: Last deploy info
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("Last deployed:"),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child("2024-12-10 14:32"),
                )
                .child(
                    div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(status::SUCCESS),
                ),
        )
        // Right: Actions
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                // Build logs
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(4.0))
                        .bg(bg::ELEVATED)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::MUTED)
                                .child("BUILD LOGS"),
                        ),
                )
                // Security scan
                .child(
                    div()
                        .px(px(10.0))
                        .py(px(4.0))
                        .bg(bg::ELEVATED)
                        .border_1()
                        .border_color(border::DEFAULT)
                        .cursor_pointer()
                        .hover(|s| s.bg(bg::HOVER))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .child(
                                    div()
                                        .w(px(6.0))
                                        .h(px(6.0))
                                        .rounded_full()
                                        .bg(status::SUCCESS),
                                )
                                .child(
                                    div()
                                        .text_size(px(9.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child("SECURITY"),
                                ),
                        ),
                ),
        )
}

/// Render a view toggle button
fn render_view_toggle(label: &str, is_active: bool) -> impl IntoElement {
    let label_owned = label.to_string();
    let (bg_color, text_color) = if is_active {
        (bg::SELECTED, text::PRIMARY)
    } else {
        (Hsla::transparent_black(), text::MUTED)
    };

    div()
        .id(SharedString::from(format!("deploy-toggle-{}", label.to_lowercase())))
        .px(px(10.0))
        .py(px(4.0))
        .bg(bg_color)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text_color)
                .child(label_owned),
        )
}

/// Render the quick actions sidebar
fn render_quick_actions_sidebar() -> impl IntoElement {
    div()
        .w(px(240.0))
        .h_full()
        .flex()
        .flex_col()
        .bg(bg::SURFACE)
        .border_l_1()
        .border_color(border::DEFAULT)
        // Header
        .child(
            div()
                .h(px(36.0))
                .w_full()
                .flex()
                .items_center()
                .px(px(12.0))
                .border_b_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("QUICK ACTIONS"),
                ),
        )
        // Actions list
        .child(
            div()
                .flex_1()
                .p(px(12.0))
                .flex()
                .flex_col()
                .gap(px(8.0))
                .child(render_action_button("Run Tests", "Verify before deploy", false))
                .child(render_action_button("Preview Deploy", "Test in staging", false))
                .child(render_action_button("Rollback", "Revert to previous", true))
                .child(render_action_button("Clear Cache", "Purge CDN cache", false))
                .child(render_action_button("View Logs", "Recent build output", false)),
        )
        // Checklist section
        .child(
            div()
                .w_full()
                .p(px(12.0))
                .bg(bg::ELEVATED)
                .border_t_1()
                .border_color(border::DEFAULT)
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .mb(px(8.0))
                        .child("PRE-DEPLOY CHECKLIST"),
                )
                .child(render_checklist_item("Tests passing", true))
                .child(render_checklist_item("No security issues", true))
                .child(render_checklist_item("Build successful", true))
                .child(render_checklist_item("Env vars configured", true)),
        )
}

/// Render an action button
fn render_action_button(title: &str, subtitle: &str, is_danger: bool) -> impl IntoElement {
    let title = title.to_string();
    let subtitle = subtitle.to_string();
    let border_color = if is_danger { status::ERROR } else { border::DEFAULT };
    let hover_bg = if is_danger { Hsla { h: 0.0, s: 0.3, l: 0.2, a: 1.0 } } else { bg::HOVER };

    div()
        .w_full()
        .p(px(10.0))
        .bg(bg::APP)
        .border_1()
        .border_color(border_color)
        .cursor_pointer()
        .hover(|s| s.bg(hover_bg))
        .child(
            div()
                .text_size(px(10.0))
                .font_family(FONT_FAMILY)
                .text_color(if is_danger { status::ERROR } else { text::PRIMARY })
                .mb(px(2.0))
                .child(title),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(text::MUTED)
                .child(subtitle),
        )
}

/// Render a checklist item
fn render_checklist_item(label: &str, checked: bool) -> impl IntoElement {
    let label = label.to_string();
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .mb(px(4.0))
        .child(
            div()
                .text_size(px(10.0))
                .text_color(if checked { status::SUCCESS } else { text::MUTED })
                .child(if checked { "+" } else { "o" }),
        )
        .child(
            div()
                .text_size(px(9.0))
                .font_family(FONT_FAMILY)
                .text_color(if checked { text::PRIMARY } else { text::MUTED })
                .child(label),
        )
}

/// Render environment settings section
fn render_environment_settings() -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .p(px(16.0))
        .bg(bg::APP)
        // Header
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(12.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("ENVIRONMENT VARIABLES"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .px(px(8.0))
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
                                .child("ADD"),
                        ),
                ),
        )
        // Env vars list
        .child(
            div()
                .id("env-vars-scroll")
                .flex_1()
                .overflow_y_scroll()
                .child(render_env_var("DATABASE_URL", "postgres://...", false))
                .child(render_env_var("API_KEY", "sk_live_...", true))
                .child(render_env_var("NODE_ENV", "production", false))
                .child(render_env_var("ENABLE_ANALYTICS", "true", false)),
        )
}

/// Render an environment variable row
fn render_env_var(key: &str, value: &str, is_secret: bool) -> impl IntoElement {
    let key = key.to_string();
    let display_value = if is_secret {
        format!("{}...", &value[..value.len().min(8)])
    } else {
        value.to_string()
    };

    div()
        .w_full()
        .h(px(40.0))
        .flex()
        .items_center()
        .justify_between()
        .px(px(12.0))
        .bg(bg::ELEVATED)
        .border_b_1()
        .border_color(border::DEFAULT)
        .hover(|s| s.bg(bg::HOVER))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(key),
                )
                .when(is_secret, |el| {
                    el.child(
                        div()
                            .text_size(px(8.0))
                            .font_family(FONT_FAMILY)
                            .text_color(status::WARNING)
                            .child("SECRET"),
                    )
                }),
        )
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
                        .child(display_value),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .text_color(text::MUTED)
                        .cursor_pointer()
                        .hover(|s| s.text_color(text::PRIMARY))
                        .child("..."),
                ),
        )
}
