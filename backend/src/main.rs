use openagents::server::config::configure_app;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

#[tokio::main]
async fn main() {
    // Initialize logging with more detail
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_env_filter("openagents=debug,tower_http=debug")
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    // Create database pool
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"))
        .await
        .expect("Failed to create database pool");

    // Configure and start the application
    let app = configure_app(pool);

    // Get port from environment variable or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8000);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("Starting server on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("✨ Server ready:");
    info!("  🌎 http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
