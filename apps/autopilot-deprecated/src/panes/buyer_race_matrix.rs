use std::collections::{BTreeMap, BTreeSet};

use wgpui::components::hud::{DotShape, DotsGrid, SignalMeter};
use wgpui::{Bounds, Component, Hsla, PaintContext, Point, Quad, theme};

use crate::app_state::NetworkRequestsState;
use crate::nip90_compute_flow::{BuyerRequestFlowSnapshot, build_buyer_request_flow_snapshot};
use crate::nip90_compute_semantics::{analyze_invoice_amount_msats, normalize_pubkey};
use crate::pane_renderer::{paint_label_line, paint_source_badge};
use crate::spark_wallet::SparkPaneState;
use crate::state::operations::{
    BuyerResolutionReason, NetworkRequestDuplicateOutcome, NetworkRequestProviderObservation,
    NetworkRequestProviderObservationHistoryEvent, NetworkRequestProviderObservationHistoryKind,
    SubmittedNetworkRequest,
};

const HEADER_HEIGHT: f32 = 72.0;
const CARD_HEIGHT: f32 = 82.0;
const LEGEND_HEIGHT: f32 = 24.0;
const FIELD_PADDING: f32 = 12.0;
const LANE_GAP: f32 = 14.0;
const LANE_MIN_WIDTH: f32 = 164.0;

pub fn paint(
    content_bounds: Bounds,
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
    paint: &mut PaintContext,
) {
    paint_source_badge(content_bounds, "buy+race", paint);
    let Some(view) = build_view(network_requests, spark_wallet) else {
        paint_empty_state(content_bounds, paint);
        return;
    };

    let header_bounds = Bounds::new(
        content_bounds.origin.x + FIELD_PADDING,
        content_bounds.origin.y + FIELD_PADDING,
        content_bounds.size.width - FIELD_PADDING * 2.0,
        HEADER_HEIGHT,
    );
    let card_bounds = Bounds::new(
        header_bounds.origin.x,
        header_bounds.max_y() + 8.0,
        header_bounds.size.width,
        CARD_HEIGHT,
    );
    let legend_bounds = Bounds::new(
        card_bounds.origin.x,
        card_bounds.max_y() + 6.0,
        card_bounds.size.width,
        LEGEND_HEIGHT,
    );
    let lanes_bounds = Bounds::new(
        legend_bounds.origin.x,
        legend_bounds.max_y() + 8.0,
        legend_bounds.size.width,
        content_bounds.max_y() - legend_bounds.max_y() - 20.0,
    );

    paint_header(header_bounds, &view, paint);
    paint_request_card(card_bounds, &view, paint);
    paint_legend(legend_bounds, paint);
    paint_lane_field(lanes_bounds, &view, paint);
}

#[derive(Clone, Debug)]
struct BuyerRaceMatrixView {
    request_id: String,
    request_type: String,
    phase_label: String,
    authority_label: String,
    next_expected_event: String,
    budget_sats: u64,
    provider_count: usize,
    history_count: usize,
    selected_label: String,
    result_label: String,
    invoice_label: String,
    payable_label: String,
    loser_summary: Option<String>,
    lanes: Vec<BuyerRaceLane>,
}

#[derive(Clone, Debug)]
struct BuyerRaceLane {
    provider_pubkey: String,
    provider_label: String,
    role_tags: Vec<&'static str>,
    result_seen: bool,
    invoice_seen: bool,
    payable_selected: bool,
    payment_queued: bool,
    wallet_pending: bool,
    wallet_settled: bool,
    blocked_over_budget: bool,
    last_status_label: String,
    reason_label: String,
    relay_count: usize,
    stage_depth: usize,
    sparks: Vec<BuyerRaceSpark>,
}

#[derive(Clone, Debug)]
struct BuyerRaceSpark {
    progress: f32,
    kind: NetworkRequestProviderObservationHistoryKind,
    emphasized: bool,
}

fn build_view(
    network_requests: &NetworkRequestsState,
    spark_wallet: &SparkPaneState,
) -> Option<BuyerRaceMatrixView> {
    let request = selected_request(network_requests)?;
    let snapshot = build_buyer_request_flow_snapshot(request, spark_wallet);
    let providers = ordered_provider_pubkeys(request, &snapshot);
    if providers.is_empty() {
        return None;
    }

    let history_len = snapshot.provider_observation_history.len().max(1);
    let lanes = providers
        .iter()
        .map(|provider_pubkey| {
            build_lane(request, &snapshot, provider_pubkey.as_str(), history_len)
        })
        .collect::<Vec<_>>();

    Some(BuyerRaceMatrixView {
        request_id: request.request_id.clone(),
        request_type: request.request_type.clone(),
        phase_label: snapshot.phase.as_str().to_string(),
        authority_label: snapshot.authority.as_str().to_string(),
        next_expected_event: snapshot.next_expected_event.clone(),
        budget_sats: request.budget_sats,
        provider_count: providers.len(),
        history_count: snapshot.provider_observation_history.len(),
        selected_label: provider_label(snapshot.selected_provider_pubkey()),
        result_label: provider_label(snapshot.result_provider_pubkey()),
        invoice_label: provider_label(snapshot.invoice_provider_pubkey()),
        payable_label: provider_label(snapshot.payable_provider_pubkey.as_deref()),
        loser_summary: snapshot.loser_reason_summary.clone(),
        lanes,
    })
}

