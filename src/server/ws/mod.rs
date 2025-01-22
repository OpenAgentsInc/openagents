pub mod handlers;
pub mod transport;
pub mod types;

use std::sync::Arc;
use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

use self::{
    transport::WebSocketState,
    handlers::{
        chat::ChatHandler,
        solver::SolverHandler,
    },
};

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebSocketState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<WebSocketState>) {
    state.handle_socket(socket).await;
}

pub fn init_websocket_state() -> Arc<WebSocketState> {
    let chat_handler = Arc::new(ChatHandler::new());
    let solver_handler = Arc::new(SolverHandler::new());
    
    Arc::new(WebSocketState::new(chat_handler, solver_handler))
}

pub fn ws_routes() -> Router<Arc<WebSocketState>> {
    Router::new()
        .route("/ws", get(ws_handler))
}