use crate::app_state::{
    MissionControlLocalRuntimeAction, MissionControlLocalRuntimeLane, PaneKind,
};
use crate::local_inference_runtime::LocalInferenceExecutionSnapshot;
use crate::pane_system::{AppleFmWorkbenchPaneAction, LocalInferencePaneAction};
use crate::state::provider_runtime::ProviderRuntimeState;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalRuntimeWorkbenchAction {
    AppleFm(AppleFmWorkbenchPaneAction),
    GptOss(LocalInferencePaneAction),
}

impl LocalRuntimeWorkbenchAction {
    pub(crate) const fn pane_kind(self) -> PaneKind {
        match self {
            Self::AppleFm(_) => PaneKind::AppleFmWorkbench,
            Self::GptOss(_) => PaneKind::LocalInference,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalRuntimeCapabilitySurface {
    pub lane: Option<MissionControlLocalRuntimeLane>,
    pub runtime_label: &'static str,
    pub workbench_label: &'static str,
    pub ready: bool,
    pub supports_sell_compute: bool,
    pub supports_run_text: bool,
    pub supports_streaming: bool,
    pub supports_structured: bool,
    pub supports_model_management: bool,
    pub supports_sessions: bool,
    pub refresh_action: Option<LocalRuntimeWorkbenchAction>,
    pub run_text_action: Option<LocalRuntimeWorkbenchAction>,
    pub run_stream_action: Option<LocalRuntimeWorkbenchAction>,
    pub run_structured_action: Option<LocalRuntimeWorkbenchAction>,
    pub warm_model_action: Option<LocalRuntimeWorkbenchAction>,
    pub unload_model_action: Option<LocalRuntimeWorkbenchAction>,
}

impl Default for LocalRuntimeCapabilitySurface {
    fn default() -> Self {
        Self {
            lane: None,
            runtime_label: "No local runtime",
            workbench_label: "Local runtime",
            ready: false,
            supports_sell_compute: false,
            supports_run_text: false,
            supports_streaming: false,
            supports_structured: false,
            supports_model_management: false,
            supports_sessions: false,
            refresh_action: None,
            run_text_action: None,
            run_stream_action: None,
            run_structured_action: None,
            warm_model_action: None,
            unload_model_action: None,
        }
    }
}

impl LocalRuntimeCapabilitySurface {
    pub(crate) const fn workbench_pane_kind(&self) -> Option<PaneKind> {
        match self.lane {
            Some(MissionControlLocalRuntimeLane::AppleFoundationModels) => {
                Some(PaneKind::AppleFmWorkbench)
            }
            Some(MissionControlLocalRuntimeLane::GptOss) => Some(PaneKind::LocalInference),
            None => None,
        }
    }
}

pub(crate) fn active_local_runtime_capability_surface(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> LocalRuntimeCapabilitySurface {
    match crate::app_state::mission_control_local_runtime_policy(
        desktop_shell_mode,
        local_inference_runtime,
    )
    .local_runtime_lane()
    {
        Some(lane) => local_runtime_capability_surface_for_lane(
            lane,
            provider_runtime,
            local_inference_runtime,
        ),
        None => LocalRuntimeCapabilitySurface::default(),
    }
}

pub(crate) fn local_runtime_capability_surface_for_lane(
    lane: MissionControlLocalRuntimeLane,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> LocalRuntimeCapabilitySurface {
    match lane {
        MissionControlLocalRuntimeLane::AppleFoundationModels => LocalRuntimeCapabilitySurface {
            lane: Some(lane),
            runtime_label: "Apple FM",
            workbench_label: "Apple FM workbench",
            ready: provider_runtime.apple_fm.is_ready(),
            supports_sell_compute: true,
            supports_run_text: true,
            supports_streaming: true,
            supports_structured: true,
            supports_model_management: false,
            supports_sessions: true,
            refresh_action: Some(LocalRuntimeWorkbenchAction::AppleFm(
                if provider_runtime.apple_fm.reachable {
                    AppleFmWorkbenchPaneAction::RefreshBridge
                } else {
                    AppleFmWorkbenchPaneAction::StartBridge
                },
            )),
            run_text_action: Some(LocalRuntimeWorkbenchAction::AppleFm(
                AppleFmWorkbenchPaneAction::RunText,
            )),
            run_stream_action: Some(LocalRuntimeWorkbenchAction::AppleFm(
                AppleFmWorkbenchPaneAction::RunStream,
            )),
            run_structured_action: Some(LocalRuntimeWorkbenchAction::AppleFm(
                AppleFmWorkbenchPaneAction::RunStructured,
            )),
            warm_model_action: None,
            unload_model_action: None,
        },
        MissionControlLocalRuntimeLane::GptOss => {
            let backend = local_inference_runtime
                .backend_label
                .trim()
                .to_ascii_lowercase();
            LocalRuntimeCapabilitySurface {
                lane: Some(lane),
                runtime_label: "GPT-OSS",
                workbench_label: "GPT-OSS workbench",
                ready: local_inference_runtime.is_ready(),
                supports_sell_compute: backend == "cuda",
                supports_run_text: true,
                supports_streaming: false,
                supports_structured: false,
                supports_model_management: true,
                supports_sessions: false,
                refresh_action: Some(LocalRuntimeWorkbenchAction::GptOss(
                    LocalInferencePaneAction::RefreshRuntime,
                )),
                run_text_action: Some(LocalRuntimeWorkbenchAction::GptOss(
                    LocalInferencePaneAction::RunPrompt,
                )),
                run_stream_action: None,
                run_structured_action: None,
                warm_model_action: Some(LocalRuntimeWorkbenchAction::GptOss(
                    LocalInferencePaneAction::WarmModel,
                )),
                unload_model_action: Some(LocalRuntimeWorkbenchAction::GptOss(
                    LocalInferencePaneAction::UnloadModel,
                )),
            }
        }
    }
}

pub(crate) fn local_runtime_capability_summary(surface: &LocalRuntimeCapabilitySurface) -> String {
    if surface.lane.is_none() {
        return "No supported local runtime capabilities detected yet.".to_string();
    }

    let mut capabilities = vec!["text".to_string()];
    if surface.supports_streaming {
        capabilities.push("streaming".to_string());
    }
    if surface.supports_structured {
        capabilities.push("structured".to_string());
    }
    if surface.supports_sessions {
        capabilities.push("sessions".to_string());
    }
    if surface.supports_model_management {
        capabilities.push("model management".to_string());
    }

    format!(
        "{} capabilities: {}.",
        surface.runtime_label,
        capabilities.join(", ")
    )
}

pub(crate) fn mission_control_local_runtime_workbench_action(
    surface: &LocalRuntimeCapabilitySurface,
    action: MissionControlLocalRuntimeAction,
) -> Option<LocalRuntimeWorkbenchAction> {
    match action {
        MissionControlLocalRuntimeAction::StartAppleFm
        | MissionControlLocalRuntimeAction::RefreshAppleFm
        | MissionControlLocalRuntimeAction::RefreshGptOss => surface.refresh_action,
        MissionControlLocalRuntimeAction::WarmGptOss => surface.warm_model_action,
        MissionControlLocalRuntimeAction::UnloadGptOss => surface.unload_model_action,
        MissionControlLocalRuntimeAction::OpenAppleFmWorkbench
        | MissionControlLocalRuntimeAction::OpenGptOssWorkbench
        | MissionControlLocalRuntimeAction::None => None,
    }
}

pub(crate) fn mission_control_preflight_workbench_action(
    desktop_shell_mode: crate::desktop_shell::DesktopShellMode,
    provider_runtime: &ProviderRuntimeState,
    local_inference_runtime: &LocalInferenceExecutionSnapshot,
) -> Option<LocalRuntimeWorkbenchAction> {
    let surface = active_local_runtime_capability_surface(
        desktop_shell_mode,
        provider_runtime,
        local_inference_runtime,
    );
    match surface.lane {
        Some(MissionControlLocalRuntimeLane::AppleFoundationModels) => {
            let bridge_starting =
                provider_runtime.apple_fm.bridge_status.as_deref() == Some("starting");
            if surface.ready || bridge_starting {
                None
            } else {
                surface.refresh_action
            }
        }
        Some(MissionControlLocalRuntimeLane::GptOss) => {
            if !surface.supports_sell_compute
                || surface.ready
                || local_inference_runtime.busy
                || local_inference_runtime.configured_model_path.is_none()
                || !local_inference_runtime.artifact_present
            {
                None
            } else {
                surface.warm_model_action
            }
        }
        None => None,
    }
}

pub(crate) const fn mission_control_preflight_action_label(
    action: LocalRuntimeWorkbenchAction,
) -> &'static str {
    match action {
        LocalRuntimeWorkbenchAction::AppleFm(
            crate::pane_system::AppleFmWorkbenchPaneAction::RefreshBridge,
        ) => "Queued Apple FM bridge refresh",
        LocalRuntimeWorkbenchAction::AppleFm(
            crate::pane_system::AppleFmWorkbenchPaneAction::StartBridge,
        ) => "Queued Apple FM bridge start",
        LocalRuntimeWorkbenchAction::GptOss(
            crate::pane_system::LocalInferencePaneAction::WarmModel,
        ) => "Queued GPT-OSS model warm",
        LocalRuntimeWorkbenchAction::AppleFm(_) | LocalRuntimeWorkbenchAction::GptOss(_) => {
            "Queued local runtime preflight action"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LocalRuntimeWorkbenchAction, local_runtime_capability_summary,
        local_runtime_capability_surface_for_lane, mission_control_local_runtime_workbench_action,
        mission_control_preflight_action_label, mission_control_preflight_workbench_action,
    };

    #[test]
    fn apple_surface_exposes_streaming_structured_and_session_capabilities() {
        let mut provider = crate::state::provider_runtime::ProviderRuntimeState::default();
        provider.apple_fm.reachable = true;
        provider.apple_fm.model_available = true;
        provider.apple_fm.ready_model = Some("apple-foundation-model".to_string());

        let surface = local_runtime_capability_surface_for_lane(
            crate::app_state::MissionControlLocalRuntimeLane::AppleFoundationModels,
            &provider,
            &crate::local_inference_runtime::LocalInferenceExecutionSnapshot::default(),
        );

        assert!(surface.ready);
        assert!(surface.supports_run_text);
        assert!(surface.supports_streaming);
        assert!(surface.supports_structured);
        assert!(surface.supports_sessions);
        assert!(!surface.supports_model_management);
        assert_eq!(
            surface.refresh_action,
            Some(LocalRuntimeWorkbenchAction::AppleFm(
                crate::pane_system::AppleFmWorkbenchPaneAction::RefreshBridge
            ))
        );
        assert_eq!(
            surface.workbench_pane_kind(),
            Some(crate::app_state::PaneKind::AppleFmWorkbench)
        );
        assert!(local_runtime_capability_summary(&surface).contains("streaming"));
        assert!(local_runtime_capability_summary(&surface).contains("structured"));
        assert!(local_runtime_capability_summary(&surface).contains("sessions"));
        assert_eq!(
            mission_control_local_runtime_workbench_action(
                &surface,
                crate::app_state::MissionControlLocalRuntimeAction::RefreshAppleFm,
            ),
            surface.refresh_action
        );
    }

    #[test]
    fn gpt_oss_surface_exposes_text_and_model_management_capabilities() {
        let local = crate::local_inference_runtime::LocalInferenceExecutionSnapshot {
            reachable: true,
            backend_label: "cuda".to_string(),
            artifact_present: true,
            configured_model_path: Some("/tmp/models/gpt-oss-20b.gguf".to_string()),
            ..crate::local_inference_runtime::LocalInferenceExecutionSnapshot::default()
        };

        let surface = local_runtime_capability_surface_for_lane(
            crate::app_state::MissionControlLocalRuntimeLane::GptOss,
            &crate::state::provider_runtime::ProviderRuntimeState::default(),
            &local,
        );

        assert!(surface.supports_run_text);
        assert!(!surface.supports_streaming);
        assert!(!surface.supports_structured);
        assert!(!surface.supports_sessions);
        assert!(surface.supports_model_management);
        assert!(surface.supports_sell_compute);
        assert_eq!(
            surface.run_text_action,
            Some(LocalRuntimeWorkbenchAction::GptOss(
                crate::pane_system::LocalInferencePaneAction::RunPrompt
            ))
        );
        assert_eq!(
            surface.warm_model_action,
            Some(LocalRuntimeWorkbenchAction::GptOss(
                crate::pane_system::LocalInferencePaneAction::WarmModel
            ))
        );
        assert_eq!(
            surface.workbench_pane_kind(),
            Some(crate::app_state::PaneKind::LocalInference)
        );
        assert!(local_runtime_capability_summary(&surface).contains("model management"));
        assert_eq!(
            mission_control_local_runtime_workbench_action(
                &surface,
                crate::app_state::MissionControlLocalRuntimeAction::WarmGptOss,
            ),
            surface.warm_model_action
        );
    }

    #[test]
    fn preflight_action_warms_configured_gpt_oss_cuda_lane() {
        let local = crate::local_inference_runtime::LocalInferenceExecutionSnapshot {
            reachable: true,
            backend_label: "cuda".to_string(),
            artifact_present: true,
            configured_model_path: Some("/tmp/models/gpt-oss-20b.gguf".to_string()),
            ..crate::local_inference_runtime::LocalInferenceExecutionSnapshot::default()
        };

        let action = mission_control_preflight_workbench_action(
            crate::desktop_shell::DesktopShellMode::Production,
            &crate::state::provider_runtime::ProviderRuntimeState::default(),
            &local,
        );

        assert_eq!(
            action,
            Some(LocalRuntimeWorkbenchAction::GptOss(
                crate::pane_system::LocalInferencePaneAction::WarmModel
            ))
        );
        assert_eq!(
            mission_control_preflight_action_label(action.expect("warm action")),
            "Queued GPT-OSS model warm"
        );
    }

    #[test]
    fn preflight_action_skips_ready_or_unsupported_gpt_oss_hosts() {
        let ready = crate::local_inference_runtime::LocalInferenceExecutionSnapshot {
            reachable: true,
            ready_model: Some("gpt-oss-20b".to_string()),
            backend_label: "cuda".to_string(),
            artifact_present: true,
            configured_model_path: Some("/tmp/models/gpt-oss-20b.gguf".to_string()),
            ..crate::local_inference_runtime::LocalInferenceExecutionSnapshot::default()
        };
        assert_eq!(
            mission_control_preflight_workbench_action(
                crate::desktop_shell::DesktopShellMode::Production,
                &crate::state::provider_runtime::ProviderRuntimeState::default(),
                &ready,
            ),
            None
        );

        let unsupported = crate::local_inference_runtime::LocalInferenceExecutionSnapshot {
            reachable: true,
            backend_label: "metal".to_string(),
            artifact_present: true,
            configured_model_path: Some("/tmp/models/gpt-oss-20b.gguf".to_string()),
            ..crate::local_inference_runtime::LocalInferenceExecutionSnapshot::default()
        };
        assert_eq!(
            mission_control_preflight_workbench_action(
                crate::desktop_shell::DesktopShellMode::Production,
                &crate::state::provider_runtime::ProviderRuntimeState::default(),
                &unsupported,
            ),
            None
        );
    }

    #[test]
    fn preflight_action_starts_apple_bridge_when_lane_is_unreachable() {
        if !cfg!(target_os = "macos") {
            return;
        }

        let provider = crate::state::provider_runtime::ProviderRuntimeState::default();
        let action = mission_control_preflight_workbench_action(
            crate::desktop_shell::DesktopShellMode::Production,
            &provider,
            &crate::local_inference_runtime::LocalInferenceExecutionSnapshot::default(),
        );

        assert_eq!(
            action,
            Some(LocalRuntimeWorkbenchAction::AppleFm(
                crate::pane_system::AppleFmWorkbenchPaneAction::StartBridge
            ))
        );
        assert_eq!(
            mission_control_preflight_action_label(action.expect("apple action")),
            "Queued Apple FM bridge start"
        );
    }
}
