use askama::Template;
use axum::{
    response::Html,
    routing::get,
    Router,
    http::header::HeaderMap,
};
use tower_http::services::ServeDir;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    
    let app = Router::new()
        .route("/", get(home))
        .route("/about", get(about))
        .nest_service("/assets", ServeDir::new(assets_path));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8000").await.unwrap();
    println!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

#[derive(Template)]
#[template(path = "base.html")]
struct PageTemplate {
    title: String,
    content: String,
}

#[derive(Template)]
#[template(path = "content.html")]
struct ContentTemplate {
    title: String,
    content: String,
}

async fn home(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Home".to_string();
    let content = "Welcome to OpenAgents".to_string();

    if is_htmx {
        let template = ContentTemplate { title, content };
        Html(template.render().unwrap())
    } else {
        let template = PageTemplate { title, content };
        Html(template.render().unwrap())
    }
}

async fn about(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    let title = "About".to_string();
    let content = "We are building the future of AI agents".to_string();

    if is_htmx {
        let template = ContentTemplate { title, content };
        Html(template.render().unwrap())
    } else {
        let template = PageTemplate { title, content };
        Html(template.render().unwrap())
    }
}