use serde::Serialize;
use serde_json::Value;

#[derive(Serialize, Clone)]
pub(crate) struct AppServerEvent {
    pub(crate) workspace_id: String,
    pub(crate) message: Value,
}

pub(crate) trait EventSink: Clone + Send + Sync + 'static {
    fn emit_app_server_event(&self, event: AppServerEvent);
}
