use askama::Template;
use axum::{
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
    http::header::{HeaderMap, HeaderValue},
};
use tower_http::services::ServeDir;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let assets_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    
    let app = Router::new()
        .route("/", get(home))
        .route("/another-page", get(another_page))
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

async fn another_page(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Another Page".to_string();
    let path = "/another-page".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert("HX-Title", HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap());
        response
    } else {
        let template = PageTemplate { title: &title, path: &path };
        Html(template.render().unwrap()).into_response()
    }
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
        response.headers_mut().insert("HX-Title", HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap());
        response
    } else {
        let template = PageTemplate { title: &title, path: &path };
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
        response.headers_mut().insert("HX-Title", HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap());
        response
    } else {
        let template = PageTemplate { title: &title, path: &path };
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
        response.headers_mut().insert("HX-Title", HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap());
        response
    } else {
        let template = PageTemplate { title: &title, path: &path };
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
        response.headers_mut().insert("HX-Title", HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap());
        response
    } else {
        let template = PageTemplate { title: &title, path: &path };
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
        response.headers_mut().insert("HX-Title", HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap());
        response
    } else {
        let template = PageTemplate { title: &title, path: &path };
        Html(template.render().unwrap()).into_response()
    }
}

async fn contact(headers: HeaderMap) -> Response {
    let is_htmx = headers.contains_key("hx-request");
    let title = "Contact".to_string();
    let path = "/contact".to_string();

    if is_htmx {
        let content = ContentTemplate { path: &path }.render().unwrap();
        let mut response = Response::new(content.into());
        response.headers_mut().insert("HX-Title", HeaderValue::from_str(&format!("OpenAgents - {}", title)).unwrap());
        response
    } else {
        let template = PageTemplate { title: &title, path: &path };
        Html(template.render().unwrap()).into_response()
    }
}