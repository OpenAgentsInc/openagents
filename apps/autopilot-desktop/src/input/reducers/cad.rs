use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use openagents_cad::contracts::{CadWarning, CadWarningCode, CadWarningSeverity};
use openagents_cad::eval::{EvalCacheEntry, EvalCacheKey, EvalCacheStats};
use openagents_cad::feature_graph::{FeatureGraph, FeatureNode};
use openagents_cad::history::{CadHistoryCommand, CadHistorySnapshot};
use openagents_cad::validity::{
    ModelValidityEntity, ModelValiditySnapshot, run_model_validity_checks,
};

use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, CadDemoPaneState, CadDemoWarningState,
    CadRebuildReceiptState, CadTimelineRowState, PaneLoadState, RenderState,
};
use crate::cad_rebuild_worker::{
    CadBackgroundRebuildWorker, CadRebuildCompleted, CadRebuildRequest, CadRebuildResponse,
};
use crate::pane_system::CadDemoPaneAction;

pub(super) fn run_cad_demo_action(state: &mut RenderState, action: CadDemoPaneAction) -> bool {
    let action_changed = apply_cad_demo_action(&mut state.cad_demo, action);
    let receipts = drain_worker_responses_from_pane(&mut state.cad_demo, 12);
    for receipt in &receipts {
        upsert_cad_rebuild_activity_event(state, receipt);
    }
    action_changed || !receipts.is_empty()
}

