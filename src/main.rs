use askama::Template;
use axum::{
    http::header::{HeaderMap, HeaderValue},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tracing::info;

#[tokio::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    dotenv::dotenv().ok();

    info!("🚀 Starting OpenAgents...");

    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");

    let app = Router::new()
        .route("/", get(home))
        .route("/mobile-app", get(mobile_app))
        .route("/video-series", get(video_series))
        .route("/services", get(business))
        .route("/company", get(company))
        .route("/coming-soon", get(coming_soon))
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

async fn home(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Home".to_string();
    let path = "/".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate {
            title: &title,
            path: &path,
        };
        Html(template.render().unwrap()).into_response()
    }
}

async fn mobile_app(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Mobile App".to_string();
    let path = "/mobile-app".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate {
            title: &title,
            path: &path,
        };
        Html(template.render().unwrap()).into_response()
    }
}

async fn business(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Services".to_string();
    let path = "/services".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate {
            title: &title,
            path: &path,
        };
        Html(template.render().unwrap()).into_response()
    }
}

async fn video_series(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Video Series".to_string();
    let path = "/video-series".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate {
            title: &title,
            path: &path,
        };
        Html(template.render().unwrap()).into_response()
    }
}

async fn company(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Company".to_string();
    let path = "/company".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate {
            title: &title,
            path: &path,
        };
        Html(template.render().unwrap()).into_response()
    }
}

async fn coming_soon(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Coming Soon".to_string();
    let path = "/coming-soon".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert(
            "HX-Title",
            HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap(),
        );
        response
    } else {
        let template = PageTemplate {
            title: &title,
            path: &path,
        };
        Html(template.render().unwrap()).into_response()
    }
}