use openagents::server::config::configure_app;
use tracing::info;
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower::service_fn;

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
    
    let make_service = service_fn(move |_| {
        let app = app.clone();
        async move { Ok::<_, std::convert::Infallible>(app) }
    });

    axum::serve(listener, make_service).await.unwrap();
}