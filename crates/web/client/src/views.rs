use wgpui::{Bounds, Point, Quad, Scene, TextSystem, theme};

use crate::hud::draw_hud_view;
use crate::nostr::{BazaarJobType, DvmJob, DvmView, JobType, Nip90EventType, RelayStatus, VerificationStatus};
use crate::state::{AppState, JobStatus};

// Re-export for timestamp access
use js_sys;

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

            // Issue reference or repo URL
            let ref_text = job.issue_ref.as_ref()
                .or(job.repo_url.as_ref())
                .map(|s| if s.len() > 25 { format!("{}...", &s[..22]) } else { s.clone() })
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
            // === FEED VIEW: NIP-90 Events ===
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
        DvmView::Directory => {
            // === DIRECTORY VIEW: NIP-89 DVMs ===
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
        DvmView::JobDetail(ref job_id) => {
            // === JOB DETAIL VIEW ===
            if let Some(job) = state.nip90.get_job(job_id) {
                draw_job_detail(scene, text_system, &job, pad, content_y, feed_w, now_secs, state);
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

    // === DUAL CTA CARDS ===
    let cards_y = dvm_y + dvm_h + 16.0;
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

    // === GLOBAL NOTES FEED ===
    let global_y = cards_y + card_h + 16.0;
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

            let content_preview = if first_line.len() > max_content_chars {
                format!("{}...", &first_line[..max_content_chars.saturating_sub(3)])
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

/// Draw job detail view
fn draw_job_detail(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    job: &DvmJob,
    pad: f32,
    start_y: f32,
    width: f32,
    now_secs: u64,
    state: &mut AppState,
) {
    let mut y = start_y;
    let row_h = 24.0;

    // Back button
    let back_bounds = Bounds::new(pad + 8.0, y, 50.0, 20.0);
    state.nip90_event_bounds.clear();
    state.nip90_event_bounds.push(back_bounds); // Use first bounds as back button
    scene.draw_quad(
        Quad::new(back_bounds)
            .with_background(theme::text::MUTED.with_alpha(0.15)),
    );
    let back_run = text_system.layout(
        "← Back",
        Point::new(pad + 12.0, y + 4.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(back_run);

    // Job ID (right side)
    let id_text = format!("#{}", &job.request.id[..8.min(job.request.id.len())]);
    let id_run = text_system.layout(
        &id_text,
        Point::new(pad + width - 80.0, y + 4.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(id_run);

    y += row_h + 8.0;

    // === REQUEST SECTION ===
    // Job type badge
    let job_type = job.request.job_type();
    let (type_color, type_label) = if let Some(jt) = job_type {
        let color = match jt {
            JobType::TextGeneration | JobType::TextExtraction | JobType::Summarization
                => theme::accent::PRIMARY,
            JobType::Translation { .. } => theme::status::SUCCESS,
            JobType::ImageGeneration => theme::status::WARNING,
            JobType::SpeechToText | JobType::TextToSpeech => theme::accent::SECONDARY,
            JobType::NostrDiscovery | JobType::NostrFiltering => theme::status::INFO,
            JobType::Unknown(_) => theme::text::MUTED,
        };
        (color, jt.label())
    } else {
        (theme::text::MUTED, "Request")
    };

    let label_run = text_system.layout(
        type_label,
        Point::new(pad + 8.0, y),
        12.0,
        type_color,
    );
    scene.draw_text(label_run);

    // Requester pubkey and time
    let meta_text = format!("from {} · {}", job.request.short_pubkey(), job.request.relative_time(now_secs));
    let meta_run = text_system.layout(
        &meta_text,
        Point::new(pad + 120.0, y + 2.0),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(meta_run);

    y += row_h;

    // Input content
    if let Nip90EventType::JobRequest { inputs, .. } = &job.request.event_type {
        for input in inputs.iter().take(3) {
            let input_type = input.input_type.as_deref().unwrap_or("text");
            let type_text = format!("[{}]", input_type.to_uppercase());
            let type_run = text_system.layout(
                &type_text,
                Point::new(pad + 16.0, y + 2.0),
                9.0,
                theme::text::MUTED,
            );
            scene.draw_text(type_run);

            // Truncate long input values
            let value = if input.value.len() > 60 {
                format!("{}...", &input.value[..57])
            } else {
                input.value.clone()
            };
            let value_run = text_system.layout(
                &value,
                Point::new(pad + 56.0, y + 2.0),
                10.0,
                theme::text::PRIMARY,
            );
            scene.draw_text(value_run);

            y += row_h - 4.0;
        }
    }

    y += 12.0;

    // === RESPONSES SECTION ===
    let dvm_count = job.dvm_count();
    let response_label = if dvm_count > 0 {
        format!("RESPONSES ({} DVMs)", dvm_count)
    } else {
        "RESPONSES".to_string()
    };
    let response_run = text_system.layout(
        &response_label,
        Point::new(pad + 8.0, y),
        10.0,
        theme::text::MUTED,
    );
    scene.draw_text(response_run);

    y += row_h;

    if job.results.is_empty() && job.feedback.is_empty() {
        let waiting_text = "Waiting for DVM responses...";
        let waiting_run = text_system.layout(
            waiting_text,
            Point::new(pad + 16.0, y),
            10.0,
            theme::text::MUTED,
        );
        scene.draw_text(waiting_run);
    } else {
        // Group results by DVM
        for (pubkey, results) in job.results_by_dvm().iter().take(4) {
            // DVM header
            let dvm_text = format!("DVM {}...", &pubkey[..8.min(pubkey.len())]);
            scene.draw_quad(
                Quad::new(Bounds::new(pad + 12.0, y, width - 32.0, row_h + 8.0))
                    .with_background(theme::status::SUCCESS.with_alpha(0.08)),
            );
            let dvm_run = text_system.layout(
                &dvm_text,
                Point::new(pad + 16.0, y + 2.0),
                10.0,
                theme::status::SUCCESS,
            );
            scene.draw_text(dvm_run);

            y += row_h;

            // Show result content
            for result in results.iter().take(2) {
                let content = result.display_content(80);
                if !content.is_empty() {
                    let content_run = text_system.layout(
                        &content,
                        Point::new(pad + 20.0, y),
                        10.0,
                        theme::text::PRIMARY,
                    );
                    scene.draw_text(content_run);
                    y += row_h - 4.0;
                }
            }

            y += 8.0;
        }

        // Show feedback events
        for feedback in job.feedback.iter().take(3) {
            if let Nip90EventType::JobFeedback { status, .. } = &feedback.event_type {
                let status_color = match status.as_str() {
                    "success" => theme::status::SUCCESS,
                    "error" => theme::status::ERROR,
                    "payment-required" => theme::status::WARNING,
                    _ => theme::text::MUTED,
                };
                let fb_text = format!("[{}] {}", status.to_uppercase(), feedback.short_pubkey());
                let fb_run = text_system.layout(
                    &fb_text,
                    Point::new(pad + 16.0, y),
                    9.0,
                    status_color,
                );
                scene.draw_text(fb_run);
                y += row_h - 6.0;
            }
        }
    }
}
