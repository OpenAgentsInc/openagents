use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use openagents_cad::analysis::{
    CadBodyAnalysisError, CadDeflectionHeuristicError, CadDeflectionHeuristicInput,
    analyze_body_properties, estimate_beam_deflection_heuristic,
};
use openagents_cad::chat_adapter::{CadIntentTranslationOutcome, translate_chat_to_cad_intent};
use openagents_cad::contracts::{CadWarning, CadWarningCode, CadWarningSeverity};
use openagents_cad::eval::{EvalCacheEntry, EvalCacheKey, EvalCacheStats};
use openagents_cad::events::{CadEvent, CadEventKind};
use openagents_cad::feature_graph::{FeatureGraph, FeatureNode};
use openagents_cad::history::{CadHistoryCommand, CadHistorySnapshot};
use openagents_cad::intent::CadIntent;
use openagents_cad::materials::{
    CadCostHeuristicError, CadCostHeuristicInput, DEFAULT_CAD_MATERIAL_ID,
    estimate_cnc_cost_heuristic_usd, material_preset_by_id,
};
use openagents_cad::validity::{
    ModelValidityEntity, ModelValiditySnapshot, run_model_validity_checks,
};

use crate::app_state::{
    ActivityEventDomain, ActivityEventRow, CadCameraViewSnap, CadDemoPaneState,
    CadDemoWarningState, CadRebuildReceiptState, CadSnapMode, CadThreeDMouseAxis,
    CadTimelineRowState, PaneLoadState, RenderState,
};
use crate::cad_rebuild_worker::{
    CadBackgroundRebuildWorker, CadRebuildCompleted, CadRebuildRequest, CadRebuildResponse,
};
use crate::pane_system::CadDemoPaneAction;

pub(super) fn apply_chat_prompt_to_cad_session(
    state: &mut RenderState,
    thread_id: &str,
    prompt: &str,
) -> bool {
    match translate_chat_to_cad_intent(prompt) {
        CadIntentTranslationOutcome::Intent(intent) => {
            let intent_name = intent.intent_name().to_string();
            match state
                .cad_demo
                .apply_chat_intent_for_thread(thread_id, &intent)
            {
                Ok(receipt) => match &intent {
                    CadIntent::Export(export_intent) => {
                        match run_step_export_from_active_mesh(
                            &state.cad_demo,
                            &export_intent.variant_id,
                        ) {
                            Ok(artifact) => {
                                state.cad_demo.last_error = None;
                                state.cad_demo.last_action = Some(format!(
                                    "CAD STEP export ready -> {} ({} bytes, hash {})",
                                    artifact.receipt.file_name,
                                    artifact.receipt.byte_count,
                                    artifact.receipt.deterministic_hash
                                ));
                                emit_cad_event(
                                    state,
                                    CadEventKind::ExportCompleted,
                                    receipt.state_revision,
                                    Some(export_intent.variant_id.clone()),
                                    Some(format!(
                                        "chat-export:{}:{}:{}",
                                        thread_id,
                                        artifact.receipt.file_name,
                                        artifact.receipt.deterministic_hash
                                    )),
                                    "CAD STEP export completed".to_string(),
                                    format!(
                                        "thread={} session={} variant={} file={} bytes={} hash={}",
                                        thread_id,
                                        state.cad_demo.session_id,
                                        export_intent.variant_id,
                                        artifact.receipt.file_name,
                                        artifact.receipt.byte_count,
                                        artifact.receipt.deterministic_hash
                                    ),
                                );
                            }
                            Err(error) => {
                                state.cad_demo.last_error =
                                    Some(format!("CAD STEP export failed: {error}"));
                                state.cad_demo.last_action = Some(
                                    "CAD STEP export rejected: inspect error and retry".to_string(),
                                );
                                emit_cad_event(
                                    state,
                                    CadEventKind::ExportFailed,
                                    receipt.state_revision,
                                    Some(export_intent.variant_id.clone()),
                                    Some(format!(
                                        "chat-export-failed:{}:{}:{}",
                                        thread_id, export_intent.variant_id, receipt.state_revision
                                    )),
                                    "CAD STEP export failed".to_string(),
                                    format!(
                                        "thread={} session={} variant={} error={} remediation={}",
                                        thread_id,
                                        state.cad_demo.session_id,
                                        export_intent.variant_id,
                                        error,
                                        error.remediation_hint()
                                    ),
                                );
                            }
                        }
                    }
                    _ => {
                        emit_cad_event(
                            state,
                            CadEventKind::ParameterUpdated,
                            receipt.state_revision,
                            Some(state.cad_demo.active_variant_id.clone()),
                            Some(format!(
                                "chat-intent:{}:{}:{}",
                                thread_id, intent_name, receipt.state_revision
                            )),
                            format!("CAD chat intent -> {}", intent_name),
                            format!(
                                "thread={} session={} revision={}",
                                thread_id, state.cad_demo.session_id, receipt.state_revision
                            ),
                        );
                    }
                },
                Err(error) => {
                    state.cad_demo.last_error = Some(format!(
                        "CAD intent dispatch failed for thread {}: {}",
                        thread_id, error
                    ));
                }
            }
            true
        }
        CadIntentTranslationOutcome::ParseFailure(error) => {
            if looks_like_cad_prompt(prompt) {
                state.cad_demo.last_error = Some(format!(
                    "CAD chat parse failure ({}) {}",
                    error.code, error.message
                ));
                state.cad_demo.last_action = Some(error.recovery_prompt);
                return true;
            }
            false
        }
    }
}

fn run_step_export_from_active_mesh(
    state: &CadDemoPaneState,
    variant_id: &str,
) -> openagents_cad::CadResult<openagents_cad::export::CadStepExportArtifact> {
    let mesh = state.last_good_mesh_payload.as_ref().ok_or_else(|| {
        openagents_cad::CadError::ExportFailed {
            format: "step".to_string(),
            reason: "no mesh payload available; rebuild before export".to_string(),
        }
    })?;
    openagents_cad::export::export_step_from_mesh(
        &state.document_id,
        state.document_revision,
        variant_id,
        mesh,
    )
}

