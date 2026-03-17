use crate::app_state::{
    AttnResLabBlockSummary, AttnResLabInferenceSummary, AttnResLabMetricPoint, AttnResLabPaneState,
    AttnResLabSnapshot, AttnResLabSublayerSnapshot, PaneLoadState, PaneStatusAccess,
};
use psionic_models::{
    AttnResConfig, AttnResDiagnosticsSnapshot, AttnResNextTokenSample, AttnResSublayerKind,
    TokenId, TokenSequence,
};
use psionic_runtime::{
    AttnResHiddenParityReport, AttnResLogitParityReport, AttnResTwoPhaseParityBudget,
    AttnResTwoPhaseParityStatus, compare_attnres_hidden_two_phase_parity,
    compare_attnres_logit_two_phase_parity,
};
use psionic_serve::{
    AttnResTextGenerationOutcome, AttnResTextGenerationRequest, AttnResTextGenerationResponse,
    LocalAttnResTextGenerationService,
};
use psionic_train::{
    AttnResTinyTrainingConfig, AttnResTinyTrainingCorpus, AttnResTinyTrainingOutcome,
    AttnResTinyTrainingStepMetrics, train_attnres_tiny_next_token,
};

const LIVE_SOURCE_BADGE: &str = "psionic.attnres";
const LIVE_REFRESH_ACTION: &str = "Refreshing Psionic AttnRes snapshot";

pub(crate) fn ensure_live_snapshot_loaded(pane_state: &mut AttnResLabPaneState) {
    if pane_state.snapshot.source_badge.starts_with("replay.") {
        refresh_live_snapshot(pane_state);
    }
}

pub(crate) fn refresh_live_snapshot(pane_state: &mut AttnResLabPaneState) {
    pane_state.load_state = PaneLoadState::Loading;
    pane_state.last_error = None;
    pane_state.last_action = Some(LIVE_REFRESH_ACTION.to_string());
    apply_refresh_result(pane_state, build_live_snapshot());
}

fn apply_refresh_result(
    pane_state: &mut AttnResLabPaneState,
    result: Result<AttnResLabSnapshot, String>,
) {
    match result {
        Ok(snapshot) => {
            let run_label = snapshot.run_label.clone();
            pane_state.snapshot = snapshot;
            pane_state.clamp_selected_sublayer();
            pane_state.pane_set_ready(format!("Loaded Psionic AttnRes snapshot for {run_label}"));
        }
        Err(error) => {
            let _ =
                pane_state.pane_set_error(format!("AttnRes live snapshot refresh failed: {error}"));
        }
    }
}

