use codex_client::{SkillsListExtraRootsForCwd, SkillsListParams};

use crate::app_state::{PaneLoadState, RenderState};
use crate::codex_lane::CodexLaneCommand;
use crate::pane_system::{SkillRegistryPaneAction, SkillTrustRevocationPaneAction};
use crate::runtime_lanes::{
    RuntimeCommandResponse, RuntimeCommandStatus, SkillTrustTier, SklDiscoveryTrustCommand,
    SklLaneSnapshot,
};

pub(super) fn apply_lane_snapshot(state: &mut RenderState, snapshot: SklLaneSnapshot) {
    state.skl_lane = snapshot;
    sync_skill_pane_snapshots(state);
    state.sync_health.last_applied_event_seq =
        state.sync_health.last_applied_event_seq.saturating_add(1);
    state.sync_health.cursor_last_advanced_seconds_ago = 0;
}

pub(super) fn apply_command_response(
    state: &mut RenderState,
    response: &RuntimeCommandResponse,
    summary: &str,
) {
    state.sync_health.last_action = Some(summary.to_string());
    if response.status != RuntimeCommandStatus::Accepted {
        state.sync_health.last_error = response
            .error
            .as_ref()
            .map(|error| format!("SKL {}: {}", error.class.label(), error.message));
        let error = response.error.as_ref().map_or_else(
            || "SKL lane command rejected".to_string(),
            |err| err.message.clone(),
        );
        state.skill_registry.last_error = Some(error.clone());
        state.skill_registry.load_state = PaneLoadState::Error;
        state.skill_trust_revocation.last_error = Some(error);
        state.skill_trust_revocation.load_state = PaneLoadState::Error;
    }
}

fn sync_skill_pane_snapshots(state: &mut RenderState) {
    state.skill_registry.manifest_a = state.skl_lane.manifest_a.clone();
    state.skill_registry.manifest_event_id = state.skl_lane.manifest_event_id.clone();
    state.skill_registry.version_event_id = state.skl_lane.version_log_event_id.clone();
    state.skill_registry.search_result_event_id = state.skl_lane.search_result_event_id.clone();
    if state.skill_registry.manifest_event_id.is_some()
        || state.skill_registry.search_result_event_id.is_some()
    {
        state.skill_registry.load_state = PaneLoadState::Ready;
    }

    state.skill_trust_revocation.trust_tier = state.skl_lane.trust_tier.label().to_string();
    state.skill_trust_revocation.manifest_a = state.skl_lane.manifest_a.clone();
    state.skill_trust_revocation.kill_switch_active = state.skl_lane.kill_switch_active;
    state.skill_trust_revocation.revocation_event_id = state.skl_lane.revocation_event_id.clone();
    state.skill_trust_revocation.attestation_count =
        trust_tier_attestation_count(state.skl_lane.trust_tier);
    if state.skill_trust_revocation.manifest_a.is_some() {
        state.skill_trust_revocation.load_state = PaneLoadState::Ready;
    }
}

fn trust_tier_attestation_count(trust_tier: SkillTrustTier) -> u32 {
    match trust_tier {
        SkillTrustTier::Unknown => 0,
        SkillTrustTier::Provisional => 1,
        SkillTrustTier::Trusted => 3,
    }
}

