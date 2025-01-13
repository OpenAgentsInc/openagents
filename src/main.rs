use askama::Template;
use axum::{
    response::Html,
    routing::get,
    Router,
};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(home));

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