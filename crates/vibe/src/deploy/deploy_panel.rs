//! Deploy panel component - One-click deploy controls

use gpui_oa::*;
use gpui_oa::prelude::*;
use theme_oa::{bg, border, status, text, FONT_FAMILY};

use crate::types::{Deployment, DeploymentStatus};

/// Render the deploy panel with deployment history and controls
pub fn render_deploy_panel(deployments: &[Deployment]) -> impl IntoElement {
    let latest = deployments.first();
    let live_deploy = deployments.iter().find(|d| d.status == DeploymentStatus::Live);

    div()
        .id("deploy-panel")
        .flex()
        .flex_col()
        .flex_1()
        .bg(bg::APP)
        // Current status card
        .child(
            div()
                .w_full()
                .p(px(16.0))
                .bg(bg::SURFACE)
                .border_b_1()
                .border_color(border::DEFAULT)
                // Status header
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
                                .child("PRODUCTION"),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .child(
                                    div()
                                        .w(px(8.0))
                                        .h(px(8.0))
                                        .rounded_full()
                                        .bg(status::SUCCESS),
                                )
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(status::SUCCESS)
                                        .child("LIVE"),
                                ),
                        ),
                )
                // Current version
                .when(live_deploy.is_some(), |el| {
                    let deploy = live_deploy.unwrap();
                    el.child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .mb(px(16.0))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(18.0))
                                            .font_family(FONT_FAMILY)
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(text::PRIMARY)
                                            .child(format!("v{}", deploy.version)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child(deploy.commit_message.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_end()
                                    .gap(px(4.0))
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::MUTED)
                                            .child(format!("Deployed {}", deploy.deployed_at.as_ref().unwrap_or(&"--".to_string()))),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .font_family(FONT_FAMILY)
                                            .text_color(text::LINK)
                                            .cursor_pointer()
                                            .child(deploy.url.clone()),
                                    ),
                            ),
                    )
                })
                // Deploy button
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        // Main deploy button
                        .child(
                            div()
                                .flex_1()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap(px(8.0))
                                .py(px(12.0))
                                .bg(Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 })
                                .cursor_pointer()
                                .hover(|s| s.bg(Hsla { h: 0.14, s: 1.0, l: 0.55, a: 1.0 }))
                                .child(
                                    div()
                                        .text_size(px(14.0))
                                        .text_color(bg::APP)
                                        .child("^"),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.0))
                                        .font_family(FONT_FAMILY)
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(bg::APP)
                                        .child("DEPLOY TO PRODUCTION"),
                                ),
                        )
                        // Preview deploy
                        .child(
                            div()
                                .px(px(16.0))
                                .py(px(12.0))
                                .bg(bg::ELEVATED)
                                .border_1()
                                .border_color(border::DEFAULT)
                                .cursor_pointer()
                                .hover(|s| s.bg(bg::HOVER))
                                .child(
                                    div()
                                        .text_size(px(10.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(text::MUTED)
                                        .child("PREVIEW"),
                                ),
                        ),
                ),
        )
        // Deployment history header
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
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child("DEPLOYMENT HISTORY"),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(format!("{} deployments", deployments.len())),
                ),
        )
        // Deployment list
        .child(
            div()
                .id("deploy-history-scroll")
                .flex_1()
                .overflow_y_scroll()
                .children(deployments.iter().map(|deploy| {
                    render_deployment_item(deploy)
                })),
        )
}

/// Render a single deployment item
fn render_deployment_item(deploy: &Deployment) -> impl IntoElement {
    let (status_color, status_bg) = match deploy.status {
        DeploymentStatus::Pending => (text::MUTED, Hsla::transparent_black()),
        DeploymentStatus::Building | DeploymentStatus::Deploying => (Hsla { h: 0.14, s: 1.0, l: 0.5, a: 1.0 }, Hsla { h: 0.14, s: 0.2, l: 0.15, a: 1.0 }),
        DeploymentStatus::Live => (status::SUCCESS, Hsla::transparent_black()),
        DeploymentStatus::Failed => (status::ERROR, Hsla { h: 0.0, s: 0.2, l: 0.15, a: 1.0 }),
        DeploymentStatus::Rolled => (status::WARNING, Hsla::transparent_black()),
    };

    div()
        .id(SharedString::from(format!("deploy-{}", deploy.id)))
        .w_full()
        .p(px(12.0))
        .bg(status_bg)
        .border_b_1()
        .border_color(border::DEFAULT)
        .cursor_pointer()
        .hover(|s| s.bg(bg::HOVER))
        // Header row
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(6.0))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .font_family(FONT_FAMILY)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(text::PRIMARY)
                                .child(format!("v{}", deploy.version)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(4.0))
                                .px(px(6.0))
                                .py(px(2.0))
                                .bg(if deploy.status == DeploymentStatus::Live { Hsla { h: 0.35, s: 0.3, l: 0.2, a: 1.0 } } else { Hsla::transparent_black() })
                                .border_1()
                                .border_color(status_color)
                                .child(
                                    div()
                                        .text_size(px(8.0))
                                        .text_color(status_color)
                                        .child(deploy.status.indicator()),
                                )
                                .child(
                                    div()
                                        .text_size(px(8.0))
                                        .font_family(FONT_FAMILY)
                                        .text_color(status_color)
                                        .child(deploy.status.label()),
                                ),
                        ),
                )
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(deploy.created_at.clone()),
                ),
        )
        // Commit message
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_size(px(9.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::MUTED)
                        .child(deploy.commit_hash.clone()),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .font_family(FONT_FAMILY)
                        .text_color(text::PRIMARY)
                        .child(deploy.commit_message.clone()),
                ),
        )
        // Footer with actions
        .child(
            div()
                .mt(px(8.0))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .when(deploy.build_duration_secs.is_some(), |el| {
                            el.child(
                                div()
                                    .text_size(px(9.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child(format!("{}s build", deploy.build_duration_secs.unwrap())),
                            )
                        })
                        .child(
                            div()
                                .text_size(px(9.0))
                                .font_family(FONT_FAMILY)
                                .text_color(text::LINK)
                                .cursor_pointer()
                                .child("VIEW LOGS"),
                        ),
                )
                // Rollback button (if not current)
                .when(deploy.status != DeploymentStatus::Live && deploy.status != DeploymentStatus::Building, |el| {
                    el.child(
                        div()
                            .px(px(8.0))
                            .py(px(2.0))
                            .bg(bg::ELEVATED)
                            .border_1()
                            .border_color(border::DEFAULT)
                            .cursor_pointer()
                            .hover(|s| s.bg(bg::HOVER))
                            .child(
                                div()
                                    .text_size(px(8.0))
                                    .font_family(FONT_FAMILY)
                                    .text_color(text::MUTED)
                                    .child("ROLLBACK"),
                            ),
                    )
                }),
        )
}
