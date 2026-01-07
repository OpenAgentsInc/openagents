//! Pylon v0.1 Release blog post

use worker::*;
use super::blog_template::{BlogMetadata, render_blog_page};

/// View the Pylon v0.1 release blog post: /blog/pylon-v0.1-release
pub async fn view_pylon_v0_1_release(_env: Env) -> Result<Response> {
    let meta = BlogMetadata {
        title: "Pylon v0.1: Regtest Alpha",
        slug: "pylon-v0.1-release",
        date: "January 8, 2026",
        description: "Node software that connects your compute to the global AI marketplace via Nostr",
    };

    let markdown_content = include_str!("../../../../../docs/blog/pylon-v0.1-release.md");
    let html = render_blog_page(&meta, markdown_content);

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