pub(super) fn run_cad_demo_action(state: &mut RenderState, action: CadDemoPaneAction) -> bool {
    let action_changed = apply_cad_demo_action(&mut state.cad_demo, action);
    if action_changed {
        emit_cad_event_for_action(state, action);
    }
    if action_changed && matches!(action, CadDemoPaneAction::CycleMaterialPreset) {
        upsert_cad_material_activity_event(state);
    }
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
            if state.variant_viewports.is_empty() {
                state.load_state = PaneLoadState::Error;
                state.last_error = Some("CAD demo has no registered variants".to_string());
                state.last_action =
                    Some("Variant cycle rejected: no variants available".to_string());
                return true;
            }

            let next_index = (state.active_variant_tile_index + 1) % state.variant_viewports.len();
            let _ = state.set_active_variant_tile(next_index);
            state.document_revision = state.document_revision.saturating_add(1);
            if let Err(error) = enqueue_rebuild_cycle(state, "cycle-variant") {
                state.load_state = PaneLoadState::Error;
                state.last_error = Some(error);
            }
            state.last_action = Some(format!(
                "CAD active tile -> {} ({})",
                state.active_variant_tile_index + 1,
                state.active_variant_id
            ));
            true
        }
        CadDemoPaneAction::ResetSession | CadDemoPaneAction::BootstrapDemo => {
            bootstrap_cad_demo_state(state)
        }
        CadDemoPaneAction::ResetCamera => {
            state.reset_camera();
            state.last_action = Some("CAD camera reset to defaults".to_string());
            true
        }
        CadDemoPaneAction::ToggleProjectionMode => {
            state.cycle_projection_mode();
            state.last_action = Some(format!(
                "CAD projection mode -> {}",
                state.projection_mode.label()
            ));
            true
        }
        CadDemoPaneAction::CycleSectionPlane => {
            let axis = state.cycle_section_axis();
            state.last_action = Some(match axis {
                Some(axis) => format!("CAD section plane -> {}", axis.label()),
                None => "CAD section plane -> off".to_string(),
            });
            true
        }
        CadDemoPaneAction::StepSectionPlaneOffset => {
            let offset = state.step_section_offset();
            state.last_action = Some(format!(
                "CAD section offset -> {offset:+.1} ({})",
                state.section_summary()
            ));
            true
        }
        CadDemoPaneAction::CycleMaterialPreset => {
            let material_id = state.cycle_material_preset();
            if let Some(payload) = state.last_good_mesh_payload.as_ref() {
                let active_variant_id = state.active_variant_id.clone();
                let analysis = analysis_snapshot_from_mesh(
                    state.document_revision,
                    &active_variant_id,
                    payload,
                    &material_id,
                );
                state.set_variant_analysis_snapshot(&active_variant_id, analysis.snapshot);
                if let Some(error) = analysis.error {
                    state.last_error = Some(format!(
                        "CAD core analysis failed ({}): {}. {}",
                        error.stable_code(),
                        error.message(),
                        error.remediation_hint()
                    ));
                }
            }
            if let Some(material) = material_preset_by_id(&material_id) {
                state.last_action = Some(format!(
                    "CAD material -> {} ({}, {} kg/m^3)",
                    material.id, material.label, material.density_kg_m3
                ));
            } else {
                state.last_action = Some(format!("CAD material -> {material_id}"));
            }
            true
        }
        CadDemoPaneAction::ToggleSnapGrid => {
            let enabled = state.toggle_snap_mode(CadSnapMode::Grid);
            state.last_action = Some(format!(
                "CAD snap grid -> {} ({})",
                if enabled { "on" } else { "off" },
                state.snap_summary()
            ));
            true
        }
        CadDemoPaneAction::ToggleSnapOrigin => {
            let enabled = state.toggle_snap_mode(CadSnapMode::Origin);
            state.last_action = Some(format!(
                "CAD snap origin -> {} ({})",
                if enabled { "on" } else { "off" },
                state.snap_summary()
            ));
            true
        }
        CadDemoPaneAction::ToggleSnapEndpoint => {
            let enabled = state.toggle_snap_mode(CadSnapMode::Endpoint);
            state.last_action = Some(format!(
                "CAD snap endpoint -> {} ({})",
                if enabled { "on" } else { "off" },
                state.snap_summary()
            ));
            true
        }
        CadDemoPaneAction::ToggleSnapMidpoint => {
            let enabled = state.toggle_snap_mode(CadSnapMode::Midpoint);
            state.last_action = Some(format!(
                "CAD snap midpoint -> {} ({})",
                if enabled { "on" } else { "off" },
                state.snap_summary()
            ));
            true
        }
        CadDemoPaneAction::CycleHotkeyProfile => match state.cycle_hotkey_profile() {
            Ok(()) => {
                state.last_action = Some(format!(
                    "CAD hotkeys -> {} ({})",
                    state.hotkey_profile,
                    state.hotkeys.summary()
                ));
                true
            }
            Err(error) => {
                state.last_error = Some(error.clone());
                state.last_action = Some(format!("CAD hotkey profile cycle failed: {error}"));
                true
            }
        },
        CadDemoPaneAction::ToggleThreeDMouseMode => {
            state.toggle_three_d_mouse_mode();
            state.last_action = Some(format!(
                "CAD 3D mouse mode -> {} ({})",
                state.three_d_mouse_mode.label(),
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::CycleThreeDMouseProfile => {
            state.cycle_three_d_mouse_profile();
            state.last_action = Some(format!(
                "CAD 3D mouse profile -> {} ({})",
                state.three_d_mouse_profile.label(),
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::ToggleThreeDMouseLockX => {
            let enabled = state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::X);
            state.last_action = Some(format!(
                "CAD 3D mouse lock x -> {} ({})",
                if enabled { "on" } else { "off" },
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::ToggleThreeDMouseLockY => {
            let enabled = state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::Y);
            state.last_action = Some(format!(
                "CAD 3D mouse lock y -> {} ({})",
                if enabled { "on" } else { "off" },
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::ToggleThreeDMouseLockZ => {
            let enabled = state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::Z);
            state.last_action = Some(format!(
                "CAD 3D mouse lock z -> {} ({})",
                if enabled { "on" } else { "off" },
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::ToggleThreeDMouseLockRx => {
            let enabled = state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::Rx);
            state.last_action = Some(format!(
                "CAD 3D mouse lock rx -> {} ({})",
                if enabled { "on" } else { "off" },
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::ToggleThreeDMouseLockRy => {
            let enabled = state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::Ry);
            state.last_action = Some(format!(
                "CAD 3D mouse lock ry -> {} ({})",
                if enabled { "on" } else { "off" },
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::ToggleThreeDMouseLockRz => {
            let enabled = state.toggle_three_d_mouse_axis_lock(CadThreeDMouseAxis::Rz);
            state.last_action = Some(format!(
                "CAD 3D mouse lock rz -> {} ({})",
                if enabled { "on" } else { "off" },
                state.three_d_mouse_status()
            ));
            true
        }
        CadDemoPaneAction::SnapViewTop => {
            state.snap_camera_to_view(CadCameraViewSnap::Top);
            state.last_action = Some("CAD camera snap -> top".to_string());
            true
        }
        CadDemoPaneAction::SnapViewFront => {
            state.snap_camera_to_view(CadCameraViewSnap::Front);
            state.last_action = Some("CAD camera snap -> front".to_string());
            true
        }
        CadDemoPaneAction::SnapViewRight => {
            state.snap_camera_to_view(CadCameraViewSnap::Right);
            state.last_action = Some("CAD camera snap -> right".to_string());
            true
        }
        CadDemoPaneAction::SnapViewIsometric => {
            state.snap_camera_to_view(CadCameraViewSnap::Isometric);
            state.last_action = Some("CAD camera snap -> isometric".to_string());
            true
        }
        CadDemoPaneAction::CycleHiddenLineMode => {
            state.hidden_line_mode = state.hidden_line_mode.next();
            state.last_action = Some(format!(
                "CAD render mode -> {}",
                state.hidden_line_mode.label()
            ));
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
        CadDemoPaneAction::StartDimensionEdit(index) => {
            if state.begin_dimension_edit(index) {
                if let Some(dimension) = state.dimensions.get(index) {
                    state.last_action = Some(format!(
                        "CAD dimension edit -> {} ({:.3} mm)",
                        dimension.label, dimension.value_mm
                    ));
                }
                state.last_error = None;
                true
            } else {
                false
            }
        }
        CadDemoPaneAction::DimensionInputChar(ch) => {
            if state.append_dimension_edit_char(ch) {
                state.last_error = None;
                state.last_action = Some(format!("CAD dimension input -> '{ch}'"));
                true
            } else {
                false
            }
        }
        CadDemoPaneAction::DimensionInputBackspace => {
            if state.backspace_dimension_edit() {
                state.last_error = None;
                state.last_action = Some("CAD dimension input backspace".to_string());
                true
            } else {
                false
            }
        }
        CadDemoPaneAction::DimensionInputCancel => {
            if state.cancel_dimension_edit() {
                state.last_error = None;
                state.last_action = Some("CAD dimension edit cancelled".to_string());
                true
            } else {
                false
            }
        }
        CadDemoPaneAction::DimensionInputCommit => match state.commit_dimension_edit() {
            Ok((dimension_id, previous, updated)) => {
                state.document_revision = state.document_revision.saturating_add(1);
                let trigger = format!("edit-dimension:{dimension_id}");
                if let Err(error) = enqueue_rebuild_cycle(state, trigger.as_str()) {
                    state.load_state = PaneLoadState::Error;
                    state.last_error = Some(error);
                } else {
                    state.last_error = None;
                }
                state.last_action = Some(format!(
                    "CAD dimension {} {:.3} -> {:.3} mm",
                    dimension_id, previous, updated
                ));
                true
            }
            Err(error) => {
                if let Some(edit) = state.dimension_edit.as_mut() {
                    edit.last_error = Some(error.clone());
                }
                state.last_error = Some(error.clone());
                state.last_action = Some(format!("CAD dimension edit failed: {error}"));
                true
            }
        },
    }
}

fn bootstrap_cad_demo_state(state: &mut CadDemoPaneState) -> bool {
    let mut bootstrap = CadDemoPaneState::default();
    bootstrap.last_action = Some("CAD demo bootstrapped to deterministic baseline".to_string());
    if let Err(error) = enqueue_rebuild_cycle(&mut bootstrap, "bootstrap-demo") {
        bootstrap.load_state = PaneLoadState::Error;
        bootstrap.last_error = Some(error);
    }
    *state = bootstrap;
    true
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
        if let Some(receipt) = apply_rebuild_response(state, response) {
            emitted.push(receipt);
        }
    }
    emitted
}

fn apply_rebuild_response(
    state: &mut CadDemoPaneState,
    response: CadRebuildResponse,
) -> Option<CadRebuildReceiptState> {
    match response {
        CadRebuildResponse::Completed(completed) => {
            if completed.document_revision < state.document_revision {
                // Keep last-good mesh steady; skip stale rebuild result.
                return None;
            }
            match apply_completed_rebuild(state, completed) {
                Ok(receipt) => Some(receipt),
                Err(error) => {
                    state.load_state = PaneLoadState::Error;
                    state.last_error = Some(error.clone());
                    state.last_action = Some(format!("CAD rebuild commit failed: {error}"));
                    None
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
            None
        }
    }
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
    let material_id = state
        .analysis_snapshot
        .material_id
        .as_deref()
        .unwrap_or(DEFAULT_CAD_MATERIAL_ID)
        .to_string();
    let analysis = analysis_snapshot_from_mesh(
        completed.document_revision,
        &receipt.variant_id,
        &completed.mesh_payload,
        &material_id,
    );
    state.set_variant_analysis_snapshot(&receipt.variant_id, analysis.snapshot);
    if let Some(error) = analysis.error {
        state.last_error = Some(format!(
            "CAD core analysis failed ({}): {}. {}",
            error.stable_code(),
            error.message(),
            error.remediation_hint()
        ));
    }
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
    let analysis = &state.cad_demo.analysis_snapshot;
    let analysis_detail = if analysis.document_revision == receipt.document_revision
        && analysis.variant_id == receipt.variant_id
    {
        format!(
            "analysis(volume_mm3={}, mass_kg={}, cog_mm={}, cost_usd={}, deflection_mm={}, deflection_confidence={}, model_id={})",
            analysis
                .volume_mm3
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "none".to_string()),
            analysis
                .mass_kg
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "none".to_string()),
            analysis
                .center_of_gravity_mm
                .map(|value| format!("{:.3},{:.3},{:.3}", value[0], value[1], value[2]))
                .unwrap_or_else(|| "none".to_string()),
            analysis
                .estimated_cost_usd
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "none".to_string()),
            analysis
                .max_deflection_mm
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "none".to_string()),
            analysis
                .estimator_metadata
                .get("deflection.confidence")
                .map(String::as_str)
                .unwrap_or("none"),
            analysis
                .estimator_metadata
                .get("model_id")
                .map(String::as_str)
                .unwrap_or("none"),
        )
    } else {
        "analysis(pending)".to_string()
    };
    emit_cad_event(
        state,
        CadEventKind::RebuildCompleted,
        receipt.document_revision,
        Some(receipt.variant_id.clone()),
        Some(format!(
            "rebuild:{}:{}:{}",
            receipt.document_revision, receipt.variant_id, receipt.rebuild_hash
        )),
        format!(
            "CAD rebuild rev={} {}ms",
            receipt.document_revision, receipt.duration_ms
        ),
        format!(
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
    );
    emit_cad_event(
        state,
        CadEventKind::AnalysisUpdated,
        receipt.document_revision,
        Some(receipt.variant_id.clone()),
        Some(format!(
            "analysis:{}:{}:{}",
            receipt.document_revision, receipt.variant_id, receipt.mesh_hash
        )),
        "CAD analysis updated".to_string(),
        analysis_detail,
    );
    if let Some(warnings) = state.cad_demo.variant_warning_sets.get(&receipt.variant_id)
        && !warnings.is_empty()
    {
        let warning_codes = warnings
            .iter()
            .map(|warning| warning.code.clone())
            .collect::<Vec<_>>()
            .join(",");
        emit_cad_event(
            state,
            CadEventKind::WarningRaised,
            receipt.document_revision,
            Some(receipt.variant_id.clone()),
            Some(format!(
                "warnings:{}:{}:{}",
                receipt.document_revision, receipt.variant_id, warning_codes
            )),
            format!("CAD warnings ({})", warnings.len()),
            format!("variant={} codes={warning_codes}", receipt.variant_id),
        );
    }
}

fn upsert_cad_material_activity_event(state: &mut RenderState) {
    let analysis = &state.cad_demo.analysis_snapshot;
    let material_id = analysis
        .material_id
        .as_deref()
        .unwrap_or(DEFAULT_CAD_MATERIAL_ID);
    let cost_model = analysis
        .estimator_metadata
        .get("model_id")
        .or_else(|| analysis.estimator_metadata.get("cost.model_id"))
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    let complexity_factor = analysis
        .estimator_metadata
        .get("derived.complexity_factor")
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    let deflection_confidence = analysis
        .estimator_metadata
        .get("deflection.confidence")
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    emit_cad_event(
        state,
        CadEventKind::ParameterUpdated,
        analysis.document_revision,
        Some(state.cad_demo.active_variant_id.clone()),
        Some(format!(
            "material:{}:{}",
            analysis.document_revision, state.cad_demo.active_variant_id
        )),
        format!("CAD material -> {material_id}"),
        format!(
            "variant={} mass_kg={} cost_usd={} deflection_mm={} deflection_confidence={} model={} complexity={}",
            analysis.variant_id,
            analysis
                .mass_kg
                .map(|value| format!("{value:.6}"))
                .unwrap_or_else(|| "none".to_string()),
            analysis
                .estimated_cost_usd
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "none".to_string()),
            analysis
                .max_deflection_mm
                .map(|value| format!("{value:.3}"))
                .unwrap_or_else(|| "none".to_string()),
            deflection_confidence,
            cost_model,
            complexity_factor,
        ),
    );
}

fn emit_cad_event_for_action(state: &mut RenderState, action: CadDemoPaneAction) {
    match action {
        CadDemoPaneAction::CycleVariant => {
            let variant_id = state.cad_demo.active_variant_id.clone();
            emit_cad_event(
                state,
                CadEventKind::VariantGenerated,
                state.cad_demo.document_revision,
                Some(variant_id.clone()),
                Some(format!(
                    "variant:{}:{}",
                    state.cad_demo.document_revision, variant_id
                )),
                format!("CAD variant active -> {variant_id}"),
                format!(
                    "tile={} session={}",
                    state.cad_demo.active_variant_tile_index, state.cad_demo.session_id
                ),
            );
        }
        CadDemoPaneAction::ResetSession | CadDemoPaneAction::BootstrapDemo => {
            emit_cad_event(
                state,
                CadEventKind::DocumentCreated,
                state.cad_demo.document_revision,
                Some(state.cad_demo.active_variant_id.clone()),
                Some("document-created".to_string()),
                "CAD document created".to_string(),
                format!(
                    "session={} document={}",
                    state.cad_demo.session_id, state.cad_demo.document_id
                ),
            );
        }
        CadDemoPaneAction::SelectWarning(_)
        | CadDemoPaneAction::SelectWarningMarker(_)
        | CadDemoPaneAction::SelectTimelineRow(_)
        | CadDemoPaneAction::TimelineSelectPrev
        | CadDemoPaneAction::TimelineSelectNext => {
            emit_cad_event(
                state,
                CadEventKind::SelectionChanged,
                state.cad_demo.document_revision,
                Some(state.cad_demo.active_variant_id.clone()),
                Some(format!(
                    "selection:{}:{}:{}",
                    state.cad_demo.document_revision,
                    state.cad_demo.active_variant_id,
                    state
                        .cad_demo
                        .focused_geometry_ref
                        .as_deref()
                        .unwrap_or("none")
                )),
                "CAD selection changed".to_string(),
                format!(
                    "focused={} hovered={}",
                    state
                        .cad_demo
                        .focused_geometry_ref
                        .as_deref()
                        .unwrap_or("none"),
                    state
                        .cad_demo
                        .hovered_geometry_ref
                        .as_deref()
                        .unwrap_or("none")
                ),
            );
        }
        CadDemoPaneAction::CycleMaterialPreset
        | CadDemoPaneAction::CycleSectionPlane
        | CadDemoPaneAction::StepSectionPlaneOffset
        | CadDemoPaneAction::DimensionInputCommit => {
            emit_cad_event(
                state,
                CadEventKind::ParameterUpdated,
                state.cad_demo.document_revision,
                Some(state.cad_demo.active_variant_id.clone()),
                Some(format!(
                    "parameter:{}:{}:{:?}:{:.2}",
                    state.cad_demo.document_revision,
                    state.cad_demo.active_variant_id,
                    state.cad_demo.section_axis,
                    state.cad_demo.section_offset_normalized
                )),
                "CAD parameter updated".to_string(),
                state
                    .cad_demo
                    .last_action
                    .clone()
                    .unwrap_or_else(|| "parameter mutation".to_string()),
            );
        }
        _ => {}
    }
}

fn emit_cad_event(
    state: &mut RenderState,
    kind: CadEventKind,
    document_revision: u64,
    variant_id: Option<String>,
    key: Option<String>,
    summary: String,
    detail: String,
) {
    let event = CadEvent::new_with_key(
        kind,
        state.cad_demo.session_id.clone(),
        state.cad_demo.document_id.clone(),
        document_revision,
        variant_id,
        summary,
        detail,
        key,
    );
    state.cad_demo.upsert_cad_event(event.clone());
    state
        .activity_feed
        .upsert_event(activity_row_from_cad_event(&event));
    state.activity_feed.load_state = PaneLoadState::Ready;
    state.activity_feed.last_action = Some(format!("CAD activity -> {}", event.kind.as_str()));
}

fn activity_row_from_cad_event(event: &CadEvent) -> ActivityEventRow {
    let occurred_at_epoch_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    ActivityEventRow {
        event_id: event.event_id.clone(),
        domain: ActivityEventDomain::Cad,
        source_tag: format!("cad.{}", event.kind.as_str()),
        occurred_at_epoch_seconds,
        summary: event.summary.clone(),
        detail: format!(
            "doc={} rev={} variant={} {}",
            event.document_id,
            event.document_revision,
            event.variant_id.as_deref().unwrap_or("none"),
            event.detail
        ),
    }
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
    let width_mm = dimension_value_mm(state, "width_mm", 390.0);
    let depth_mm = dimension_value_mm(state, "depth_mm", 226.0);
    let height_mm = dimension_value_mm(state, "height_mm", 88.0);
    let wall_mm = dimension_value_mm(state, "wall_mm", 6.0);

    let (variant_width_mm, variant_depth_mm, variant_height_mm, vent_spacing_mm, vent_count) =
        match state.active_variant_id.as_str() {
            "variant.lightweight" => (
                (width_mm - 8.0).max(300.0),
                (depth_mm + 2.0).max(140.0),
                (height_mm - 2.0).max(40.0),
                34.0,
                5_u32,
            ),
            "variant.low-cost" => (
                (width_mm + 2.0).max(300.0),
                (depth_mm - 2.0).max(140.0),
                height_mm.max(40.0),
                28.0,
                6_u32,
            ),
            "variant.stiffness" => (
                (width_mm + 5.0).max(300.0),
                (depth_mm + 6.0).max(140.0),
                (height_mm + 3.0).max(40.0),
                24.0,
                8_u32,
            ),
            _ => (
                width_mm.max(300.0),
                depth_mm.max(140.0),
                height_mm.max(40.0),
                30.0,
                6_u32,
            ),
        };

    let feature_nodes = vec![
        FeatureNode {
            id: "feature.base".to_string(),
            name: "base".to_string(),
            operation_key: "primitive.box.v1".to_string(),
            depends_on: Vec::new(),
            params: BTreeMap::from([
                ("width_param".to_string(), "width_mm".to_string()),
                ("depth_param".to_string(), "depth_mm".to_string()),
                ("height_param".to_string(), "height_mm".to_string()),
                ("variant".to_string(), state.active_variant_id.clone()),
                ("width_mm".to_string(), format!("{variant_width_mm:.3}")),
                ("depth_mm".to_string(), format!("{variant_depth_mm:.3}")),
                ("height_mm".to_string(), format!("{variant_height_mm:.3}")),
                ("wall_mm".to_string(), format!("{wall_mm:.3}")),
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
                ("mount_hole_radius_mm".to_string(), "4.400".to_string()),
                (
                    "mount_hole_depth_mm".to_string(),
                    format!("{:.3}", (variant_height_mm * 0.15).max(6.0)),
                ),
                ("width_mm".to_string(), format!("{variant_width_mm:.3}")),
                ("depth_mm".to_string(), format!("{variant_depth_mm:.3}")),
                ("height_mm".to_string(), format!("{variant_height_mm:.3}")),
            ]),
        },
        FeatureNode {
            id: "feature.vent_pattern".to_string(),
            name: "vent_pattern".to_string(),
            operation_key: "linear.pattern.v1".to_string(),
            depends_on: vec!["feature.mount_hole".to_string()],
            params: BTreeMap::from([
                ("count_param".to_string(), "vent_count".to_string()),
                ("spacing_param".to_string(), "vent_spacing_mm".to_string()),
                ("vent_count".to_string(), vent_count.to_string()),
                (
                    "vent_spacing_mm".to_string(),
                    format!("{vent_spacing_mm:.3}"),
                ),
                ("width_mm".to_string(), format!("{variant_width_mm:.3}")),
                ("depth_mm".to_string(), format!("{variant_depth_mm:.3}")),
                ("height_mm".to_string(), format!("{variant_height_mm:.3}")),
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
                (
                    "edge_radius_mm".to_string(),
                    format!("{:.3}", (wall_mm * 0.45).max(1.0)),
                ),
                ("width_mm".to_string(), format!("{variant_width_mm:.3}")),
                ("depth_mm".to_string(), format!("{variant_depth_mm:.3}")),
                ("height_mm".to_string(), format!("{variant_height_mm:.3}")),
                ("wall_mm".to_string(), format!("{wall_mm:.3}")),
            ]),
        },
    ];

    FeatureGraph {
        nodes: feature_nodes,
    }
}

fn dimension_value_mm(state: &CadDemoPaneState, dimension_id: &str, fallback: f64) -> f64 {
    state.dimension_value_mm(dimension_id).unwrap_or(fallback)
}

fn refresh_warning_state(state: &mut CadDemoPaneState, document_revision: u64, variant_id: &str) {
    let snapshot = build_demo_validity_snapshot(document_revision, variant_id);
    let receipt = run_model_validity_checks(&snapshot);
    let warnings = receipt
        .warnings
        .iter()
        .enumerate()
        .map(|(index, warning)| warning_to_pane_state(index, warning))
        .collect::<Vec<_>>();
    state.set_variant_warning_set(variant_id, warnings);
    state.set_focused_geometry_for_active_variant(None);
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
        state.set_focused_geometry_for_active_variant(Some(format!(
            "cad://feature/{}",
            state.timeline_rows[index].feature_id
        )));
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

struct CadAnalysisComputation {
    snapshot: openagents_cad::contracts::CadAnalysis,
    error: Option<CadAnalysisComputationError>,
}

enum CadAnalysisComputationError {
    Body(CadBodyAnalysisError),
    Cost(CadCostHeuristicError),
    Deflection(CadDeflectionHeuristicError),
}

impl CadAnalysisComputationError {
    fn stable_code(&self) -> &'static str {
        match self {
            Self::Body(error) => error.code.stable_code(),
            Self::Cost(error) => error.code.stable_code(),
            Self::Deflection(error) => error.code.stable_code(),
        }
    }

    fn message(&self) -> &str {
        match self {
            Self::Body(error) => error.message.as_str(),
            Self::Cost(error) => error.message.as_str(),
            Self::Deflection(error) => error.message.as_str(),
        }
    }

    fn remediation_hint(&self) -> &'static str {
        match self {
            Self::Body(error) => error.remediation_hint(),
            Self::Cost(error) => error.remediation_hint(),
            Self::Deflection(error) => error.remediation_hint(),
        }
    }
}

fn analysis_snapshot_from_mesh(
    document_revision: u64,
    variant_id: &str,
    mesh_payload: &openagents_cad::mesh::CadMeshPayload,
    material_id: &str,
) -> CadAnalysisComputation {
    let material = material_preset_by_id(material_id)
        .or_else(|| material_preset_by_id(DEFAULT_CAD_MATERIAL_ID))
        .expect("default CAD material preset should always resolve");
    match analyze_body_properties(mesh_payload, material.density_kg_m3) {
        Ok(receipt) => {
            let mass_kg = Some(receipt.properties.mass_kg);
            let mut estimator_metadata = BTreeMap::new();
            let mut error = None;
            const DEFLECTION_LOAD_KG: f64 = 10.0;
            let estimated_cost_usd = match estimate_cnc_cost_heuristic_usd(
                CadCostHeuristicInput {
                    mass_kg: receipt.properties.mass_kg,
                    volume_mm3: receipt.properties.volume_mm3,
                    surface_area_mm2: receipt.properties.surface_area_mm2,
                    triangle_count: receipt.triangle_count,
                },
                material,
            ) {
                Ok(cost) => {
                    estimator_metadata = cost.metadata;
                    Some(cost.total_cost_usd)
                }
                Err(cost_error) => {
                    estimator_metadata.insert(
                        "cost.error.code".to_string(),
                        cost_error.code.stable_code().to_string(),
                    );
                    estimator_metadata
                        .insert("cost.error.message".to_string(), cost_error.message.clone());
                    estimator_metadata.insert(
                        "cost.error.remediation_hint".to_string(),
                        cost_error.remediation_hint().to_string(),
                    );
                    error = Some(CadAnalysisComputationError::Cost(cost_error));
                    None
                }
            };
            let mut bounds_size = receipt.properties.bounds_size_mm;
            bounds_size.sort_by(|lhs, rhs| lhs.total_cmp(rhs));
            let max_deflection_mm =
                match estimate_beam_deflection_heuristic(CadDeflectionHeuristicInput {
                    span_mm: bounds_size[2],
                    width_mm: bounds_size[1],
                    thickness_mm: bounds_size[0],
                    load_kg: DEFLECTION_LOAD_KG,
                    youngs_modulus_gpa: material.youngs_modulus_gpa,
                }) {
                    Ok(deflection) => {
                        for (key, value) in deflection.metadata {
                            estimator_metadata.insert(format!("deflection.{key}"), value);
                        }
                        Some(deflection.max_deflection_mm)
                    }
                    Err(deflection_error) => {
                        estimator_metadata.insert(
                            "deflection.error.code".to_string(),
                            deflection_error.code.stable_code().to_string(),
                        );
                        estimator_metadata.insert(
                            "deflection.error.message".to_string(),
                            deflection_error.message.clone(),
                        );
                        estimator_metadata.insert(
                            "deflection.error.remediation_hint".to_string(),
                            deflection_error.remediation_hint().to_string(),
                        );
                        if error.is_none() {
                            error = Some(CadAnalysisComputationError::Deflection(deflection_error));
                        }
                        None
                    }
                };
            CadAnalysisComputation {
                snapshot: openagents_cad::contracts::CadAnalysis {
                    document_revision,
                    variant_id: variant_id.to_string(),
                    material_id: Some(material.id.to_string()),
                    volume_mm3: Some(receipt.properties.volume_mm3),
                    mass_kg,
                    center_of_gravity_mm: Some(receipt.properties.center_of_gravity_mm),
                    estimated_cost_usd,
                    max_deflection_mm,
                    estimator_metadata,
                    objective_scores: BTreeMap::new(),
                },
                error,
            }
        }
        Err(error) => {
            let remediation_hint = error.remediation_hint().to_string();
            let error_code = error.code.stable_code().to_string();
            let error_message = error.message.clone();
            CadAnalysisComputation {
                snapshot: openagents_cad::contracts::CadAnalysis {
                    document_revision,
                    variant_id: variant_id.to_string(),
                    material_id: Some(material.id.to_string()),
                    volume_mm3: None,
                    mass_kg: None,
                    center_of_gravity_mm: None,
                    estimated_cost_usd: None,
                    max_deflection_mm: None,
                    estimator_metadata: BTreeMap::from([
                        ("analysis.error.code".to_string(), error_code),
                        ("analysis.error.message".to_string(), error_message),
                        (
                            "analysis.error.remediation_hint".to_string(),
                            remediation_hint,
                        ),
                    ]),
                    objective_scores: BTreeMap::new(),
                },
                error: Some(CadAnalysisComputationError::Body(error)),
            }
        }
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
        analysis: state.analysis_snapshot.clone(),
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
    let warning_code = warning.code.clone();
    let warning_entity_id = warning.entity_id.clone();
    let deep_link = warning.deep_link.clone();
    let fallback = format!("cad://feature/{}", warning.feature_id);
    state.warning_hover_index = Some(warning_index);
    state.focused_warning_index = Some(warning_index);
    state.set_focused_geometry_for_active_variant(deep_link.or(Some(fallback)));
    state.last_action = Some(format!(
        "CAD warning focus -> {} ({})",
        warning_code, warning_entity_id
    ));
}

fn select_timeline_row(state: &mut CadDemoPaneState, index: usize) {
    if index >= state.timeline_rows.len() {
        return;
    }
    state.timeline_selected_index = Some(index);
    state.timeline_scroll_offset = auto_scroll_offset(index, state.timeline_scroll_offset, 10);
    state.selected_feature_params = state.timeline_rows[index].params.clone();
    state.set_focused_geometry_for_active_variant(Some(format!(
        "cad://feature/{}",
        state.timeline_rows[index].feature_id
    )));
    state.last_action = Some(format!(
        "CAD timeline selected -> {}",
        state.timeline_rows[index].feature_name
    ));
}

fn looks_like_cad_prompt(prompt: &str) -> bool {
    let lower = prompt.to_ascii_lowercase();
    [
        "cad",
        "rack",
        "variant",
        "material",
        "objective",
        "vent",
        "export",
        "select",
        "wall",
        "thickness",
    ]
    .iter()
    .any(|token| lower.contains(token))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{Duration, Instant};

    use super::{
        activity_row_from_cad_event, analysis_snapshot_from_mesh, apply_cad_demo_action,
        apply_rebuild_response, drain_worker_responses_from_pane, run_step_export_from_active_mesh,
    };
    use crate::app_state::{ActivityEventDomain, CadDemoPaneState, CadTimelineRowState};
    use crate::cad_rebuild_worker::{CadRebuildFailed, CadRebuildResponse};
    use crate::pane_system::CadDemoPaneAction;
    use openagents_cad::chat_adapter::{CadIntentTranslationOutcome, translate_chat_to_cad_intent};
    use openagents_cad::events::{CadEvent, CadEventKind};
    use openagents_cad::intent::parse_cad_intent_json;
    use openagents_cad::mesh::{
        CadMeshBounds, CadMeshMaterialSlot, CadMeshPayload, CadMeshTopology, CadMeshVertex,
    };
    use serde_json::{Value, json};

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

    fn bootstrap_signature(state: &CadDemoPaneState) -> String {
        let viewport_signature = state
            .variant_viewports
            .iter()
            .map(|viewport| {
                format!(
                    "{}:{:.2}:{:.2}:{:.2}:{:.2}:{:.2}:{}:{}",
                    viewport.variant_id,
                    viewport.camera_zoom,
                    viewport.camera_pan_x,
                    viewport.camera_pan_y,
                    viewport.camera_orbit_yaw_deg,
                    viewport.camera_orbit_pitch_deg,
                    viewport.selected_ref.as_deref().unwrap_or("none"),
                    viewport.hovered_ref.as_deref().unwrap_or("none"),
                )
            })
            .collect::<Vec<_>>()
            .join("|");
        format!(
            "load={:?}|err={:?}|action={:?}|session={}|doc={}|rev={}|variant={}|tile={}|next={}\
|pending={:?}|mesh={:?}|warnings={}|events={}|section={}|snap={}|projection={}|viewport={}",
            state.load_state,
            state.last_error,
            state.last_action,
            state.session_id,
            state.document_id,
            state.document_revision,
            state.active_variant_id,
            state.active_variant_tile_index,
            state.next_rebuild_request_id,
            state.pending_rebuild_request_id,
            state.last_good_mesh_id,
            state.warnings.len(),
            state.cad_events.len(),
            state.section_summary(),
            state.snap_summary(),
            state.projection_mode.label(),
            viewport_signature,
        )
    }

    fn interaction_fixture_path() -> String {
        let root = env!("CARGO_MANIFEST_DIR");
        format!("{root}/tests/goldens/cad_followup_parameter_edit_interaction.json")
    }

    fn interaction_snapshot(
        state: &CadDemoPaneState,
        prompt: &str,
        intent_name: &str,
        prompt_state_revision: u64,
    ) -> Value {
        let receipts = state
            .rebuild_receipts
            .iter()
            .map(|receipt| {
                json!({
                    "event_id": receipt.event_id,
                    "document_revision": receipt.document_revision,
                    "variant_id": receipt.variant_id,
                    "rebuild_hash": receipt.rebuild_hash,
                    "mesh_hash": receipt.mesh_hash,
                    "duration_ms": receipt.duration_ms,
                    "feature_count": receipt.feature_count,
                    "vertex_count": receipt.vertex_count,
                    "triangle_count": receipt.triangle_count,
                    "edge_count": receipt.edge_count,
                    "cache_hits": receipt.cache_hits,
                    "cache_misses": receipt.cache_misses,
                    "cache_evictions": receipt.cache_evictions,
                })
            })
            .collect::<Vec<_>>();
        let warnings = state
            .warnings
            .iter()
            .map(|warning| {
                json!({
                    "code": warning.code,
                    "severity": warning.severity,
                    "feature_id": warning.feature_id,
                    "entity_id": warning.entity_id,
                    "semantic_refs": warning.semantic_refs,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "prompt": prompt,
            "intent": intent_name,
            "prompt_state_revision": prompt_state_revision,
            "active_variant_id": state.active_variant_id,
            "selected_feature": state.focused_geometry_ref,
            "timeline_selected_index": state.timeline_selected_index,
            "width_mm": state.dimension_value_mm("width_mm"),
            "rebuild_receipts": receipts,
            "analysis": {
                "document_revision": state.analysis_snapshot.document_revision,
                "variant_id": state.analysis_snapshot.variant_id,
                "material_id": state.analysis_snapshot.material_id,
                "volume_mm3": state.analysis_snapshot.volume_mm3,
                "mass_kg": state.analysis_snapshot.mass_kg,
                "estimated_cost_usd": state.analysis_snapshot.estimated_cost_usd,
                "max_deflection_mm": state.analysis_snapshot.max_deflection_mm,
                "center_of_gravity_mm": state.analysis_snapshot.center_of_gravity_mm,
            },
            "warnings": warnings,
            "last_action": state.last_action,
            "last_error": state.last_error,
        })
    }

    fn interaction_semantic_diff(expected: &Value, actual: &Value) -> String {
        let mut lines = Vec::<String>::new();
        for field in [
            "prompt",
            "intent",
            "prompt_state_revision",
            "active_variant_id",
            "selected_feature",
            "timeline_selected_index",
            "width_mm",
            "last_error",
        ] {
            if expected.get(field) != actual.get(field) {
                lines.push(format!(
                    "{field} expected={} actual={}",
                    expected
                        .get(field)
                        .map(Value::to_string)
                        .unwrap_or_else(|| "null".to_string()),
                    actual
                        .get(field)
                        .map(Value::to_string)
                        .unwrap_or_else(|| "null".to_string())
                ));
            }
        }
        for field in [
            "document_revision",
            "variant_id",
            "material_id",
            "volume_mm3",
            "mass_kg",
            "estimated_cost_usd",
            "max_deflection_mm",
            "center_of_gravity_mm",
        ] {
            if expected.get("analysis").and_then(|value| value.get(field))
                != actual.get("analysis").and_then(|value| value.get(field))
            {
                lines.push(format!(
                    "analysis.{field} expected={} actual={}",
                    expected
                        .get("analysis")
                        .and_then(|value| value.get(field))
                        .map(Value::to_string)
                        .unwrap_or_else(|| "null".to_string()),
                    actual
                        .get("analysis")
                        .and_then(|value| value.get(field))
                        .map(Value::to_string)
                        .unwrap_or_else(|| "null".to_string())
                ));
            }
        }

        let expected_receipts = expected
            .get("rebuild_receipts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let actual_receipts = actual
            .get("rebuild_receipts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if expected_receipts.len() != actual_receipts.len() {
            lines.push(format!(
                "rebuild_receipts length expected={} actual={}",
                expected_receipts.len(),
                actual_receipts.len()
            ));
        }
        for (index, (expected_receipt, actual_receipt)) in expected_receipts
            .iter()
            .zip(actual_receipts.iter())
            .enumerate()
        {
            for field in [
                "document_revision",
                "variant_id",
                "rebuild_hash",
                "mesh_hash",
                "duration_ms",
                "feature_count",
                "vertex_count",
                "triangle_count",
                "edge_count",
                "cache_hits",
                "cache_misses",
                "cache_evictions",
            ] {
                if expected_receipt.get(field) != actual_receipt.get(field) {
                    lines.push(format!(
                        "rebuild_receipts[{index}].{field} expected={} actual={}",
                        expected_receipt
                            .get(field)
                            .map(Value::to_string)
                            .unwrap_or_else(|| "null".to_string()),
                        actual_receipt
                            .get(field)
                            .map(Value::to_string)
                            .unwrap_or_else(|| "null".to_string())
                    ));
                }
            }
        }

        let warning_signature = |warning: &Value| {
            format!(
                "{}|{}|{}|{}|{}",
                warning
                    .get("code")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "null".to_string()),
                warning
                    .get("severity")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "null".to_string()),
                warning
                    .get("feature_id")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "null".to_string()),
                warning
                    .get("entity_id")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "null".to_string()),
                warning
                    .get("semantic_refs")
                    .map(Value::to_string)
                    .unwrap_or_else(|| "null".to_string()),
            )
        };
        let expected_warning_signatures = expected
            .get("warnings")
            .and_then(Value::as_array)
            .map(|warnings| warnings.iter().map(warning_signature).collect::<Vec<_>>())
            .unwrap_or_default();
        let actual_warning_signatures = actual
            .get("warnings")
            .and_then(Value::as_array)
            .map(|warnings| warnings.iter().map(warning_signature).collect::<Vec<_>>())
            .unwrap_or_default();
        if expected_warning_signatures != actual_warning_signatures {
            lines.push(format!(
                "warning signatures expected={} actual={}",
                expected_warning_signatures.join(";"),
                actual_warning_signatures.join(";")
            ));
        }

        if lines.is_empty() {
            "no semantic interaction diff".to_string()
        } else {
            lines.join("\n")
        }
    }

    fn script_fixture_path(name: &str) -> String {
        let root = env!("CARGO_MANIFEST_DIR");
        format!("{root}/tests/scripts/{name}")
    }

    fn load_script_fixture(name: &str) -> Value {
        let path = script_fixture_path(name);
        let payload = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read script fixture {path}: {error}"));
        serde_json::from_str(&payload)
            .unwrap_or_else(|error| panic!("failed to parse script fixture {path}: {error}"))
    }

    fn required_object<'a>(value: &'a Value, path: &str) -> &'a serde_json::Map<String, Value> {
        value
            .as_object()
            .unwrap_or_else(|| panic!("expected object at {path}, found {value}"))
    }

    fn required_array<'a>(value: &'a Value, path: &str) -> &'a Vec<Value> {
        value
            .as_array()
            .unwrap_or_else(|| panic!("expected array at {path}, found {value}"))
    }

    fn required_str(map: &serde_json::Map<String, Value>, key: &str, path: &str) -> String {
        map.get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| panic!("expected string at {path}.{key}"))
    }

    fn optional_u64(map: &serde_json::Map<String, Value>, key: &str) -> Option<u64> {
        map.get(key).and_then(Value::as_u64)
    }

    fn required_u64(map: &serde_json::Map<String, Value>, key: &str, path: &str) -> u64 {
        optional_u64(map, key).unwrap_or_else(|| panic!("expected u64 at {path}.{key}"))
    }

    fn deterministic_seeded_count(seed: u64, step_index: usize, min: u64, max: u64) -> u64 {
        if min > max {
            panic!("invalid randomized cycle bounds: min {min} > max {max}");
        }
        if min == max {
            return min;
        }
        let span = max.saturating_sub(min).saturating_add(1);
        let mixed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1)
            .wrapping_add((step_index as u64).wrapping_mul(1442695040888963407));
        min.saturating_add(mixed % span)
    }

    fn receipt_json(receipt: &crate::app_state::CadRebuildReceiptState) -> Value {
        json!({
            "event_id": receipt.event_id,
            "document_revision": receipt.document_revision,
            "variant_id": receipt.variant_id,
            "rebuild_hash": receipt.rebuild_hash,
            "mesh_hash": receipt.mesh_hash,
            "duration_ms": receipt.duration_ms,
            "feature_count": receipt.feature_count,
            "vertex_count": receipt.vertex_count,
            "triangle_count": receipt.triangle_count,
            "edge_count": receipt.edge_count,
            "cache_hits": receipt.cache_hits,
            "cache_misses": receipt.cache_misses,
            "cache_evictions": receipt.cache_evictions,
        })
    }

    fn assert_json_subset(expected: &Value, actual: &Value, path: &str) {
        match expected {
            Value::Object(expected_map) => {
                let actual_map = actual.as_object().unwrap_or_else(|| {
                    panic!("expected object at {path}, found actual value {actual}")
                });
                for (key, expected_value) in expected_map {
                    let child_path = format!("{path}.{key}");
                    let actual_value = actual_map.get(key).unwrap_or_else(|| {
                        panic!("missing key {key} at {path}; actual object: {actual}")
                    });
                    assert_json_subset(expected_value, actual_value, &child_path);
                }
            }
            Value::Array(expected_items) => {
                let actual_items = actual
                    .as_array()
                    .unwrap_or_else(|| panic!("expected array at {path}, found {actual}"));
                assert_eq!(
                    expected_items.len(),
                    actual_items.len(),
                    "array length mismatch at {path}"
                );
                for (index, expected_value) in expected_items.iter().enumerate() {
                    assert_json_subset(
                        expected_value,
                        &actual_items[index],
                        &format!("{path}[{index}]"),
                    );
                }
            }
            _ => {
                assert_eq!(expected, actual, "value mismatch at {path}");
            }
        }
    }

    fn script_final_snapshot(state: &CadDemoPaneState, total_duration_ms: u64) -> Value {
        let warning_codes = state
            .warnings
            .iter()
            .map(|warning| warning.code.clone())
            .collect::<Vec<_>>();
        let critical_count = state
            .warnings
            .iter()
            .filter(|warning| warning.severity.eq_ignore_ascii_case("critical"))
            .count() as u64;
        json!({
            "final_state_revision": state.document_revision,
            "active_variant_id": state.active_variant_id,
            "last_error": state.last_error,
            "receipts": state.rebuild_receipts.iter().map(receipt_json).collect::<Vec<_>>(),
            "warnings": {
                "count": state.warnings.len(),
                "critical_count": critical_count,
                "codes": warning_codes,
            },
            "analysis": {
                "document_revision": state.analysis_snapshot.document_revision,
                "variant_id": state.analysis_snapshot.variant_id,
                "material_id": state.analysis_snapshot.material_id,
                "volume_mm3": state.analysis_snapshot.volume_mm3,
                "mass_kg": state.analysis_snapshot.mass_kg,
                "estimated_cost_usd": state.analysis_snapshot.estimated_cost_usd,
                "max_deflection_mm": state.analysis_snapshot.max_deflection_mm,
            },
            "timing": {
                "total_duration_ms": total_duration_ms,
            }
        })
    }

    fn run_headless_cad_script_fixture(name: &str) -> Value {
        let script = load_script_fixture(name);
        let root = required_object(&script, "script");
        let script_id = required_str(root, "script_id", "script");
        let seed = optional_u64(root, "seed").unwrap_or(0);
        let thread_id = root
            .get("thread_id")
            .and_then(Value::as_str)
            .unwrap_or("thread.cad-script")
            .to_string();
        let timing_cfg = root.get("timing").map(|value| required_object(value, "script.timing"));
        let steps = root
            .get("steps")
            .map(|value| required_array(value, "script.steps"))
            .unwrap_or_else(|| panic!("script {script_id} must include steps"));

        let mut state = CadDemoPaneState::default();
        let mut step_reports = Vec::<Value>::new();
        let started = Instant::now();

        for (index, step_value) in steps.iter().enumerate() {
            let step_path = format!("script.steps[{index}]");
            let step = required_object(step_value, &step_path);
            let kind = required_str(step, "kind", &step_path);
            let name = step
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or(kind.as_str())
                .to_string();
            let step_started = Instant::now();

            let result = match kind.as_str() {
                "intent_json" => {
                    let payload = required_str(step, "payload", &step_path);
                    match parse_cad_intent_json(&payload) {
                        Ok(intent) => match state.apply_chat_intent_for_thread(&thread_id, &intent) {
                            Ok(receipt) => json!({
                                "status": "applied",
                                "intent": intent.intent_name(),
                                "state_revision": receipt.state_revision,
                                "session_id": state.session_id,
                            }),
                            Err(error) => json!({
                                "status": "rejected_dispatch",
                                "error": error.to_string(),
                            }),
                        },
                        Err(error) => json!({
                            "status": "rejected_parse",
                            "error_code": error.code,
                            "error_message": error.message,
                        }),
                    }
                }
                "cycle_variant" => {
                    let mut count = optional_u64(step, "count").unwrap_or(1);
                    if let Some(randomized) = step.get("randomized") {
                        let randomized = required_object(randomized, &format!("{step_path}.randomized"));
                        let min = required_u64(randomized, "min", &format!("{step_path}.randomized"));
                        let max = required_u64(randomized, "max", &format!("{step_path}.randomized"));
                        count = deterministic_seeded_count(seed, index, min, max);
                    }
                    for _ in 0..count {
                        assert!(
                            apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant),
                            "cycle variant step should mutate state"
                        );
                        wait_for_receipt(&mut state);
                    }
                    let warning_codes = state
                        .warnings
                        .iter()
                        .map(|warning| warning.code.clone())
                        .collect::<Vec<_>>();
                    json!({
                        "status": "cycled",
                        "count": count,
                        "state_revision": state.document_revision,
                        "active_variant_id": state.active_variant_id,
                        "last_receipt": state.last_rebuild_receipt.as_ref().map(receipt_json),
                        "warning_codes": warning_codes,
                    })
                }
                "inject_rebuild_failure" => {
                    let request_id = optional_u64(step, "request_id")
                        .or(state.pending_rebuild_request_id)
                        .unwrap_or_else(|| 9000 + index as u64);
                    let trigger = step
                        .get("trigger")
                        .and_then(Value::as_str)
                        .unwrap_or("script-injected-failure")
                        .to_string();
                    let error = required_str(step, "error", &step_path);
                    let variant_id = step
                        .get("variant_id")
                        .and_then(Value::as_str)
                        .unwrap_or(state.active_variant_id.as_str())
                        .to_string();
                    let document_revision = optional_u64(step, "document_revision")
                        .unwrap_or(state.document_revision);
                    state.pending_rebuild_request_id = Some(request_id);
                    let failed = CadRebuildFailed {
                        request_id,
                        trigger,
                        session_id: state.session_id.clone(),
                        document_revision,
                        variant_id,
                        error: error.clone(),
                    };
                    let _ = apply_rebuild_response(&mut state, CadRebuildResponse::Failed(failed));
                    json!({
                        "status": "failure_injected",
                        "request_id": request_id,
                        "state_revision": state.document_revision,
                        "last_error": state.last_error,
                        "load_state": format!("{:?}", state.load_state),
                    })
                }
                "assert_warning_escalation" => {
                    let min_warning_count = required_u64(step, "min_warning_count", &step_path);
                    let min_critical_count = required_u64(step, "min_critical_count", &step_path);
                    let warning_count = state.warnings.len() as u64;
                    let critical_count = state
                        .warnings
                        .iter()
                        .filter(|warning| warning.severity.eq_ignore_ascii_case("critical"))
                        .count() as u64;
                    assert!(
                        warning_count >= min_warning_count,
                        "warning escalation failed: warning_count {} < min_warning_count {}",
                        warning_count,
                        min_warning_count
                    );
                    assert!(
                        critical_count >= min_critical_count,
                        "warning escalation failed: critical_count {} < min_critical_count {}",
                        critical_count,
                        min_critical_count
                    );
                    json!({
                        "status": "asserted",
                        "warning_count": warning_count,
                        "critical_count": critical_count,
                    })
                }
                other => panic!("unsupported script step kind '{other}' at {step_path}"),
            };

            let step_duration_ms = step_started.elapsed().as_millis() as u64;
            if let Some(max_step_duration_ms) = optional_u64(step, "max_duration_ms") {
                assert!(
                    step_duration_ms <= max_step_duration_ms,
                    "step '{}' exceeded max_duration_ms {} with {}",
                    name,
                    max_step_duration_ms,
                    step_duration_ms
                );
            }
            if let Some(expected) = step.get("expect") {
                assert_json_subset(
                    expected,
                    &result,
                    &format!("script.step_result[{}:{}]", index, name),
                );
            }
            step_reports.push(json!({
                "index": index,
                "name": name,
                "kind": kind,
                "duration_ms": step_duration_ms,
                "result": result,
            }));
        }

        let total_duration_ms = started.elapsed().as_millis() as u64;
        if let Some(timing_cfg) = timing_cfg {
            if let Some(max_total_duration_ms) = optional_u64(timing_cfg, "max_total_duration_ms") {
                assert!(
                    total_duration_ms <= max_total_duration_ms,
                    "script '{}' exceeded total timing budget {} with {}",
                    script_id,
                    max_total_duration_ms,
                    total_duration_ms
                );
            }
        }

        let final_snapshot = script_final_snapshot(&state, total_duration_ms);
        if let Some(expected) = root.get("expect") {
            assert_json_subset(expected, &final_snapshot, "script.expect");
        }

        json!({
            "script_id": script_id,
            "seed": seed,
            "thread_id": thread_id,
            "steps": step_reports,
            "final": final_snapshot,
        })
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
    fn bootstrap_demo_action_is_idempotent_and_reset_alias_compatible() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        state.camera_zoom = 2.7;
        state.camera_pan_x = 132.0;
        state.camera_pan_y = -92.0;
        state.camera_orbit_yaw_deg = 71.0;
        state.camera_orbit_pitch_deg = -28.0;
        state.focused_geometry_ref = Some("cad://feature/feature.custom".to_string());
        state.warning_filter_code = "cad.test.warning".to_string();
        state.warning_filter_severity = "critical".to_string();

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::BootstrapDemo
        ));
        let first_signature = bootstrap_signature(&state);
        assert_eq!(state.load_state, crate::app_state::PaneLoadState::Loading);
        assert_eq!(state.pending_rebuild_request_id, Some(1));
        assert_eq!(state.document_revision, 0);
        assert_eq!(state.active_variant_id, "variant.baseline");
        assert_eq!(state.warning_filter_code, "all");
        assert_eq!(state.warning_filter_severity, "all");

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::BootstrapDemo
        ));
        let second_signature = bootstrap_signature(&state);
        assert_eq!(
            first_signature, second_signature,
            "bootstrap action must be idempotent"
        );

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ResetSession
        ));
        let reset_signature = bootstrap_signature(&state);
        assert_eq!(
            first_signature, reset_signature,
            "legacy reset action must remain equivalent to bootstrap"
        );
    }

    #[test]
    fn step_export_rejects_when_mesh_is_missing() {
        let state = CadDemoPaneState::default();
        let error = run_step_export_from_active_mesh(&state, "variant.baseline")
            .expect_err("export without mesh should fail");
        assert!(error.to_string().contains("no mesh payload available"));
    }

    #[test]
    fn step_export_succeeds_after_rebuild_receipt() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);
        let variant = state.active_variant_id.clone();
        let artifact = run_step_export_from_active_mesh(&state, &variant)
            .expect("export should succeed after rebuild receipt");
        assert_eq!(artifact.receipt.variant_id, variant);
        assert_eq!(artifact.receipt.document_id, state.document_id);
        assert!(artifact.receipt.byte_count > 0);
        assert!(!artifact.receipt.deterministic_hash.is_empty());
    }

    #[test]
    fn dimension_commit_queues_rebuild_and_updates_value() {
        let mut state = CadDemoPaneState::default();
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::StartDimensionEdit(0)
        ));
        for _ in 0..16 {
            let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::DimensionInputBackspace);
        }
        for ch in ['4', '2', '1', '.', '0'] {
            assert!(apply_cad_demo_action(
                &mut state,
                CadDemoPaneAction::DimensionInputChar(ch)
            ));
        }
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::DimensionInputCommit
        ));
        assert_eq!(state.dimension_value_mm("width_mm"), Some(421.0));
        assert_eq!(state.document_revision, 1);
        assert_eq!(state.load_state, crate::app_state::PaneLoadState::Loading);
        wait_for_receipt(&mut state);
        assert_eq!(state.load_state, crate::app_state::PaneLoadState::Ready);
    }

    #[test]
    fn hidden_line_mode_cycles_deterministically() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(state.hidden_line_mode.label(), "shaded");

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleHiddenLineMode
        ));
        assert_eq!(state.hidden_line_mode.label(), "shaded+edges");

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleHiddenLineMode
        ));
        assert_eq!(state.hidden_line_mode.label(), "wireframe");

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleHiddenLineMode
        ));
        assert_eq!(state.hidden_line_mode.label(), "shaded");
    }

    #[test]
    fn reset_camera_restores_default_pose() {
        let mut state = CadDemoPaneState::default();
        state.camera_zoom = 2.4;
        state.camera_pan_x = 220.0;
        state.camera_pan_y = -180.0;
        state.camera_orbit_yaw_deg = 71.0;
        state.camera_orbit_pitch_deg = -32.0;
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ResetCamera
        ));
        assert_eq!(state.camera_zoom, 1.0);
        assert_eq!(state.camera_pan_x, 0.0);
        assert_eq!(state.camera_pan_y, 0.0);
        assert_eq!(state.camera_orbit_yaw_deg, 26.0);
        assert_eq!(state.camera_orbit_pitch_deg, 18.0);
    }

    #[test]
    fn snap_view_actions_set_expected_camera_pose() {
        let mut state = CadDemoPaneState::default();
        state.camera_pan_x = 140.0;
        state.camera_pan_y = -92.0;

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::SnapViewTop
        ));
        assert_eq!(state.camera_orbit_yaw_deg, 0.0);
        assert_eq!(state.camera_orbit_pitch_deg, 89.0);
        assert_eq!(state.camera_pan_x, 0.0);
        assert_eq!(state.camera_pan_y, 0.0);

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::SnapViewFront
        ));
        assert_eq!(state.camera_orbit_yaw_deg, 0.0);
        assert_eq!(state.camera_orbit_pitch_deg, 0.0);

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::SnapViewRight
        ));
        assert_eq!(state.camera_orbit_yaw_deg, 90.0);
        assert_eq!(state.camera_orbit_pitch_deg, 0.0);

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::SnapViewIsometric
        ));
        assert_eq!(state.camera_orbit_yaw_deg, 45.0);
        assert_eq!(state.camera_orbit_pitch_deg, 35.264);
        assert_eq!(
            state.active_view_snap(),
            Some(crate::app_state::CadCameraViewSnap::Isometric)
        );
    }

    #[test]
    fn toggle_projection_mode_cycles_between_ortho_and_perspective() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(
            state.projection_mode,
            crate::app_state::CadProjectionMode::Orthographic
        );

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleProjectionMode
        ));
        assert_eq!(
            state.projection_mode,
            crate::app_state::CadProjectionMode::Perspective
        );

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleProjectionMode
        ));
        assert_eq!(
            state.projection_mode,
            crate::app_state::CadProjectionMode::Orthographic
        );
    }

    #[test]
    fn section_plane_and_offset_controls_cycle_deterministically() {
        let mut state = CadDemoPaneState::default();
        assert!(state.section_axis.is_none());
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleSectionPlane
        ));
        assert_eq!(state.section_summary(), "x/0");
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::StepSectionPlaneOffset
        ));
        assert_eq!(state.section_offset_normalized, 0.2);
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleSectionPlane
        ));
        assert_eq!(state.section_summary(), "y/0.2");
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleSectionPlane
        ));
        assert_eq!(state.section_summary(), "z/0.2");
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleSectionPlane
        ));
        assert_eq!(state.section_summary(), "off");
    }

    #[test]
    fn cycling_material_recomputes_mass_and_cost_paths() {
        let mut state = CadDemoPaneState::default();
        let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::CycleVariant);
        wait_for_receipt(&mut state);

        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-6061-t6")
        );
        let first_mass = state
            .analysis_snapshot
            .mass_kg
            .expect("mass should exist after rebuild");
        let first_cost = state
            .analysis_snapshot
            .estimated_cost_usd
            .expect("cost should exist after rebuild");
        assert_eq!(
            state
                .analysis_snapshot
                .estimator_metadata
                .get("model_id")
                .map(String::as_str),
            Some(openagents_cad::materials::CAD_COST_HEURISTIC_MODEL_ID)
        );
        assert!(
            state
                .analysis_snapshot
                .estimator_metadata
                .contains_key("assumption.machine_rate_usd_per_min"),
            "cost metadata should expose estimator assumptions"
        );
        assert!(
            state
                .analysis_snapshot
                .estimator_metadata
                .contains_key("deflection.confidence"),
            "deflection metadata should expose confidence label"
        );
        assert!(
            state.analysis_snapshot.max_deflection_mm.unwrap_or(0.0) > 0.0,
            "deflection heuristic should compute deterministic value"
        );

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleMaterialPreset
        ));
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("al-5052-h32")
        );
        let second_mass = state
            .analysis_snapshot
            .mass_kg
            .expect("mass should remain available after material change");
        let second_cost = state
            .analysis_snapshot
            .estimated_cost_usd
            .expect("cost should remain available after material change");
        assert_ne!(first_mass, second_mass);
        assert_ne!(first_cost, second_cost);

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleMaterialPreset
        ));
        assert_eq!(
            state.analysis_snapshot.material_id.as_deref(),
            Some("steel-1018")
        );
        let steel_mass = state
            .analysis_snapshot
            .mass_kg
            .expect("steel mass should be computed");
        assert!(
            steel_mass > second_mass,
            "steel density should produce larger mass for same volume"
        );
        assert!(
            state.analysis_snapshot.estimated_cost_usd.unwrap_or(0.0) > 0.0,
            "material assignment should keep deterministic cost estimate available"
        );
    }

    #[test]
    fn snap_toggle_actions_flip_state_deterministically() {
        let mut state = CadDemoPaneState::default();
        assert!(state.snap_toggles.grid);
        assert!(state.snap_toggles.origin);
        assert!(!state.snap_toggles.endpoint);
        assert!(!state.snap_toggles.midpoint);

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleSnapGrid
        ));
        assert!(!state.snap_toggles.grid);
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleSnapEndpoint
        ));
        assert!(state.snap_toggles.endpoint);
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleSnapMidpoint
        ));
        assert!(state.snap_toggles.midpoint);
    }

    #[test]
    fn cycle_hotkey_profile_action_updates_profile_with_conflict_checks() {
        let mut state = CadDemoPaneState::default();
        assert_eq!(state.hotkey_profile, "default");
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleHotkeyProfile
        ));
        assert_eq!(state.hotkey_profile, "compact");
        assert_eq!(state.hotkeys.snap_top, "7");
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleHotkeyProfile
        ));
        assert_eq!(state.hotkey_profile, "default");
        assert_eq!(state.hotkeys.snap_top, "t");
    }

    #[test]
    fn three_d_mouse_profile_mode_and_axis_locks_toggle_deterministically() {
        let mut state = CadDemoPaneState::default();
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleThreeDMouseMode
        ));
        assert_eq!(state.three_d_mouse_mode.label(), "rotate");

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleThreeDMouseProfile
        ));
        assert_eq!(state.three_d_mouse_profile.label(), "fast");

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleThreeDMouseLockRx
        ));
        assert!(state.three_d_mouse_axis_locks.rx);

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::ToggleThreeDMouseLockRx
        ));
        assert!(!state.three_d_mouse_axis_locks.rx);
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
        assert!(
            state.analysis_snapshot.volume_mm3.unwrap_or(0.0) > 0.0,
            "rebuild should compute deterministic volume"
        );
        assert!(
            state.analysis_snapshot.mass_kg.unwrap_or(0.0) > 0.0,
            "rebuild should compute deterministic mass"
        );
        assert!(
            state.analysis_snapshot.center_of_gravity_mm.is_some(),
            "rebuild should compute deterministic center of gravity"
        );
        assert!(
            state.analysis_snapshot.max_deflection_mm.unwrap_or(0.0) > 0.0,
            "rebuild should compute deterministic deflection estimate"
        );
    }

    #[test]
    fn analysis_snapshot_classifies_invalid_mesh_failures_explicitly() {
        let payload = CadMeshPayload {
            mesh_id: "mesh.invalid.analysis".to_string(),
            document_revision: 7,
            variant_id: "variant.baseline".to_string(),
            topology: CadMeshTopology::Triangles,
            vertices: vec![CadMeshVertex {
                position_mm: [0.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0, 0.0],
                material_slot: 0,
                flags: 0,
            }],
            triangle_indices: vec![0, 1, 2],
            edges: Vec::new(),
            material_slots: vec![CadMeshMaterialSlot::default()],
            bounds: CadMeshBounds {
                min_mm: [0.0, 0.0, 0.0],
                max_mm: [0.0, 0.0, 0.0],
            },
        };
        let result = analysis_snapshot_from_mesh(7, "variant.baseline", &payload, "al-6061-t6");
        assert!(result.snapshot.volume_mm3.is_none());
        let error = result.error.expect("analysis error should be surfaced");
        assert_eq!(error.stable_code(), "CAD-ANALYSIS-MISSING-VERTEX");
        assert!(!error.remediation_hint().is_empty());
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

    #[test]
    fn cad_event_upsert_is_dedupe_safe() {
        let mut state = CadDemoPaneState::default();
        let event = CadEvent::new_with_key(
            CadEventKind::ParameterUpdated,
            state.session_id.clone(),
            state.document_id.clone(),
            4,
            Some("variant.baseline".to_string()),
            "CAD parameter updated",
            "width_base_mm -> 192".to_string(),
            Some("param:width_base_mm:4".to_string()),
        );
        assert!(state.upsert_cad_event(event.clone()));
        let baseline = state.cad_events.len();
        assert!(!state.upsert_cad_event(event));
        assert_eq!(
            state.cad_events.len(),
            baseline,
            "same logical event id should dedupe"
        );
    }

    #[test]
    fn cad_event_maps_to_cad_activity_row() {
        let event = CadEvent::new_with_key(
            CadEventKind::SelectionChanged,
            "cad.session.local",
            "cad.doc.demo-rack",
            9,
            Some("variant.stiffness".to_string()),
            "CAD selection changed",
            "focused=cad://feature/feature.base".to_string(),
            Some("selection:9".to_string()),
        );
        let row = activity_row_from_cad_event(&event);
        assert_eq!(row.domain, ActivityEventDomain::Cad);
        assert!(row.source_tag.starts_with("cad.selection.changed"));
        assert!(row.detail.contains("variant.stiffness"));
    }

    #[test]
    fn follow_up_parameter_edit_interaction_matches_golden_receipts() {
        let mut state = CadDemoPaneState::default();
        let prompt = "Select rack_outer_face";
        let intent = match translate_chat_to_cad_intent(prompt) {
            CadIntentTranslationOutcome::Intent(intent) => intent,
            CadIntentTranslationOutcome::ParseFailure(error) => {
                panic!("prompt translation should succeed: {error:?}")
            }
        };
        let prompt_state_revision = state
            .apply_chat_intent_for_thread("thread.followup", &intent)
            .expect("prompt intent should apply")
            .state_revision;
        assert_eq!(prompt_state_revision, 1);
        assert_eq!(state.last_chat_intent_name.as_deref(), Some("Select"));
        assert_eq!(state.session_id, "cad.session.chat.thread-followup");

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::CycleVariant
        ));
        wait_for_receipt(&mut state);
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::SelectTimelineRow(0)
        ));
        assert_eq!(state.timeline_selected_index, Some(0));
        assert!(state.focused_geometry_ref.is_some());

        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::StartDimensionEdit(0)
        ));
        for _ in 0..16 {
            let _ = apply_cad_demo_action(&mut state, CadDemoPaneAction::DimensionInputBackspace);
        }
        for ch in ['4', '2', '1', '.', '0'] {
            assert!(apply_cad_demo_action(
                &mut state,
                CadDemoPaneAction::DimensionInputChar(ch)
            ));
        }
        assert!(apply_cad_demo_action(
            &mut state,
            CadDemoPaneAction::DimensionInputCommit
        ));
        wait_for_receipt(&mut state);

        assert_eq!(state.dimension_value_mm("width_mm"), Some(421.0));
        assert_eq!(state.analysis_snapshot.document_revision, state.document_revision);
        assert_eq!(state.analysis_snapshot.variant_id, state.active_variant_id);
        assert!(
            !state.warnings.is_empty(),
            "interaction should keep deterministic warning output"
        );

        let actual = interaction_snapshot(&state, prompt, "Select", prompt_state_revision);
        let actual_json = serde_json::to_string_pretty(&actual)
            .expect("interaction snapshot should serialize deterministically");
        let fixture_path = interaction_fixture_path();
        if std::env::var("CAD_UPDATE_GOLDENS").as_deref() == Ok("1") {
            if let Some(parent) = std::path::Path::new(&fixture_path).parent() {
                fs::create_dir_all(parent).expect("fixture parent directory should exist");
            }
            fs::write(&fixture_path, actual_json).expect("fixture should write");
            return;
        }

        let expected_json = fs::read_to_string(&fixture_path).unwrap_or_else(|error| {
            panic!(
                "missing interaction fixture {fixture_path}: {error}\nset CAD_UPDATE_GOLDENS=1 to regenerate.\nactual snapshot:\n{actual_json}"
            )
        });
        let expected =
            serde_json::from_str::<Value>(&expected_json).expect("fixture should parse as JSON");
        if expected != actual {
            let diff = interaction_semantic_diff(&expected, &actual);
            panic!(
                "follow-up interaction snapshot mismatch against {fixture_path}\nsemantic diff:\n{diff}\n\nactual snapshot:\n{actual_json}"
            );
        }
    }

    #[test]
    fn cad_headless_script_harness_runs_canonical_demo_script() {
        let report = run_headless_cad_script_fixture("cad_demo_canonical_script.json");
        let final_state = report
            .get("final")
            .expect("canonical script report should include final snapshot");
        assert_eq!(
            final_state
                .get("final_state_revision")
                .and_then(Value::as_u64),
            Some(3),
            "canonical script should end at deterministic revision"
        );
    }

    #[test]
    fn cad_headless_script_harness_supports_failure_path_scripts() {
        let report = run_headless_cad_script_fixture("cad_demo_failure_paths_script.json");
        let final_state = report
            .get("final")
            .expect("failure script report should include final snapshot");
        assert!(
            final_state
                .get("warnings")
                .and_then(|warnings| warnings.get("critical_count"))
                .and_then(Value::as_u64)
                .unwrap_or_default()
                >= 1,
            "failure-path script should preserve critical warning coverage"
        );
    }

    #[test]
    fn cad_release_gate_reliability_reuses_canonical_script_fixture() {
        let report = run_headless_cad_script_fixture("cad_demo_canonical_script.json");
        let step_count = report
            .get("steps")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or_default();
        assert!(
            step_count >= 3,
            "canonical script fixture should remain non-trivial for reliability checks"
        );
    }
}
