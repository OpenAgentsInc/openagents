use crate::app_state::{PaneLoadState, RenderState};
use crate::local_inference_runtime::{
    LocalInferenceExecutionCompleted, LocalInferenceExecutionFailed,
    LocalInferenceExecutionStarted, LocalInferenceRuntimeSnapshot, LocalInferenceRuntimeUpdate,
};

const OUTPUT_PREVIEW_CHAR_LIMIT: usize = 480;

pub(super) fn apply_runtime_update(
    state: &mut RenderState,
    update: &LocalInferenceRuntimeUpdate,
) -> bool {
    match update {
        LocalInferenceRuntimeUpdate::Snapshot(snapshot) => apply_snapshot(state, snapshot),
        LocalInferenceRuntimeUpdate::Started(started) => apply_started(state, started),
        LocalInferenceRuntimeUpdate::Completed(completed) => apply_completed(state, completed),
        LocalInferenceRuntimeUpdate::Failed(failed) => apply_failed(state, failed),
    }
}

fn apply_snapshot(state: &mut RenderState, snapshot: &LocalInferenceRuntimeSnapshot) -> bool {
    if state.local_inference.pending_request_id.is_some() {
        state.local_inference.load_state = PaneLoadState::Loading;
    } else {
        state.local_inference.load_state = snapshot_load_state(snapshot);
    }

    if let Some(action) = snapshot.last_action.as_ref() {
        state.local_inference.last_action = Some(action.clone());
    }
    state.local_inference.last_error = snapshot.last_error.clone();
    if let Some(request_id) = snapshot.last_request_id.as_ref() {
        state.local_inference.last_request_id = Some(request_id.clone());
    }
    if let Some(metrics) = snapshot.last_metrics.as_ref() {
        state.local_inference.last_metrics = Some(metrics.clone());
    }
    true
}

fn apply_started(state: &mut RenderState, started: &LocalInferenceExecutionStarted) -> bool {
    if !pane_owns_request(state, started.request_id.as_str()) {
        return false;
    }
    state.local_inference.load_state = PaneLoadState::Loading;
    state.local_inference.last_error = None;
    state.local_inference.last_action = Some(format!(
        "Workbench prompt started request={} model={}",
        started.request_id, started.model
    ));
    state.local_inference.last_request_id = Some(started.request_id.clone());
    state.local_inference.last_model = Some(started.model.clone());
    true
}

fn apply_completed(state: &mut RenderState, completed: &LocalInferenceExecutionCompleted) -> bool {
    if !pane_owns_request(state, completed.request_id.as_str()) {
        return false;
    }
    state.local_inference.pending_request_id = None;
    state.local_inference.load_state = PaneLoadState::Ready;
    state.local_inference.last_error = None;
    state.local_inference.last_action = Some(format!(
        "Workbench prompt completed request={} model={}",
        completed.request_id, completed.model
    ));
    state.local_inference.last_request_id = Some(completed.request_id.clone());
    state.local_inference.last_model = Some(completed.model.clone());
    state.local_inference.last_metrics = Some(completed.metrics.clone());
    state.local_inference.last_provenance = Some(completed.provenance.clone());
    state.local_inference.output_chars = completed.output.chars().count();
    state.local_inference.output_preview =
        truncate_output_preview(completed.output.as_str(), OUTPUT_PREVIEW_CHAR_LIMIT);
    true
}

fn apply_failed(state: &mut RenderState, failed: &LocalInferenceExecutionFailed) -> bool {
    if !pane_owns_request(state, failed.request_id.as_str()) {
        return false;
    }
    state.local_inference.pending_request_id = None;
    state.local_inference.load_state = PaneLoadState::Error;
    state.local_inference.last_action = Some(format!(
        "Workbench prompt failed request={}",
        failed.request_id
    ));
    state.local_inference.last_error = Some(failed.error.clone());
    state.local_inference.last_request_id = Some(failed.request_id.clone());
    true
}

fn pane_owns_request(state: &RenderState, request_id: &str) -> bool {
    state.local_inference.pending_request_id.as_deref() == Some(request_id)
}

fn snapshot_load_state(snapshot: &LocalInferenceRuntimeSnapshot) -> PaneLoadState {
    if snapshot.last_error.is_some() {
        PaneLoadState::Error
    } else if snapshot.reachable {
        PaneLoadState::Ready
    } else {
        PaneLoadState::Loading
    }
}

fn truncate_output_preview(raw: &str, limit: usize) -> String {
    let mut preview = String::new();
    let mut count = 0usize;
    for ch in raw.chars() {
        if count >= limit {
            break;
        }
        preview.push(ch);
        count = count.saturating_add(1);
    }
    preview.trim().to_string()
}
