//! Unified Actix server

use actix_web::{web, App, HttpServer};
use std::net::TcpListener;

use super::routes;
use super::state::AppState;

/// Start the unified server
pub async fn start_server() -> anyhow::Result<u16> {
    // Bind to an available port
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();

    // Create shared state
    let state = web::Data::new(AppState::new());

    // Start server
    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .configure(routes::configure)
    })
    .listen(listener)?
    .run();

    // Spawn server task
    actix_web::rt::spawn(server);

    Ok(port)
}
