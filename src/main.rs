use axum::{
    routing::{get, post},
    Router,
};
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::server::services::{
    deepseek::{DeepSeekService, Tool},
    github_issue::GitHubService,
    model_router::ModelRouter,
};
use crate::server::ws::transport::WebSocketState;

pub mod configuration;
pub mod database;
pub mod filters;
pub mod repo;
pub mod repomap;
pub mod server;

#[tokio::main]
async fn main() {
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

    let github_service = Arc::new(GitHubService::new(
        env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
    ));

    // Create available tools
    let tools = create_tools();

    // Create WebSocket state with services
    let ws_state = Arc::new(WebSocketState::new(
        tool_model,
        chat_model,
        github_service.clone(),
        tools,
    ));

    // Create the router
    let app = Router::new()
        .route("/", get(home))
        .route("/chat", get(chat))
        .route("/ws", get(server::ws::ws_handler))
        .route("/mobile-app", get(mobile_app))
        .route("/business", get(business))
        .route("/video-series", get(video_series))
        .route("/company", get(company))
        .route("/coming-soon", get(coming_soon))
        .route("/repomap", get(repomap))
        .route("/repomap/generate", post(generate_repomap))
        // Static files
        .nest_service("/static", ServeDir::new("./static").precompressed_gzip())
        // Template files
        .nest_service(
            "/templates",
            ServeDir::new("./templates").precompressed_gzip(),
        )
        .with_state(ws_state);

    // Run the server
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    info!("Listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
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

async fn home() -> &'static str {
    "Hello, World!"
}

async fn chat() -> &'static str {
    "Chat page"
}

async fn mobile_app() -> &'static str {
    "Mobile app page"
}

async fn business() -> &'static str {
    "Business page"
}

async fn video_series() -> &'static str {
    "Video series page"
}

async fn company() -> &'static str {
    "Company page"
}

async fn coming_soon() -> &'static str {
    "Coming soon page"
}

async fn repomap() -> &'static str {
    "Repomap page"
}

async fn generate_repomap() -> &'static str {
    "Generate repomap"
}