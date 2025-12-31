use wgpui::{Bounds, Point, Quad, Scene, TextSystem, theme};

use crate::hud::draw_hud_view;
use crate::state::{AppState, JobStatus};

pub(crate) fn build_landing_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    _scale_factor: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP),
    );

    let pad = 24.0;

    // === HEADER ===
    let title = "THE BAZAAR";
    let title_run = text_system.layout(
        title,
        Point::new(pad, pad),
        28.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(title_run);

    let tagline = "An open market for agent work";
    let tagline_run = text_system.layout(
        tagline,
        Point::new(pad, pad + 36.0),
        14.0,
        theme::text::MUTED,
    );
    scene.draw_text(tagline_run);

    // === LIVE MARKET FEED ===
    let feed_y = pad + 80.0;
    let feed_w = width - pad * 2.0;
    let row_h = 28.0;
    let num_jobs = state.market_jobs.len();
    let feed_h = (num_jobs as f32 * row_h) + 36.0;

    // Feed container
    scene.draw_quad(
        Quad::new(Bounds::new(pad, feed_y, feed_w, feed_h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Job rows
    state.job_bounds.clear();
    let mut row_y = feed_y + 4.0;
    for job in &state.market_jobs {
        let status_color = match job.status {
            JobStatus::Paid => theme::status::SUCCESS,
            JobStatus::Verifying => theme::status::WARNING,
            JobStatus::Working => theme::accent::PRIMARY,
        };

        // Status dot
        scene.draw_quad(
            Quad::new(Bounds::new(pad + 12.0, row_y + 10.0, 8.0, 8.0))
                .with_background(status_color)
                .with_corner_radius(4.0),
        );

        // Provider name
        let provider_run = text_system.layout(
            job.provider,
            Point::new(pad + 28.0, row_y + 6.0),
            12.0,
            theme::text::PRIMARY,
        );
        scene.draw_text(provider_run);

        // Repo
        let repo_run = text_system.layout(
            job.repo,
            Point::new(pad + 118.0, row_y + 6.0),
            12.0,
            theme::text::MUTED,
        );
        scene.draw_text(repo_run);

        // Amount
        let amount_text = format!("{} sats", job.amount_sats);
        let amount_run = text_system.layout(
            &amount_text,
            Point::new(pad + feed_w - 160.0, row_y + 6.0),
            12.0,
            theme::text::PRIMARY,
        );
        scene.draw_text(amount_run);

        // Status text
        let status_text = match job.status {
            JobStatus::Paid => "PAID",
            JobStatus::Verifying => "VERIFYING",
            JobStatus::Working => "WORKING",
        };
        let status_run = text_system.layout(
            status_text,
            Point::new(pad + feed_w - 70.0, row_y + 6.0),
            10.0,
            status_color,
        );
        scene.draw_text(status_run);

        state.job_bounds.push(Bounds::new(pad, row_y, feed_w, row_h));
        row_y += row_h;
    }

    // Stats bar
    let stats_text = format!(
        "Jobs: {} | Cleared: {} sats | Providers: {}",
        state.market_stats.jobs_today,
        state.market_stats.cleared_sats,
        state.market_stats.providers
    );
    let stats_run = text_system.layout(
        &stats_text,
        Point::new(pad + 12.0, row_y + 4.0),
        11.0,
        theme::text::MUTED,
    );
    scene.draw_text(stats_run);

    // === DUAL CTA CARDS ===
    let cards_y = feed_y + feed_h + 24.0;
    let card_gap = 16.0;
    let card_w = (feed_w - card_gap) / 2.0;
    let card_h = 140.0;

    // Left card
    let left_x = pad;
    state.left_cta_bounds = Bounds::new(left_x, cards_y, card_w, card_h);
    scene.draw_quad(
        Quad::new(state.left_cta_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let left_title_run = text_system.layout("GET WORK DONE", Point::new(left_x + 16.0, cards_y + 16.0), 16.0, theme::text::PRIMARY);
    scene.draw_text(left_title_run);
    let left_l1 = text_system.layout("Point Autopilot at your issue backlog.", Point::new(left_x + 16.0, cards_y + 44.0), 12.0, theme::text::MUTED);
    scene.draw_text(left_l1);
    let left_l2 = text_system.layout("Wake up to PRs.", Point::new(left_x + 16.0, cards_y + 60.0), 12.0, theme::text::MUTED);
    scene.draw_text(left_l2);

    // Left button
    let left_btn_text = if state.loading { "Connecting..." } else { "Connect GitHub" };
    let left_btn_y = cards_y + card_h - 48.0;
    let left_btn_bg = if state.left_cta_hovered && !state.loading { theme::accent::PRIMARY } else { theme::accent::PRIMARY.with_alpha(0.85) };
    scene.draw_quad(Quad::new(Bounds::new(left_x + 16.0, left_btn_y, 140.0, 32.0)).with_background(left_btn_bg).with_border(theme::border::DEFAULT, 1.0));
    let left_btn_run = text_system.layout(left_btn_text, Point::new(left_x + 32.0, left_btn_y + 8.0), 13.0, theme::bg::APP);
    scene.draw_text(left_btn_run);

    // Right card
    let right_x = pad + card_w + card_gap;
    state.right_cta_bounds = Bounds::new(right_x, cards_y, card_w, card_h);
    scene.draw_quad(
        Quad::new(state.right_cta_bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let right_title_run = text_system.layout("DO WORK FOR BITCOIN", Point::new(right_x + 16.0, cards_y + 16.0), 16.0, theme::text::PRIMARY);
    scene.draw_text(right_title_run);
    let right_l1 = text_system.layout("Bring your coding agent. Accept jobs.", Point::new(right_x + 16.0, cards_y + 44.0), 12.0, theme::text::MUTED);
    scene.draw_text(right_l1);
    let right_l2 = text_system.layout("Average: 47,000 sats/day", Point::new(right_x + 16.0, cards_y + 64.0), 11.0, theme::status::SUCCESS);
    scene.draw_text(right_l2);

    // Right button
    let right_btn_y = cards_y + card_h - 48.0;
    let right_btn_bg = if state.right_cta_hovered { theme::status::SUCCESS } else { theme::status::SUCCESS.with_alpha(0.85) };
    scene.draw_quad(Quad::new(Bounds::new(right_x + 16.0, right_btn_y, 120.0, 32.0)).with_background(right_btn_bg).with_border(theme::border::DEFAULT, 1.0));
    let right_btn_run = text_system.layout("Start Earning", Point::new(right_x + 32.0, right_btn_y + 8.0), 13.0, theme::bg::APP);
    scene.draw_text(right_btn_run);

    // Set button_bounds for main CTA
    if !state.loading {
        state.button_bounds = Bounds::new(left_x + 16.0, left_btn_y, 140.0, 32.0);
    } else {
        state.button_bounds = Bounds::ZERO;
    }

    state.landing_issue_bounds = Bounds::ZERO;
    state.landing_issue_url = None;
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
