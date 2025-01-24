use askama::Template;
use axum::{
    http::header::{HeaderMap, HeaderValue},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use std::{env, path::PathBuf, sync::Arc};
use tower_http::services::ServeDir;
use tracing::info;

use openagents::{
    configuration::get_configuration, generate_repomap, repomap, server::services::RepomapService,
    server::ws, ChatContentTemplate, ChatPageTemplate, ContentTemplate, PageTemplate,
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
    let _configuration = get_configuration().expect("Failed to read configuration");

    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let repomap_service = Arc::new(RepomapService::new(aider_api_key.clone()));

    let app = Router::new()
        .route("/", get(home))
        .route("/ws", get(ws::ws_handler))
        .route("/chat", get(chat))
        .route("/onyx", get(mobile_app))
        .route("/video-series", get(video_series))
        .route("/services", get(business))
        .route("/company", get(company))
        .route("/coming-soon", get(coming_soon))
        .route("/health", get(health_check))
        .route("/repomap", get(repomap))
        .route("/repomap/generate", post(generate_repomap))
        .nest_service("/assets", ServeDir::new(&assets_path))
        .fallback_service(ServeDir::new(assets_path.clone()))
        .with_state(repomap_service);

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
