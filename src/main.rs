use openagents::server::config::configure_app;
use tracing::info;

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

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting server on {}", addr);

    // Start the server
    info!("✨ Server ready:");
    info!("  🌎 http://{}", addr);
    
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C signal handler");
}