fn selected_request<'a>(
    network_requests: &'a NetworkRequestsState,
) -> Option<&'a SubmittedNetworkRequest> {
    network_requests
        .submitted
        .iter()
        .find(|request| {
            !request.status.is_terminal()
                && (!request.provider_observations.is_empty()
                    || !request.provider_observation_history.is_empty()
                    || !request.target_provider_pubkeys.is_empty())
        })
        .or_else(|| {
            network_requests.submitted.iter().find(|request| {
                !request.provider_observations.is_empty()
                    || !request.provider_observation_history.is_empty()
                    || !request.target_provider_pubkeys.is_empty()
            })
        })
        .or_else(|| network_requests.submitted.first())
}

fn ordered_provider_pubkeys(
    request: &SubmittedNetworkRequest,
    snapshot: &BuyerRequestFlowSnapshot,
) -> Vec<String> {
    let mut providers = BTreeSet::<String>::new();
    for provider in &request.target_provider_pubkeys {
        let normalized = normalize_pubkey(provider);
        if !normalized.is_empty() {
            providers.insert(normalized);
        }
    }
    for observation in &request.provider_observations {
        let normalized = normalize_pubkey(observation.provider_pubkey.as_str());
        if !normalized.is_empty() {
            providers.insert(normalized);
        }
    }
    for history_event in &snapshot.provider_observation_history {
        if let Some(provider_pubkey) = history_event.provider_pubkey.as_deref() {
            let normalized = normalize_pubkey(provider_pubkey);
            if !normalized.is_empty() {
                providers.insert(normalized);
            }
        }
        if let Some(previous_provider_pubkey) = history_event.previous_provider_pubkey.as_deref() {
            let normalized = normalize_pubkey(previous_provider_pubkey);
            if !normalized.is_empty() {
                providers.insert(normalized);
            }
        }
    }
    for role_provider in [
        snapshot.selected_provider_pubkey(),
        snapshot.result_provider_pubkey(),
        snapshot.invoice_provider_pubkey(),
        snapshot.payable_provider_pubkey.as_deref(),
    ] {
        if let Some(provider_pubkey) = role_provider {
            let normalized = normalize_pubkey(provider_pubkey);
            if !normalized.is_empty() {
                providers.insert(normalized);
            }
        }
    }

    let history_rank = request
        .provider_observation_history
        .iter()
        .enumerate()
        .fold(
            BTreeMap::<String, usize>::new(),
            |mut ranks, (index, event)| {
                for provider_pubkey in [
                    event.provider_pubkey.as_deref(),
                    event.previous_provider_pubkey.as_deref(),
                ] {
                    if let Some(provider_pubkey) = provider_pubkey {
                        ranks
                            .entry(normalize_pubkey(provider_pubkey))
                            .or_insert(index);
                    }
                }
                ranks
            },
        );

    let mut ordered = providers.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        provider_role_priority(left.as_str(), snapshot)
            .cmp(&provider_role_priority(right.as_str(), snapshot))
            .then_with(|| {
                history_rank
                    .get(left)
                    .copied()
                    .unwrap_or(usize::MAX)
                    .cmp(&history_rank.get(right).copied().unwrap_or(usize::MAX))
            })
            .then_with(|| left.cmp(right))
    });
    ordered
}

fn provider_role_priority(provider_pubkey: &str, snapshot: &BuyerRequestFlowSnapshot) -> u8 {
    let normalized = normalize_pubkey(provider_pubkey);
    if snapshot
        .payable_provider_pubkey
        .as_deref()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        return 0;
    }
    if snapshot
        .selected_provider_pubkey()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        return 1;
    }
    if snapshot
        .result_provider_pubkey()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        return 2;
    }
    if snapshot
        .invoice_provider_pubkey()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        return 3;
    }
    4
}

