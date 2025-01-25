use axum::{
    routing::{get, post},
    Router,
};
use std::{env, sync::Arc};
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::server::services::{
    deepseek::{DeepSeekService, Tool},
    github_issue::GitHubService,
    RepomapService,
};
use crate::server::ws::transport::WebSocketState;

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

    // Create shared services
    let tool_model = Arc::new(DeepSeekService::new(
        env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set"),
    ));

    let chat_model = Arc::new(DeepSeekService::new(
        env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set"),
    ));

    let github_service = Arc::new(
        GitHubService::new(Some(
            env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
        ))
        .expect("Failed to create GitHub service"),
    );

    // Create available tools
    let tools = create_tools();

    // Create WebSocket state with services
    let ws_state = WebSocketState::new(tool_model, chat_model, github_service.clone(), tools);

    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let repomap_service = Arc::new(RepomapService::new(aider_api_key));

    // Create the router with WebSocket state
    let app = Router::new()
        .route("/", get(routes::home))
        .route("/chat", get(routes::chat))
        .route("/ws", get(server::ws::ws_handler))
        .route("/onyx", get(routes::mobile_app))
        .route("/services", get(routes::business))
        .route("/video-series", get(routes::video_series))
        .route("/company", get(routes::company))
        .route("/coming-soon", get(routes::coming_soon))
        .route("/health", get(routes::health_check))
        .route("/repomap", get(routes::repomap))
        .with_state(ws_state);

    // Add repomap routes with repomap state
    let app = app
        .route("/repomap/generate", post(routes::generate_repomap))
        .with_state(repomap_service);

    // Static files
    let app = app
        .nest_service("/assets", ServeDir::new("./assets").precompressed_gzip())
        .nest_service(
            "/templates",
            ServeDir::new("./templates").precompressed_gzip(),
        );

    // Get port from environment variable or use default
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(8000);

    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let address = format!("{}:{}", host, port);

    let listener = tokio::net::TcpListener::bind(&address).await.unwrap();
    info!("✨ Server ready:");
    info!("  🌎 http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

fn create_tools() -> Vec<Tool> {
    vec![
        // GitHub issue tool
        DeepSeekService::create_tool(
            "read_github_issue".to_string(),
            Some("Read a GitHub issue by number".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "The owner of the repository"
                    },
                    "repo": {
                        "type": "string",
                        "description": "The name of the repository"
                    },
                    "issue_number": {
                        "type": "integer",
                        "description": "The issue number"
                    }
                },
                "required": ["owner", "repo", "issue_number"]
            }),
        ),
        // Calculator tool
        DeepSeekService::create_tool(
            "calculate".to_string(),
            Some("Perform a calculation".to_string()),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
        ),
    ]
}
