use axum::{
    extract::{ws::WebSocket, State, WebSocketUpgrade},
    http::StatusCode,
    response::{Response, IntoResponse},
    body::Body,
};
use axum_extra::extract::cookie::CookieJar;
use tracing::{error, info};

use crate::server::{
    config::AppState,
    handlers::oauth::session::SESSION_COOKIE_NAME,
};

pub mod handlers;
pub mod transport;
pub mod types;

use transport::WebSocketTransport;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    cookies: CookieJar,
) -> Response {
    info!("WebSocket upgrade request received");

    // Get user ID from session cookie
    let user_id = if let Some(session_cookie) = cookies.get(SESSION_COOKIE_NAME) {
        session_cookie.value().to_string()
    } else {
        error!("No session cookie found in WebSocket request");
        return Response::builder()
            .status(StatusCode::UNAUTHORIZED)
            .body(Body::empty())
            .unwrap();
    };

    info!("WebSocket connection for user: {}", user_id);
    
    ws.on_upgrade(move |socket| handle_socket(socket, state, user_id))
}

async fn handle_socket(socket: WebSocket, state: AppState, user_id: String) {
    let transport = WebSocketTransport::new(state.ws_state.clone(), state);
    
    match transport.handle_socket(socket, user_id).await {
        Ok(_) => {
            info!("WebSocket connection closed normally");
        }
        Err(e) => {
            error!("WebSocket error: {:?}", e);
        }
    }
}