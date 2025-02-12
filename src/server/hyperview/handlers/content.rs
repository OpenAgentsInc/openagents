use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::Response,
};
use serde::Deserialize;
use crate::server::config::AppState;

#[derive(Deserialize)]
pub struct ContentQuery {
    section: String,
}

pub async fn content(
    State(_state): State<AppState>,
    Query(params): Query<ContentQuery>,
) -> Response {
    let xml = format!(
        r#"<view xmlns="https://hyperview.org/hyperview" id="content">
        <text style="welcomeText">Content section: {}</text>
    </view>"#,
        params.section
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}