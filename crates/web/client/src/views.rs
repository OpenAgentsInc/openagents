use wgpui::{Bounds, Point, Quad, Scene, TextSystem, theme};

use crate::hud::draw_hud_view;
use crate::state::AppState;

pub(crate) fn build_landing_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    let has_live = state.landing_live.is_some() && state.hud_context.is_some();
    if has_live {
        draw_hud_view(scene, text_system, state, width, height, scale_factor);
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, width, height))
                .with_background(theme::bg::APP.with_alpha(0.12)),
        );
    } else {
        scene.draw_quad(
            Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP),
        );
    }

    state.landing_issue_bounds = Bounds::ZERO;
    state.landing_issue_url = None;

    let pad = 12.0;
    if let Some(live) = state.landing_live.as_ref() {
        let banner_h = 22.0;
        let live_label = "LIVE";
        let label_size = 10.0;
        let label_w = live_label.len() as f32 * label_size * 0.6 + 10.0;
        let label_bounds = Bounds::new(pad, pad, label_w, banner_h);

        scene.draw_quad(
            Quad::new(label_bounds)
                .with_background(theme::status::ERROR)
                .with_border(theme::border::DEFAULT, 1.0)
                .with_corner_radius(0.0),
        );

        let label_run = text_system.layout(
            live_label,
            Point::new(label_bounds.origin.x + 5.0, label_bounds.origin.y + 5.0),
            label_size,
            theme::bg::APP,
        );
        scene.draw_text(label_run);

        let issue_text = live
            .issue
            .as_ref()
            .map(|issue| format!("Autopilot is working on {}", issue.label))
            .unwrap_or_else(|| "Autopilot is working live".to_string());
        let issue_size = 12.0;
        let issue_x = label_bounds.origin.x + label_bounds.size.width + 8.0;
        let issue_y = label_bounds.origin.y + 4.0;
        let issue_run = text_system.layout(
            &issue_text,
            Point::new(issue_x, issue_y),
            issue_size,
            theme::text::PRIMARY,
        );
        scene.draw_text(issue_run);

        if let Some(issue) = live.issue.as_ref() {
            let issue_w = issue_text.len() as f32 * issue_size * 0.6;
            state.landing_issue_bounds = Bounds::new(issue_x, issue_y, issue_w, banner_h);
            state.landing_issue_url = Some(issue.url.clone());
        }

        let repo_label = format!("@{}/{}", live.hud_context.username, live.hud_context.repo);
        let repo_run = text_system.layout(
            &repo_label,
            Point::new(pad, label_bounds.origin.y + label_bounds.size.height + 6.0),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(repo_run);

        if let Some(issue) = live.issue.as_ref().and_then(|issue| issue.title.as_ref()) {
            let title_run = text_system.layout(
                issue,
                Point::new(pad, label_bounds.origin.y + label_bounds.size.height + 20.0),
                11.0,
                theme::text::MUTED,
            );
            scene.draw_text(title_run);
        }
    } else {
        let placeholder = text_system.layout(
            "No live session is broadcasting right now.",
            Point::new(pad, pad),
            12.0,
            theme::text::MUTED,
        );
        scene.draw_text(placeholder);
    }

    let panel_h = if height < 560.0 { 108.0 } else { 128.0 };
    let panel_w = (width - pad * 2.0).max(240.0);
    let panel_x = pad;
    let panel_y = height - panel_h - pad;
    let panel_bounds = Bounds::new(panel_x, panel_y, panel_w, panel_h);

    scene.draw_quad(
        Quad::new(panel_bounds)
            .with_background(theme::bg::SURFACE.with_alpha(0.94))
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(0.0),
    );

    let title = "Autopilot for code";
    let title_run = text_system.layout(
        title,
        Point::new(panel_x + 12.0, panel_y + 10.0),
        if width < 600.0 { 18.0 } else { 22.0 },
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let subtitle = "Watch it work. Connect GitHub to get your own HUD in under 30 seconds.";
    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(panel_x + 12.0, panel_y + 40.0),
        12.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    let (button_text, button_bg_base): (&str, _) = if state.loading {
        ("Connecting...", theme::text::MUTED)
    } else {
        ("Connect GitHub → Get Your Own Autopilot", theme::accent::PRIMARY)
    };

    let button_font_size = 13.0;
    let button_text_width = button_text.len() as f32 * button_font_size * 0.6;
    let button_padding_x = 18.0;
    let button_padding_y = 10.0;
    let button_width = button_text_width + button_padding_x * 2.0;
    let button_height = button_font_size + button_padding_y * 2.0;
    let button_x = panel_x + 12.0;
    let button_y = panel_y + panel_h - button_height - 12.0;

    if !state.loading {
        state.button_bounds = Bounds::new(button_x, button_y, button_width, button_height);
    } else {
        state.button_bounds = Bounds::ZERO;
    }

    let button_bg = if state.button_hovered && !state.loading {
        button_bg_base
    } else {
        button_bg_base.with_alpha(0.85)
    };

    scene.draw_quad(
        Quad::new(Bounds::new(button_x, button_y, button_width, button_height))
            .with_background(button_bg)
            .with_border(theme::border::DEFAULT, 1.0)
            .with_corner_radius(0.0),
    );

    let button_text_run = text_system.layout(
        button_text,
        Point::new(button_x + button_padding_x, button_y + button_padding_y),
        button_font_size,
        theme::bg::APP,
    );
    scene.draw_text(button_text_run);
}

pub(crate) fn build_repo_selector(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
) {
    scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP));

    let padding = 24.0;
    let mut y = padding;

    let header = format!(
        "Welcome, {}",
        state.user.github_username.as_deref().unwrap_or("User")
    );
    let header_run = text_system.layout(
        &header,
        Point::new(padding, y),
        24.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(header_run);

    let logout_text = "Logout";
    let logout_size = 12.0;
    let logout_width = logout_text.len() as f32 * logout_size * 0.6 + 16.0;
    let logout_x = width - padding - logout_width;
    state.button_bounds = Bounds::new(logout_x, y - 4.0, logout_width, 24.0);

    let logout_bg = if state.button_hovered {
        theme::status::ERROR
    } else {
        theme::status::ERROR.with_alpha(0.7)
    };

    scene.draw_quad(
        Quad::new(state.button_bounds)
            .with_background(logout_bg)
            .with_corner_radius(4.0),
    );

    let logout_run = text_system.layout(
        logout_text,
        Point::new(logout_x + 8.0, y),
        logout_size,
        theme::text::PRIMARY,
    );
    scene.draw_text(logout_run);

    y += 28.0;

    if let Some(npub) = state.user.nostr_npub.as_deref() {
        let npub_text = format!("npub: {}", npub);
        let npub_run = text_system.layout(
            &npub_text,
            Point::new(padding, y),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(npub_run);
        y += 18.0;
    }

    y += 16.0;

    let subtitle = "Select a repository:";
    let subtitle_run = text_system.layout(
        subtitle,
        Point::new(padding, y),
        14.0,
        theme::text::MUTED,
    );
    scene.draw_text(subtitle_run);

    y += 32.0;

    state.repo_bounds.clear();

    if state.repos_loading {
        let loading_run = text_system.layout(
            "Loading repositories...",
            Point::new(padding, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(loading_run);
    } else if state.repos.is_empty() {
        let empty_run = text_system.layout(
            "No repositories found",
            Point::new(padding, y),
            14.0,
            theme::text::MUTED,
        );
        scene.draw_text(empty_run);
    } else {
        let row_height = 56.0;
        for (i, repo) in state.repos.iter().enumerate() {
            let row_y = y + (i as f32 * row_height) - state.scroll_offset;

            if row_y + row_height < y || row_y > height {
                state.repo_bounds.push(Bounds::ZERO);
                continue;
            }

            let row_bounds = Bounds::new(padding, row_y, width - padding * 2.0, row_height - 4.0);
            state.repo_bounds.push(row_bounds);

            let is_hovered = state.hovered_repo_idx == Some(i);
            let is_selected = state.selected_repo.as_ref() == Some(&repo.full_name);

            let row_bg = if is_selected {
                theme::accent::PRIMARY.with_alpha(0.2)
            } else if is_hovered {
                theme::bg::HOVER
            } else {
                theme::bg::SURFACE
            };

            scene.draw_quad(
                Quad::new(row_bounds)
                    .with_background(row_bg)
                    .with_border(theme::border::DEFAULT, 1.0),
            );

            let name_run = text_system.layout(
                &repo.full_name,
                Point::new(padding + 12.0, row_y + 10.0),
                14.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(name_run);

            if repo.private {
                let badge_text = "Private";
                let badge_x = padding + 12.0 + repo.full_name.len() as f32 * 14.0 * 0.6 + 8.0;
                let badge_bounds = Bounds::new(badge_x, row_y + 10.0, 50.0, 16.0);
                scene.draw_quad(
                    Quad::new(badge_bounds)
                        .with_background(theme::status::WARNING.with_alpha(0.2))
                        .with_border(theme::status::WARNING, 1.0),
                );
                let badge_run = text_system.layout(
                    badge_text,
                    Point::new(badge_x + 6.0, row_y + 11.0),
                    10.0,
                    theme::status::WARNING,
                );
                scene.draw_text(badge_run);
            }

            if let Some(desc) = &repo.description {
                let desc_truncated = if desc.len() > 80 {
                    format!("{}...", &desc[..77])
                } else {
                    desc.clone()
                };
                let desc_run = text_system.layout(
                    &desc_truncated,
                    Point::new(padding + 12.0, row_y + 32.0),
                    11.0,
                    theme::text::MUTED,
                );
                scene.draw_text(desc_run);
            }
        }

        let total_height = state.repos.len() as f32 * row_height;
        let visible_height = height - y;
        if total_height > visible_height {
            let scroll_track_height = visible_height - 20.0;
            let scroll_thumb_height = (visible_height / total_height) * scroll_track_height;
            let scroll_thumb_y = y + 10.0 + (state.scroll_offset / total_height) * scroll_track_height;

            scene.draw_quad(
                Quad::new(Bounds::new(width - 8.0, y, 4.0, scroll_track_height))
                    .with_background(theme::bg::SURFACE),
            );

            scene.draw_quad(
                Quad::new(Bounds::new(width - 8.0, scroll_thumb_y, 4.0, scroll_thumb_height))
                    .with_background(theme::text::MUTED),
            );

            let max_scroll = total_height - visible_height;
            state.scroll_offset = state.scroll_offset.min(max_scroll).max(0.0);
        }
    }
}

pub(crate) fn build_repo_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    draw_hud_view(scene, text_system, state, width, height, scale_factor);
}
