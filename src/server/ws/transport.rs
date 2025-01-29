use crate::server::services::model_router::ModelRouter;
use crate::server::ws::types::ChatMessage;
use std::sync::Arc;

pub struct WebSocketState {
    pub model_router: Arc<ModelRouter>,
}

impl WebSocketState {
    pub fn new(model_router: Arc<ModelRouter>) -> Self {
        Self { model_router }
    }
}