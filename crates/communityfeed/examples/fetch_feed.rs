//! Example: fetch CommunityFeed feed and dump raw response + parse result.
//!
//! Use this to debug "error decoding response body" by seeing the exact
//! JSON and full serde error.
//!
//! Usage:
//!   cargo run -p communityfeed --example fetch_feed
//!
//! Optional: save raw response to a file for fixture tests:
//!   cargo run -p communityfeed --example fetch_feed 2>&1 | head -1 | jq . > tests/fixtures/feed_response.json

use communityfeed::{CommunityFeedClient, PostSort, Result};
use std::path::PathBuf;

fn api_key_from_env() -> Option<String> {
    std::env::var("COMMUNITYFEED_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
}

fn api_key_from_config() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = PathBuf::from(home).join(".config/communityfeed/credentials.json");
    let contents = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&contents).ok()?;
    let key = json.get("api_key")?.as_str()?;
    if key.is_empty() {
        return None;
    }
    Some(key.to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    let api_key = api_key_from_env()
        .or_else(api_key_from_config)
        .ok_or_else(|| communityfeed::CommunityFeedError::Api {
            status: 401,
            error: "No CommunityFeed API key".to_string(),
            hint: Some(
                "Set COMMUNITYFEED_API_KEY or create ~/.config/communityfeed/credentials.json".to_string(),
            ),
        })?;

    let client = CommunityFeedClient::new(api_key.clone())?;

    // 1) Fetch raw body so we can see exactly what the API returns
    let url = "https://www.communityfeed.com/api/v1/posts?sort=new&limit=2";
    let raw_response = reqwest::Client::new()
        .get(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(communityfeed::CommunityFeedError::Http)?
        .text()
        .await
        .map_err(communityfeed::CommunityFeedError::Http)?;

    eprintln!("--- Raw response (first 2500 chars) ---");
    let snippet = raw_response.chars().take(2500).collect::<String>();
    eprintln!("{}", snippet);
    if raw_response.len() > 2500 {
        eprintln!("... (truncated, total {} bytes)", raw_response.len());
    }
    eprintln!("--- End raw response ---");

    // 2) Parse with our client and print full error if it fails
    match client.posts_feed(PostSort::New, Some(2), None).await {
        Ok(posts) => {
            println!("OK: parsed {} posts", posts.len());
            for (i, p) in posts.iter().enumerate() {
                println!(
                    "  [{}] id={} title={:?} author={:?}",
                    i,
                    p.id,
                    p.title.as_deref(),
                    p.author.as_ref().map(|a| a.name.as_str())
                );
            }
        }
        Err(e) => {
            eprintln!("Parse failed:");
            eprintln!("  {}", e);
            if let communityfeed::CommunityFeedError::Api { hint: Some(h), .. } = &e {
                eprintln!("  (hint/body snippet): {}", h);
            }
            return Err(e);
        }
    }

    Ok(())
}
