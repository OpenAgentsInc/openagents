mod auth;
mod content;
mod issue_analysis;
mod pages;
mod repomap;
mod repos;
mod solver;
mod status;
mod user;

pub use auth::mobile_logout;
pub use content::*;
pub use issue_analysis::*;
pub use pages::*;
pub use repomap::*;
pub use repos::*;
pub use solver::*;
pub use status::{connected_status, disconnected_status};
pub use user::user_info;

use axum::{extract::Query, response::Response};
use std::collections::HashMap;

pub async fn append_content(Query(params): Query<HashMap<String, String>>) -> Response {
    let content = params.get("content").map(|s| s.as_str()).unwrap_or("");
    let xml = format!(
        r###"<?xml version="1.0" encoding="UTF-8"?>
<view xmlns="https://hyperview.org/hyperview">
  <text style="deepseekChunk">{}</text>
</view>"###,
        content
    );

    Response::builder()
        .header("Content-Type", "application/vnd.hyperview+xml")
        .body(xml.into())
        .unwrap()
}