fn apply_cad_demo_action(state: &mut CadDemoPaneState, action: CadDemoPaneAction) -> bool {
    match action {
        CadDemoPaneAction::Noop => false,
        CadDemoPaneAction::CycleVariant => {
            if state.variant_ids.is_empty() {
                state.load_state = PaneLoadState::Error;
                state.last_error = Some("CAD demo has no registered variants".to_string());
                state.last_action =
                    Some("Variant cycle rejected: no variants available".to_string());
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
            if let Err(error) = enqueue_rebuild_cycle(state, "cycle-variant") {
                state.load_state = PaneLoadState::Error;
                state.last_error = Some(error);
            }
            true
        }
        CadDemoPaneAction::ResetSession => {
            let mut reset = CadDemoPaneState::default();
            reset.last_action = Some("CAD demo session reset".to_string());
            if let Err(error) = enqueue_rebuild_cycle(&mut reset, "reset-session") {
                reset.load_state = PaneLoadState::Error;
                reset.last_error = Some(error);
            }
            *state = reset;
            true
        }
        CadDemoPaneAction::CycleWarningSeverityFilter => {
            state.warning_filter_severity =
                next_warning_severity_filter(&state.warning_filter_severity);
            state.warning_filter_code = "all".to_string();
            state.warning_hover_index = None;
            state.focused_warning_index = None;
            state.last_action = Some(format!(
                "CAD warning severity filter -> {}",
                state.warning_filter_severity
            ));
            true
        }
        CadDemoPaneAction::CycleWarningCodeFilter => {
            state.warning_filter_code = next_warning_code_filter(state);
            state.warning_hover_index = None;
            state.focused_warning_index = None;
            state.last_action = Some(format!(
                "CAD warning code filter -> {}",
                state.warning_filter_code
            ));
            true
        }
        CadDemoPaneAction::SelectWarning(visible_index) => {
            if let Some(actual_index) = visible_warning_indices(state).get(visible_index).copied() {
                focus_warning(state, actual_index);
                true
            } else {
                false
            }
        }
        CadDemoPaneAction::SelectWarningMarker(marker_index) => {
            if let Some(actual_index) = visible_warning_indices(state).get(marker_index).copied() {
                focus_warning(state, actual_index);
                true
            } else {
                false
            }
        }
        CadDemoPaneAction::SelectTimelineRow(visible_index) => {
            let actual_index = state.timeline_scroll_offset.saturating_add(visible_index);
            if actual_index < state.timeline_rows.len() {
                select_timeline_row(state, actual_index);
                true
            } else {
                false
            }
        }
        CadDemoPaneAction::TimelineSelectPrev => {
            if state.timeline_rows.is_empty() {
                return false;
            }
            let current = state.timeline_selected_index.unwrap_or(0);
            let next = current.saturating_sub(1);
            select_timeline_row(state, next);
            true
        }
        CadDemoPaneAction::TimelineSelectNext => {
            if state.timeline_rows.is_empty() {
                return false;
            }
            let current = state.timeline_selected_index.unwrap_or(0);
            let next = (current + 1).min(state.timeline_rows.len().saturating_sub(1));
            select_timeline_row(state, next);
            true
        }
    }
}

fn ensure_worker(state: &mut CadDemoPaneState) -> &CadBackgroundRebuildWorker {
    state
        .rebuild_worker
        .get_or_insert_with(CadBackgroundRebuildWorker::spawn)
}

fn enqueue_rebuild_cycle(state: &mut CadDemoPaneState, trigger: &str) -> Result<(), String> {
    let request_id = state.next_rebuild_request_id;
    state.next_rebuild_request_id = state.next_rebuild_request_id.saturating_add(1);
    let request = CadRebuildRequest {
        request_id,
        trigger: trigger.to_string(),
        session_id: state.session_id.clone(),
        document_revision: state.document_revision,
        variant_id: state.active_variant_id.clone(),
        graph: build_demo_feature_graph(state),
    };
    ensure_worker(state).enqueue(request)?;
    state.pending_rebuild_request_id = Some(request_id);
    state.load_state = PaneLoadState::Loading;
    state.last_error = None;
    state.last_action = Some(format!(
        "CAD rebuild {} queued (request #{request_id}); rendering last-good mesh",
        trigger
    ));
    Ok(())
}

fn drain_worker_responses_from_pane(
    state: &mut CadDemoPaneState,
    max_items: usize,
) -> Vec<CadRebuildReceiptState> {
    let responses = state
        .rebuild_worker
        .as_ref()
        .map(|worker| worker.drain_ready(max_items))
        .unwrap_or_default();
    if responses.is_empty() {
        return Vec::new();
    }

    let mut emitted = Vec::new();
    for response in responses {
        match response {
            CadRebuildResponse::Completed(completed) => {
                if completed.document_revision < state.document_revision {
                    // Keep last-good mesh steady; skip stale rebuild result.
                    continue;
                }
                match apply_completed_rebuild(state, completed) {
                    Ok(receipt) => emitted.push(receipt),
                    Err(error) => {
                        state.load_state = PaneLoadState::Error;
                        state.last_error = Some(error.clone());
                        state.last_action = Some(format!("CAD rebuild commit failed: {error}"));
                    }
                }
            }
            CadRebuildResponse::Failed(failed) => {
                if state.pending_rebuild_request_id == Some(failed.request_id) {
                    state.pending_rebuild_request_id = None;
                }
                state.load_state = PaneLoadState::Error;
                state.last_error = Some(failed.error.clone());
                state.last_action = Some(format!(
                    "CAD rebuild {} failed for request #{}",
                    failed.trigger, failed.request_id
                ));
            }
        }
    }
    emitted
}

fn apply_completed_rebuild(
    state: &mut CadDemoPaneState,
    completed: CadRebuildCompleted,
) -> Result<CadRebuildReceiptState, String> {
    let before_snapshot = history_snapshot_from_state(state);
    let before_stats = state.eval_cache.stats();
    let node_by_id = completed
        .graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();

    for feature_id in &completed.result.ordered_feature_ids {
        let Some(node) = node_by_id.get(feature_id.as_str()) else {
            return Err(format!(
                "rebuild commit could not resolve node during cache stage: {}",
                feature_id
            ));
        };
        let key = EvalCacheKey::from_feature_node(completed.document_revision, node);
        if state.eval_cache.get(&key).is_none() {
            let Some(hash) = completed.result.feature_hashes.get(feature_id).cloned() else {
                return Err(format!(
                    "rebuild commit could not resolve feature hash during cache stage: {}",
                    feature_id
                ));
            };
            state.eval_cache.insert(
                key,
                EvalCacheEntry {
                    geometry_hash: hash,
                },
            );
        }
    }

    let after_stats = state.eval_cache.stats();
    let stats_delta = stats_delta(before_stats, after_stats);
    let duration_ms = synthetic_duration_ms(completed.result.records.len(), stats_delta);
    let event_id = format!(
        "{}:{}:{}",
        completed.session_id,
        completed.document_revision,
        state.rebuild_receipts.len().saturating_add(1)
    );
    let receipt = CadRebuildReceiptState {
        event_id,
        document_revision: completed.document_revision,
        variant_id: completed.variant_id,
        rebuild_hash: completed.result.rebuild_hash.clone(),
        mesh_hash: completed.tessellation_receipt.mesh_hash.clone(),
        duration_ms,
        cache_hits: stats_delta.hits,
        cache_misses: stats_delta.misses,
        cache_evictions: stats_delta.evictions,
        feature_count: completed.result.records.len(),
        vertex_count: completed.tessellation_receipt.vertex_count,
        triangle_count: completed.tessellation_receipt.triangle_count,
        edge_count: completed.tessellation_receipt.edge_count,
    };

    state.last_rebuild_receipt = Some(receipt.clone());
    state.rebuild_receipts.push(receipt.clone());
    if state.rebuild_receipts.len() > 32 {
        let overflow = state.rebuild_receipts.len().saturating_sub(32);
        state.rebuild_receipts.drain(0..overflow);
    }
    state.pending_rebuild_request_id = None;
    state.last_good_mesh_payload = Some(completed.mesh_payload.clone());
    state.last_good_mesh_id = Some(completed.mesh_payload.mesh_id.clone());
    refresh_warning_state(state, completed.document_revision, &receipt.variant_id);
    refresh_timeline_state(
        state,
        &completed.graph,
        provenance_from_trigger(&completed.trigger),
    );
    let after_snapshot = history_snapshot_from_state(state);
    state.history_stack.push_transition(
        CadHistoryCommand::ApplyIntent {
            intent_key: completed.trigger.clone(),
            summary: format!("cad rebuild {}", completed.trigger),
        },
        before_snapshot,
        after_snapshot,
    );
    state.load_state = PaneLoadState::Ready;
    state.last_error = None;
    state.last_action = Some(format!(
        "CAD rebuild {} committed: {}ms hash={} mesh={}",
        completed.trigger, duration_ms, receipt.rebuild_hash, receipt.mesh_hash
    ));

    Ok(receipt)
}

fn upsert_cad_rebuild_activity_event(state: &mut RenderState, receipt: &CadRebuildReceiptState) {
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
            "variant={} hash={} mesh={} features={} tris={} verts={} cache(h={},m={},e={})",
            receipt.variant_id,
            receipt.rebuild_hash,
            receipt.mesh_hash,
            receipt.feature_count,
            receipt.triangle_count,
            receipt.vertex_count,
            receipt.cache_hits,
            receipt.cache_misses,
            receipt.cache_evictions
        ),
    });
    state.activity_feed.load_state = PaneLoadState::Ready;
    state.activity_feed.last_action = Some(format!(
        "CAD rebuild receipt captured ({})",
        receipt.mesh_hash
    ));
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
    let (width_param, vent_spacing_param, vent_count_param) = match state.active_variant_id.as_str()
    {
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
                (
                    "radius_param".to_string(),
                    "mount_hole_radius_mm".to_string(),
                ),
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

    FeatureGraph {
        nodes: feature_nodes,
    }
}

