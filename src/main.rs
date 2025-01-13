use askama::Template;
use axum::{
    response::Html,
    routing::get,
    Router,
};
use tower_http::services::ServeDir;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    
    let app = Router::new()
        .route("/", get(home))
        .route("/mobile-app", get(mobile_app))
        .route("/business", get(business))
        .route("/video-series", get(video_series))
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
}

async fn home() -> Html<String> {
    let template = PageTemplate {
        title: "Home".to_string(),
    };
    Html(template.render().unwrap())
}

async fn mobile_app() -> Html<String> {
    let template = PageTemplate {
        title: "Mobile App".to_string(),
    };
    Html(template.render().unwrap())
}

async fn business() -> Html<String> {
    let template = PageTemplate {
        title: "Services".to_string(),
    };
    Html(template.render().unwrap())
}

async fn video_series() -> Html<String> {
    let template = PageTemplate {
        title: "Video Series".to_string(),
    };
    Html(template.render().unwrap())
}

async fn company() -> Html<String> {
    let template = PageTemplate {
        title: "Company".to_string(),
    };
    Html(template.render().unwrap())
}

async fn contact() -> Html<String> {
    let template = PageTemplate {
        title: "Contact".to_string(),
    };
    Html(template.render().unwrap())
}