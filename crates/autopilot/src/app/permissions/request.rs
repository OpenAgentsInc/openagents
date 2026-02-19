use serde_json::Value;
use tokio::sync::oneshot;

#[derive(Clone, Debug)]
pub(crate) struct PermissionRequest {
    pub(crate) tool_name: String,
    pub(crate) tool_use_id: String,
    pub(crate) input: Value,
    pub(crate) suggestions: Option<Value>,
    pub(crate) blocked_path: Option<String>,
    pub(crate) decision_reason: Option<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub(crate) enum PermissionResult {
    Allow {
        updated_input: Value,
        updated_permissions: Option<Value>,
        tool_use_id: Option<String>,
        accept_for_session: Option<bool>,
    },
    Deny {
        message: String,
        interrupt: Option<bool>,
        tool_use_id: Option<String>,
    },
}

impl PermissionResult {
    pub(crate) fn deny_and_interrupt(message: &str) -> Self {
        PermissionResult::Deny {
            message: message.to_string(),
            interrupt: Some(true),
            tool_use_id: None,
        }
    }
}

pub(crate) struct PermissionPending {
    pub(crate) request: PermissionRequest,
    pub(crate) respond_to: oneshot::Sender<PermissionResult>,
}