fn refresh_warning_state(state: &mut CadDemoPaneState, document_revision: u64, variant_id: &str) {
    let snapshot = build_demo_validity_snapshot(document_revision, variant_id);
    let receipt = run_model_validity_checks(&snapshot);
    state.warnings = receipt
        .warnings
        .iter()
        .enumerate()
        .map(|(index, warning)| warning_to_pane_state(index, warning))
        .collect();
    state.warning_hover_index = None;
    state.focused_warning_index = None;
    state.focused_geometry_ref = None;
}

fn refresh_timeline_state(state: &mut CadDemoPaneState, graph: &FeatureGraph, provenance: String) {
    let prior_selected_feature_id = state.timeline_selected_index.and_then(|index| {
        state
            .timeline_rows
            .get(index)
            .map(|row| row.feature_id.clone())
    });

    let ordered_ids = graph
        .deterministic_topo_order()
        .unwrap_or_else(|_| graph.nodes.iter().map(|node| node.id.clone()).collect());
    let node_by_id = graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let warnings_by_feature = state.warnings.iter().fold(
        BTreeMap::<String, Vec<&CadDemoWarningState>>::new(),
        |mut map, warning| {
            map.entry(warning.feature_id.clone())
                .or_default()
                .push(warning);
            map
        },
    );

    state.timeline_rows = ordered_ids
        .iter()
        .filter_map(|feature_id| node_by_id.get(feature_id.as_str()))
        .map(|node| CadTimelineRowState {
            feature_id: node.id.clone(),
            feature_name: node.name.clone(),
            op_type: node.operation_key.clone(),
            status_badge: timeline_status_badge(node.id.as_str(), &warnings_by_feature),
            provenance: provenance.clone(),
            params: node
                .params
                .iter()
                .map(|(name, value)| (name.clone(), value.clone()))
                .collect(),
        })
        .collect();

    let selected_index = prior_selected_feature_id
        .as_ref()
        .and_then(|feature_id| {
            state
                .timeline_rows
                .iter()
                .position(|row| &row.feature_id == feature_id)
        })
        .or_else(|| (!state.timeline_rows.is_empty()).then_some(0));
    state.timeline_selected_index = selected_index;
    if let Some(index) = selected_index {
        state.timeline_scroll_offset = auto_scroll_offset(index, state.timeline_scroll_offset, 10);
        state.selected_feature_params = state.timeline_rows[index].params.clone();
        state.focused_geometry_ref = Some(format!(
            "cad://feature/{}",
            state.timeline_rows[index].feature_id
        ));
    } else {
        state.timeline_scroll_offset = 0;
        state.selected_feature_params.clear();
    }
}

