use wgpui::{Bounds, PaintContext, Point, theme};

use crate::app_state::{ContributorBetaPaneState, ContributorSubmissionOutcome};
use crate::pane_renderer::{
    paint_action_button, paint_disabled_button, paint_label_line, paint_multiline_phrase,
    paint_primary_button, paint_secondary_button, paint_selectable_row_background,
    paint_source_badge, paint_state_summary,
};
use crate::pane_system::{contributor_beta_action_button_bounds, contributor_beta_row_bounds};

const MAX_ROWS: usize = 4;

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
            "Run Tailnet Pack",
            paint,
        );
        paint_primary_button(
            contributor_beta_action_button_bounds(content_bounds, 3),
            "Submit Disagreement",
            paint,
        );
        paint_action_button(
            contributor_beta_action_button_bounds(content_bounds, 4),
            "Cycle Role",
            paint,
        );
        paint_primary_button(
            contributor_beta_action_button_bounds(content_bounds, 5),
            "Run Governed Role",
            paint,
        );
    } else {
        for index in 2..6 {
            paint_disabled_button(
                contributor_beta_action_button_bounds(content_bounds, index),
                match index {
                    2 => "Run Tailnet Pack",
                    3 => "Submit Disagreement",
                    4 => "Cycle Role",
                    _ => "Run Governed Role",
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
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Tailnet pilot",
        &format!(
            "{}{}",
            pane_state.tailnet_pilot_label,
            pane_state
                .tailnet_current_tailnet
                .as_deref()
                .map(|tailnet| format!(" ({tailnet})"))
                .unwrap_or_default()
        ),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Nodes",
        &tailnet_nodes_summary(pane_state),
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
        "States",
        &pane_state.submission_state_summary,
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
        "Review queue",
        &format!(
            "{} ({}) :: owner {}",
            pane_state.review_queue_depth,
            pane_state.review_sla_label,
            pane_state.review_owner_label
        ),
    );
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Confirmed / provisional / hold",
        &format!(
            "{} / {} / {} sats",
            pane_state.confirmed_credit_sats,
            pane_state.pending_credit_sats,
            pane_state.review_hold_credit_sats
        ),
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
    y = paint_label_line(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Payment linkage",
        &pane_state.payment_link_state,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Credit rules",
        &pane_state.provisional_credit_rulebook,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Credit policy",
        &pane_state.credit_policy_summary,
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Pilot digests",
        &format!(
            "run {} :: xtrain {} :: report {}",
            compact_digest(&pane_state.tailnet_last_governed_run_digest),
            compact_digest(&pane_state.tailnet_last_xtrain_receipt_digest),
            compact_digest(&pane_state.tailnet_last_operational_report_digest)
        ),
    );
    y = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Latest runtime",
        &latest_runtime_summary(pane_state),
    );
    let _ = paint_multiline_phrase(
        paint,
        content_bounds.origin.x + 12.0,
        y,
        "Known operators",
        &known_operator_summary(pane_state),
    );

    let list_title_y = content_bounds.max_y() - 176.0;
    paint.scene.draw_text(paint.text.layout_mono(
        "RECENT TAILNET BETA SUBMISSIONS",
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
            .map(|reason| {
                format!(
                    "{} :: {} :: {}",
                    review_reason_label(reason),
                    lineage_summary(row),
                    compact_digest(&row.digest)
                )
            })
            .unwrap_or_else(|| {
                format!(
                    "{} :: {}",
                    lineage_summary(row),
                    compact_digest(&row.digest)
                )
            });
        let lineage = row
            .source_receipt_id
            .as_deref()
            .map(|receipt_id| format!("{detail} :: {receipt_id}"))
            .unwrap_or(detail);
        paint.scene.draw_text(paint.text.layout_mono(
            &lineage,
            Point::new(row_bounds.origin.x + 10.0, row_bounds.origin.y + 37.0),
            9.0,
            theme::text::MUTED,
        ));
    }
}

fn tailnet_nodes_summary(pane_state: &ContributorBetaPaneState) -> String {
    if pane_state.tailnet_nodes.is_empty() {
        return "tailnet roster unavailable".to_string();
    }
    pane_state
        .tailnet_nodes
        .iter()
        .map(|node| {
            let ip = node.tailnet_ip.as_deref().unwrap_or("ip-pending");
            format!("{} {} {} {}", node.role, node.device_name, node.status, ip)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn latest_runtime_summary(pane_state: &ContributorBetaPaneState) -> String {
    match pane_state.latest_runtime_receipt_id.as_deref() {
        Some(receipt_id) => format!(
            "{} :: {} :: {} :: review path replay_candidate_pending_review",
            receipt_id,
            pane_state
                .latest_runtime_authority_path
                .as_deref()
                .unwrap_or("authority-pending"),
            pane_state
                .latest_runtime_confidence_band
                .as_deref()
                .unwrap_or("confidence-pending")
        ),
        None => "no runtime disagreement captured yet".to_string(),
    }
}

fn review_reason_label(reason: &str) -> &'static str {
    match reason {
        "contract_mismatch" => "contract mismatch",
        "digest_mismatch" => "digest mismatch",
        "schema_violation" => "schema violation",
        "low_signal_duplicate" => "low-signal duplicate",
        "confidence_anomaly" => "confidence anomaly",
        "disagreement_retained_for_replay_review" => "replay-review disagreement",
        "no_accepted_benchmark_lineage" => "no accepted benchmark lineage",
        _ => "manual review",
    }
}

fn lineage_summary(row: &crate::app_state::ContributorBetaSubmissionRow) -> String {
    format!(
        "{} -> {} -> {} -> {} -> {}",
        row.staging_state,
        row.quarantine_state,
        row.replay_status,
        row.training_impact,
        row.credit_disposition.label()
    )
}

fn known_operator_summary(pane_state: &ContributorBetaPaneState) -> String {
    if pane_state.known_operators.is_empty() {
        return "no known external operators admitted yet".to_string();
    }
    pane_state
        .known_operators
        .iter()
        .map(|operator| {
            format!(
                "{} {} {} {}",
                operator.display_name,
                operator.contract_status,
                operator.readiness_status,
                operator.credit_posture
            )
        })
        .collect::<Vec<_>>()
        .join(" | ")
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
