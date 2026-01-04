//! Jobs panel - NIP-90 job list with status

use wgpui::{Bounds, Hsla, Point, Quad, Scene, Size, TextSystem};

use crate::state::{FmVizState, InputFocus, JobStatus};

use super::{accent_cyan, accent_green, panel_bg, text_dim};

/// Draw jobs panel (left side)
pub fn draw_jobs_panel(
    scene: &mut Scene,
    text: &mut TextSystem,
    state: &FmVizState,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) {
    // Panel background
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        })
        .with_background(panel_bg()),
    );

    // Header with focus indicator
    let header_color = if state.input_focus == InputFocus::Jobs {
        accent_cyan()
    } else {
        text_dim()
    };
    let run = text.layout("JOBS", Point::new(x + 12.0, y + 8.0), 11.0, header_color);
    scene.draw_text(run);

    // Job count
    let count_text = format!("{}", state.jobs.len());
    let count_x = x + width - 30.0;
    let run = text.layout(&count_text, Point::new(count_x, y + 8.0), 11.0, text_dim());
    scene.draw_text(run);

    // Divider
    scene.draw_quad(
        Quad::new(Bounds {
            origin: Point::new(x + 12.0, y + 28.0),
            size: Size::new(width - 24.0, 1.0),
        })
        .with_background(text_dim().with_alpha(0.3)),
    );

    // Job list
    let list_y = y + 36.0;
    let row_height = 28.0;
    let max_rows = ((height - 44.0) / row_height).floor() as usize;

    for (i, job) in state.jobs.iter().take(max_rows).enumerate() {
        let row_y = list_y + i as f32 * row_height;

        // Highlight current job
        if Some(&job.id) == state.current_job_id.as_ref() {
            scene.draw_quad(
                Quad::new(Bounds {
                    origin: Point::new(x + 4.0, row_y),
                    size: Size::new(width - 8.0, row_height - 2.0),
                })
                .with_background(accent_cyan().with_alpha(0.1)),
            );
        }

        // Direction + Status icon
        // Incoming (we serve): > for serving, + for complete
        // Outgoing (we requested): < for pending, . for received
        let (icon, icon_color) = if job.is_outgoing {
            // Outgoing job - we requested this
            match job.status {
                JobStatus::Pending => ("<", accent_cyan()),  // Awaiting result
                JobStatus::Serving => ("<", accent_cyan()),  // Still processing
                JobStatus::Complete => (".", accent_green()), // Received result
                JobStatus::Failed => ("x", Hsla::new(0.0, 0.9, 0.5, 1.0)),
            }
        } else {
            // Incoming job - we serve this
            match job.status {
                JobStatus::Pending => ("*", text_dim()),
                JobStatus::Serving => (">", accent_cyan()),
                JobStatus::Complete => ("+", accent_green()),
                JobStatus::Failed => ("x", Hsla::new(0.0, 0.9, 0.5, 1.0)),
            }
        };

        let run = text.layout(icon, Point::new(x + 12.0, row_y + 6.0), 12.0, icon_color);
        scene.draw_text(run);

        // Kind (show direction)
        let kind_text = if job.is_outgoing { "REQ" } else { "5050" };
        let run = text.layout(kind_text, Point::new(x + 28.0, row_y + 6.0), 10.0, text_dim());
        scene.draw_text(run);

        // From/To pubkey (shortened)
        let pubkey_short = if job.from_pubkey.len() > 8 {
            format!("{}...", &job.from_pubkey[..8])
        } else {
            job.from_pubkey.clone()
        };
        let direction_text = if job.is_outgoing {
            format!("to {}", pubkey_short)  // We requested from network
        } else {
            format!("from {}", pubkey_short)  // We received from this pubkey
        };
        let run = text.layout(&direction_text, Point::new(x + 70.0, row_y + 6.0), 10.0, text_dim());
        scene.draw_text(run);

        // Status text (right aligned) - different for incoming vs outgoing
        let status_text = if job.is_outgoing {
            match job.status {
                JobStatus::Pending => "WAIT",
                JobStatus::Serving => "WAIT",
                JobStatus::Complete => "RECV",
                JobStatus::Failed => "FAIL",
            }
        } else {
            match job.status {
                JobStatus::Pending => "PEND",
                JobStatus::Serving => "SERV",
                JobStatus::Complete => "+1",
                JobStatus::Failed => "FAIL",
            }
        };
        let status_color = match job.status {
            JobStatus::Pending => text_dim(),
            JobStatus::Serving => accent_cyan(),
            JobStatus::Complete => accent_green(),
            JobStatus::Failed => Hsla::new(0.0, 0.9, 0.5, 1.0),
        };
        let status_width = text.measure(status_text, 10.0);
        let status_x = x + width - status_width - 12.0;
        let run = text.layout(status_text, Point::new(status_x, row_y + 6.0), 10.0, status_color);
        scene.draw_text(run);
    }

    // Empty state
    if state.jobs.is_empty() {
        let empty_text = "No jobs yet";
        let empty_x = x + (width - text.measure(empty_text, 11.0)) / 2.0;
        let empty_y = y + height / 2.0;
        let run = text.layout(empty_text, Point::new(empty_x, empty_y), 11.0, text_dim());
        scene.draw_text(run);
    }
}