fn build_live_snapshot() -> Result<AttnResLabSnapshot, String> {
    let corpus = lab_corpus();
    let training_config =
        AttnResTinyTrainingConfig::reference().map_err(|error| error.to_string())?;
    let training_outcome = train_attnres_tiny_next_token(&corpus, &training_config)
        .map_err(|error| error.to_string())?;

    let prompt_sample = corpus
        .held_out_samples
        .first()
        .cloned()
        .or_else(|| corpus.training_samples.first().cloned())
        .ok_or_else(|| String::from("AttnRes lab corpus is empty"))?;
    let request = AttnResTextGenerationRequest::new(
        "attnres-lab-preview",
        prompt_sample.input_tokens.clone(),
        2,
    )
    .with_requested_model_id(
        training_outcome
            .trained_model
            .descriptor()
            .model
            .model_id
            .clone(),
    );
    let generation_service =
        LocalAttnResTextGenerationService::new().with_model(training_outcome.trained_model.clone());
    let generation_response = match generation_service
        .execute(&request)
        .map_err(|error| error.to_string())?
    {
        AttnResTextGenerationOutcome::Completed { response } => response,
        AttnResTextGenerationOutcome::Refused { refusal } => {
            return Err(format!("AttnRes generation refused: {}", refusal.detail));
        }
    };

    let inspection_sequence = generation_response.full_sequence.clone();
    let (_, diagnostics) = training_outcome
        .trained_model
        .forward_hidden_with_diagnostics(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let standard_hidden = training_outcome
        .trained_model
        .forward_hidden(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let standard_logits = training_outcome
        .trained_model
        .forward(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let two_phase_hidden = training_outcome
        .trained_model
        .forward_two_phase_hidden(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let two_phase_logits = training_outcome
        .trained_model
        .forward_two_phase(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;

    let parity_budget = AttnResTwoPhaseParityBudget::default();
    let hidden_parity = compare_attnres_hidden_two_phase_parity(
        standard_hidden.values(),
        two_phase_hidden.values(),
        parity_budget.hidden,
    )
    .map_err(|error| error.to_string())?;
    let logit_parity = compare_attnres_logit_two_phase_parity(
        standard_logits.values(),
        two_phase_logits.values(),
        parity_budget.logits,
    )
    .map_err(|error| error.to_string())?;

    let (_, baseline_diagnostics) = training_outcome
        .initial_model
        .forward_hidden_with_diagnostics(std::slice::from_ref(&inspection_sequence))
        .map_err(|error| error.to_string())?;
    let baseline_selectivity = mean_selectivity_from_diagnostics(&baseline_diagnostics);
    let sublayers = map_sublayers(&diagnostics, &corpus.config);
    let avg_selectivity = mean_selectivity(sublayers.as_slice());
    let block_summaries = build_block_summaries(sublayers.as_slice(), corpus.config.num_blocks);
    let final_block_index = block_summaries
        .last()
        .map(|block| block.block_index)
        .unwrap_or_default();
    let current_block_fill = sublayers
        .iter()
        .filter(|sublayer| sublayer.target_block == final_block_index)
        .count();
    let completed_blocks = diagnostics.final_completed_blocks;
    let active_block = completed_blocks
        + if diagnostics.final_partial_block_present {
            1
        } else {
            0
        };
    let active_block = active_block.max(1);
    let final_training_loss = training_outcome.summary.final_training_mean_loss;
    let final_ema_loss = ema_loss(
        training_outcome.step_metrics.as_slice(),
        final_training_loss,
    );
    let metrics = build_metric_points(
        &training_outcome,
        baseline_selectivity,
        avg_selectivity,
        final_ema_loss,
    );
    let inference = build_inference_summary(
        &corpus.config,
        &diagnostics,
        &generation_response,
        &hidden_parity,
        &logit_parity,
    );
    let events = build_events(
        &corpus,
        &training_config,
        &training_outcome,
        &generation_response,
        &hidden_parity,
        &logit_parity,
    );
    let model_descriptor = training_outcome.trained_model.descriptor();

    Ok(AttnResLabSnapshot {
        source_badge: LIVE_SOURCE_BADGE.to_string(),
        model_label: format!(
            "{} // {}",
            model_descriptor.model.model_id, model_descriptor.model.revision
        ),
        architecture_label: format!(
            "{} sublayers // {} residual blocks // {} heads",
            corpus.config.num_layers, corpus.config.num_blocks, corpus.config.num_heads
        ),
        run_label: format!(
            "{} // request {}",
            training_config.run_id, generation_response.request_id
        ),
        run_status: String::from("psionic snapshot ready"),
        step: training_outcome.summary.run_summary.completed_steps,
        max_steps: training_outcome.summary.run_summary.budget.max_steps,
        speed_multiplier: 1,
        training_loss: final_training_loss,
        ema_loss: final_ema_loss,
        avg_selectivity,
        active_block,
        current_block_fill,
        completed_blocks,
        metrics,
        sublayers,
        block_summaries,
        inference,
        events,
    })
}

fn lab_corpus() -> AttnResTinyTrainingCorpus {
    let config = AttnResConfig::new(8, 4, 2)
        .with_num_heads(2)
        .with_d_ff(16)
        .with_vocab_size(8);
    AttnResTinyTrainingCorpus {
        description: String::from("OpenAgents desktop AttnRes lab reference corpus"),
        config,
        training_samples: vec![
            sample("train-001", &[0, 1, 2], 3),
            sample("train-002", &[1, 2, 3], 4),
            sample("train-003", &[2, 3, 4], 5),
            sample("train-004", &[3, 4, 5], 6),
            sample("train-005", &[4, 5, 6], 7),
            sample("train-006", &[5, 6, 7], 0),
        ],
        held_out_samples: vec![
            sample("hold-001", &[6, 7, 0], 1),
            sample("hold-002", &[7, 0, 1], 2),
        ],
    }
}

fn sample(sample_id: &str, input_tokens: &[u32], target_token: u32) -> AttnResNextTokenSample {
    AttnResNextTokenSample::new(
        sample_id,
        TokenSequence::new(
            input_tokens
                .iter()
                .copied()
                .map(TokenId)
                .collect::<Vec<_>>(),
        ),
        TokenId(target_token),
    )
}

fn build_metric_points(
    outcome: &AttnResTinyTrainingOutcome,
    baseline_selectivity: f32,
    final_selectivity: f32,
    final_ema_loss: f32,
) -> Vec<AttnResLabMetricPoint> {
    vec![
        AttnResLabMetricPoint {
            global_step: 0,
            training_loss: outcome.summary.initial_training_mean_loss,
            ema_loss: outcome.summary.initial_training_mean_loss,
            selectivity: baseline_selectivity,
        },
        AttnResLabMetricPoint {
            global_step: outcome.summary.run_summary.completed_steps,
            training_loss: outcome.summary.final_training_mean_loss,
            ema_loss: final_ema_loss,
            selectivity: final_selectivity,
        },
    ]
}

fn build_inference_summary(
    config: &AttnResConfig,
    diagnostics: &AttnResDiagnosticsSnapshot,
    generation_response: &AttnResTextGenerationResponse,
    hidden_parity: &AttnResHiddenParityReport,
    logit_parity: &AttnResLogitParityReport,
) -> AttnResLabInferenceSummary {
    let sublayers = map_sublayers(diagnostics, config);
    let partial_merge_share = sublayers
        .iter()
        .map(|sublayer| sublayer.partial_mass)
        .sum::<f32>()
        / sublayers.len().max(1) as f32;
    let cache_merge_share = sublayers
        .iter()
        .map(|sublayer| sublayer.cache_mass)
        .sum::<f32>()
        / sublayers.len().max(1) as f32;
    let block_cache_fill_share =
        diagnostics.final_completed_blocks as f32 / config.num_blocks.max(1) as f32;
    let boundary_layers = config
        .boundary_transformer_layers()
        .unwrap_or_default()
        .into_iter()
        .map(|layer| format!("L{layer}"))
        .collect::<Vec<_>>();

    AttnResLabInferenceSummary {
        hidden_parity_label: parity_status_label(hidden_parity.status).to_string(),
        logit_parity_label: parity_status_label(logit_parity.status).to_string(),
        hidden_max_abs_diff: hidden_parity.summary.max_abs_delta,
        logit_max_abs_diff: logit_parity.summary.max_abs_delta,
        partial_merge_share,
        cache_merge_share,
        block_cache_fill_share,
        schedule_note: format!(
            "decoded {} tokens over {} sublayers; block boundaries at {}",
            generation_response.full_sequence.len(),
            diagnostics.sublayers.len(),
            join_or_dash(boundary_layers.as_slice())
        ),
        merge_note: format!(
            "average routing mass stayed {:.0}% partial vs {:.0}% cached across the inspected decode path",
            partial_merge_share * 100.0,
            cache_merge_share * 100.0
        ),
        cache_note: format!(
            "{} completed blocks cached with partial block still present={}",
            diagnostics.final_completed_blocks, diagnostics.final_partial_block_present
        ),
    }
}

fn build_events(
    corpus: &AttnResTinyTrainingCorpus,
    training_config: &AttnResTinyTrainingConfig,
    training_outcome: &AttnResTinyTrainingOutcome,
    generation_response: &AttnResTextGenerationResponse,
    hidden_parity: &AttnResHiddenParityReport,
    logit_parity: &AttnResLogitParityReport,
) -> Vec<String> {
    let loss_direction = if training_outcome.summary.final_training_mean_loss
        <= training_outcome.summary.initial_training_mean_loss
    {
        "improved"
    } else {
        "moved"
    };
    vec![
        format!(
            "seeded {} with {} train / {} held-out samples",
            training_config.run_id,
            corpus.training_samples.len(),
            corpus.held_out_samples.len()
        ),
        format!(
            "completed {} steps in the bounded tiny-training lane",
            training_outcome.summary.run_summary.completed_steps
        ),
        format!(
            "training loss {loss_direction} {:.3} -> {:.3}",
            training_outcome.summary.initial_training_mean_loss,
            training_outcome.summary.final_training_mean_loss
        ),
        format!(
            "held-out routing delta {:.3} across {} improved cases",
            training_outcome.summary.held_out_eval.mean_routing_l2_delta,
            training_outcome.summary.held_out_eval.improved_case_count
        ),
        format!(
            "generated [{}] from prompt [{}]",
            format_token_sequence(generation_response.generated_tokens.as_slice()),
            format_token_sequence(generation_response.prompt_tokens.as_slice())
        ),
        format!(
            "two-phase parity hidden={} logits={}",
            parity_status_label(hidden_parity.status),
            parity_status_label(logit_parity.status)
        ),
    ]
}

fn map_sublayers(
    diagnostics: &AttnResDiagnosticsSnapshot,
    config: &AttnResConfig,
) -> Vec<AttnResLabSublayerSnapshot> {
    let block_size = config.block_size().unwrap_or(1).max(1);
    diagnostics
        .sublayers
        .iter()
        .map(|sublayer| {
            let source_labels = source_labels(sublayer.completed_blocks_before, sublayer);
            let source_logits =
                aggregate_source_values(&sublayer.source_logits, sublayer.source_shape);
            let routing_weights =
                aggregate_source_values(&sublayer.routing_weights, sublayer.source_shape);
            let (dominant_source_label, dominant_weight) =
                dominant_source(source_labels.as_slice(), routing_weights.as_slice());
            let partial_mass = source_labels
                .iter()
                .enumerate()
                .find(|(_, label)| label.as_str() == "partial")
                .and_then(|(index, _)| routing_weights.get(index).copied())
                .unwrap_or(0.0);
            let cache_mass = (routing_weights.iter().sum::<f32>() - partial_mass).clamp(0.0, 1.0);
            let selectivity = selectivity_from_weights(routing_weights.as_slice());
            let target_block = sublayer.sublayer_index / block_size;
            AttnResLabSublayerSnapshot {
                sublayer_index: sublayer.sublayer_index,
                label: format!(
                    "L{} {}",
                    sublayer.transformer_layer_index,
                    kind_label(sublayer.kind)
                ),
                kind_label: kind_label(sublayer.kind).to_lowercase(),
                target_block,
                dominant_source_label: dominant_source_label.clone(),
                dominant_weight,
                selectivity,
                query_norm: sublayer.query_norm,
                partial_mass,
                cache_mass,
                source_labels,
                source_logits,
                routing_weights,
                route_note: route_note(
                    sublayer,
                    target_block,
                    dominant_weight,
                    cache_mass,
                    partial_mass,
                    dominant_source_label.as_str(),
                ),
                starts_new_block_before: sublayer.starts_new_block_before,
                completed_blocks_before: sublayer.completed_blocks_before,
                completed_blocks_after: sublayer.completed_blocks_after,
                partial_block_present_before: sublayer.partial_block_present_before,
                partial_block_present_after: sublayer.partial_block_present_after,
            }
        })
        .collect()
}

fn source_labels(
    completed_blocks_before: usize,
    sublayer: &psionic_models::AttnResSublayerSnapshot,
) -> Vec<String> {
    let source_count = sublayer.source_shape[0];
    if completed_blocks_before == 0 && source_count == 1 {
        return vec![String::from("seed")];
    }
    let mut labels = (0..completed_blocks_before.min(source_count))
        .map(|index| format!("block-{index}"))
        .collect::<Vec<_>>();
    while labels.len() < source_count {
        if sublayer.partial_block_present_before && labels.len() + 1 == source_count {
            labels.push(String::from("partial"));
        } else {
            labels.push(format!("source-{}", labels.len()));
        }
    }
    labels
}

fn aggregate_source_values(values: &[f32], source_shape: [usize; 3]) -> Vec<f32> {
    let source_count = source_shape[0];
    if source_count == 0 {
        return Vec::new();
    }
    let per_source = source_shape[1].saturating_mul(source_shape[2]).max(1);
    (0..source_count)
        .map(|source_index| {
            let start = source_index.saturating_mul(per_source);
            if start >= values.len() {
                return 0.0;
            }
            let end = (start + per_source).min(values.len());
            let slice = &values[start..end];
            slice.iter().sum::<f32>() / slice.len().max(1) as f32
        })
        .collect()
}

fn dominant_source(labels: &[String], weights: &[f32]) -> (String, f32) {
    weights
        .iter()
        .copied()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, weight)| {
            (
                labels
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| String::from("source")),
                weight,
            )
        })
        .unwrap_or_else(|| (String::from("source"), 0.0))
}

fn selectivity_from_weights(weights: &[f32]) -> f32 {
    if weights.len() <= 1 {
        return 0.0;
    }
    let sum = weights.iter().copied().sum::<f32>().max(f32::EPSILON);
    let entropy = weights.iter().fold(0.0, |acc, weight| {
        let normalized = (*weight / sum).clamp(0.0, 1.0);
        if normalized <= f32::EPSILON {
            acc
        } else {
            acc - normalized * normalized.ln()
        }
    });
    let max_entropy = (weights.len() as f32).ln().max(f32::EPSILON);
    (1.0 - (entropy / max_entropy)).clamp(0.0, 1.0)
}

fn mean_selectivity_from_diagnostics(diagnostics: &AttnResDiagnosticsSnapshot) -> f32 {
    let values = diagnostics
        .sublayers
        .iter()
        .map(|sublayer| {
            let weights = aggregate_source_values(&sublayer.routing_weights, sublayer.source_shape);
            selectivity_from_weights(weights.as_slice())
        })
        .collect::<Vec<_>>();
    mean_selectivity_values(values.as_slice())
}

fn mean_selectivity(sublayers: &[AttnResLabSublayerSnapshot]) -> f32 {
    let values = sublayers
        .iter()
        .map(|sublayer| sublayer.selectivity)
        .collect::<Vec<_>>();
    mean_selectivity_values(values.as_slice())
}

fn mean_selectivity_values(values: &[f32]) -> f32 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f32>() / values.len() as f32
    }
}

