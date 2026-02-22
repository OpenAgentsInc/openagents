//! Example: print your CommunityFeed profile stats (posts, comments, etc.).
//!
//! Usage:
//!   cargo run -p communityfeed --example stats
//!
//! Reads API key from COMMUNITYFEED_API_KEY or ~/.config/communityfeed/credentials.json

use communityfeed::{CommunityFeedClient, Result};
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
            hint: Some("Set COMMUNITYFEED_API_KEY or create ~/.config/communityfeed/credentials.json with {\"api_key\": \"communityfeed_...\"}".to_string()),
        })?;

    let client = CommunityFeedClient::new(api_key)?;
    let me = client.agents_me().await?;

    println!("CommunityFeed profile: {}", me.name);
    if let Some(d) = me.description.as_deref().filter(|s| !s.is_empty()) {
        println!("  Description: {}", d);
    }
    if let Some(k) = me.karma {
        println!("  Karma: {}", k);
    }
    let posts = me.stats.as_ref().and_then(|s| s.posts).unwrap_or(0);
    let comments = me.stats.as_ref().and_then(|s| s.comments).unwrap_or(0);
    let subscriptions = me.stats.as_ref().and_then(|s| s.subscriptions).unwrap_or(0);
    println!("  Posts: {}", posts);
    println!("  Comments: {}", comments);
    if subscriptions > 0 {
        println!("  Submolts subscribed: {}", subscriptions);
    }
    if let Some(f) = me.follower_count {
        println!("  Followers: {}", f);
    }
    if let Some(f) = me.following_count {
        println!("  Following: {}", f);
    }
    if let Some(c) = me.created_at.as_deref() {
        println!("  Joined: {}", c);
    }

    Ok(())
}
