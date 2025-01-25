use axum::{
    extract::WebSocketUpgrade,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use dotenvy::dotenv;
use openagents::{
    server::{
        services::{
            deepseek::{DeepSeekService, Tool},
            github_issue::GitHubService,
            RepomapService,
        },
        ws::{transport::WebSocketState, ws_handler},
    },
    repomap::generate_repo_map,
};
use serde_json::json;
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::services::ServeDir;
use tracing::info;
use tracing_subscriber;

fn create_tools() -> Vec<Tool> {
    vec![
        // GitHub issue tool
        DeepSeekService::create_tool(
            "read_github_issue".to_string(),
            Some("Read a GitHub issue by number".to_string()),
            json!({
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
            json!({
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

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Load environment variables
    dotenv().ok();

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

    let repomap_service = Arc::new(RepomapService::new(
        env::var("FIRECRAWL_API_KEY").expect("FIRECRAWL_API_KEY must be set"),
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
        .route("/mobile-app", get(mobile_app))
        .route("/business", get(business))
        .route("/video-series", get(video_series))
        .route("/company", get(company))
        .route("/coming-soon", get(coming_soon))
        .route("/repomap", get(repomap))
        .route("/repomap/generate", post(generate_repomap))
        .route("/ws", get(ws_handler))
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

async fn home() -> impl IntoResponse {
    "Hello, World!"
}

async fn chat() -> impl IntoResponse {
    "Chat page"
}

async fn mobile_app() -> impl IntoResponse {
    "Mobile app page"
}

async fn business() -> impl IntoResponse {
    "Business page"
}

async fn video_series() -> impl IntoResponse {
    "Video series page"
}

async fn company() -> impl IntoResponse {
    "Company page"
}

async fn coming_soon() -> impl IntoResponse {
    "Coming soon page"
}

async fn repomap() -> impl IntoResponse {
    "Repomap page"
}

async fn generate_repomap() -> impl IntoResponse {
    let map = generate_repo_map(".");
    map
}