fn build_block_summaries(
    sublayers: &[AttnResLabSublayerSnapshot],
    block_count: usize,
) -> Vec<AttnResLabBlockSummary> {
    (0..block_count)
        .filter_map(|block_index| {
            let block_sublayers = sublayers
                .iter()
                .filter(|sublayer| sublayer.target_block == block_index)
                .collect::<Vec<_>>();
            if block_sublayers.is_empty() {
                return None;
            }
            Some(AttnResLabBlockSummary {
                block_index,
                avg_selectivity: block_sublayers
                    .iter()
                    .map(|sublayer| sublayer.selectivity)
                    .sum::<f32>()
                    / block_sublayers.len() as f32,
                avg_query_norm: block_sublayers
                    .iter()
                    .map(|sublayer| sublayer.query_norm)
                    .sum::<f32>()
                    / block_sublayers.len() as f32,
                sublayers: block_sublayers.len(),
            })
        })
        .collect()
}

fn route_note(
    sublayer: &psionic_models::AttnResSublayerSnapshot,
    target_block: usize,
    dominant_weight: f32,
    cache_mass: f32,
    partial_mass: f32,
    dominant_source_label: &str,
) -> String {
    let boundary_note = if sublayer.starts_new_block_before {
        format!("Boundary opened block {}. ", target_block + 1)
    } else {
        String::new()
    };
    let routing_mode = if partial_mass > cache_mass {
        "partial lane led"
    } else if cache_mass > 0.0 {
        "cache lanes led"
    } else {
        "single source routed"
    };
    format!(
        "{boundary_note}{routing_mode}; {dominant_source_label} carried {:.0}% of the averaged routing mass.",
        dominant_weight * 100.0
    )
}

