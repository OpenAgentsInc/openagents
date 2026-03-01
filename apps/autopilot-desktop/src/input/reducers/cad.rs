use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use openagents_cad::eval::{
    EvalCacheEntry, EvalCacheKey, EvalCacheStats, evaluate_feature_graph_deterministic,
};
use openagents_cad::feature_graph::{FeatureGraph, FeatureNode};

use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, CadDemoPaneState, CadRebuildReceiptState,
    PaneLoadState, RenderState,
};
use crate::pane_system::CadDemoPaneAction;

pub(super) fn run_cad_demo_action(state: &mut RenderState, action: CadDemoPaneAction) -> bool {
    let changed = apply_cad_demo_action(&mut state.cad_demo, action);
    if changed
        && let Some(receipt) = state.cad_demo.last_rebuild_receipt.clone()
    {
        upsert_cad_rebuild_activity_event(state, receipt);
    }
    changed
}

fn apply_cad_demo_action(state: &mut CadDemoPaneState, action: CadDemoPaneAction) -> bool {
    match action {
        CadDemoPaneAction::Noop => false,
        CadDemoPaneAction::CycleVariant => {
            if state.variant_ids.is_empty() {
                state.load_state = PaneLoadState::Error;
                state.last_error = Some("CAD demo has no registered variants".to_string());
                state.last_action = Some("Variant cycle rejected: no variants available".to_string());
                return true;
            }

            let current_index = state
                .variant_ids
                .iter()
                .position(|variant| variant == &state.active_variant_id)
                .unwrap_or(0);
            let next_index = (current_index + 1) % state.variant_ids.len();
            state.active_variant_id = state.variant_ids[next_index].clone();
            state.document_revision = state.document_revision.saturating_add(1);
            if let Err(error) = run_rebuild_cycle(state, "cycle-variant") {
                state.load_state = PaneLoadState::Error;
                state.last_error = Some(error);
            }
            true
        }
        CadDemoPaneAction::ResetSession => {
            let mut reset = CadDemoPaneState::default();
            reset.last_action = Some("CAD demo session reset".to_string());
            if let Err(error) = run_rebuild_cycle(&mut reset, "reset-session") {
                reset.load_state = PaneLoadState::Error;
                reset.last_error = Some(error);
            }
            *state = reset;
            true
        }
    }
}

fn upsert_cad_rebuild_activity_event(state: &mut RenderState, receipt: CadRebuildReceiptState) {
    let occurred_at_epoch_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    state.activity_feed.upsert_event(ActivityEventRow {
        event_id: format!("cad.rebuild.{}", receipt.event_id),
        domain: ActivityEventDomain::Sync,
        source_tag: "cad.eval".to_string(),
        occurred_at_epoch_seconds,
        summary: format!(
            "CAD rebuild rev={} {}ms",
            receipt.document_revision, receipt.duration_ms
        ),
        detail: format!(
            "variant={} hash={} features={} cache(h={},m={},e={})",
            receipt.variant_id,
            receipt.rebuild_hash,
            receipt.feature_count,
            receipt.cache_hits,
            receipt.cache_misses,
            receipt.cache_evictions
        ),
    });
    state.activity_feed.load_state = PaneLoadState::Ready;
    state.activity_feed.last_action = Some(format!(
        "CAD rebuild receipt captured ({})",
        receipt.rebuild_hash
    ));
}

fn run_rebuild_cycle(state: &mut CadDemoPaneState, trigger: &str) -> Result<(), String> {
    let graph = build_demo_feature_graph(state);
    let result = evaluate_feature_graph_deterministic(&graph).map_err(|error| error.to_string())?;
    let before_stats = state.eval_cache.stats();

    let node_by_id = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    for feature_id in &result.ordered_feature_ids {
        let Some(node) = node_by_id.get(feature_id.as_str()) else {
            return Err(format!(
                "rebuild could not resolve node during cache stage: {}",
                feature_id
            ));
        };
        let key = EvalCacheKey::from_feature_node(state.document_revision, node);
        if state.eval_cache.get(&key).is_none() {
            let Some(hash) = result.feature_hashes.get(feature_id).cloned() else {
                return Err(format!(
                    "rebuild could not resolve feature hash during cache stage: {}",
                    feature_id
                ));
            };
            state
                .eval_cache
                .insert(key, EvalCacheEntry { geometry_hash: hash });
        }
    }
    let after_stats = state.eval_cache.stats();
    let stats_delta = stats_delta(before_stats, after_stats);
    let duration_ms = synthetic_duration_ms(result.records.len(), stats_delta);
    let event_id = format!(
        "{}:{}:{}",
        state.session_id,
        state.document_revision,
        state.rebuild_receipts.len().saturating_add(1)
    );
    let receipt = CadRebuildReceiptState {
        event_id,
        document_revision: state.document_revision,
        variant_id: state.active_variant_id.clone(),
        rebuild_hash: result.rebuild_hash.clone(),
        duration_ms,
        cache_hits: stats_delta.hits,
        cache_misses: stats_delta.misses,
        cache_evictions: stats_delta.evictions,
        feature_count: result.records.len(),
    };
    state.last_rebuild_receipt = Some(receipt.clone());
    state.rebuild_receipts.push(receipt);
    if state.rebuild_receipts.len() > 32 {
        let overflow = state.rebuild_receipts.len().saturating_sub(32);
        state.rebuild_receipts.drain(0..overflow);
    }
    state.load_state = PaneLoadState::Ready;
    state.last_error = None;
    state.last_action = Some(format!(
        "CAD rebuild {} completed: {}ms hash={}",
        trigger, duration_ms, result.rebuild_hash
    ));
    Ok(())
}

