//! Integration tests for feed response parsing.
//!
//! Run with: cargo test -p communityfeed
//! With live API: COMMUNITYFEED_API_KEY=... cargo test -p communityfeed feed_parse

use communityfeed::{CommunityFeedClient, PostSort};
use serde_json::json;

/// Minimal post: id (string), optional fields.
#[test]
fn parse_feed_array_minimal() {
    let body = json!([
        {
            "id": "post-123",
            "title": "Hello",
            "author": { "name": "TestAgent" }
        }
    ]);
    let posts: Vec<communityfeed::Post> = serde_json::from_value(body).expect("parse array");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].id, "post-123");
    assert_eq!(posts[0].title.as_deref(), Some("Hello"));
    assert_eq!(
        posts[0].author.as_ref().map(|a| a.name.as_str()),
        Some("TestAgent")
    );
}

/// API may return id as number.
#[test]
fn parse_feed_array_id_as_number() {
    let body = json!([
        {
            "id": 45678,
            "title": "Numeric id",
            "author": { "name": "Bot" }
        }
    ]);
    let posts: Vec<communityfeed::Post> =
        serde_json::from_value(body).expect("parse array with numeric id");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].id, "45678");
}

/// Object with "posts" key.
#[test]
fn parse_feed_object_posts() {
    let body = json!({
        "success": true,
        "posts": [
            { "id": "p1", "title": "One", "author": { "name": "A" } },
            { "id": "p2", "title": "Two", "author": { "name": "B" } }
        ]
    });
    let wrapper: communityfeed::PostsResponse = serde_json::from_value(body).expect("parse object");
    let posts = wrapper.into_posts();
    assert_eq!(posts.len(), 2);
    assert_eq!(posts[0].id, "p1");
    assert_eq!(posts[1].id, "p2");
}

/// Object with "data" key.
#[test]
fn parse_feed_object_data() {
    let body = json!({
        "data": [
            { "id": "d1", "author": { "name": "D" } }
        ]
    });
    let wrapper: communityfeed::PostsResponse = serde_json::from_value(body).expect("parse object data");
    let posts = wrapper.into_posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].id, "d1");
}

/// CamelCase fields (commentCount, createdAt).
#[test]
fn parse_feed_camel_case() {
    let body = json!([
        {
            "id": "c1",
            "commentCount": 5,
            "createdAt": "2025-01-30T12:00:00Z",
            "author": { "name": "C", "avatarUrl": "https://example.com/a.png" }
        }
    ]);
    let posts: Vec<communityfeed::Post> = serde_json::from_value(body).expect("parse camelCase");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].comment_count, Some(5));
    assert_eq!(posts[0].created_at.as_deref(), Some("2025-01-30T12:00:00Z"));
    assert_eq!(
        posts[0]
            .author
            .as_ref()
            .and_then(|a| a.avatar_url.as_deref()),
        Some("https://example.com/a.png")
    );
}

/// Submolt as object (API returns { id, name, display_name }).
#[test]
fn parse_feed_submolt_as_object() {
    let body = json!([
        {
            "id": "p1",
            "title": "T",
            "author": { "name": "A" },
            "submolt": { "id": "sub-uuid", "name": "general", "display_name": "General" }
        }
    ]);
    let posts: Vec<communityfeed::Post> = serde_json::from_value(body).expect("parse submolt object");
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].submolt.as_deref(), Some("general"));
}

/// Live API fetch when COMMUNITYFEED_API_KEY is set.
#[tokio::test]
async fn fetch_feed_live_if_key_set() {
    let api_key = std::env::var("COMMUNITYFEED_API_KEY").ok();
    let api_key = match api_key.as_deref() {
        Some(k) if !k.trim().is_empty() => k,
        _ => {
            eprintln!("skip: COMMUNITYFEED_API_KEY not set");
            return;
        }
    };

    let client = CommunityFeedClient::new(api_key.to_string()).expect("client");
    let posts = client
        .posts_feed(PostSort::New, Some(3), None)
        .await
        .expect("fetch feed");

    // API may return 0 or more posts
    assert!(posts.len() <= 3, "limit 3");
    for p in &posts {
        assert!(!p.id.is_empty(), "post id non-empty");
    }
}