fn build_lane(
    request: &SubmittedNetworkRequest,
    snapshot: &BuyerRequestFlowSnapshot,
    provider_pubkey: &str,
    history_len: usize,
) -> BuyerRaceLane {
    let observation = provider_observation(request, provider_pubkey);
    let normalized = normalize_pubkey(provider_pubkey);
    let role_tags = lane_role_tags(provider_pubkey, snapshot);
    let result_seen = observation.is_some_and(provider_has_non_error_result);
    let invoice_seen = observation.is_some_and(provider_has_invoice_signal);
    let payable_selected = snapshot
        .payable_provider_pubkey
        .as_deref()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized);
    let payment_queued = payable_selected && request.pending_bolt11.is_some();
    let wallet_settled = payable_selected && request.payment_sent_at_epoch_seconds.is_some();
    let wallet_pending = payable_selected
        && !wallet_settled
        && (request.last_payment_pointer.is_some() || request.pending_bolt11.is_some());
    let blocked_over_budget = provider_invoice_amount_sats(observation)
        .is_some_and(|amount| amount > request.budget_sats);
    let stage_depth = if wallet_settled || wallet_pending || payment_queued {
        4
    } else if payable_selected {
        3
    } else if invoice_seen {
        2
    } else if result_seen {
        1
    } else {
        0
    };
    let last_status_label = lane_last_status_label(observation);
    let reason_label = lane_reason_label(request, provider_pubkey, observation, payable_selected);
    let sparks = lane_sparks(
        snapshot.provider_observation_history.as_slice(),
        provider_pubkey,
        history_len,
    );
    let relay_count = observation
        .map(|observation| {
            observation
                .last_feedback_relay_urls
                .len()
                .max(observation.last_result_relay_urls.len())
        })
        .unwrap_or_default();

    BuyerRaceLane {
        provider_pubkey: provider_pubkey.to_string(),
        provider_label: short_id(provider_pubkey),
        role_tags,
        result_seen,
        invoice_seen,
        payable_selected,
        payment_queued,
        wallet_pending,
        wallet_settled,
        blocked_over_budget,
        last_status_label,
        reason_label,
        relay_count,
        stage_depth,
        sparks,
    }
}

fn lane_role_tags(provider_pubkey: &str, snapshot: &BuyerRequestFlowSnapshot) -> Vec<&'static str> {
    let normalized = normalize_pubkey(provider_pubkey);
    let mut tags = Vec::new();
    if snapshot
        .selected_provider_pubkey()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        tags.push("SEL");
    }
    if snapshot
        .result_provider_pubkey()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        tags.push("RES");
    }
    if snapshot
        .invoice_provider_pubkey()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        tags.push("INV");
    }
    if snapshot
        .payable_provider_pubkey
        .as_deref()
        .is_some_and(|provider| normalize_pubkey(provider) == normalized)
    {
        tags.push("PAY");
    }
    tags
}

fn lane_sparks(
    history: &[NetworkRequestProviderObservationHistoryEvent],
    provider_pubkey: &str,
    history_len: usize,
) -> Vec<BuyerRaceSpark> {
    let normalized = normalize_pubkey(provider_pubkey);
    history
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            let event_provider = event.provider_pubkey.as_deref().map(normalize_pubkey);
            let previous_provider = event
                .previous_provider_pubkey
                .as_deref()
                .map(normalize_pubkey);
            if event_provider.as_deref() != Some(normalized.as_str())
                && previous_provider.as_deref() != Some(normalized.as_str())
            {
                return None;
            }
            let emphasized = event_provider.as_deref() == Some(normalized.as_str());
            let progress = if history_len <= 1 {
                1.0
            } else {
                index as f32 / (history_len.saturating_sub(1) as f32)
            };
            Some(BuyerRaceSpark {
                progress,
                kind: event.kind,
                emphasized,
            })
        })
        .collect()
}

fn lane_reason_label(
    request: &SubmittedNetworkRequest,
    provider_pubkey: &str,
    observation: Option<&NetworkRequestProviderObservation>,
    payable_selected: bool,
) -> String {
    if payable_selected {
        if request.payment_error.is_some() {
            return "wallet failed".to_string();
        }
        if request.payment_sent_at_epoch_seconds.is_some() {
            return "buyer wallet settled".to_string();
        }
        if request.last_payment_pointer.is_some() {
            return "wallet pending".to_string();
        }
        if request.pending_bolt11.is_some() {
            return "payment queued".to_string();
        }
        return "current payable winner".to_string();
    }

    if let Some(observation) = observation {
        if provider_invoice_amount_sats(Some(observation))
            .is_some_and(|amount| amount > request.budget_sats)
        {
            return format!(
                "over budget ({})",
                provider_invoice_amount_sats(Some(observation))
                    .map(|amount| format!("{amount} sats"))
                    .unwrap_or_else(|| "unknown".to_string())
            );
        }
    }

    if duplicate_reason_for_provider(
        request,
        provider_pubkey,
        BuyerResolutionReason::LateResultUnpaid,
    ) {
        return "late result".to_string();
    }
    if duplicate_reason_for_provider(request, provider_pubkey, BuyerResolutionReason::LostRace) {
        return "lost race".to_string();
    }

    if let Some(observation) = observation {
        if provider_has_non_error_result(observation) && !provider_has_valid_invoice(observation) {
            return "no invoice".to_string();
        }
        if provider_has_error_only_signal(observation) {
            return "error-only".to_string();
        }
        if provider_has_valid_invoice(observation) && !provider_has_non_error_result(observation) {
            return "invoice without result".to_string();
        }
        if provider_has_processing_feedback(observation) {
            return "processing".to_string();
        }
        if observation.last_feedback_event_id.is_some()
            || observation.last_result_event_id.is_some()
        {
            return "observed".to_string();
        }
    }

    "waiting".to_string()
}

