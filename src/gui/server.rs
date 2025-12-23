//! Unified Actix server

use actix_web::{web, App, HttpServer};

use super::routes;
use super::state::{AppState, fetch_claude_info_fast, fetch_claude_model};

/// Start the unified server
pub async fn start_server() -> anyhow::Result<u16> {
    // Initialize authentication token
    let auth_token = auth::AuthToken::init().await?;
    let auth_token = web::Data::new(auth_token);

    // Print token for CLI usage
    println!("\n┌──────────────────────────────────────────────────────────────┐");
    println!("│ OpenAgents API Authentication Token                         │");
    println!("├──────────────────────────────────────────────────────────────┤");
    println!("│ Token: {}  │", auth_token.token());
    println!("│ File:  {:60} │", auth_token.token_file_path().display().to_string());
    println!("├──────────────────────────────────────────────────────────────┤");
    println!("│ Use this token to authenticate API requests:                │");
    println!("│ Authorization: Bearer {}           │", auth_token.token());
    println!("└──────────────────────────────────────────────────────────────┘\n");

    // Create shared state
    let state = web::Data::new(AppState::new());

    // Spawn background task to fetch Claude info (fast version - instant)
    let state_clone = state.clone();
    tokio::spawn(async move {
        // Fast check first (file reads + version command - instant)
        let info = fetch_claude_info_fast().await;
        {
            let mut guard = state_clone.claude_info.write().await;
            *guard = info;
        }

        // Then fetch current model in background (slow - makes API call)
        if let Some(model) = fetch_claude_model().await {
            let mut guard = state_clone.claude_info.write().await;
            guard.model = Some(model);
        }
    });

    // Start server on random available port
    let auth_token_clone = auth_token.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .app_data(auth_token_clone.clone())
            .configure(routes::configure)
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    // Spawn server with tokio (not actix_web::rt::spawn which needs LocalSet)
    tokio::spawn(server.run());

    Ok(port)
}
