use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tauri::AppHandle;
use tokio::sync::Mutex;

use crate::types::AppSettings;
use crate::backend::app_server::WorkspaceSession;
use crate::acp::AcpConnection;
use crate::agent::manager::AgentManager;

pub(crate) struct AppState {
    pub(crate) sessions: Mutex<HashMap<String, Arc<WorkspaceSession>>>,
    pub(crate) acp_connections: Mutex<HashMap<String, Arc<AcpConnection>>>,
    pub(crate) app_settings: Mutex<AppSettings>,
    pub(crate) agent_manager: Arc<Mutex<AgentManager>>,
    pub(crate) unified_forwarder_started: AtomicBool,
}

impl AppState {
    pub(crate) fn load(_app: &AppHandle) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            acp_connections: Mutex::new(HashMap::new()),
            app_settings: Mutex::new(AppSettings::default()),
            agent_manager: Arc::new(Mutex::new(AgentManager::new())),
            unified_forwarder_started: AtomicBool::new(false),
        }
    }
}
