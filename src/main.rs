use askama::Template;
use axum::{
    http::header::{HeaderMap, HeaderValue},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
    extract::Extension,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{path::PathBuf, env};
use tower_http::services::ServeDir;
use tracing::info;

use crate::server::services::RepomapService;

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    dotenvy::dotenv().ok();

    info!("🚀 Starting OpenAgents...");

    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let repomap_service = RepomapService::new(aider_api_key);

    let app = Router::new()
        .route("/", get(home))
        .route("/onyx", get(mobile_app))
        .route("/video-series", get(video_series))
        .route("/services", get(business))
        .route("/company", get(company))
        .route("/coming-soon", get(coming_soon))
        .route("/health", get(health_check))
        .route("/repomap", get(repomap))
        .route("/repomap/generate", post(generate_repomap))
        .layer(Extension(repomap_service))
        .nest_service("/assets", ServeDir::new(&assets_path))
        .fallback_service(ServeDir::new(assets_path));

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

#[derive(Template)]
#[template(path = "layouts/base.html")]
struct PageTemplate<'a> {
    title: &'a str,
    path: &'a str,
}

#[derive(Template)]
#[template(path = "layouts/content.html")]
struct ContentTemplate<'a> {
    path: &'a str,
}

#[derive(Debug, Deserialize)]
struct RepomapRequest {
    repo_url: String,
}

#[derive(Debug, Serialize)]
struct RepomapResponse {
    repo_map: String,
    metadata: serde_json::Value,
}

async fn repomap(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Repository Map";
    let path = "/repomap";

    if is_htmx {
        let content = ContentTemplate { path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate {
            title,
            path,
        };
        Html(template.render().unwrap()).into_response()
    }
}

async fn generate_repomap(
    Extension(service): Extension<RepomapService>,
    Json(req): Json<RepomapRequest>,
) -> impl IntoResponse {
    match service.generate_repomap(req.repo_url).await {
        Ok(repomap) => Json(repomap).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to generate repomap: {}", e)
            }))
        ).into_response(),
    }
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
        let template = PageTemplate {
            title,
            path,
        };
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
        let template = PageTemplate {
            title,
            path,
        };
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
        let template = PageTemplate {
            title,
            path,
        };
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
        let template = PageTemplate {
            title,
            path,
        };
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
        let template = PageTemplate {
            title,
            path,
        };
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
        let template = PageTemplate {
            title,
            path,
        };
        Html(template.render().unwrap()).into_response()
    }
}