fn timeline_status_badge(
    feature_id: &str,
    warnings_by_feature: &BTreeMap<String, Vec<&CadDemoWarningState>>,
) -> String {
    let Some(warnings) = warnings_by_feature.get(feature_id) else {
        return "ok".to_string();
    };
    if warnings
        .iter()
        .any(|warning| warning.severity.eq_ignore_ascii_case("critical"))
    {
        return "fail".to_string();
    }
    if warnings
        .iter()
        .any(|warning| warning.severity.eq_ignore_ascii_case("warning"))
    {
        return "warn".to_string();
    }
    "ok".to_string()
}

fn auto_scroll_offset(selected_index: usize, current_offset: usize, visible_rows: usize) -> usize {
    if selected_index < current_offset {
        return selected_index;
    }
    let max_visible_index = current_offset.saturating_add(visible_rows.saturating_sub(1));
    if selected_index > max_visible_index {
        return selected_index.saturating_sub(visible_rows.saturating_sub(1));
    }
    current_offset
}

fn provenance_from_trigger(trigger: &str) -> String {
    if trigger.contains("ai") {
        "ai".to_string()
    } else {
        "manual".to_string()
    }
}

fn history_snapshot_from_state(state: &CadDemoPaneState) -> CadHistorySnapshot {
    let warnings = state
        .warnings
        .iter()
        .map(cad_warning_from_pane_warning)
        .collect::<Vec<_>>();
    let stable_ids = state
        .timeline_rows
        .iter()
        .map(|row| (row.feature_id.clone(), row.feature_id.clone()))
        .collect::<BTreeMap<_, _>>();
    CadHistorySnapshot {
        document_revision: state.document_revision,
        geometry_hash: state
            .last_rebuild_receipt
            .as_ref()
            .map(|receipt| receipt.rebuild_hash.clone())
            .unwrap_or_else(|| "mesh.none".to_string()),
        stable_ids,
        warnings,
        analysis: openagents_cad::contracts::CadAnalysis {
            document_revision: state.document_revision,
            variant_id: state.active_variant_id.clone(),
            material_id: None,
            volume_mm3: None,
            mass_kg: None,
            center_of_gravity_mm: None,
            estimated_cost_usd: None,
            max_deflection_mm: None,
            objective_scores: BTreeMap::new(),
        },
    }
}

