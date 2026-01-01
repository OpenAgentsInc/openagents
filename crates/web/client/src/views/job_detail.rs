use wgpui::{Bounds, Point, Quad, Scene, TextSystem, theme};

use crate::nostr::{DvmJob, JobType, Nip90EventType};
use crate::state::AppState;

/// Draw job detail view
pub(crate) fn draw_job_detail(
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

            // Truncate long input values (UTF-8 safe)
            let value = if input.value.chars().count() > 60 {
                let safe_end = input.value.char_indices().nth(57).map(|(i, _)| i).unwrap_or(input.value.len());
                format!("{}...", &input.value[..safe_end])
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
