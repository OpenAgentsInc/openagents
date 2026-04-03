use openagents_provider_substrate::{
    ProviderPooledInferenceAvailability, ProviderPooledInferenceTargetStatus,
};

use crate::desktop_control::{
    DesktopControlPooledInferenceStatus, current_pooled_inference_status,
};

pub(crate) fn current_pooled_inference_availability() -> ProviderPooledInferenceAvailability {
    pooled_inference_availability_from_status(&current_pooled_inference_status())
}

pub(crate) fn pooled_inference_availability_from_status(
    status: &DesktopControlPooledInferenceStatus,
) -> ProviderPooledInferenceAvailability {
    ProviderPooledInferenceAvailability {
        available: status.available,
        source: status.source.clone(),
        management_base_url: status.management_base_url.clone(),
        topology_digest: status.topology_digest.clone(),
        default_model: status.default_model.clone(),
        membership_state: status.membership_state.clone(),
        member_count: status.member_count,
        warm_replica_count: status.warm_replica_count,
        local_worker_id: status.local_worker_id.clone(),
        local_serving_state: status.local_serving_state.clone(),
        served_mesh_role: status.served_mesh_role.clone(),
        served_mesh_posture: status.served_mesh_posture.clone(),
        execution_mode: status.execution_mode.clone(),
        execution_engine: status.execution_engine.clone(),
        fallback_posture: status.fallback_posture.clone(),
        last_error: status.last_error.clone(),
        targetable_models: status
            .targetable_models
            .iter()
            .map(|target| ProviderPooledInferenceTargetStatus {
                model: target.model.clone(),
                family: target.family.clone(),
                supported_endpoints: target.supported_endpoints.clone(),
                structured_outputs: target.structured_outputs,
                tool_calling: target.tool_calling,
                response_state: target.response_state,
                warm_replica_count: target.warm_replica_count,
                local_warm_replica: target.local_warm_replica,
            })
            .collect(),
    }
}
