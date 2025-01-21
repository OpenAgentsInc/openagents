use askama::Template;
use axum::{
    http::header::{HeaderMap, HeaderValue},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use openagents::{
    handle_solver,
    server::{app_router, services::SolverService},
};
use serde_json::json;
use std::{env, path::PathBuf, sync::Arc};
use tokio::sync::broadcast;
use tower_http::services::ServeDir;
use tracing::info;

use openagents::{
    configuration::get_configuration,
    generate_repomap,
    nostr::{
        axum_relay::{ws_handler, RelayState},
        db::Database,
    },
    repomap,
    server::services::RepomapService,
    solver_page, ChatContentTemplate, ChatPageTemplate, ContentTemplate, PageTemplate,
};

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    dotenvy::dotenv().ok();

    info!("🚀 Starting OpenAgents...");

    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    // Load configuration
    let configuration = get_configuration().expect("Failed to read configuration");

    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let repomap_service = Arc::new(RepomapService::new(aider_api_key.clone()));
    let solver_service = Arc::new(SolverService::new());

    // Initialize Nostr components
    let (event_tx, _) = broadcast::channel(1024);

    // Create database connection using configuration
    let db = Arc::new(
        Database::new_with_options(configuration.database.connect_options())
            .await
            .expect("Failed to connect to database"),
    );

    let relay_state = Arc::new(RelayState::new(event_tx, db));

    // Create separate routers for different state types
    let nostr_router = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(relay_state);

    let solver_router = Router::new()
        .route("/", get(solver_page))
        .route("/", post(handle_solver))
        .with_state(solver_service.clone());

    let main_router = Router::new()
        .route("/", get(home))
        .route("/chat", get(chat))
        .route("/onyx", get(mobile_app))
        .route("/video-series", get(video_series))
        .route("/services", get(business))
        .route("/company", get(company))
        .route("/coming-soon", get(coming_soon))
        .route("/health", get(health_check))
        .route("/repomap", get(repomap))
        .route("/repomap/generate", post(generate_repomap))
        .nest("/solver", solver_router)
        .nest_service("/assets", ServeDir::new(&assets_path))
        .fallback_service(ServeDir::new(assets_path.clone()))
        .with_state(repomap_service);

    // Merge routers
    let app = main_router.nest("/nostr", nostr_router).merge(app_router()); // Mount the WebSocket router

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

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({ "status": "healthy" }))
}

async fn home(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Home";
    let path = "/";

    if is_htmx {
        let content = ContentTemplate { path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}

async fn chat(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Chat";
    let path = "/chat";

    if is_htmx {
        let content = ChatContentTemplate.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = ChatPageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}

async fn mobile_app(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Mobile App";
    let path = "/onyx";

    if is_htmx {
        let content = ContentTemplate { path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}

async fn business(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Services";
    let path = "/services";

    if is_htmx {
        let content = ContentTemplate { path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}

async fn video_series(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Video Series";
    let path = "/video-series";

    if is_htmx {
        let content = ContentTemplate { path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}

async fn company(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Company";
    let path = "/company";

    if is_htmx {
        let content = ContentTemplate { path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}

async fn coming_soon(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Coming Soon";
    let path = "/coming-soon";

    if is_htmx {
        let content = ContentTemplate { path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate { title, path };
        Html(template.render().unwrap()).into_response()
    }
}