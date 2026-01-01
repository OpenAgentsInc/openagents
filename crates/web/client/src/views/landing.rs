use wgpui::{Bounds, Hsla, Point, Quad, Scene, TextSystem, theme};
use wgpui::animation::AnimatorState;
use wgpui::components::Component;
use wgpui::components::hud::{DotsGrid, DotsOrigin, Frame};
use wgpui::PaintContext;

use crate::nostr::{BazaarJobType, DvmView, JobType, Nip90EventType, RelayStatus, VerificationStatus};
use crate::state::{AppState, JobStatus};

use js_sys;

pub(crate) fn build_landing_page(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    // Background
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP),
    );

    // Dots grid background with animation
    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    state.dots_grid = DotsGrid::new()
        .color(Hsla::new(0.0, 0.0, 1.0, 0.15)) // Faint white dots
        .distance(36.0)
        .size(1.0)
        .origin(DotsOrigin::Center);
    state.dots_grid.update(AnimatorState::Entered);
    state.dots_grid.paint(Bounds::new(0.0, 0.0, width, height), &mut cx);
    let scene = cx.scene;
    let text_system = cx.text;

    // === CENTERED HERO CARD (only show when auth status is known) ===
    let (scene, text_system) = if !state.loading {
        let card_w = 480.0;
        let card_h = 180.0;
        let card_x = (width - card_w) / 2.0;
        let card_y = (height - card_h) / 2.0 - 40.0;

        // Update frame animator
        if !state.cta_frames_started {
            state.cta_frames_started = true;
        }
        let anim_state = if state.cta_frames_started {
            AnimatorState::Entering
        } else {
            AnimatorState::Exited
        };
        let frame_progress = state.left_cta_animator.update(anim_state);

        // Create PaintContext for Frame component
        let mut cx = PaintContext::new(scene, text_system, scale_factor);

        // Hero frame
        state.left_cta_bounds = Bounds::new(card_x, card_y, card_w, card_h);

        let mut hero_frame = Frame::corners()
            .line_color(Hsla::new(0.0, 0.0, 1.0, 1.0)) // White corners
            .bg_color(Hsla::new(0.0, 0.0, 1.0, 0.02)) // White 2% opacity
            .glow_color(Hsla::new(0.0, 0.0, 1.0, 0.2))
            .stroke_width(1.0)
            .corner_length(40.0) // 2x default
            .animation_progress(frame_progress);

        hero_frame.paint(state.left_cta_bounds, &mut cx);

        // Content sizing
        let title_size = 32.0;
        let subtitle_size = 16.0;
        let btn_h = 40.0;
        let gap1 = 8.0;  // title to subtitle
        let gap2 = 28.0; // subtitle to button

        // Calculate total content height and center vertically
        let content_h = title_size + gap1 + subtitle_size + gap2 + btn_h;
        let content_start_y = card_y + (card_h - content_h) / 2.0;

        // Title - "Autopilot"
        let title_text = "Autopilot";
        let title_width = cx.text.measure(title_text, title_size);
        let title_x = card_x + (card_w - title_width) / 2.0;
        let title_run = cx.text.layout(title_text, Point::new(title_x, content_start_y), title_size, theme::text::PRIMARY);
        cx.scene.draw_text(title_run);

        // Subtitle - "Early access"
        let subtitle_text = "Early access";
        let subtitle_width = cx.text.measure(subtitle_text, subtitle_size);
        let subtitle_x = card_x + (card_w - subtitle_width) / 2.0;
        let subtitle_y = content_start_y + title_size + gap1;
        let subtitle_run = cx.text.layout(subtitle_text, Point::new(subtitle_x, subtitle_y), subtitle_size, theme::text::MUTED);
        cx.scene.draw_text(subtitle_run);

        // Button - "Log in with GitHub"
        let btn_text = "Log in with GitHub";
        let btn_font_size = 15.0;
        let btn_w = 180.0;
        let btn_x = card_x + (card_w - btn_w) / 2.0;
        let btn_y = subtitle_y + subtitle_size + gap2;
        let btn_bg = if state.left_cta_hovered {
            theme::bg::ELEVATED
        } else {
            theme::bg::SURFACE
        };
        // Draw button border manually to ensure all sides show
        let border_color = theme::border::DEFAULT;
        // Top border
        cx.scene.draw_quad(Quad::new(Bounds::new(btn_x, btn_y, btn_w, 1.0)).with_background(border_color));
        // Bottom border
        cx.scene.draw_quad(Quad::new(Bounds::new(btn_x, btn_y + btn_h - 1.0, btn_w, 1.0)).with_background(border_color));
        // Left border
        cx.scene.draw_quad(Quad::new(Bounds::new(btn_x, btn_y, 1.0, btn_h)).with_background(border_color));
        // Right border
        cx.scene.draw_quad(Quad::new(Bounds::new(btn_x + btn_w - 1.0, btn_y, 1.0, btn_h)).with_background(border_color));
        // Background (inside the border)
        cx.scene.draw_quad(Quad::new(Bounds::new(btn_x + 1.0, btn_y + 1.0, btn_w - 2.0, btn_h - 2.0)).with_background(btn_bg));
        let btn_text_width = cx.text.measure(btn_text, btn_font_size);
        let btn_text_x = btn_x + (btn_w - btn_text_width) / 2.0;
        let btn_text_y = btn_y + (btn_h - btn_font_size) / 2.0;
        let btn_run = cx.text.layout(btn_text, Point::new(btn_text_x, btn_text_y), btn_font_size, theme::text::PRIMARY);
        cx.scene.draw_text(btn_run);

        // Set button bounds for click handling
        state.button_bounds = Bounds::new(btn_x, btn_y, btn_w, btn_h);

        (cx.scene, cx.text)
    } else {
        // Still loading - hide everything
        state.button_bounds = Bounds::ZERO;
        state.left_cta_bounds = Bounds::ZERO;
        (scene, text_system)
    };

    state.landing_issue_bounds = Bounds::ZERO;
    state.landing_issue_url = None;
    state.right_cta_bounds = Bounds::ZERO;

    // === LIVE MARKET FEED (hidden for now) ===
    let pad = 24.0;
    let feed_w = width - pad * 2.0;
    let feed_y = height; // Push off screen
    let row_h = 28.0;

    // Use real Bazaar jobs if available, otherwise fall back to dummy data
    let has_bazaar_jobs = !state.bazaar.jobs.is_empty();
    let num_jobs = if has_bazaar_jobs { state.bazaar.jobs.len() } else { state.market_jobs.len() };
    let feed_h = (num_jobs as f32 * row_h) + 36.0;

    // Feed container
    scene.draw_quad(
        Quad::new(Bounds::new(pad, feed_y, feed_w, feed_h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Job rows - use bazaar_job_bounds for real jobs
    state.bazaar_job_bounds.clear();
    state.job_bounds.clear();
    let mut row_y = feed_y + 4.0;

    if has_bazaar_jobs {
        // Render real Bazaar jobs
        for job in &state.bazaar.jobs {
            let status_color = match job.status {
                VerificationStatus::Paid => theme::status::SUCCESS,
                VerificationStatus::Verified => theme::status::SUCCESS,
                VerificationStatus::Verifying => theme::status::WARNING,
                VerificationStatus::Working => theme::accent::PRIMARY,
                VerificationStatus::Disputed | VerificationStatus::Failed => theme::status::ERROR,
            };

            // Job type badge color
            let badge_color = match job.job_type {
                BazaarJobType::PatchGen => theme::accent::PRIMARY,
                BazaarJobType::CodeReview => theme::accent::SECONDARY,
                BazaarJobType::SandboxRun => theme::status::WARNING,
                BazaarJobType::RepoIndex => theme::text::MUTED,
            };

            // Badge background
            let badge_text = format!("[{}]", job.job_type.badge());
            scene.draw_quad(
                Quad::new(Bounds::new(pad + 8.0, row_y + 4.0, 52.0, 18.0))
                    .with_background(badge_color.with_alpha(0.15)),
            );
            let badge_run = text_system.layout(
                &badge_text,
                Point::new(pad + 12.0, row_y + 6.0),
                10.0,
                badge_color,
            );
            scene.draw_text(badge_run);

            // Pubkey (truncated)
            let pubkey_text = job.display_pubkey();
            let pubkey_run = text_system.layout(
                &pubkey_text,
                Point::new(pad + 68.0, row_y + 6.0),
                11.0,
                theme::text::MUTED,
            );
            scene.draw_text(pubkey_run);

            // Issue reference or repo URL (UTF-8 safe truncation)
            let ref_text = job.issue_ref.as_ref()
                .or(job.repo_url.as_ref())
                .map(|s| if s.chars().count() > 25 {
                    let safe_end = s.char_indices().nth(22).map(|(i, _)| i).unwrap_or(s.len());
                    format!("{}...", &s[..safe_end])
                } else { s.clone() })
                .unwrap_or_else(|| "—".to_string());
            let ref_run = text_system.layout(
                &ref_text,
                Point::new(pad + 150.0, row_y + 6.0),
                11.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(ref_run);

            // Amount
            let sats = job.display_sats();
            let amount_text = if sats > 0 { format!("{} sats", sats) } else { "—".to_string() };
            let amount_run = text_system.layout(
                &amount_text,
                Point::new(pad + feed_w - 170.0, row_y + 6.0),
                11.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(amount_run);

            // Status with icon
            let status_text = match job.status {
                VerificationStatus::Paid => "⚡ PAID",
                _ => job.status.label(),
            };
            let status_run = text_system.layout(
                status_text,
                Point::new(pad + feed_w - 80.0, row_y + 6.0),
                10.0,
                status_color,
            );
            scene.draw_text(status_run);

            state.bazaar_job_bounds.push(Bounds::new(pad, row_y, feed_w, row_h));
            row_y += row_h;
        }
    } else {
        // Fallback: render dummy market jobs
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
    }

    // Stats bar - calculate from real data if available
    let (jobs_count, cleared_sats, providers_count) = if has_bazaar_jobs {
        let jobs = state.bazaar.jobs.len() as u32;
        let cleared: u32 = state.bazaar.jobs.iter()
            .filter(|j| j.status == VerificationStatus::Paid)
            .map(|j| j.display_sats())
            .sum();
        let providers: u32 = state.bazaar.jobs.iter()
            .filter_map(|j| j.provider_pubkey.as_ref())
            .collect::<std::collections::HashSet<_>>()
            .len() as u32;
        (jobs, cleared, providers)
    } else {
        (state.market_stats.jobs_today, state.market_stats.cleared_sats, state.market_stats.providers)
    };

    let stats_text = format!(
        "Jobs: {} | Cleared: {} sats | Providers: {}",
        jobs_count, cleared_sats, providers_count
    );
    let stats_run = text_system.layout(
        &stats_text,
        Point::new(pad + 12.0, row_y + 4.0),
        11.0,
        theme::text::MUTED,
    );
    scene.draw_text(stats_run);

    // === DVM MARKETPLACE (TABBED) ===
    let dvm_y = feed_y + feed_h + 16.0;
    let nip90_row_h = 26.0;
    let max_visible_rows = 8; // Fixed visible height (8 rows)

    // Fixed container height based on max visible rows
    let dvm_h = 56.0 + (max_visible_rows as f32 * nip90_row_h);

    // DVM container
    scene.draw_quad(
        Quad::new(Bounds::new(pad, dvm_y, feed_w, dvm_h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Header: "DVM MARKETPLACE"
    let dvm_title = "DVM MARKETPLACE";
    let dvm_title_run = text_system.layout(
        dvm_title,
        Point::new(pad + 12.0, dvm_y + 8.0),
        12.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(dvm_title_run);

    // Tab bar: [FEED] [DVMs]
    let tab_y = dvm_y + 28.0;
    let tab_h = 20.0;
    let feed_tab_w = 50.0;
    let dvms_tab_w = 50.0;

    let is_feed_tab = state.dvm_directory.current_view == DvmView::Feed;

    // Feed tab
    let feed_tab_bounds = Bounds::new(pad + 12.0, tab_y, feed_tab_w, tab_h);
    state.dvm_tab_bounds[0] = feed_tab_bounds;
    let feed_tab_bg = if is_feed_tab { theme::accent::PRIMARY.with_alpha(0.2) } else { theme::bg::SURFACE };
    let feed_tab_color = if is_feed_tab { theme::accent::PRIMARY } else { theme::text::MUTED };
    scene.draw_quad(
        Quad::new(feed_tab_bounds).with_background(feed_tab_bg),
    );
    let feed_tab_run = text_system.layout(
        "FEED",
        Point::new(pad + 20.0, tab_y + 4.0),
        10.0,
        feed_tab_color,
    );
    scene.draw_text(feed_tab_run);

    // DVMs tab
    let dvms_tab_bounds = Bounds::new(pad + 12.0 + feed_tab_w + 8.0, tab_y, dvms_tab_w, tab_h);
    state.dvm_tab_bounds[1] = dvms_tab_bounds;
    let dvms_tab_bg = if !is_feed_tab { theme::accent::PRIMARY.with_alpha(0.2) } else { theme::bg::SURFACE };
    let dvms_tab_color = if !is_feed_tab { theme::accent::PRIMARY } else { theme::text::MUTED };
    scene.draw_quad(
        Quad::new(dvms_tab_bounds).with_background(dvms_tab_bg),
    );
    // Show count in DVMs tab
    let dvm_count = state.dvm_directory.dvms.len();
    let dvms_tab_text = if dvm_count > 0 { format!("DVMs ({})", dvm_count) } else { "DVMs".to_string() };
    let dvms_tab_run = text_system.layout(
        &dvms_tab_text,
        Point::new(pad + 12.0 + feed_tab_w + 12.0, tab_y + 4.0),
        10.0,
        dvms_tab_color,
    );
    scene.draw_text(dvms_tab_run);

    // Relay status indicator (right side)
    let (status_color, status_label) = match state.nip90.relay_status {
        RelayStatus::Connected => (theme::status::SUCCESS, "LIVE"),
        RelayStatus::Connecting => (theme::status::WARNING, "..."),
        RelayStatus::Disconnected => (theme::text::MUTED, "OFF"),
        RelayStatus::Error => (theme::status::ERROR, "ERR"),
    };
    scene.draw_quad(
        Quad::new(Bounds::new(pad + feed_w - 50.0, dvm_y + 10.0, 8.0, 8.0))
            .with_background(status_color)
            .with_corner_radius(4.0),
    );
    let status_label_run = text_system.layout(
        status_label,
        Point::new(pad + feed_w - 38.0, dvm_y + 8.0),
        10.0,
        status_color,
    );
    scene.draw_text(status_label_run);

    // Get current time for relative timestamps
    let now_secs = (js_sys::Date::now() / 1000.0) as u64;
    let content_y = tab_y + tab_h + 8.0;
    let content_h = max_visible_rows as f32 * nip90_row_h;
    let content_bottom = content_y + content_h;

    // Set the scrollable content bounds for wheel event detection
    state.dvm_content_bounds = Bounds::new(pad, content_y, feed_w, content_h);

    // Content based on current view
    state.nip90_event_bounds.clear();

    match state.dvm_directory.current_view {
        DvmView::Feed => {
            draw_feed_view(scene, text_system, state, pad, feed_w, content_y, content_h, content_bottom, nip90_row_h, max_visible_rows, now_secs);
        }
        DvmView::Directory => {
            draw_directory_view(scene, text_system, state, pad, feed_w, content_y, content_h, content_bottom, nip90_row_h, max_visible_rows);
        }
        DvmView::JobDetail(ref job_id) => {
            if let Some(job) = state.nip90.get_job(job_id) {
                super::job_detail::draw_job_detail(scene, text_system, &job, pad, content_y, feed_w, now_secs, state);
            } else {
                let error_run = text_system.layout(
                    "Job not found",
                    Point::new(pad + 12.0, content_y + 8.0),
                    11.0,
                    theme::text::MUTED,
                );
                scene.draw_text(error_run);
            }
        }
    }

    // === GLOBAL NOTES FEED ===
    draw_global_notes(scene, text_system, state, pad, feed_w, dvm_y + dvm_h + 16.0, height, now_secs);
}

fn draw_feed_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    pad: f32,
    feed_w: f32,
    content_y: f32,
    content_h: f32,
    content_bottom: f32,
    nip90_row_h: f32,
    max_visible_rows: usize,
    now_secs: u64,
) {
    let scroll_offset = state.nip90.scroll_offset;
    let total_events = state.nip90.events.len();
    let total_height = total_events as f32 * nip90_row_h;
    let max_scroll = (total_height - content_h).max(0.0);
    state.nip90.scroll_offset = scroll_offset.min(max_scroll);

    if state.nip90.events.is_empty() {
        let empty_text = match state.nip90.relay_status {
            RelayStatus::Connecting => "Connecting to relay.damus.io...",
            RelayStatus::Connected => "Listening for DVM jobs...",
            RelayStatus::Disconnected => "Not connected",
            RelayStatus::Error => "Connection error",
        };
        let empty_run = text_system.layout(
            empty_text,
            Point::new(pad + 12.0, content_y + 8.0),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(empty_run);
    } else {
        for (i, event) in state.nip90.events.iter().enumerate() {
            let event_row_y = content_y + (i as f32 * nip90_row_h) - state.nip90.scroll_offset;

            // Skip rows outside visible area (must be fully within bounds)
            if event_row_y < content_y || event_row_y + nip90_row_h > content_bottom {
                state.nip90_event_bounds.push(Bounds::ZERO);
                continue;
            }
            // Get type-specific color
            let (type_color, badge_bg_alpha) = match &event.event_type {
                Nip90EventType::JobRequest { job_type, .. } => {
                    match job_type {
                        JobType::TextGeneration | JobType::TextExtraction | JobType::Summarization
                            => (theme::accent::PRIMARY, 0.2),
                        JobType::Translation { .. } => (theme::status::SUCCESS, 0.2),
                        JobType::ImageGeneration => (theme::status::WARNING, 0.2),
                        JobType::SpeechToText | JobType::TextToSpeech => (theme::accent::SECONDARY, 0.2),
                        JobType::NostrDiscovery | JobType::NostrFiltering => (theme::status::INFO, 0.2),
                        JobType::Unknown(_) => (theme::text::MUTED, 0.15),
                    }
                }
                Nip90EventType::JobResult { .. } => (theme::status::SUCCESS, 0.15),
                Nip90EventType::JobFeedback { status, .. } => {
                    match status.as_str() {
                        "success" => (theme::status::SUCCESS, 0.15),
                        "error" => (theme::status::ERROR, 0.15),
                        "payment-required" => (theme::status::WARNING, 0.15),
                        _ => (theme::text::MUTED, 0.15),
                    }
                }
            };

            // Badge (compact, left side)
            let badge_text = event.badge();
            let badge_w = 48.0;
            let badge_bounds = Bounds::new(pad + 8.0, event_row_y + 3.0, badge_w, 18.0);
            scene.draw_quad(
                Quad::new(badge_bounds)
                    .with_background(type_color.with_alpha(badge_bg_alpha)),
            );
            let badge_run = text_system.layout(
                &badge_text,
                Point::new(pad + 12.0, event_row_y + 5.0),
                10.0,
                type_color,
            );
            scene.draw_text(badge_run);

            // Pubkey (after badge)
            let pubkey_x = pad + badge_w + 16.0;
            let pubkey_run = text_system.layout(
                &event.short_pubkey(),
                Point::new(pubkey_x, event_row_y + 5.0),
                10.0,
                theme::text::MUTED,
            );
            scene.draw_text(pubkey_run);

            // Content preview (main area - uses most of the width)
            let content_start_x = pubkey_x + 80.0;
            let time_width = 36.0;
            let content_max_width = feed_w - (content_start_x - pad) - time_width - 16.0;
            let max_content_chars = (content_max_width / 6.5) as usize;

            let content_preview = event.display_content(max_content_chars);
            if !content_preview.is_empty() {
                let content_run = text_system.layout(
                    &content_preview,
                    Point::new(content_start_x, event_row_y + 5.0),
                    10.0,
                    theme::text::PRIMARY,
                );
                scene.draw_text(content_run);
            }

            // Timestamp (right-aligned)
            let time_text = event.relative_time(now_secs);
            let time_run = text_system.layout(
                &time_text,
                Point::new(pad + feed_w - time_width, event_row_y + 5.0),
                10.0,
                theme::text::MUTED,
            );
            scene.draw_text(time_run);

            state.nip90_event_bounds.push(Bounds::new(pad, event_row_y, feed_w, nip90_row_h));
        }

        // Draw scroll indicator if content overflows
        if total_events > max_visible_rows {
            let scroll_pct = state.nip90.scroll_offset / max_scroll.max(1.0);
            let track_height = content_h - 20.0;
            let thumb_height = (content_h / total_height) * track_height;
            let thumb_y = content_y + 10.0 + scroll_pct * (track_height - thumb_height);

            // Track
            scene.draw_quad(
                Quad::new(Bounds::new(pad + feed_w - 8.0, content_y + 5.0, 4.0, track_height))
                    .with_background(theme::bg::SURFACE),
            );
            // Thumb
            scene.draw_quad(
                Quad::new(Bounds::new(pad + feed_w - 8.0, thumb_y, 4.0, thumb_height.max(20.0)))
                    .with_background(theme::text::MUTED),
            );
        }
    }
}

fn draw_directory_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    pad: f32,
    feed_w: f32,
    content_y: f32,
    content_h: f32,
    content_bottom: f32,
    nip90_row_h: f32,
    max_visible_rows: usize,
) {
    let scroll_offset = state.dvm_directory.scroll_offset;
    let total_dvms = state.dvm_directory.dvms.len();
    let total_height = total_dvms as f32 * nip90_row_h;
    let max_scroll = (total_height - content_h).max(0.0);
    state.dvm_directory.scroll_offset = scroll_offset.min(max_scroll);

    if state.dvm_directory.dvms.is_empty() {
        let empty_text = match state.nip90.relay_status {
            RelayStatus::Connecting => "Discovering DVMs...",
            RelayStatus::Connected => "No DVMs found yet",
            _ => "Connect to discover DVMs",
        };
        let empty_run = text_system.layout(
            empty_text,
            Point::new(pad + 12.0, content_y + 8.0),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(empty_run);
    } else {
        for (i, dvm) in state.dvm_directory.dvms.iter().enumerate() {
            let dvm_row_y = content_y + (i as f32 * nip90_row_h) - state.dvm_directory.scroll_offset;

            // Skip rows outside visible area (must be fully within bounds)
            if dvm_row_y < content_y || dvm_row_y + nip90_row_h > content_bottom {
                continue;
            }
            // DVM name
            let name = dvm.display_name();
            let name_run = text_system.layout(
                &name,
                Point::new(pad + 12.0, dvm_row_y + 5.0),
                11.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(name_run);

            // Supported job type badges
            let badge_start_x = pad + 140.0;
            let mut badge_x = badge_start_x;
            for job_type in dvm.supported_job_types().iter().take(4) {
                let badge_text = job_type.badge();
                let badge_w = 36.0;
                let badge_color = match job_type {
                    JobType::TextGeneration | JobType::TextExtraction | JobType::Summarization
                        => theme::accent::PRIMARY,
                    JobType::Translation { .. } => theme::status::SUCCESS,
                    JobType::ImageGeneration => theme::status::WARNING,
                    JobType::SpeechToText | JobType::TextToSpeech => theme::accent::SECONDARY,
                    JobType::NostrDiscovery | JobType::NostrFiltering => theme::status::INFO,
                    JobType::Unknown(_) => theme::text::MUTED,
                };
                scene.draw_quad(
                    Quad::new(Bounds::new(badge_x, dvm_row_y + 3.0, badge_w, 16.0))
                        .with_background(badge_color.with_alpha(0.15)),
                );
                let badge_run = text_system.layout(
                    badge_text,
                    Point::new(badge_x + 4.0, dvm_row_y + 5.0),
                    9.0,
                    badge_color,
                );
                scene.draw_text(badge_run);
                badge_x += badge_w + 4.0;
            }

            // Show +N if more than 4 types
            let total_types = dvm.supported_job_types().len();
            if total_types > 4 {
                let more_text = format!("+{}", total_types - 4);
                let more_run = text_system.layout(
                    &more_text,
                    Point::new(badge_x + 4.0, dvm_row_y + 5.0),
                    9.0,
                    theme::text::MUTED,
                );
                scene.draw_text(more_run);
            }

            // Pubkey (right side)
            let pubkey_run = text_system.layout(
                &dvm.short_pubkey(),
                Point::new(pad + feed_w - 80.0, dvm_row_y + 5.0),
                10.0,
                theme::text::MUTED,
            );
            scene.draw_text(pubkey_run);
        }

        // Draw scroll indicator if content overflows
        if total_dvms > max_visible_rows {
            let scroll_pct = state.dvm_directory.scroll_offset / max_scroll.max(1.0);
            let track_height = content_h - 20.0;
            let thumb_height = (content_h / total_height) * track_height;
            let thumb_y = content_y + 10.0 + scroll_pct * (track_height - thumb_height);

            // Track
            scene.draw_quad(
                Quad::new(Bounds::new(pad + feed_w - 8.0, content_y + 5.0, 4.0, track_height))
                    .with_background(theme::bg::SURFACE),
            );
            // Thumb
            scene.draw_quad(
                Quad::new(Bounds::new(pad + feed_w - 8.0, thumb_y, 4.0, thumb_height.max(20.0)))
                    .with_background(theme::text::MUTED),
            );
        }
    }
}

fn draw_global_notes(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    pad: f32,
    feed_w: f32,
    global_y: f32,
    _height: f32,
    now_secs: u64,
) {
    let global_row_h = 26.0;
    let max_global_rows = 6; // Fixed visible height
    let global_h = 48.0 + (max_global_rows as f32 * global_row_h);

    // Global feed container
    scene.draw_quad(
        Quad::new(Bounds::new(pad, global_y, feed_w, global_h))
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    // Header: "GLOBAL NOTES"
    let global_title = "GLOBAL NOTES";
    let global_title_run = text_system.layout(
        global_title,
        Point::new(pad + 12.0, global_y + 8.0),
        12.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(global_title_run);

    // Relay status indicator (right side)
    let (global_status_color, global_status_label) = match state.nip90.relay_status {
        RelayStatus::Connected => (theme::status::SUCCESS, "LIVE"),
        RelayStatus::Connecting => (theme::status::WARNING, "..."),
        RelayStatus::Disconnected => (theme::text::MUTED, "OFF"),
        RelayStatus::Error => (theme::status::ERROR, "ERR"),
    };
    scene.draw_quad(
        Quad::new(Bounds::new(pad + feed_w - 50.0, global_y + 10.0, 8.0, 8.0))
            .with_background(global_status_color)
            .with_corner_radius(4.0),
    );
    let global_status_run = text_system.layout(
        global_status_label,
        Point::new(pad + feed_w - 38.0, global_y + 8.0),
        10.0,
        global_status_color,
    );
    scene.draw_text(global_status_run);

    // Content area
    let global_content_y = global_y + 32.0;
    let global_content_h = max_global_rows as f32 * global_row_h;
    let global_content_bottom = global_content_y + global_content_h;

    // Set scrollable content bounds
    state.global_feed_bounds = Bounds::new(pad, global_content_y, feed_w, global_content_h);

    // Clear note bounds for this frame
    state.global_feed_note_bounds.clear();

    // Calculate scroll limits
    let total_notes = state.global_feed.notes.len();
    let total_height = total_notes as f32 * global_row_h;
    let max_scroll = (total_height - global_content_h).max(0.0);
    state.global_feed.scroll_offset = state.global_feed.scroll_offset.min(max_scroll).max(0.0);

    if state.global_feed.notes.is_empty() {
        let empty_text = match state.nip90.relay_status {
            RelayStatus::Connecting => "Connecting to relays...",
            RelayStatus::Connected => "Loading notes...",
            RelayStatus::Disconnected => "Not connected",
            RelayStatus::Error => "Connection error",
        };
        let empty_run = text_system.layout(
            empty_text,
            Point::new(pad + 12.0, global_content_y + 8.0),
            11.0,
            theme::text::MUTED,
        );
        scene.draw_text(empty_run);
    } else {
        for (i, note) in state.global_feed.notes.iter().enumerate() {
            let note_y = global_content_y + (i as f32 * global_row_h) - state.global_feed.scroll_offset;

            // Skip rows outside visible area
            if note_y + global_row_h < global_content_y || note_y > global_content_bottom {
                state.global_feed_note_bounds.push(Bounds::ZERO);
                continue;
            }

            // Author name (from metadata or short pubkey)
            let author_name = state.global_feed.authors
                .get(&note.pubkey)
                .and_then(|a| a.best_name())
                .unwrap_or_else(|| &note.pubkey[..8.min(note.pubkey.len())]);

            let name_run = text_system.layout(
                author_name,
                Point::new(pad + 8.0, note_y + 5.0),
                10.0,
                theme::accent::PRIMARY,
            );
            scene.draw_text(name_run);

            // Content preview (main area) - single line only
            let content_start_x = pad + 100.0;
            let time_width = 36.0;
            let content_max_width = feed_w - (content_start_x - pad) - time_width - 16.0;
            let max_content_chars = (content_max_width / 6.5) as usize;

            // Take first line only, strip whitespace
            let first_line = note.content
                .lines()
                .next()
                .unwrap_or("")
                .trim();

            let content_preview = if first_line.chars().count() > max_content_chars {
                // Safe UTF-8 truncation using char boundaries
                let truncate_at = max_content_chars.saturating_sub(3);
                let safe_end = first_line
                    .char_indices()
                    .nth(truncate_at)
                    .map(|(i, _)| i)
                    .unwrap_or(first_line.len());
                format!("{}...", &first_line[..safe_end])
            } else {
                first_line.to_string()
            };

            if !content_preview.is_empty() {
                let content_run = text_system.layout(
                    &content_preview,
                    Point::new(content_start_x, note_y + 5.0),
                    10.0,
                    theme::text::PRIMARY,
                );
                scene.draw_text(content_run);
            }

            // Timestamp (right-aligned)
            let time_text = note.relative_time(now_secs);
            let time_run = text_system.layout(
                &time_text,
                Point::new(pad + feed_w - time_width, note_y + 5.0),
                10.0,
                theme::text::MUTED,
            );
            scene.draw_text(time_run);

            state.global_feed_note_bounds.push(Bounds::new(pad, note_y, feed_w, global_row_h));
        }

        // Draw scroll indicator if content overflows
        if total_notes > max_global_rows {
            let scroll_pct = state.global_feed.scroll_offset / max_scroll.max(1.0);
            let track_height = global_content_h - 20.0;
            let thumb_height = (global_content_h / total_height) * track_height;
            let thumb_y = global_content_y + 10.0 + scroll_pct * (track_height - thumb_height);

            // Track
            scene.draw_quad(
                Quad::new(Bounds::new(pad + feed_w - 8.0, global_content_y + 5.0, 4.0, track_height))
                    .with_background(theme::bg::SURFACE),
            );
            // Thumb
            scene.draw_quad(
                Quad::new(Bounds::new(pad + feed_w - 8.0, thumb_y, 4.0, thumb_height.max(20.0)))
                    .with_background(theme::text::MUTED),
            );
        }
    }
}
