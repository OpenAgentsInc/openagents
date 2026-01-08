//! Pylon and Nexus - Episode 203 transcript

use worker::*;
use super::episode_template::{EpisodeMetadata, render_episode_page};

const TWEET_EMBED: &str = r#"<!-- Tweet embed will be added -->"#;

/// View the pylon and nexus episode: /pylon-and-nexus
pub async fn view_pylon_and_nexus(_env: Env) -> Result<Response> {
    let meta = EpisodeMetadata {
        number: 203,
        title: "Pylon and Nexus",
        slug: "pylon-and-nexus",
        date: "January 7, 2026",
        description: "Launching Pylon v0.1 and Nexus v0.1 - the swarm compute node and relay for OpenAgents",
        tweet_embed: TWEET_EMBED,
    };

    let markdown_content = include_str!("../../../../../docs/transcripts/203-pylon-and-nexus.md");
    let html = render_episode_page(&meta, markdown_content);

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