fn cad_warning_from_pane_warning(warning: &CadDemoWarningState) -> CadWarning {
    let code = match warning.code.as_str() {
        "CAD-WARN-NON-MANIFOLD" => CadWarningCode::NonManifoldBody,
        "CAD-WARN-SELF-INTERSECTION" => CadWarningCode::SelfIntersection,
        "CAD-WARN-ZERO-THICKNESS" => CadWarningCode::ZeroThicknessFace,
        "CAD-WARN-SLIVER-FACE" => CadWarningCode::SliverFace,
        "CAD-WARN-FILLET-FAILED" => CadWarningCode::FilletFailed,
        "CAD-WARN-SEMANTIC-REF-EXPIRED" => CadWarningCode::SemanticRefExpired,
        other => CadWarningCode::Unknown(other.to_string()),
    };
    let severity = if warning.severity.eq_ignore_ascii_case("critical") {
        CadWarningSeverity::Critical
    } else if warning.severity.eq_ignore_ascii_case("warning") {
        CadWarningSeverity::Warning
    } else {
        CadWarningSeverity::Info
    };
    CadWarning {
        code,
        severity,
        message: warning.message.clone(),
        remediation_hint: warning.remediation_hint.clone(),
        semantic_refs: warning.semantic_refs.clone(),
        metadata: BTreeMap::from([
            ("feature_id".to_string(), warning.feature_id.clone()),
            ("entity_id".to_string(), warning.entity_id.clone()),
            (
                "deep_link".to_string(),
                warning
                    .deep_link
                    .clone()
                    .unwrap_or_else(|| format!("cad://feature/{}", warning.feature_id)),
            ),
        ]),
    }
}

fn warning_to_pane_state(index: usize, warning: &CadWarning) -> CadDemoWarningState {
    let feature_id = warning
        .metadata
        .get("feature_id")
        .cloned()
        .unwrap_or_else(|| "feature.unknown".to_string());
    let entity_id = warning
        .metadata
        .get("entity_id")
        .cloned()
        .unwrap_or_else(|| "entity.unknown".to_string());
    let deep_link = warning.metadata.get("deep_link").cloned();
    CadDemoWarningState {
        warning_id: format!("warning.{index:03}"),
        code: warning.code.stable_code().to_string(),
        severity: match warning.severity {
            CadWarningSeverity::Info => "info".to_string(),
            CadWarningSeverity::Warning => "warning".to_string(),
            CadWarningSeverity::Critical => "critical".to_string(),
        },
        message: warning.message.clone(),
        remediation_hint: warning.remediation_hint.clone(),
        semantic_refs: warning.semantic_refs.clone(),
        deep_link,
        feature_id,
        entity_id,
    }
}

fn build_demo_validity_snapshot(document_revision: u64, variant_id: &str) -> ModelValiditySnapshot {
    let entities = match variant_id {
        "variant.lightweight" => vec![
            ModelValidityEntity {
                entity_id: "face.zero_thickness".to_string(),
                feature_id: "feature.base".to_string(),
                semantic_ref: Some("rack_outer_face".to_string()),
                is_manifold: true,
                self_intersection_count: 0,
                min_thickness_mm: 0.005,
                min_face_area_mm2: 22.0,
                sliver_face_count: 0,
                fillet_failure_reason: None,
            },
            ModelValidityEntity {
                entity_id: "edge.fillet_fail".to_string(),
                feature_id: "feature.edge_marker".to_string(),
                semantic_ref: Some("edge_blend_set".to_string()),
                is_manifold: true,
                self_intersection_count: 0,
                min_thickness_mm: 1.4,
                min_face_area_mm2: 12.0,
                sliver_face_count: 0,
                fillet_failure_reason: Some("radius too large".to_string()),
            },
        ],
        "variant.low-cost" => vec![ModelValidityEntity {
            entity_id: "face.sliver".to_string(),
            feature_id: "feature.vent_pattern".to_string(),
            semantic_ref: Some("vent_sliver_face".to_string()),
            is_manifold: true,
            self_intersection_count: 0,
            min_thickness_mm: 1.6,
            min_face_area_mm2: 0.00005,
            sliver_face_count: 2,
            fillet_failure_reason: None,
        }],
        "variant.stiffness" => vec![ModelValidityEntity {
            entity_id: "body.self_intersect".to_string(),
            feature_id: "feature.vent_pattern".to_string(),
            semantic_ref: Some("vent_face_set".to_string()),
            is_manifold: true,
            self_intersection_count: 1,
            min_thickness_mm: 1.8,
            min_face_area_mm2: 20.0,
            sliver_face_count: 0,
            fillet_failure_reason: None,
        }],
        _ => vec![ModelValidityEntity {
            entity_id: "body.non_manifold".to_string(),
            feature_id: "feature.base".to_string(),
            semantic_ref: Some("rack_outer_face".to_string()),
            is_manifold: false,
            self_intersection_count: 0,
            min_thickness_mm: 2.0,
            min_face_area_mm2: 40.0,
            sliver_face_count: 0,
            fillet_failure_reason: None,
        }],
    };

    ModelValiditySnapshot {
        document_revision,
        variant_id: variant_id.to_string(),
        tolerance_mm: 0.01,
        entities,
    }
}

