use axum::{
    response::{Html, IntoResponse, Response},
    Json,
    http::header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use crate::server::services::repomap::{RepomapService, RepomapRequest};

#[derive(Template)]
#[template(path = "pages/repomap.html")]
struct RepomapTemplate<'a> {
    title: &'a str,
    path: &'a str,
}

pub async fn get_repomap(headers: HeaderMap) -> Response {
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

pub async fn generate_repomap(
    Json(req): Json<RepomapRequest>,
    Extension(service): Extension<RepomapService>,
) -> impl IntoResponse {
    match service.generate_repomap(req.repo_url).await {
        Ok(repomap) => Json(repomap).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to generate repomap: {}", e)
            }))
        ).into_response(),
    }
}