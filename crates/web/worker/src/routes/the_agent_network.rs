//! The Agent Network - Episode 200 transcript

use worker::*;
use super::episode_template::{EpisodeMetadata, render_episode_page};

const TWEET_EMBED: &str = r#"<blockquote class="twitter-tweet" data-media-max-width="720"><p lang="en" dir="ltr">Episode 200: The Agent Network<br><br>We predict six major themes for 2026: local &amp; swarm AI, open &gt; closed, agents &gt; models, autopilots, and agent networks.<br><br>We introduce Reed&#39;s Law of group-forming networks, a concept from network economics crucial for understanding agent networks.… <a href="https://t.co/dIatR1rLCU">https://t.co/dIatR1rLCU</a> <a href="https://t.co/gZIXIy8xUQ">pic.twitter.com/gZIXIy8xUQ</a></p>&mdash; OpenAgents (@OpenAgentsInc) <a href="https://twitter.com/OpenAgentsInc/status/2006956979298685216?ref_src=twsrc%5Etfw">January 2, 2026</a></blockquote>"#;

/// View the agent network blog post: /the-agent-network
pub async fn view_the_agent_network(_env: Env) -> Result<Response> {
    let meta = EpisodeMetadata {
        number: 200,
        title: "The Agent Network",
        slug: "the-agent-network",
        date: "January 1, 2026",
        description: "Predictions for 2026, Reed's Law of group-forming networks, and how agent networks will pay you",
        tweet_embed: TWEET_EMBED,
    };

    let markdown_content = include_str!("../../../../../docs/transcripts/200-the-agent-network.md");
    let html = render_episode_page(&meta, markdown_content);

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
