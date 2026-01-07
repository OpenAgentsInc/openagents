//! Fracking Apple Silicon - Episode 201 transcript

use worker::*;
use super::episode_template::{EpisodeMetadata, render_episode_page};

const TWEET_EMBED: &str = r#"<blockquote class="twitter-tweet" data-media-max-width="720"><p lang="en" dir="ltr">Episode 201: Fracking Apple Silicon<br><br>We plan to connect millions of Apple silicon chips into the world's largest network for agentic compute. <a href="https://t.co/oumEA9cErO">https://t.co/oumEA9cErO</a> <a href="https://t.co/d8rTypdwOq">pic.twitter.com/d8rTypdwOq</a></p>&mdash; OpenAgents (@OpenAgentsInc) <a href="https://twitter.com/OpenAgentsInc/status/2008326849613476335?ref_src=twsrc%5Etfw">January 5, 2026</a></blockquote>"#;

/// View the Fracking Apple Silicon blog post: /fracking-apple-silicon
pub async fn view_fracking_apple_silicon(_env: Env) -> Result<Response> {
    let meta = EpisodeMetadata {
        number: 201,
        title: "Fracking Apple Silicon",
        slug: "fracking-apple-silicon",
        date: "January 5, 2026",
        description: "Stranded compute, compute fracking, wildcatters, and why 110M Macs matter",
        tweet_embed: TWEET_EMBED,
    };

    let markdown_content = include_str!("../../../../../docs/transcripts/201-fracking-apple-silicon.md");
    let html = render_episode_page(&meta, markdown_content);

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
