//! Forum board browsing and topic listing.
//!
//! The routes are the ones `packages/openagents-cli/src/forum-client.ts` calls:
//! `GET /api/v1/forum` for boards and `GET /api/v1/forum/topics?forum=<slug>` for a
//! board's topics. An earlier version of this module called `/api/v1/forum/boards`,
//! which does not exist, and answered the resulting non-2xx with a hardcoded pair of
//! boards — inventing a `dev` board the server has never served. Nothing here
//! substitutes a value the server did not send: a refusal is returned as
//! [`ForumError`] and the command exits non-zero.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumBoard {
    /// The board's stable UUID.
    pub id: String,
    /// The URL-safe short name, and what `oa forum topics --board` takes.
    pub slug: String,
    /// The board's display title.
    pub title: String,
    pub description: String,
    pub topic_count: u64,
    pub post_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumTopic {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub state: String,
    /// The author's display name as the server rendered it, when it sent one.
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub posts_count: u64,
}

/// Why a forum read did not produce data. Never a substitute for data.
#[derive(Debug)]
pub enum ForumError {
    /// The request never completed.
    Transport(String),
    /// The server answered, and refused. Carries the status and its message.
    Refused { status: u16, body: String },
    /// The server answered 2xx with a body this client cannot read.
    Malformed(String),
}

impl fmt::Display for ForumError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(why) => write!(f, "Could not reach the forum API: {}", why),
            Self::Refused { status, body } => {
                write!(f, "The forum API refused the request (HTTP {})", status)?;
                let trimmed = body.trim();
                if !trimmed.is_empty() {
                    write!(f, ": {}", &trimmed[..trimmed.len().min(400)])?;
                }
                Ok(())
            }
            Self::Malformed(why) => write!(f, "The forum API returned an unreadable body: {}", why),
        }
    }
}

impl std::error::Error for ForumError {}

pub struct ForumClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl ForumClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", tok)) {
                map.insert(AUTHORIZATION, val);
            }
        }
        map
    }

    /// `GET` a forum route and return its parsed body, or the server's refusal.
    async fn get_json(&self, path: &str) -> Result<serde_json::Value, ForumError> {
        let url = format!("{}/{}", self.api_base, path);
        let resp = self
            .http
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| ForumError::Transport(e.to_string()))?;

        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ForumError::Transport(e.to_string()))?;

        if !status.is_success() {
            return Err(ForumError::Refused {
                status: status.as_u16(),
                body,
            });
        }
        serde_json::from_str(&body).map_err(|e| ForumError::Malformed(e.to_string()))
    }

    pub async fn list_boards(&self) -> Result<Vec<ForumBoard>, ForumError> {
        let body = self.get_json("forum").await?;
        let items = body
            .get("boards")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ForumError::Malformed("no `boards` array in the response".into()))?;

        Ok(items
            .iter()
            .map(|item| ForumBoard {
                id: string_field(item, "id"),
                slug: string_field(item, "slug"),
                title: string_field(item, "title"),
                description: string_field(item, "description"),
                topic_count: number_field(item, "topic_count"),
                post_count: number_field(item, "post_count"),
            })
            .collect())
    }

    /// List a board's topics. `board` is a slug, as `list_boards` reports it.
    pub async fn list_topics(&self, board: &str) -> Result<Vec<ForumTopic>, ForumError> {
        let body = self
            .get_json(&format!("forum/topics?forum={}", urlencode(board)))
            .await?;
        let items = body
            .get("topics")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ForumError::Malformed("no `topics` array in the response".into()))?;

        Ok(items.iter().map(parse_topic).collect())
    }

    /// Search topics across boards.
    pub async fn search_topics(&self, query: &str) -> Result<Vec<ForumTopic>, ForumError> {
        let body = self
            .get_json(&format!("forum/topics?q={}", urlencode(query)))
            .await?;
        let items = body
            .get("topics")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ForumError::Malformed("no `topics` array in the response".into()))?;

        Ok(items.iter().map(parse_topic).collect())
    }
}

fn parse_topic(item: &serde_json::Value) -> ForumTopic {
    ForumTopic {
        id: string_field(item, "id"),
        slug: string_field(item, "slug"),
        title: string_field(item, "title"),
        state: string_field(item, "state"),
        author: item
            .get("author")
            .and_then(|a| a.get("display_name"))
            .and_then(|v| v.as_str())
            .map(String::from),
        created_at: item.get("created_at").and_then(|v| v.as_str()).map(String::from),
        updated_at: item.get("updated_at").and_then(|v| v.as_str()).map(String::from),
        posts_count: number_field(item, "posts_count"),
    }
}

/// Read a string field, or the empty string when the server omitted it. The empty
/// string is what the server sent — it is not a stand-in for a value it withheld.
fn string_field(item: &serde_json::Value, key: &str) -> String {
    item.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn number_field(item: &serde_json::Value, key: &str) -> u64 {
    item.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// Percent-encode a query-string value. Only unreserved characters pass through.
fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}
