//! Recursive Language Models - Episode 202 transcript

use worker::*;
use super::episode_template::{EpisodeMetadata, render_episode_page};

const TWEET_EMBED: &str = r#"<blockquote class="twitter-tweet" data-media-max-width="720"><p lang="en" dir="ltr">Episode 202: Recursive Language Models<br><br>We discuss recursive language models (RLMs) and their importance to our swarm compute network.<br><br>In episode 201 we discussed how we&#39;ll connect millions of edge inference devices into a single network. RLMs answer the question of why now. <a href="https://t.co/n6hUOrbyP6">https://t.co/n6hUOrbyP6</a> <a href="https://t.co/YXJyh86jos">pic.twitter.com/YXJyh86jos</a></p>&mdash; OpenAgents (@OpenAgentsInc) <a href="https://twitter.com/OpenAgentsInc/status/2008704591110541567?ref_src=twsrc%5Etfw">January 7, 2026</a></blockquote>"#;

/// View the recursive language models blog post: /recursive-language-models
pub async fn view_recursive_language_models(_env: Env) -> Result<Response> {
    let meta = EpisodeMetadata {
        number: 202,
        title: "Recursive Language Models",
        slug: "recursive-language-models",
        date: "January 6, 2026",
        description: "We discuss recursive language models (RLMs) and their importance to our swarm compute network",
        tweet_embed: TWEET_EMBED,
    };

    let markdown_content = include_str!("../../../../../docs/transcripts/202-recursive-language-models.md");
    let html = render_episode_page(&meta, markdown_content);

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}
