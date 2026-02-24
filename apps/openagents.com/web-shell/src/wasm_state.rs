use crate::wasm::{MembershipRecord, PolicyDecision, RouteSplitStatus, RuntimeThreadRecord};

#[derive(Debug, Clone, Default)]
pub(crate) struct SyncRuntimeState {
    pub(super) subscribed_topics: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ManagementSurfaceState {
    pub(super) loaded_session_id: Option<String>,
    pub(super) memberships: Vec<MembershipRecord>,
    pub(super) active_org_id: Option<String>,
    pub(super) route_split_status: Option<RouteSplitStatus>,
    pub(super) billing_policy: Option<PolicyDecision>,
    pub(super) last_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CodexHistoryState {
    pub(super) loaded_session_id: Option<String>,
    pub(super) loaded_thread_id: Option<String>,
    pub(super) threads: Vec<RuntimeThreadRecord>,
    pub(super) active_thread_exists: Option<bool>,
    pub(super) last_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SettingsSurfaceState {
    pub(super) loaded_session_id: Option<String>,
    pub(super) profile_id: Option<String>,
    pub(super) profile_name: String,
    pub(super) profile_email: String,
    pub(super) autopilot_display_name: String,
    pub(super) autopilot_tagline: String,
    pub(super) autopilot_owner_display_name: String,
    pub(super) autopilot_persona_summary: String,
    pub(super) autopilot_voice: String,
    pub(super) autopilot_principles_text: String,
    pub(super) resend_connected: Option<bool>,
    pub(super) resend_secret_last4: Option<String>,
    pub(super) google_connected: Option<bool>,
    pub(super) google_secret_last4: Option<String>,
    pub(super) last_status: Option<String>,
    pub(super) last_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct L402SurfaceState {
    pub(super) loaded_session_id: Option<String>,
    pub(super) loaded_route_path: Option<String>,
    pub(super) payload: Option<serde_json::Value>,
    pub(super) last_error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct AdminWorkerSurfaceState {
    pub(super) loaded_session_id: Option<String>,
    pub(super) loaded_route_path: Option<String>,
    pub(super) selected_worker_id: Option<String>,
    pub(super) workers: Vec<serde_json::Value>,
    pub(super) worker_snapshot: Option<serde_json::Value>,
    pub(super) worker_stream: Option<serde_json::Value>,
    pub(super) last_response: Option<serde_json::Value>,
    pub(super) last_status: Option<String>,
    pub(super) last_error: Option<String>,
}