fn duplicate_reason_for_provider(
    request: &SubmittedNetworkRequest,
    provider_pubkey: &str,
    reason: BuyerResolutionReason,
) -> bool {
    request
        .duplicate_outcomes
        .iter()
        .any(|outcome: &NetworkRequestDuplicateOutcome| {
            normalize_pubkey(outcome.provider_pubkey.as_str()) == normalize_pubkey(provider_pubkey)
                && outcome.reason_code == reason.code()
        })
}

fn provider_observation<'a>(
    request: &'a SubmittedNetworkRequest,
    provider_pubkey: &str,
) -> Option<&'a NetworkRequestProviderObservation> {
    let normalized = normalize_pubkey(provider_pubkey);
    request
        .provider_observations
        .iter()
        .find(|observation| normalize_pubkey(observation.provider_pubkey.as_str()) == normalized)
}

fn provider_has_non_error_result(observation: &NetworkRequestProviderObservation) -> bool {
    observation.last_result_event_id.is_some()
        && !matches!(
            observation
                .last_result_status
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("error")
        )
}

fn provider_has_valid_invoice(observation: &NetworkRequestProviderObservation) -> bool {
    observation
        .last_feedback_bolt11
        .as_deref()
        .map(str::trim)
        .is_some_and(|bolt11| !bolt11.is_empty())
}

fn provider_has_invoice_signal(observation: &NetworkRequestProviderObservation) -> bool {
    provider_has_valid_invoice(observation)
        || observation.last_feedback_amount_msats.is_some()
        || observation
            .last_feedback_status
            .as_deref()
            .is_some_and(|status| status.eq_ignore_ascii_case("payment-required"))
}

fn provider_has_processing_feedback(observation: &NetworkRequestProviderObservation) -> bool {
    observation
        .last_feedback_status
        .as_deref()
        .is_some_and(|status| status.eq_ignore_ascii_case("processing"))
}

fn provider_has_error_only_signal(observation: &NetworkRequestProviderObservation) -> bool {
    !provider_has_non_error_result(observation)
        && !provider_has_valid_invoice(observation)
        && observation
            .last_feedback_status
            .as_deref()
            .or(observation.last_result_status.as_deref())
            .is_some_and(|status| status.eq_ignore_ascii_case("error"))
}

fn provider_invoice_amount_sats(
    observation: Option<&NetworkRequestProviderObservation>,
) -> Option<u64> {
    analyze_invoice_amount_msats(
        observation.and_then(|observation| observation.last_feedback_amount_msats),
        observation.and_then(|observation| observation.last_feedback_bolt11.as_deref()),
    )
    .effective_amount_msats
    .map(msats_to_sats_ceil)
    .filter(|amount| *amount > 0)
}

fn lane_last_status_label(observation: Option<&NetworkRequestProviderObservation>) -> String {
    if let Some(observation) = observation {
        if let Some(status) = observation.last_feedback_status.as_deref() {
            let status = status.trim();
            if !status.is_empty() {
                return format!("feedback:{status}");
            }
        }
        if let Some(status) = observation.last_result_status.as_deref() {
            let status = status.trim();
            if !status.is_empty() {
                return format!("result:{status}");
            }
        }
        if observation.last_result_event_id.is_some() {
            return "result:observed".to_string();
        }
        if observation.last_feedback_event_id.is_some() {
            return "feedback:observed".to_string();
        }
    }
    "idle".to_string()
}

fn provider_label(provider_pubkey: Option<&str>) -> String {
    provider_pubkey
        .map(short_id)
        .unwrap_or_else(|| "none".to_string())
}

fn short_id(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 12 {
        trimmed.to_string()
    } else {
        format!("{}..{}", &trimmed[..6], &trimmed[trimmed.len() - 4..])
    }
}

fn msats_to_sats_ceil(msats: u64) -> u64 {
    if msats == 0 {
        return 0;
    }
    msats.saturating_add(999) / 1_000
}

