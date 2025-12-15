use std::sync::Arc;

use crate::core::AuthManager;
use crate::core::RolloutRecorder;
use crate::core::mcp_connection_manager::McpConnectionManager;
use crate::core::openai_models::models_manager::ModelsManager;
use crate::core::skills::SkillsManager;
use crate::core::tools::sandboxing::ApprovalStore;
use crate::core::unified_exec::UnifiedExecSessionManager;
use crate::core::user_notification::UserNotifier;
use crate::stubs::otel::otel_manager::OtelManager;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

pub(crate) struct SessionServices {
    pub(crate) mcp_connection_manager: Arc<RwLock<McpConnectionManager>>,
    pub(crate) mcp_startup_cancellation_token: CancellationToken,
    pub(crate) unified_exec_manager: UnifiedExecSessionManager,
    pub(crate) notifier: UserNotifier,
    pub(crate) rollout: Mutex<Option<RolloutRecorder>>,
    pub(crate) user_shell: Arc<crate::shell::Shell>,
    pub(crate) show_raw_agent_reasoning: bool,
    pub(crate) auth_manager: Arc<AuthManager>,
    pub(crate) models_manager: Arc<ModelsManager>,
    pub(crate) otel_manager: OtelManager,
    pub(crate) tool_approvals: Mutex<ApprovalStore>,
    pub(crate) skills_manager: Arc<SkillsManager>,
}