fn next_warning_severity_filter(current: &str) -> String {
    match current {
        "all" => "critical".to_string(),
        "critical" => "warning".to_string(),
        "warning" => "info".to_string(),
        _ => "all".to_string(),
    }
}

fn next_warning_code_filter(state: &CadDemoPaneState) -> String {
    let mut codes = state
        .warnings
        .iter()
        .map(|warning| warning.code.clone())
        .collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    let mut options = vec!["all".to_string()];
    options.extend(codes);
    let position = options
        .iter()
        .position(|value| value.eq_ignore_ascii_case(&state.warning_filter_code))
        .unwrap_or(0);
    let next = (position + 1) % options.len();
    options[next].clone()
}

fn warning_visible(state: &CadDemoPaneState, warning: &CadDemoWarningState) -> bool {
    let severity_ok = state.warning_filter_severity == "all"
        || warning
            .severity
            .eq_ignore_ascii_case(&state.warning_filter_severity);
    let code_ok = state.warning_filter_code == "all"
        || warning
            .code
            .eq_ignore_ascii_case(&state.warning_filter_code);
    severity_ok && code_ok
}

fn visible_warning_indices(state: &CadDemoPaneState) -> Vec<usize> {
    state
        .warnings
        .iter()
        .enumerate()
        .filter(|(_, warning)| warning_visible(state, warning))
        .map(|(index, _)| index)
        .collect()
}

fn focus_warning(state: &mut CadDemoPaneState, warning_index: usize) {
    let warning = &state.warnings[warning_index];
    state.warning_hover_index = Some(warning_index);
    state.focused_warning_index = Some(warning_index);
    let fallback = format!("cad://feature/{}", warning.feature_id);
    state.focused_geometry_ref = warning.deep_link.clone().or(Some(fallback));
    state.last_action = Some(format!(
        "CAD warning focus -> {} ({})",
        warning.code, warning.entity_id
    ));
}