fn paint_empty_state(content_bounds: Bounds, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "BUYER RACE MATRIX  //  IDLE",
        Point::new(
            content_bounds.origin.x + FIELD_PADDING,
            content_bounds.origin.y + FIELD_PADDING + 8.0,
        ),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "No buyer request has provider competition data yet. Open the pane while a NIP-90 request is racing to see result, invoice, and payable winner lanes.",
        Point::new(
            content_bounds.origin.x + FIELD_PADDING,
            content_bounds.origin.y + FIELD_PADDING + 28.0,
        ),
        10.0,
        theme::text::MUTED,
    ));
}

fn paint_header(bounds: Bounds, view: &BuyerRaceMatrixView, paint: &mut PaintContext) {
    paint.scene.draw_text(paint.text.layout_mono(
        "BUYER RACE MATRIX  //  LIVE NIP-90 PROVIDER COMPETITION",
        Point::new(bounds.origin.x, bounds.origin.y + 6.0),
        12.0,
        theme::text::PRIMARY,
    ));
    paint.scene.draw_text(paint.text.layout(
        "One lane per provider pubkey. Role chips split `SEL`, `RES`, `INV`, and `PAY`, while replay sparks preserve observed order without forcing the pane to guess.",
        Point::new(bounds.origin.x, bounds.origin.y + 26.0),
        10.0,
        theme::text::MUTED,
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "req={}  phase={}  authority={}  next={}  providers={}  history={}",
                short_id(view.request_id.as_str()),
                view.phase_label,
                view.authority_label,
                view.next_expected_event,
                view.provider_count,
                view.history_count,
            )
            .as_str(),
            Point::new(bounds.origin.x, bounds.origin.y + 46.0),
            10.0,
            theme::text::SECONDARY,
        ),
    );

    let meter_level =
        ((view.provider_count.min(5) as f32) / 5.0).max((view.history_count.min(8) as f32) / 8.0);
    let mut meter = SignalMeter::new()
        .bars(6)
        .gap(2.0)
        .level(meter_level)
        .min_bar_height(0.15)
        .active_color(Hsla::from_hex(0x52d273).with_alpha(0.9))
        .inactive_color(theme::bg::ELEVATED.with_alpha(0.6));
    meter.paint(
        Bounds::new(bounds.max_x() - 54.0, bounds.origin.y + 4.0, 40.0, 42.0),
        paint,
    );
}

fn paint_request_card(bounds: Bounds, view: &BuyerRaceMatrixView, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x08141d).with_alpha(0.96))
            .with_border(Hsla::from_hex(0x3cb6ff).with_alpha(0.28), 1.0)
            .with_corner_radius(10.0),
    );

    let mut grid = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(24.0)
        .size(1.1)
        .color(Hsla::from_hex(0x59b8ff).with_alpha(0.08))
        .animation_progress(1.0);
    grid.paint(
        Bounds::new(
            bounds.origin.x + 8.0,
            bounds.origin.y + 8.0,
            bounds.size.width - 16.0,
            bounds.size.height - 16.0,
        ),
        paint,
    );

    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "{}  //  kind={}  budget={} sats",
                view.request_id, view.request_type, view.budget_sats,
            )
            .as_str(),
            Point::new(bounds.origin.x + 12.0, bounds.origin.y + 16.0),
            11.0,
            theme::text::PRIMARY,
        ),
    );

    let left_x = bounds.origin.x + 12.0;
    let right_x = bounds.origin.x + bounds.size.width * 0.55;
    let mut left_y = bounds.origin.y + 34.0;
    left_y = paint_label_line(
        paint,
        left_x,
        left_y,
        "Selected",
        view.selected_label.as_str(),
    );
    left_y = paint_label_line(paint, left_x, left_y, "Result", view.result_label.as_str());
    let _ = paint_label_line(
        paint,
        left_x,
        left_y,
        "Invoice",
        view.invoice_label.as_str(),
    );

    let mut right_y = bounds.origin.y + 34.0;
    right_y = paint_label_line(
        paint,
        right_x,
        right_y,
        "Payable",
        view.payable_label.as_str(),
    );
    right_y = paint_label_line(
        paint,
        right_x,
        right_y,
        "Replay strip",
        if view.history_count > 0 {
            "ready"
        } else {
            "awaiting events"
        },
    );
    let _ = paint_label_line(
        paint,
        right_x,
        right_y,
        "Losers",
        view.loser_summary.as_deref().unwrap_or("none"),
    );
}

