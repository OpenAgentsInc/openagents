use std::sync::Arc;

use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    response::IntoResponse,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{error, info};

use self::transport::WebSocketState;

pub mod handlers;
pub mod transport;
pub mod types;

// Add the debug handler attribute to improve error messages
#[axum::debug_handler]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebSocketState>>,
    jar: CookieJar,
) -> impl IntoResponse {
    // Validate session and get user_id
    match WebSocketState::validate_session(&jar).await {
        Ok(user_id) => {
            info!("WebSocket connection authenticated for user {}", user_id);
            // Create chat handler
            let chat_handler = WebSocketState::create_handlers(state.clone());
            
            // Upgrade connection with user_id
            ws.on_upgrade(move |socket| handle_socket(socket, state, chat_handler, user_id))
        }
        Err(e) => {
            error!("WebSocket authentication failed: {}", e);
            (
                axum::http::StatusCode::UNAUTHORIZED,
                "Unauthorized"
            ).into_response()
        }
    }
}

async fn handle_socket(
    socket: WebSocket,
    state: Arc<WebSocketState>,
    chat_handler: Arc<handlers::chat::ChatHandler>,
    user_id: i32,
) {
    state.handle_socket(socket, chat_handler, user_id).await;
}