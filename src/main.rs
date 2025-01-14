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
        .route("/mobile-app", get(mobile_app))
        .route("/video-series", get(video_series))
        .route("/services", get(business))
        .route("/company", get(company))
        .route("/contact", get(contact))
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
    path: String,
}

#[derive(Template)]
#[template(path = "content.html")]
struct ContentTemplate {
    title: String,
    content: String,
}

#[derive(Template)]
#[template(path = "pages/home.html")]
struct HomeTemplate;

async fn home(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    
    if is_htmx {
        let template = ContentTemplate { 
            title: "Home".to_string(),
            content: "Welcome to OpenAgents".to_string()
        };
        Html(template.render().unwrap())
    } else {
        let template = HomeTemplate;
        Html(template.render().unwrap())
    }
}

async fn mobile_app(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Mobile App".to_string();
    let content = "Our mobile app is coming soon".to_string();
    let path = "/mobile-app".to_string();

    if is_htmx {
        let template = ContentTemplate { title, content };
        Html(template.render().unwrap())
    } else {
        let template = PageTemplate { title, content, path };
        Html(template.render().unwrap())
    }
}

async fn business(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Services".to_string();
    let content = "Enterprise AI solutions".to_string();
    let path = "/services".to_string();

    if is_htmx {
        let template = ContentTemplate { title, content };
        Html(template.render().unwrap())
    } else {
        let template = PageTemplate { title, content, path };
        Html(template.render().unwrap())
    }
}

async fn video_series(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Video Series".to_string();
    let content = "Watch our latest content".to_string();
    let path = "/video-series".to_string();

    if is_htmx {
        let template = ContentTemplate { title, content };
        Html(template.render().unwrap())
    } else {
        let template = PageTemplate { title, content, path };
        Html(template.render().unwrap())
    }
}

async fn company(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Company".to_string();
    let content = "About our mission and team".to_string();
    let path = "/company".to_string();

    if is_htmx {
        let template = ContentTemplate { title, content };
        Html(template.render().unwrap())
    } else {
        let template = PageTemplate { title, content, path };
        Html(template.render().unwrap())
    }
}

async fn contact(headers: HeaderMap) -> Html<String> {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Contact".to_string();
    let content = "Get in touch with us".to_string();
    let path = "/contact".to_string();

    if is_htmx {
        let template = ContentTemplate { title, content };
        Html(template.render().unwrap())
    } else {
        let template = PageTemplate { title, content, path };
        Html(template.render().unwrap())
    }
}