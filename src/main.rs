use std::net::TcpListener;
use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use openagents::{
    server::{
        configuration::get_configuration,
        services::RepomapService,
        ws,
    },
    repomap::generate_repo_map,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let configuration = get_configuration()?;
    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );

    // Create services
    let repomap_service = RepomapService::new();

    // Build router
    let app = Router::new()
        .route("/", get(|| async { "OpenAgents" }))
        .route("/ws", get(ws::ws_handler))
        .route("/repomap", post(move |body| async move {
            repomap_service.generate_repomap(body).await
        }));

    // Start server
    let listener = TcpListener::bind(&address)?;
    println!("Listening on {}", address);
    axum::Server::from_tcp(listener)?
        .serve(app.into_make_service())
        .await?;

    Ok(())
}