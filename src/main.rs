use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub mod configuration;
pub mod database;
pub mod filters;
pub mod repo;
pub mod repomap;
pub mod routes;
pub mod server;

#[tokio::main]
async fn main() {
    // Load .env file
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "openagents=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Get port from environment variable or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8000);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let address = format!("{}:{}", host, port);

    // Configure the application
    let app = server::config::configure_app();

    // Start the server
    let listener = tokio::net::TcpListener::bind(&address).await.unwrap();
    info!("✨ Server ready:");
    info!("  🌎 http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
