use claude_agent_sdk::permissions::PermissionRequest;
use claude_agent_sdk::protocol::PermissionResult;
use tokio::sync::oneshot;

pub(crate) struct PermissionPending {
    pub(crate) request: PermissionRequest,
    pub(crate) respond_to: oneshot::Sender<PermissionResult>,
}
