//! Unified Actix server

use actix_web::{web, App, HttpServer};

use super::routes;
use super::state::{AppState, fetch_claude_info};

/// Start the unified server
pub async fn start_server() -> anyhow::Result<u16> {
    // Create shared state
    let state = web::Data::new(AppState::new());

    // Spawn background task to fetch Claude info
    let state_clone = state.clone();
    tokio::spawn(async move {
        let info = fetch_claude_info().await;
        let mut guard = state_clone.claude_info.write().await;
        *guard = info;
    });

    // Start server on random available port
    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(routes::configure)
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    // Spawn server with tokio (not actix_web::rt::spawn which needs LocalSet)
    tokio::spawn(server.run());

    Ok(port)
}