fn paint_legend(bounds: Bounds, paint: &mut PaintContext) {
    let legend = [
        ("SEL", Hsla::from_hex(0x63c7ff)),
        ("RES", Hsla::from_hex(0x75b7ff)),
        ("INV", Hsla::from_hex(0xffbf69)),
        ("PAY", Hsla::from_hex(0x77dd77)),
    ];
    for (index, (label, color)) in legend.into_iter().enumerate() {
        let chip_bounds = Bounds::new(
            bounds.origin.x + index as f32 * 70.0,
            bounds.origin.y + 2.0,
            56.0,
            18.0,
        );
        paint_role_chip(chip_bounds, label, color, paint);
    }
    paint.scene.draw_text(paint.text.layout(
        "Role chips stay separate even when one provider owns multiple roles.",
        Point::new(bounds.origin.x + 310.0, bounds.origin.y + 14.0),
        9.0,
        theme::text::MUTED,
    ));
}

fn paint_lane_field(bounds: Bounds, view: &BuyerRaceMatrixView, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(Hsla::from_hex(0x060d13).with_alpha(0.92))
            .with_border(Hsla::from_hex(0x2f4858).with_alpha(0.28), 1.0)
            .with_corner_radius(10.0),
    );

    let mut grid = DotsGrid::new()
        .shape(DotShape::Cross)
        .distance(22.0)
        .size(1.0)
        .color(Hsla::from_hex(0x2a9d8f).with_alpha(0.07))
        .animation_progress(1.0);
    grid.paint(
        Bounds::new(
            bounds.origin.x + 10.0,
            bounds.origin.y + 10.0,
            bounds.size.width - 20.0,
            bounds.size.height - 20.0,
        ),
        paint,
    );

    let lane_count = view.lanes.len().max(1);
    let lane_width = ((bounds.size.width - 24.0 - ((lane_count - 1) as f32 * LANE_GAP))
        / lane_count as f32)
        .max(LANE_MIN_WIDTH.min(bounds.size.width - 24.0));
    let total_width =
        lane_width * lane_count as f32 + LANE_GAP * (lane_count.saturating_sub(1) as f32);
    let start_x = bounds.origin.x + ((bounds.size.width - total_width) * 0.5).max(12.0);

    for (index, lane) in view.lanes.iter().enumerate() {
        let lane_bounds = Bounds::new(
            start_x + index as f32 * (lane_width + LANE_GAP),
            bounds.origin.y + 12.0,
            lane_width,
            bounds.size.height - 24.0,
        );
        paint_lane(lane_bounds, lane, paint);
    }
}

fn paint_lane(bounds: Bounds, lane: &BuyerRaceLane, paint: &mut PaintContext) {
    let accent = lane_accent(lane);
    let shell_background = if lane.payable_selected {
        Hsla::from_hex(0x0f2a19).with_alpha(0.95)
    } else {
        Hsla::from_hex(0x0a121a).with_alpha(0.94)
    };
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(shell_background)
            .with_border(accent.with_alpha(0.35), 1.0)
            .with_corner_radius(10.0),
    );

    paint.scene.draw_text(paint.text.layout_mono(
        lane.provider_label.as_str(),
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 14.0),
        11.0,
        if lane.payable_selected {
            theme::text::PRIMARY
        } else {
            theme::text::SECONDARY
        },
    ));
    paint.scene.draw_text(paint.text.layout_mono(
        lane.last_status_label.as_str(),
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 30.0),
        9.0,
        theme::text::MUTED,
    ));

    let chip_width = 34.0;
    let chip_gap = 4.0;
    for (index, tag) in lane.role_tags.iter().enumerate() {
        let chip_bounds = Bounds::new(
            bounds.max_x() - 10.0 - chip_width - index as f32 * (chip_width + chip_gap),
            bounds.origin.y + 10.0,
            chip_width,
            16.0,
        );
        paint_role_chip(chip_bounds, tag, accent, paint);
    }

    let mut meter = SignalMeter::new()
        .bars(5)
        .gap(2.0)
        .level((lane.stage_depth as f32 / 4.0).max(lane.relay_count.min(5) as f32 / 5.0))
        .min_bar_height(0.16)
        .active_color(accent.with_alpha(0.92))
        .inactive_color(theme::bg::ELEVATED.with_alpha(0.45));
    meter.paint(
        Bounds::new(bounds.max_x() - 38.0, bounds.origin.y + 42.0, 24.0, 56.0),
        paint,
    );

    let rail_x = bounds.origin.x + bounds.size.width * 0.5;
    let rail_top = bounds.origin.y + 70.0;
    let rail_bottom = bounds.max_y() - 54.0;
    let rail_height = rail_bottom - rail_top;
    paint.scene.draw_quad(
        Quad::new(Bounds::new(rail_x - 2.0, rail_top, 4.0, rail_height))
            .with_background(theme::bg::ELEVATED.with_alpha(0.6))
            .with_corner_radius(2.0),
    );
    if lane.stage_depth > 0 {
        let active_height = rail_height * (lane.stage_depth as f32 / 4.0);
        paint.scene.draw_quad(
            Quad::new(Bounds::new(rail_x - 2.0, rail_top, 4.0, active_height))
                .with_background(accent.with_alpha(0.96))
                .with_corner_radius(2.0),
        );
    }

    let stage_specs = [
        ("RESULT", lane.result_seen, Hsla::from_hex(0x75b7ff)),
        ("INVOICE", lane.invoice_seen, Hsla::from_hex(0xffbf69)),
        ("PAYABLE", lane.payable_selected, Hsla::from_hex(0x77dd77)),
        (
            if lane.wallet_settled {
                "SETTLED"
            } else if lane.wallet_pending {
                "WALLET"
            } else if lane.payment_queued {
                "QUEUE"
            } else if lane.blocked_over_budget {
                "BUDGET"
            } else {
                "WAIT"
            },
            lane.wallet_settled
                || lane.wallet_pending
                || lane.payment_queued
                || lane.blocked_over_budget,
            if lane.blocked_over_budget {
                Hsla::from_hex(0xff7f50)
            } else {
                Hsla::from_hex(0x52d273)
            },
        ),
    ];
    for (index, (label, active, color)) in stage_specs.into_iter().enumerate() {
        let progress = index as f32 / 3.0;
        let y = rail_top + rail_height * progress;
        paint_stage_node(bounds, rail_x, y, label, active, color, paint);
    }

    for spark in &lane.sparks {
        let y = rail_top + rail_height * spark.progress.clamp(0.0, 1.0);
        let (color, size) = spark_visuals(spark);
        let offset = if spark.emphasized { -10.0 } else { 10.0 };
        paint.scene.draw_quad(
            Quad::new(Bounds::new(
                rail_x + offset - size * 0.5,
                y - size * 0.5,
                size,
                size,
            ))
            .with_background(color)
            .with_corner_radius(size * 0.5),
        );
    }

    paint.scene.draw_text(paint.text.layout(
        lane.reason_label.as_str(),
        Point::new(bounds.origin.x + 10.0, bounds.max_y() - 28.0),
        9.0,
        if lane.payable_selected {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));
    paint.scene.draw_text(
        paint.text.layout_mono(
            format!(
                "provider={}  relays={}",
                short_id(lane.provider_pubkey.as_str()),
                lane.relay_count,
            )
            .as_str(),
            Point::new(bounds.origin.x + 10.0, bounds.max_y() - 12.0),
            8.5,
            theme::text::MUTED,
        ),
    );
}