fn select_timeline_row(state: &mut CadDemoPaneState, index: usize) {
    if index >= state.timeline_rows.len() {
        return;
    }
    state.timeline_selected_index = Some(index);
    state.timeline_scroll_offset = auto_scroll_offset(index, state.timeline_scroll_offset, 10);
    state.selected_feature_params = state.timeline_rows[index].params.clone();
    state.focused_geometry_ref = Some(format!(
        "cad://feature/{}",
        state.timeline_rows[index].feature_id
    ));
    state.last_action = Some(format!(
        "CAD timeline selected -> {}",
        state.timeline_rows[index].feature_name
    ));
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{apply_cad_demo_action, drain_worker_responses_from_pane};
    use crate::app_state::{CadDemoPaneState, CadTimelineRowState};
    use crate::pane_system::CadDemoPaneAction;

    fn wait_for_receipt(state: &mut CadDemoPaneState) {
        for _ in 0..64 {
            let receipts = drain_worker_responses_from_pane(state, 8);
            if !receipts.is_empty() {
                return;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("timed out waiting for background CAD rebuild receipt");
    }

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
    fn cycle_variant_queues_background_rebuild() {
        let mut state = CadDemoPaneState::default();
        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        assert!(changed);
        assert_eq!(state.document_revision, 1);
        assert_eq!(state.active_variant_id, "variant.lightweight");
        assert_eq!(state.load_state, crate::app_state::PaneLoadState::Loading);
        assert!(state.pending_rebuild_request_id.is_some());
        assert!(state.last_rebuild_receipt.is_none());
    }

    #[test]
    fn background_rebuild_commits_receipt_with_cache_stats() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);
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
        assert!(state.pending_rebuild_request_id.is_none());
        assert!(state.last_good_mesh_id.is_some());
        assert!(
            !state.warnings.is_empty(),
            "warnings should refresh on rebuild commit"
        );
    }

    #[test]
    fn last_good_mesh_is_preserved_while_next_rebuild_is_pending() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);
        let baseline_mesh = state
            .last_good_mesh_id
            .clone()
            .expect("first mesh should commit");

        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        assert_eq!(state.load_state, crate::app_state::PaneLoadState::Loading);
        assert_eq!(
            state.last_good_mesh_id.as_deref(),
            Some(baseline_mesh.as_str())
        );
    }

    #[test]
    fn warning_filters_and_focus_actions_work_with_fallback_geometry_focus() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);
        assert!(!state.warnings.is_empty());

        let changed =
            apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleWarningSeverityFilter);
        assert!(changed);
        assert_eq!(state.warning_filter_severity, "critical");

        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleWarningCodeFilter);
        assert!(changed);
        assert_ne!(state.warning_filter_code, "");

        // Return to all-severity view before selecting a warning row.
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleWarningSeverityFilter);
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleWarningSeverityFilter);
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleWarningSeverityFilter);

        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::SelectWarning(0));
        assert!(changed);
        assert!(state.focused_warning_index.is_some());
        assert!(state.focused_geometry_ref.is_some());
    }

    #[test]
    fn stale_warning_markers_are_cleared_after_new_rebuild_commit() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::SelectWarning(0));
        assert!(state.focused_warning_index.is_some());
        assert!(state.warning_hover_index.is_some());

        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);
        assert!(state.warning_hover_index.is_none());
        assert!(state.focused_warning_index.is_none());
        assert!(
            state.focused_geometry_ref.is_some(),
            "timeline selection may keep feature focus after rebuild"
        );
    }

    #[test]
    fn selecting_timeline_row_highlights_corresponding_feature() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);
        assert!(!state.timeline_rows.is_empty());

        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::SelectTimelineRow(0));
        assert!(changed);
        assert_eq!(state.timeline_selected_index, Some(0));
        let selected = state.timeline_rows[0].feature_id.clone();
        assert_eq!(
            state.focused_geometry_ref.as_deref(),
            Some(format!("cad://feature/{selected}").as_str())
        );
        assert_eq!(state.selected_feature_params, state.timeline_rows[0].params);
    }

    #[test]
    fn timeline_keyboard_navigation_auto_scrolls_for_long_lists() {
        let mut state = CadDemoPaneState::default();
        state.timeline_rows = (0..24)
            .map(|index| CadTimelineRowState {
                feature_id: format!("feature.{index:03}"),
                feature_name: format!("Feature {index:03}"),
                op_type: "primitive.box.v1".to_string(),
                status_badge: "ok".to_string(),
                provenance: "manual".to_string(),
                params: vec![("width".to_string(), index.to_string())],
            })
            .collect();
        state.timeline_selected_index = Some(0);
        for _ in 0..14 {
            let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::TimelineSelectNext);
            assert!(changed);
        }
        assert_eq!(state.timeline_selected_index, Some(14));
        assert!(state.timeline_scroll_offset > 0);

        let changed = apply_cad_demo_action(&mut state, CadDemoPaneAction::TimelineSelectPrev);
        assert!(changed);
        assert_eq!(state.timeline_selected_index, Some(13));
    }
}