pub(super) fn run_skill_registry_action(
    state: &mut RenderState,
    action: SkillRegistryPaneAction,
) -> bool {
    match action {
        SkillRegistryPaneAction::DiscoverSkills => {
            let cwd = match std::env::current_dir() {
                Ok(cwd) => cwd,
                Err(error) => {
                    state.skill_registry.last_error =
                        Some(format!("Failed to resolve cwd for codex skills/list: {error}"));
                    state.skill_registry.load_state = PaneLoadState::Error;
                    return true;
                }
            };
            let repo_skills_root = cwd.join("skills");
            state.skill_registry.repo_skills_root = Some(repo_skills_root.display().to_string());
            state.skill_registry.source = "codex".to_string();

            if !repo_skills_root.is_absolute() {
                state.skill_registry.last_error = Some(
                    "Invalid skills root for codex skills/list: expected absolute path".to_string(),
                );
                state.skill_registry.load_state = PaneLoadState::Error;
                return true;
            }
            if !repo_skills_root.exists() {
                state.skill_registry.last_error = Some(format!(
                    "Codex skills/list root missing: {}",
                    repo_skills_root.display()
                ));
                state.skill_registry.load_state = PaneLoadState::Error;
                return true;
            }

            let params = SkillsListParams {
                cwds: vec![cwd.clone()],
                force_reload: true,
                per_cwd_extra_user_roots: Some(vec![SkillsListExtraRootsForCwd {
                    cwd,
                    extra_user_roots: vec![repo_skills_root],
                }]),
            };

            match state.queue_codex_command(CodexLaneCommand::SkillsList(params)) {
                Ok(command_seq) => {
                    state.skill_registry.last_error = None;
                    state.skill_registry.load_state = PaneLoadState::Ready;
                    state.skill_registry.last_action = Some(format!(
                        "Queued codex skills/list command #{command_seq} (source: codex)"
                    ));
                    state.skill_registry.discovered_skills.clear();
                    state.skill_registry.discovery_errors.clear();
                }
                Err(error) => {
                    state.skill_registry.last_error = Some(error);
                    state.skill_registry.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        SkillRegistryPaneAction::InspectManifest => {
            match state.queue_skl_command(SklDiscoveryTrustCommand::PublishSkillManifest {
                skill_slug: state.skill_registry.manifest_slug.clone(),
                version: state.skill_registry.manifest_version.clone(),
            }) {
                Ok(command_seq) => {
                    state.skill_registry.last_error = None;
                    state.skill_registry.load_state = PaneLoadState::Ready;
                    state.skill_registry.last_action =
                        Some(format!("Queued manifest inspect command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_registry.last_error = Some(error);
                    state.skill_registry.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        SkillRegistryPaneAction::InstallSelectedSkill => {
            match state.queue_skl_command(SklDiscoveryTrustCommand::PublishSkillVersionLog {
                skill_slug: state.skill_registry.manifest_slug.clone(),
                version: state.skill_registry.manifest_version.clone(),
                summary: "installed from skill registry pane".to_string(),
            }) {
                Ok(command_seq) => {
                    state.skill_registry.last_error = None;
                    state.skill_registry.load_state = PaneLoadState::Ready;
                    state.skill_registry.last_action =
                        Some(format!("Queued install command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_registry.last_error = Some(error);
                    state.skill_registry.load_state = PaneLoadState::Error;
                }
            }
            true
        }
    }
}

pub(super) fn run_skill_trust_revocation_action(
    state: &mut RenderState,
    action: SkillTrustRevocationPaneAction,
) -> bool {
    match action {
        SkillTrustRevocationPaneAction::RefreshTrust => {
            let query = state
                .skill_trust_revocation
                .manifest_a
                .clone()
                .unwrap_or_else(|| "skill:trust.refresh".to_string());
            match state
                .queue_skl_command(SklDiscoveryTrustCommand::SubmitSkillSearch { query, limit: 8 })
            {
                Ok(command_seq) => {
                    state.skill_trust_revocation.last_error = None;
                    state.skill_trust_revocation.load_state = PaneLoadState::Ready;
                    state.skill_trust_revocation.last_action =
                        Some(format!("Queued trust refresh command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_trust_revocation.last_error = Some(error);
                    state.skill_trust_revocation.load_state = PaneLoadState::Error;
                }
            }
            true
        }
        SkillTrustRevocationPaneAction::InspectAttestations => {
            let trust_count = match state.skl_lane.trust_tier {
                SkillTrustTier::Unknown => 0,
                SkillTrustTier::Provisional => 1,
                SkillTrustTier::Trusted => 3,
            };
            state.skill_trust_revocation.attestation_count = trust_count;
            state.skill_trust_revocation.last_error = None;
            state.skill_trust_revocation.load_state = PaneLoadState::Ready;
            state.skill_trust_revocation.last_action =
                Some(format!("Loaded {trust_count} trust attestations"));
            true
        }
        SkillTrustRevocationPaneAction::ToggleKillSwitch => {
            state.skill_trust_revocation.kill_switch_active =
                !state.skill_trust_revocation.kill_switch_active;
            state.skill_trust_revocation.trust_tier =
                if state.skill_trust_revocation.kill_switch_active {
                    "revoked".to_string()
                } else {
                    "trusted".to_string()
                };
            state.skill_trust_revocation.last_error = None;
            state.skill_trust_revocation.load_state = PaneLoadState::Ready;
            state.skill_trust_revocation.last_action = Some(format!(
                "Kill-switch {}",
                if state.skill_trust_revocation.kill_switch_active {
                    "enabled"
                } else {
                    "disabled"
                }
            ));
            true
        }
        SkillTrustRevocationPaneAction::RevokeSkill => {
            match state.queue_skl_command(SklDiscoveryTrustCommand::SubmitSkillSearch {
                query: "skill:revocation".to_string(),
                limit: 1,
            }) {
                Ok(command_seq) => {
                    state.skill_trust_revocation.kill_switch_active = true;
                    state.skill_trust_revocation.trust_tier = "revoked".to_string();
                    state.skill_trust_revocation.revocation_event_id =
                        Some(format!("skl:revocation:pending:{command_seq}"));
                    state.skill_trust_revocation.last_error = None;
                    state.skill_trust_revocation.load_state = PaneLoadState::Ready;
                    state.skill_trust_revocation.last_action =
                        Some(format!("Queued skill revocation command #{command_seq}"));
                }
                Err(error) => {
                    state.skill_trust_revocation.last_error = Some(error);
                    state.skill_trust_revocation.load_state = PaneLoadState::Error;
                }
            }
            true
        }
    }
}
