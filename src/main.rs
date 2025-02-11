use openagents::server::config::configure_app;
use tracing::info;
use std::net::SocketAddr;
use axum::routing::Router;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Create and configure the app
    let app = configure_app();

    // Get port from environment variable or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8000);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("✨ Server ready:");
    info!("  🌎 http://{}", listener.local_addr().unwrap());
    
    let app = app.layer(TraceLayer::new_for_http());
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}