fn paint_stage_node(
    bounds: Bounds,
    rail_x: f32,
    y: f32,
    label: &str,
    active: bool,
    color: Hsla,
    paint: &mut PaintContext,
) {
    let dot_color = if active {
        color.with_alpha(0.98)
    } else {
        theme::bg::ELEVATED.with_alpha(0.75)
    };
    paint.scene.draw_quad(
        Quad::new(Bounds::new(rail_x - 7.0, y - 7.0, 14.0, 14.0))
            .with_background(dot_color)
            .with_corner_radius(7.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 10.0, y + 4.0),
        8.5,
        if active {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        },
    ));
}

fn paint_role_chip(bounds: Bounds, label: &str, color: Hsla, paint: &mut PaintContext) {
    paint.scene.draw_quad(
        Quad::new(bounds)
            .with_background(color.with_alpha(0.12))
            .with_border(color.with_alpha(0.42), 1.0)
            .with_corner_radius(7.0),
    );
    paint.scene.draw_text(paint.text.layout_mono(
        label,
        Point::new(bounds.origin.x + 7.0, bounds.origin.y + 10.0),
        8.5,
        color.with_alpha(0.96),
    ));
}

fn lane_accent(lane: &BuyerRaceLane) -> Hsla {
    if lane.payable_selected {
        Hsla::from_hex(0x77dd77)
    } else if lane.blocked_over_budget {
        Hsla::from_hex(0xff7f50)
    } else if lane.invoice_seen && lane.result_seen {
        Hsla::from_hex(0x63c7ff)
    } else if lane.invoice_seen {
        Hsla::from_hex(0xffbf69)
    } else if lane.result_seen {
        Hsla::from_hex(0x75b7ff)
    } else if lane.reason_label == "error-only" {
        Hsla::from_hex(0xf25f5c)
    } else {
        Hsla::from_hex(0x5d6d7e)
    }
}

