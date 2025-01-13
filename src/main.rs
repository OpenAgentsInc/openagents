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