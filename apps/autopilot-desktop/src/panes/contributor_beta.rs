use wgpui::{Bounds, PaintContext, Point, theme};

use crate::app_state::{ContributorBetaPaneState, ContributorSubmissionOutcome};
use crate::pane_renderer::{
    paint_action_button, paint_disabled_button, paint_label_line, paint_multiline_phrase,
    paint_primary_button, paint_secondary_button, paint_selectable_row_background,
    paint_source_badge, paint_state_summary,
};
use crate::pane_system::{
    contributor_beta_action_button_bounds, contributor_beta_row_bounds,
};

const MAX_ROWS: usize = 5;

pub fn paint(
    content_bounds: Bounds,
    pane_state: &ContributorBetaPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "autopilot.contributor_beta", paint);

    paint_action_button(
        contributor_beta_action_button_bounds(content_bounds, 0),
        "Connect",
        paint,
    );
    if pane_state.contract_accepted {
        paint_secondary_button(
            contributor_beta_action_button_bounds(content_bounds, 1),
            "Contract OK",
            paint,
        );
    } else {
        paint_primary_button(
            contributor_beta_action_button_bounds(content_bounds, 1),
            "Accept Contract",
            paint,
        );
    }
    if pane_state.identity_connected && pane_state.contract_accepted {
        paint_primary_button(
            contributor_beta_action_button_bounds(content_bounds, 2),
            "Run Benchmark",
            paint,
        );
        paint_primary_button(
            contributor_beta_action_button_bounds(content_bounds, 3),
            "Submit Receipt",
            paint,
        );
        paint_action_button(
            contributor_beta_action_button_bounds(content_bounds, 4),
            "Cycle Worker",
            paint,
        );
        paint_primary_button(
            contributor_beta_action_button_bounds(content_bounds, 5),
            "Run Worker",
            paint,
        );
    } else {
        for index in 2..6 {
            paint_disabled_button(
                contributor_beta_action_button_bounds(content_bounds, index),
                match index {
                    2 => "Run Benchmark",
                    3 => "Submit Receipt",
                    4 => "Cycle Worker",
                    _ => "Run Worker",
                },
                paint,
            );
        }
    }

    let mut y = paint_state_summary(
        paint,
        content_bounds.origin.x + 12.0,
        contributor_beta_action_button_bounds(content_bounds, 0).max_y() + 12.0,
        pane_state.load_state,
        "Bounded external contributor beta",
        pane_state.last_action.as_deref(),
        pane_state.last_error.as_deref(),
    );

    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Contributor",
        &pane_state.contributor_id,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Trust tier",
        pane_state.trust_tier.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Environment",
        &pane_state.environment_class,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Capabilities",
        &pane_state.capability_summary,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Contract",
        &format!(
            "{} ({})",
            pane_state.contract_version,
            if pane_state.contract_accepted {
                "accepted"
            } else {
                "pending"
            }
        ),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Admitted family",
        &pane_state.admitted_family,
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Worker role",
        pane_state.worker_role.label(),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Accepted / review",
        &format!(
            "{} / {}",
            pane_state.accepted_submission_count, pane_state.review_submission_count
        ),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Rejected / quarantined",
        &format!(
            "{} / {}",
            pane_state.rejected_submission_count, pane_state.quarantined_submission_count
        ),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Pending credit sats",
        &pane_state.pending_credit_sats.to_string(),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Credit account",
        pane_state
            .contributor_credit_account_id
            .as_deref()
            .unwrap_or("pending identity"),
    );
    let _ = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Payment linkage",
        &pane_state.payment_link_state,
    );

    let list_title_y = content_bounds.max_y() - 176.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "RECENT BETA SUBMISSIONS",
        Point::new(content_bounds.origin.x + 12.0, list_title_y),
        10.0,
        theme::text::MUTED,
    ));

    for (row_index, row) in pane_state.submissions.iter().take(MAX_ROWS).enumerate() {
        let row_bounds = contributor_beta_row_bounds(content_bounds, row_index);
        paint_selectable_row_background(paint, row_bounds, row_index == 0);
        paint.scene.draw_text(paint.text.layout_mono(
            &format!("{} :: {}", row.source.label(), outcome_chip(row.outcome)),
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 7.0),
            10.0,
            theme::text::PRIMARY,
        ));
        paint.scene.draw_text(paint.text.layout(
            &row.summary,
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 23.0),
            10.0,
            theme::text::PRIMARY,
        ));
        let detail = row
            .review_reason
            .as_deref()
            .map(|reason| format!("{reason} :: {}", compact_digest(&row.digest)))
            .unwrap_or_else(|| compact_digest(&row.digest));
        paint.scene.draw_text(paint.text.layout_mono(
            &detail,
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 37.0),
            9.0,
            theme::text::MUTED,
        ));
    }
}

fn compact_digest(digest: &str) -> String {
    if digest.len() <= 20 {
        return digest.to_string();
    }
    format!("{}…{}", &digest[..10], &digest[digest.len() - 8..])
}

fn outcome_chip(outcome: ContributorSubmissionOutcome) -> &'static str {
    match outcome {
        ContributorSubmissionOutcome::Accepted => "ACCEPTED",
        ContributorSubmissionOutcome::Rejected => "REJECTED",
        ContributorSubmissionOutcome::Quarantined => "QUARANTINED",
        ContributorSubmissionOutcome::Review => "REVIEW",
    }
}