fn spark_visuals(spark: &BuyerRaceSpark) -> (Hsla, f32) {
    match spark.kind {
        NetworkRequestProviderObservationHistoryKind::FeedbackObserved => (
            Hsla::from_hex(0xffbf69).with_alpha(if spark.emphasized { 0.96 } else { 0.7 }),
            if spark.emphasized { 8.0 } else { 6.0 },
        ),
        NetworkRequestProviderObservationHistoryKind::ResultObserved => (
            Hsla::from_hex(0x75b7ff).with_alpha(if spark.emphasized { 0.96 } else { 0.72 }),
            if spark.emphasized { 8.0 } else { 6.0 },
        ),
        NetworkRequestProviderObservationHistoryKind::PayableWinnerSelected => (
            Hsla::from_hex(0x77dd77).with_alpha(0.98),
            if spark.emphasized { 10.0 } else { 7.0 },
        ),
        NetworkRequestProviderObservationHistoryKind::PayableWinnerCleared => (
            Hsla::from_hex(0xf25f5c).with_alpha(0.92),
            if spark.emphasized { 10.0 } else { 7.0 },
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::build_view;
    use crate::app_state::MISSION_CONTROL_BUY_MODE_REQUEST_TYPE;
    use crate::spark_wallet::SparkPaneState;
    use crate::state::operations::{
        BuyerResolutionMode, NetworkRequestSubmission, NetworkRequestsState,
    };

    #[test]
    fn buyer_race_matrix_splits_provider_roles_and_loser_reasons_from_evidence() {
        let mut requests = NetworkRequestsState::default();
        let request_id = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-race-pane-001".to_string()),
                request_type: MISSION_CONTROL_BUY_MODE_REQUEST_TYPE.to_string(),
                payload: "test race".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec![
                    "provideralpha001".to_string(),
                    "providerbeta002".to_string(),
                    "providergamma003".to_string(),
                ],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 2,
                timeout_seconds: 60,
                authority_command_seq: 1,
            })
            .expect("request should queue");
        requests.apply_nip90_request_publish_outcome(
            request_id.as_str(),
            "event-pane-request",
            1,
            0,
            None,
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            "provideralpha001",
            "event-pane-result-alpha",
            Some("success"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            "providerbeta002",
            "event-pane-result-beta",
            Some("error"),
        );
        requests.apply_nip90_buyer_result_event(
            request_id.as_str(),
            "providergamma003",
            "event-pane-result-gamma",
            Some("success"),
        );
        requests.apply_nip90_buyer_feedback_event(
            request_id.as_str(),
            "providergamma003",
            "event-pane-feedback-gamma",
            Some("payment-required"),
            Some("invoice ready"),
            Some(2_000),
            Some("lnbc2n1gamma"),
        );

        let view = build_view(&requests, &SparkPaneState::default()).expect("view should exist");
        let alpha_lane = view
            .lanes
            .iter()
            .find(|lane| lane.provider_pubkey == "provideralpha001")
            .expect("alpha lane should exist");
        assert!(alpha_lane.result_seen);
        assert!(!alpha_lane.invoice_seen);
        assert_eq!(alpha_lane.reason_label, "no invoice");

        let beta_lane = view
            .lanes
            .iter()
            .find(|lane| lane.provider_pubkey == "providerbeta002")
            .expect("beta lane should exist");
        assert_eq!(beta_lane.reason_label, "error-only");

        let gamma_lane = view
            .lanes
            .iter()
            .find(|lane| lane.provider_pubkey == "providergamma003")
            .expect("gamma lane should exist");
        assert!(gamma_lane.payable_selected);
        assert!(gamma_lane.role_tags.contains(&"SEL"));
        assert!(gamma_lane.role_tags.contains(&"RES"));
        assert!(gamma_lane.role_tags.contains(&"INV"));
        assert!(gamma_lane.role_tags.contains(&"PAY"));
    }

    #[test]
    fn buyer_race_matrix_prefers_live_request_with_provider_activity() {
        let mut requests = NetworkRequestsState::default();
        let older = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-older".to_string()),
                request_type: "text-generation".to_string(),
                payload: "older".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec!["providerolder001".to_string()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 5,
                timeout_seconds: 60,
                authority_command_seq: 1,
            })
            .expect("older request should queue");
        requests.apply_nip90_request_publish_outcome(older.as_str(), "event-older", 1, 0, None);

        let newer = requests
            .queue_request_submission(NetworkRequestSubmission {
                request_id: Some("req-newer".to_string()),
                request_type: "text-generation".to_string(),
                payload: "newer".to_string(),
                resolution_mode: BuyerResolutionMode::Race,
                target_provider_pubkeys: vec!["providernewer001".to_string()],
                skill_scope_id: None,
                credit_envelope_ref: None,
                budget_sats: 5,
                timeout_seconds: 60,
                authority_command_seq: 2,
            })
            .expect("newer request should queue");
        requests.apply_nip90_request_publish_outcome(newer.as_str(), "event-newer", 1, 0, None);
        requests.apply_nip90_buyer_result_event(
            newer.as_str(),
            "providernewer001",
            "event-newer-result",
            Some("success"),
        );

        let view = build_view(&requests, &SparkPaneState::default()).expect("view should exist");
        assert_eq!(view.request_id, "req-newer");
        assert_eq!(view.provider_count, 1);
    }
}