fn synthetic_duration_ms(feature_count: usize, stats_delta: EvalCacheStats) -> u64 {
    let base = 2_u64;
    let feature_cost = (feature_count as u64).saturating_mul(3);
    let cache_cost = stats_delta
        .misses
        .saturating_add(stats_delta.evictions)
        .saturating_add(stats_delta.hits / 2);
    base.saturating_add(feature_cost).saturating_add(cache_cost)
}

fn stats_delta(before: EvalCacheStats, after: EvalCacheStats) -> EvalCacheStats {
    EvalCacheStats {
        hits: after.hits.saturating_sub(before.hits),
        misses: after.misses.saturating_sub(before.misses),
        evictions: after.evictions.saturating_sub(before.evictions),
    }
}

fn build_demo_feature_graph(state: &CadDemoPaneState) -> FeatureGraph {
    let (width_param, vent_spacing_param, vent_count_param) = match state.active_variant_id.as_str() {
        "variant.lightweight" => ("width_light_mm", "vent_spacing_wide_mm", "vent_count_low"),
        "variant.low-cost" => ("width_cost_mm", "vent_spacing_cost_mm", "vent_count_mid"),
        "variant.stiffness" => ("width_stiff_mm", "vent_spacing_tight_mm", "vent_count_high"),
        _ => ("width_base_mm", "vent_spacing_base_mm", "vent_count_base"),
    };

    let feature_nodes = vec![
        FeatureNode {
            id: "feature.base".to_string(),
            name: "base".to_string(),
            operation_key: "primitive.box.v1".to_string(),
            depends_on: Vec::new(),
            params: BTreeMap::from([
                ("width_param".to_string(), width_param.to_string()),
                ("depth_param".to_string(), "depth_mm".to_string()),
                ("height_param".to_string(), "height_mm".to_string()),
                ("variant".to_string(), state.active_variant_id.clone()),
            ]),
        },
        FeatureNode {
            id: "feature.mount_hole".to_string(),
            name: "mount_hole".to_string(),
            operation_key: "cut.hole.v1".to_string(),
            depends_on: vec!["feature.base".to_string()],
            params: BTreeMap::from([
                ("radius_param".to_string(), "mount_hole_radius_mm".to_string()),
                ("depth_param".to_string(), "mount_hole_depth_mm".to_string()),
            ]),
        },
        FeatureNode {
            id: "feature.vent_pattern".to_string(),
            name: "vent_pattern".to_string(),
            operation_key: "linear.pattern.v1".to_string(),
            depends_on: vec!["feature.mount_hole".to_string()],
            params: BTreeMap::from([
                ("count_param".to_string(), vent_count_param.to_string()),
                ("spacing_param".to_string(), vent_spacing_param.to_string()),
            ]),
        },
        FeatureNode {
            id: "feature.edge_marker".to_string(),
            name: "edge_marker".to_string(),
            operation_key: "fillet.placeholder.v1".to_string(),
            depends_on: vec!["feature.base".to_string()],
            params: BTreeMap::from([
                ("radius_param".to_string(), "edge_radius_mm".to_string()),
                ("kind".to_string(), "fillet".to_string()),
            ]),
        },
    ];

    FeatureGraph { nodes: feature_nodes }
}

#[cfg(test)]
mod tests {
    use super::{apply_cad_demo_action, run_rebuild_cycle};
    use crate::app_state::CadDemoPaneState;
    use crate::pane_system::CadDemoPaneAction;

    #[test]
    fn noop_action_is_stable_no_op() {
        let mut state = CadDemoPaneState::default();
        let baseline = state.document_revision;
        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::Noop);
        assert!(!changed);
        assert_eq!(state.document_revision, baseline);
        assert_eq!(state.active_variant_id, "variant.baseline");
    }

    #[test]
    fn cycle_variant_updates_revision_and_selection() {
        let mut state = CadDemoPaneState::default();
        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        assert!(changed);
        assert_eq!(state.document_revision, 1);
        assert_eq!(state.active_variant_id, "variant.lightweight");
        assert!(state.last_rebuild_receipt.is_some());
        assert_eq!(state.rebuild_receipts.len(), 1);
    }

    #[test]
    fn reset_restores_default_session_state() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::ResetSession);
        assert!(changed);
        assert_eq!(state.document_revision, 0);
        assert_eq!(state.active_variant_id, "variant.baseline");
        assert_eq!(state.session_id, "cad.session.local");
        assert!(state.last_action.is_some());
        assert_eq!(state.rebuild_receipts.len(), 1);
    }

    #[test]
    fn rebuild_cycle_produces_receipt_with_cache_stats() {
        let mut state = CadDemoPaneState::default();
        let result = run_rebuild_cycle(&mut state, "test");
        assert!(result.is_ok());
        let receipt = state
            .last_rebuild_receipt
            .as_ref()
            .expect("receipt should be recorded");
        assert!(receipt.duration_ms > 0);
        assert_eq!(receipt.feature_count, 4);
        assert!(
            receipt.cache_hits + receipt.cache_misses >= 4,
            "cache stats should capture feature lookups"
        );
    }
}