fn kind_label(kind: AttnResSublayerKind) -> &'static str {
    match kind {
        AttnResSublayerKind::Attention => "Attention",
        AttnResSublayerKind::FeedForward => "MLP",
    }
}

fn parity_status_label(status: AttnResTwoPhaseParityStatus) -> &'static str {
    match status {
        AttnResTwoPhaseParityStatus::Exact => "exact",
        AttnResTwoPhaseParityStatus::WithinBudget => "within budget",
        AttnResTwoPhaseParityStatus::OutsideBudget => "outside budget",
    }
}

fn ema_loss(step_metrics: &[AttnResTinyTrainingStepMetrics], fallback: f32) -> f32 {
    let mut iter = step_metrics.iter();
    let Some(first) = iter.next() else {
        return fallback;
    };
    iter.fold(first.training_mean_loss, |ema, step| {
        (ema * 0.6) + (step.training_mean_loss * 0.4)
    })
}

fn join_or_dash(values: &[String]) -> String {
    if values.is_empty() {
        String::from("-")
    } else {
        values.join(", ")
    }
}

fn format_token_sequence(tokens: &[TokenId]) -> String {
    if tokens.is_empty() {
        return String::from("-");
    }
    tokens
        .iter()
        .map(|token| token.as_u32().to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        LIVE_SOURCE_BADGE, aggregate_source_values, apply_refresh_result, build_live_snapshot,
        kind_label, parity_status_label, selectivity_from_weights,
    };
    use crate::app_state::{AttnResLabPaneState, AttnResLabSnapshot, AttnResLabViewMode};
    use psionic_models::{
        AttnResDiagnosticsSnapshot, AttnResSublayerKind, AttnResSublayerSnapshot,
    };
    use psionic_runtime::AttnResTwoPhaseParityStatus;

    #[test]
    fn refresh_result_replaces_replay_snapshot_and_clamps_selection() {
        let mut pane_state = AttnResLabPaneState::default();
        pane_state.selected_view = AttnResLabViewMode::Inference;
        pane_state.selected_sublayer = usize::MAX;
        let snapshot = AttnResLabSnapshot {
            source_badge: LIVE_SOURCE_BADGE.to_string(),
            model_label: String::from("model"),
            architecture_label: String::from("arch"),
            run_label: String::from("run"),
            run_status: String::from("ready"),
            step: 1,
            max_steps: 1,
            speed_multiplier: 1,
            training_loss: 1.0,
            ema_loss: 1.0,
            avg_selectivity: 0.5,
            active_block: 1,
            current_block_fill: 1,
            completed_blocks: 0,
            metrics: Vec::new(),
            sublayers: vec![crate::app_state::AttnResLabSublayerSnapshot {
                sublayer_index: 0,
                label: String::from("L0 Attention"),
                kind_label: String::from("attention"),
                target_block: 0,
                dominant_source_label: String::from("seed"),
                dominant_weight: 1.0,
                selectivity: 0.0,
                query_norm: 0.0,
                partial_mass: 0.0,
                cache_mass: 1.0,
                source_labels: vec![String::from("seed")],
                source_logits: vec![0.0],
                routing_weights: vec![1.0],
                route_note: String::from("route"),
                starts_new_block_before: false,
                completed_blocks_before: 0,
                completed_blocks_after: 0,
                partial_block_present_before: false,
                partial_block_present_after: true,
            }],
            block_summaries: Vec::new(),
            inference: crate::app_state::AttnResLabInferenceSummary {
                hidden_parity_label: String::from("exact"),
                logit_parity_label: String::from("exact"),
                hidden_max_abs_diff: 0.0,
                logit_max_abs_diff: 0.0,
                partial_merge_share: 0.0,
                cache_merge_share: 1.0,
                block_cache_fill_share: 0.0,
                schedule_note: String::from("schedule"),
                merge_note: String::from("merge"),
                cache_note: String::from("cache"),
            },
            events: vec![String::from("loaded")],
        };

        apply_refresh_result(&mut pane_state, Ok(snapshot));

        assert_eq!(pane_state.snapshot.source_badge, LIVE_SOURCE_BADGE);
        assert_eq!(pane_state.selected_view, AttnResLabViewMode::Inference);
        assert_eq!(pane_state.selected_sublayer, 0);
        assert!(pane_state.last_error.is_none());
    }

    #[test]
    fn aggregated_source_values_average_each_source_plane() {
        let values = aggregate_source_values(&[0.2, 0.4, 0.6, 0.8], [2, 1, 2]);
        assert!((values[0] - 0.3).abs() < 1.0e-6);
        assert!((values[1] - 0.7).abs() < 1.0e-6);
    }

    #[test]
    fn selectivity_is_zero_for_single_source_and_positive_for_peaked_routes() {
        assert_eq!(selectivity_from_weights(&[1.0]), 0.0);
        assert!(selectivity_from_weights(&[0.95, 0.05]) > 0.5);
    }

    #[test]
    fn live_snapshot_builds_real_psionic_payload() {
        let snapshot = build_live_snapshot().expect("live snapshot should build");
        assert_eq!(snapshot.source_badge, LIVE_SOURCE_BADGE);
        assert!(!snapshot.events.is_empty());
        assert!(!snapshot.sublayers.is_empty());
        assert!(
            snapshot.inference.hidden_parity_label.contains("exact")
                || snapshot.inference.hidden_parity_label.contains("budget")
        );
    }

    #[test]
    fn helper_labels_match_runtime_contract() {
        assert_eq!(kind_label(AttnResSublayerKind::Attention), "Attention");
        assert_eq!(kind_label(AttnResSublayerKind::FeedForward), "MLP");
        assert_eq!(
            parity_status_label(AttnResTwoPhaseParityStatus::OutsideBudget),
            "outside budget"
        );
    }

    #[test]
    fn diagnostics_snapshot_shape_matches_adapter_expectations() {
        let diagnostics = AttnResDiagnosticsSnapshot {
            batch_size: 1,
            sequence_length: 2,
            hidden_size: 8,
            final_completed_blocks: 1,
            final_partial_block_present: true,
            sublayers: vec![AttnResSublayerSnapshot {
                sublayer_index: 0,
                transformer_layer_index: 0,
                kind: AttnResSublayerKind::Attention,
                starts_new_block_before: false,
                completed_blocks_before: 0,
                completed_blocks_after: 0,
                partial_block_present_before: true,
                partial_block_present_after: true,
                source_shape: [2, 1, 2],
                source_logits: vec![0.1, 0.3, 0.6, 0.9],
                routing_weights: vec![0.2, 0.4, 0.6, 0.8],
                query_norm: 0.5,
            }],
        };
        assert_eq!(diagnostics.sublayers[0].source_shape, [2, 1, 2]);
    }
}
