//! Forum board browsing, topic listing, search, and reading one topic.
//!
//! The routes are the ones `packages/openagents-cli/src/forum-client.ts` calls:
//! `GET /api/v1/forum` for boards, `GET /api/v1/forum/topics?forum=<slug>` for a
//! board's topics, `GET /api/v1/forum/topics?q=<query>` for search, and
//! `GET /api/v1/forum/topics/<id>` for one topic and its posts. Each accepts a
//! one-based `page`. An earlier version of this module called
//! `/api/v1/forum/boards`, which does not exist, and answered the resulting
//! non-2xx with a hardcoded pair of boards — inventing a `dev` board the server
//! has never served. Nothing here substitutes a value the server did not send: a
//! refusal is returned as [`ForumError`] and the command exits non-zero, and a
//! field the server omitted is `None` rather than a plausible default.
//!
//! The write half — `post`, `reply`, `claim`, `claims` — is deliberately absent.
//! It needs signed Nostr event authoring, which is out of scope here.

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

/// The board a search result belongs to, as the topic row named it.
///
/// The topic routes send this only on search results, where a row can come from
/// any board. Absent means the server did not send it, not `general`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumTopicBoard {
    pub slug: Option<String>,
    pub title: Option<String>,
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
    /// The topic's canonical web address, when the server sent one.
    pub url: Option<String>,
    /// The author's actor reference, which is not the display name.
    pub actor_ref: Option<String>,
    pub pinned: Option<bool>,
    pub tip_count: Option<u64>,
    pub tip_sats: Option<u64>,
    /// Which board the topic lives on. Search rows carry it; board listings,
    /// where the board is already the question, do not.
    pub board: Option<ForumTopicBoard>,
}

/// One post in a topic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumPost {
    pub id: String,
    /// The post's position in the topic. `None` when the server omitted it —
    /// printing `#0` for an unnumbered post would be a number nobody sent.
    pub post_number: Option<u64>,
    pub state: String,
    /// The topic this post belongs to, when the server named it.
    pub topic_id: Option<String>,
    pub author: Option<String>,
    pub actor_ref: Option<String>,
    pub body_text: Option<String>,
    pub created_at: Option<String>,
    pub url: Option<String>,
    pub tip_count: Option<u64>,
    pub tip_sats: Option<u64>,
}

/// The server's own account of where this page sits in the whole result.
///
/// Every field is optional because every field is the server's to send. A
/// listing that reports `page 1 of 5` when the server said nothing about pages
/// is the same class of defect as a board list nobody served.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ForumPagination {
    pub total: Option<u64>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
    pub total_pages: Option<u64>,
}

/// One page of topic rows, and the server's account of the rest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumTopicList {
    pub topics: Vec<ForumTopic>,
    pub pagination: Option<ForumPagination>,
}

/// One topic, one page of its posts, and the server's account of the rest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForumTopicPage {
    pub topic: ForumTopic,
    pub posts: Vec<ForumPost>,
    pub pagination: Option<ForumPagination>,
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
                    // The body is the server's, so the 400-byte bound has to
                    // land on a character boundary or rendering the refusal
                    // panics instead of reporting it.
                    let end = crate::tracker::floor_char_boundary(trimmed, 400);
                    write!(f, ": {}", &trimmed[..end])?;
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
        crate::diag::request("GET", &url);
        let resp = self
            .http
            .get(&url)
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| {
                crate::diag::transport(&url, &e.to_string());
                ForumError::Transport(e.to_string())
            })?;

        let status = resp.status();
        crate::diag::response(status.as_u16(), &url);
        let body = resp
            .text()
            .await
            .map_err(|e| ForumError::Transport(e.to_string()))?;

        if !status.is_success() {
            crate::diag::refused(status.as_u16(), &body);
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
    ///
    /// `page` is one-based and sent only when the caller asked for one, so an
    /// unpaged call gets whatever the server calls page one.
    pub async fn list_topics(
        &self,
        board: &str,
        page: Option<u32>,
    ) -> Result<ForumTopicList, ForumError> {
        let mut path = format!("forum/topics?forum={}", urlencode(board));
        push_page(&mut path, page);
        let body = self.get_json(&path).await?;
        parse_topic_list(&body)
    }

    /// Search topics, optionally within one board, optionally on a later page.
    pub async fn search_topics(
        &self,
        query: &str,
        board: Option<&str>,
        page: Option<u32>,
    ) -> Result<ForumTopicList, ForumError> {
        let mut path = format!("forum/topics?q={}", urlencode(query));
        if let Some(slug) = board {
            path.push_str(&format!("&forum={}", urlencode(slug)));
        }
        push_page(&mut path, page);
        let body = self.get_json(&path).await?;
        parse_topic_list(&body)
    }

    /// Read one topic and a page of its posts. The server accepts a full topic
    /// id or the prefix a topic URL starts with.
    pub async fn read_topic(
        &self,
        id: &str,
        page: Option<u32>,
    ) -> Result<ForumTopicPage, ForumError> {
        let mut path = format!("forum/topics/{}", urlencode(id));
        if let Some(number) = page {
            path.push_str(&format!("?page={}", number));
        }
        let body = self.get_json(&path).await?;

        let topic = body
            .get("topic")
            .filter(|v| v.is_object())
            .ok_or_else(|| ForumError::Malformed("no `topic` object in the response".into()))?;
        let posts = body
            .get("posts")
            .and_then(|v| v.as_array())
            .ok_or_else(|| ForumError::Malformed("no `posts` array in the response".into()))?;

        Ok(ForumTopicPage {
            topic: parse_topic(topic),
            posts: posts.iter().map(parse_post).collect(),
            pagination: parse_pagination(&body),
        })
    }
}

/// Append `&page=N`, and only when the caller named a page.
fn push_page(path: &mut String, page: Option<u32>) {
    if let Some(number) = page {
        path.push_str(&format!("&page={}", number));
    }
}

fn parse_topic_list(body: &serde_json::Value) -> Result<ForumTopicList, ForumError> {
    let items = body
        .get("topics")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ForumError::Malformed("no `topics` array in the response".into()))?;

    Ok(ForumTopicList {
        topics: items.iter().map(parse_topic).collect(),
        pagination: parse_pagination(body),
    })
}

/// Read the `pagination` object, or `None` when the server sent none.
fn parse_pagination(body: &serde_json::Value) -> Option<ForumPagination> {
    let block = body.get("pagination").filter(|v| v.is_object())?;
    Some(ForumPagination {
        total: optional_number(block, "total"),
        page: optional_number(block, "page"),
        per_page: optional_number(block, "per_page"),
        total_pages: optional_number(block, "total_pages"),
    })
}

fn parse_topic(item: &serde_json::Value) -> ForumTopic {
    ForumTopic {
        id: string_field(item, "id"),
        slug: string_field(item, "slug"),
        title: string_field(item, "title"),
        state: string_field(item, "state"),
        author: display_name(item),
        created_at: optional_string(item, "created_at"),
        updated_at: optional_string(item, "updated_at"),
        posts_count: number_field(item, "posts_count"),
        url: optional_string(item, "url"),
        actor_ref: optional_string(item, "actor_ref"),
        pinned: item.get("pinned").and_then(|v| v.as_bool()),
        tip_count: optional_number(item, "tip_count"),
        tip_sats: optional_number(item, "tip_sats"),
        board: item
            .get("board")
            .filter(|v| v.is_object())
            .map(|board| ForumTopicBoard {
                slug: optional_string(board, "slug"),
                title: optional_string(board, "title"),
            }),
    }
}

fn parse_post(item: &serde_json::Value) -> ForumPost {
    ForumPost {
        id: string_field(item, "id"),
        post_number: optional_number(item, "post_number"),
        state: string_field(item, "state"),
        topic_id: optional_string(item, "topic_id"),
        author: display_name(item),
        actor_ref: item
            .get("author")
            .and_then(|a| a.get("ref"))
            .and_then(|v| v.as_str())
            .map(String::from),
        body_text: optional_string(item, "body_text"),
        created_at: optional_string(item, "created_at"),
        url: optional_string(item, "url"),
        tip_count: optional_number(item, "tip_count"),
        tip_sats: optional_number(item, "tip_sats"),
    }
}

fn display_name(item: &serde_json::Value) -> Option<String> {
    item.get("author")
        .and_then(|a| a.get("display_name"))
        .and_then(|v| v.as_str())
        .map(String::from)
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

/// A field the server may not have sent at all. `None` is "not sent"; it is not
/// zero, not false, and not the empty string.
fn optional_string(item: &serde_json::Value, key: &str) -> Option<String> {
    item.get(key).and_then(|v| v.as_str()).map(String::from)
}

fn optional_number(item: &serde_json::Value, key: &str) -> Option<u64> {
    item.get(key).and_then(|v| v.as_u64())
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

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// The first eight characters of an id, which is how both CLIs print one.
///
/// The id comes from the server, so the bound is floored to a character
/// boundary. An id that is not a UUID would otherwise panic the listing.
pub fn short_id(id: &str) -> &str {
    &id[..crate::tracker::floor_char_boundary(id, 8)]
}

/// How much of a post body a listing line shows. The TypeScript CLI cuts at 120
/// (`packages/openagents-cli/src/cli.ts`, `forumTopicCommand`).
const BODY_PREVIEW: usize = 120;

/// `{short id} — {title} ({n} posts)`, then the server's page note.
pub fn topic_rows(list: &ForumTopicList) -> Vec<String> {
    if list.topics.is_empty() {
        return vec![empty_line("No topics found.", "topics", list.pagination)];
    }
    let mut lines: Vec<String> = list
        .topics
        .iter()
        .map(|t| {
            format!(
                "{} — {} ({} posts)",
                short_id(&t.id),
                t.title,
                t.posts_count
            )
        })
        .collect();
    lines.extend(page_note(list.pagination, list.topics.len(), "topics"));
    lines
}

/// `{short id} — {title} — {author} [{board}]`, then the server's page note.
///
/// The board suffix and the `?` for a missing author are what the TypeScript
/// CLI prints; a search row that dropped its board read as if every match came
/// from the same place.
pub fn search_rows(list: &ForumTopicList) -> Vec<String> {
    if list.topics.is_empty() {
        return vec![empty_line("No topics match.", "matches", list.pagination)];
    }
    let mut lines: Vec<String> = list
        .topics
        .iter()
        .map(|t| {
            let who = t.author.as_deref().unwrap_or("?");
            let where_ = t
                .board
                .as_ref()
                .and_then(|b| b.slug.as_deref())
                .map(|slug| format!(" [{}]", slug))
                .unwrap_or_default();
            format!("{} — {} — {}{}", short_id(&t.id), t.title, who, where_)
        })
        .collect();
    lines.extend(page_note(list.pagination, list.topics.len(), "matches"));
    lines
}

/// The topic's title, then `#{n} {author}: {body}` per post, then the page note.
pub fn topic_page_rows(page: &ForumTopicPage) -> Vec<String> {
    let mut lines = vec![page.topic.title.clone()];
    if page.posts.is_empty() {
        lines.push(empty_line(
            "No posts on this topic.",
            "posts",
            page.pagination,
        ));
        return lines;
    }
    for post in &page.posts {
        let number = post
            .post_number
            .map(|n| n.to_string())
            .unwrap_or_else(|| "?".to_string());
        let who = post.author.as_deref().unwrap_or("?");
        let body = post.body_text.as_deref().unwrap_or("");
        // The body is the server's, so the preview bound is floored to a
        // character boundary rather than cutting the raw bytes.
        let end = crate::tracker::floor_char_boundary(body, BODY_PREVIEW);
        lines.push(format!("#{} {}: {}", number, who, &body[..end]));
    }
    lines.extend(page_note(page.pagination, page.posts.len(), "posts"));
    lines
}

/// What to say when the server sent no rows.
///
/// An empty page of a board that has 107 topics is not an empty board, and
/// saying "no topics found" there would be the same silence this fixes. When
/// the server's own pagination shows there is something to find, the line says
/// where it is instead.
fn empty_line(nothing_at_all: &str, noun: &str, pagination: Option<ForumPagination>) -> String {
    let Some(page) = pagination else {
        return nothing_at_all.to_string();
    };
    match (page.total, page.page, page.total_pages) {
        (Some(total), Some(number), Some(pages)) if total > 0 => format!(
            "No {} on page {}. The server reports {} {} across {} pages.",
            noun, number, total, noun, pages
        ),
        (Some(total), Some(number), None) if total > 0 => format!(
            "No {} on page {}. The server reports {} {} in total.",
            noun, number, total, noun
        ),
        _ => nothing_at_all.to_string(),
    }
}

/// The line that says this page is not the whole result.
///
/// Without it a 25-row page of a 107-topic board reads as the whole board, and
/// nothing in the output disagrees. Built only from numbers the server sent: no
/// pagination block, no line.
fn page_note(pagination: Option<ForumPagination>, shown: usize, noun: &str) -> Option<String> {
    let page = pagination?;
    if let (Some(number), Some(pages)) = (page.page, page.total_pages) {
        if pages > 1 {
            let mut note = match page.total {
                Some(total) => format!("Page {} of {} — {} {}", number, pages, total, noun),
                None => format!("Page {} of {}", number, pages),
            };
            if number < pages {
                note.push_str(&format!(". Pass --page {} for the next.", number + 1));
            } else {
                note.push('.');
            }
            return Some(note);
        }
        return None;
    }
    // No page count, but a total that the rows on screen do not account for.
    match page.total {
        Some(total) if total > shown as u64 => Some(format!(
            "The server reports {} {} in total; this page has {}.",
            total, noun, shown
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// `--json`
// ---------------------------------------------------------------------------

/// The `--json` shape for a topic list.
///
/// The client parses the server's body into typed rows, so unlike the tracker
/// there is no verbatim body to hand back. A key appears here only when the
/// server sent it: an absent `board` is an absent key, never `null` standing in
/// for a board the row never named.
pub fn topic_list_value(list: &ForumTopicList) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    out.insert(
        "topics".to_string(),
        serde_json::Value::Array(list.topics.iter().map(topic_value).collect()),
    );
    if let Some(page) = list.pagination {
        out.insert("pagination".to_string(), pagination_value(page));
    }
    serde_json::Value::Object(out)
}

/// The `--json` shape for one topic and a page of its posts.
pub fn topic_page_value(page: &ForumTopicPage) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    out.insert("topic".to_string(), topic_value(&page.topic));
    out.insert(
        "posts".to_string(),
        serde_json::Value::Array(page.posts.iter().map(post_value).collect()),
    );
    if let Some(pagination) = page.pagination {
        out.insert("pagination".to_string(), pagination_value(pagination));
    }
    serde_json::Value::Object(out)
}

fn topic_value(topic: &ForumTopic) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    out.insert("id".to_string(), topic.id.clone().into());
    out.insert("slug".to_string(), topic.slug.clone().into());
    out.insert("title".to_string(), topic.title.clone().into());
    out.insert("state".to_string(), topic.state.clone().into());
    out.insert(
        "author".to_string(),
        match &topic.author {
            Some(name) => name.clone().into(),
            None => serde_json::Value::Null,
        },
    );
    insert_string(&mut out, "created_at", &topic.created_at);
    insert_string(&mut out, "updated_at", &topic.updated_at);
    out.insert("posts_count".to_string(), topic.posts_count.into());
    insert_string(&mut out, "url", &topic.url);
    insert_string(&mut out, "actor_ref", &topic.actor_ref);
    if let Some(pinned) = topic.pinned {
        out.insert("pinned".to_string(), pinned.into());
    }
    insert_number(&mut out, "tip_count", topic.tip_count);
    insert_number(&mut out, "tip_sats", topic.tip_sats);
    if let Some(board) = &topic.board {
        let mut nested = serde_json::Map::new();
        insert_string(&mut nested, "slug", &board.slug);
        insert_string(&mut nested, "title", &board.title);
        out.insert("board".to_string(), serde_json::Value::Object(nested));
    }
    serde_json::Value::Object(out)
}

fn post_value(post: &ForumPost) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    out.insert("id".to_string(), post.id.clone().into());
    insert_number(&mut out, "post_number", post.post_number);
    out.insert("state".to_string(), post.state.clone().into());
    insert_string(&mut out, "topic_id", &post.topic_id);
    out.insert(
        "author".to_string(),
        match &post.author {
            Some(name) => name.clone().into(),
            None => serde_json::Value::Null,
        },
    );
    insert_string(&mut out, "actor_ref", &post.actor_ref);
    insert_string(&mut out, "body_text", &post.body_text);
    insert_string(&mut out, "created_at", &post.created_at);
    insert_string(&mut out, "url", &post.url);
    insert_number(&mut out, "tip_count", post.tip_count);
    insert_number(&mut out, "tip_sats", post.tip_sats);
    serde_json::Value::Object(out)
}

fn pagination_value(page: ForumPagination) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    insert_number(&mut out, "total", page.total);
    insert_number(&mut out, "page", page.page);
    insert_number(&mut out, "per_page", page.per_page);
    insert_number(&mut out, "total_pages", page.total_pages);
    serde_json::Value::Object(out)
}

fn insert_string(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &Option<String>,
) {
    if let Some(text) = value {
        map.insert(key.to_string(), text.clone().into());
    }
}

fn insert_number(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<u64>,
) {
    if let Some(number) = value {
        map.insert(key.to_string(), number.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(body: &str) -> ForumTopicList {
        parse_topic_list(&serde_json::from_str(body).unwrap()).unwrap()
    }

    /// A search row keeps the six fields the struct used to drop.
    ///
    /// The old `ForumTopic` modelled eight fields and re-encoded them, so
    /// `board`, `url`, `pinned`, `tip_count`, `tip_sats`, and `actor_ref` never
    /// reached `--json`. Each assertion below names the value this fixture sent.
    #[test]
    fn a_search_row_carries_the_fields_the_server_sent() {
        let parsed = list(
            r#"{"topics":[{"id":"9946bf38-788b-45f3-b17b-b0e36bb8dc60",
                "slug":"why-not","title":"Why not?","state":"open",
                "author":{"ref":"agent:user_b3ce","display_name":"Sneaky"},
                "url":"https://openagents.com/forum/t/9946bf38",
                "actor_ref":"agent:user_b3ce","pinned":true,
                "tip_count":3,"tip_sats":210,"posts_count":14,
                "board":{"title":"Work Requests","slug":"work-requests"}}]}"#,
        );
        let row = &parsed.topics[0];
        assert_eq!(
            row.url.as_deref(),
            Some("https://openagents.com/forum/t/9946bf38")
        );
        assert_eq!(row.actor_ref.as_deref(), Some("agent:user_b3ce"));
        assert_eq!(row.pinned, Some(true));
        assert_eq!(row.tip_count, Some(3));
        assert_eq!(row.tip_sats, Some(210));
        assert_eq!(
            row.board.as_ref().and_then(|b| b.slug.as_deref()),
            Some("work-requests")
        );

        let json = topic_list_value(&parsed);
        assert_eq!(json["topics"][0]["pinned"], true);
        assert_eq!(json["topics"][0]["tip_sats"], 210);
        assert_eq!(json["topics"][0]["tip_count"], 3);
        assert_eq!(json["topics"][0]["actor_ref"], "agent:user_b3ce");
        assert_eq!(
            json["topics"][0]["url"],
            "https://openagents.com/forum/t/9946bf38"
        );
        assert_eq!(json["topics"][0]["board"]["slug"], "work-requests");

        // And the human line carries the board suffix the TypeScript CLI
        // prints. Recorded from `openagents forum search "acceptance gate"`:
        //   9946bf38 — Why are you not running … — Sneaky [work-requests]
        assert_eq!(
            search_rows(&parsed)[0],
            "9946bf38 — Why not? — Sneaky [work-requests]"
        );
    }

    /// A field the server withheld stays absent rather than becoming a default.
    ///
    /// `pinned: false` and `tip_sats: 0` are values. Printing them for a row
    /// that carried neither is the fabrication this module exists to avoid.
    #[test]
    fn a_field_the_server_withheld_is_absent_not_defaulted() {
        let parsed =
            list(r#"{"topics":[{"id":"abc","title":"t","state":"open","posts_count":1}]}"#);
        let row = &parsed.topics[0];
        assert_eq!(row.pinned, None);
        assert_eq!(row.tip_sats, None);
        assert!(row.board.is_none());

        let json = topic_list_value(&parsed);
        let object = json["topics"][0].as_object().unwrap();
        assert!(
            !object.contains_key("pinned"),
            "invented `pinned`: {object:?}"
        );
        assert!(!object.contains_key("tip_sats"));
        assert!(!object.contains_key("board"));

        // With no board the suffix is empty, and the author falls back to `?`,
        // which is what the TypeScript CLI prints.
        assert_eq!(search_rows(&parsed)[0], "abc — t — ?");
    }

    /// A page of a longer result says so, and names the page to ask for next.
    #[test]
    fn a_page_that_is_not_the_whole_board_says_which_page_it_is() {
        let parsed = list(
            r#"{"topics":[{"id":"a1b2c3d4","title":"one","state":"open","posts_count":2}],
                "pagination":{"total":107,"page":1,"per_page":25,"total_pages":5}}"#,
        );
        let rows = topic_rows(&parsed);
        assert_eq!(rows[0], "a1b2c3d4 — one (2 posts)");
        assert_eq!(
            rows[1],
            "Page 1 of 5 — 107 topics. Pass --page 2 for the next.",
        );

        // The server's own numbers reach `--json` too.
        let json = topic_list_value(&parsed);
        assert_eq!(json["pagination"]["total"], 107);
        assert_eq!(json["pagination"]["total_pages"], 5);
        assert_eq!(json["pagination"]["page"], 1);
    }

    /// The last page does not advertise a page that does not exist.
    #[test]
    fn the_last_page_does_not_point_past_itself() {
        let parsed = list(
            r#"{"topics":[{"id":"z","title":"last","state":"open","posts_count":1}],
                "pagination":{"total":107,"page":5,"per_page":25,"total_pages":5}}"#,
        );
        let rows = topic_rows(&parsed);
        assert_eq!(rows[1], "Page 5 of 5 — 107 topics.");
        assert!(!rows[1].contains("--page 6"), "{}", rows[1]);
    }

    /// A single-page result says nothing about paging, because there is none.
    #[test]
    fn a_result_that_fits_on_one_page_gets_no_page_note() {
        let parsed = list(
            r#"{"topics":[{"id":"z","title":"only","state":"open","posts_count":1}],
                "pagination":{"total":1,"page":1,"per_page":25,"total_pages":1}}"#,
        );
        assert_eq!(topic_rows(&parsed).len(), 1);
    }

    /// A server that sent no pagination gets no invented page numbers.
    #[test]
    fn no_pagination_block_means_no_page_line() {
        let parsed =
            list(r#"{"topics":[{"id":"z","title":"only","state":"open","posts_count":1}]}"#);
        assert_eq!(topic_rows(&parsed), vec!["z — only (1 posts)"]);
        assert!(topic_list_value(&parsed).get("pagination").is_none());
    }

    /// An empty page of a board with topics is not reported as an empty board.
    ///
    /// The live route answers `?page=9` on a five-page board with `topics: []`
    /// and the same pagination block. "No topics found." there is false.
    #[test]
    fn an_empty_page_of_a_full_board_reports_the_board_not_a_void() {
        let parsed = list(
            r#"{"topics":[],"pagination":{"total":107,"page":9,"per_page":25,"total_pages":5}}"#,
        );
        assert_eq!(
            topic_rows(&parsed),
            vec!["No topics on page 9. The server reports 107 topics across 5 pages."]
        );
    }

    /// A board the server really says is empty still reads as empty.
    #[test]
    fn a_board_the_server_says_is_empty_reads_as_empty() {
        let parsed = list(
            r#"{"topics":[],"pagination":{"total":0,"page":1,"per_page":25,"total_pages":0}}"#,
        );
        assert_eq!(topic_rows(&parsed), vec!["No topics found."]);
    }

    /// A body with no `topics` array is malformed, not an empty board.
    #[test]
    fn a_body_without_topics_is_malformed_rather_than_empty() {
        let error = parse_topic_list(&serde_json::json!({"pagination": {"total": 3}}))
            .expect_err("a body with no `topics` array must not read as an empty board");
        match error {
            ForumError::Malformed(why) => assert!(why.contains("topics"), "{why}"),
            other => panic!("expected malformed, got {other}"),
        }
    }

    /// The topic reader prints the title, then one line per post.
    ///
    /// Recorded from `openagents forum topic 9946bf38-…`:
    ///   Why are you not running agents around the clock?
    ///   #1 Sneaky: A quiet observation for the agents arriving today: …
    #[test]
    fn a_topic_renders_its_title_then_its_posts() {
        let body = serde_json::json!({
            "topic": {"id": "9946bf38-788b", "title": "Why not?", "state": "open",
                      "posts_count": 2, "slug": "why-not"},
            "posts": [
                {"id": "p1", "post_number": 1, "state": "visible",
                 "author": {"display_name": "Sneaky", "ref": "agent:user_b3ce"},
                 "body_text": "A quiet observation.", "tip_sats": 21},
                {"id": "p2", "post_number": 2, "state": "visible",
                 "body_text": "Seconding this."}
            ],
            "pagination": {"total": 14, "page": 1, "per_page": 50, "total_pages": 1}
        });
        let page = ForumTopicPage {
            topic: parse_topic(&body["topic"]),
            posts: body["posts"]
                .as_array()
                .unwrap()
                .iter()
                .map(parse_post)
                .collect(),
            pagination: parse_pagination(&body),
        };
        assert_eq!(
            topic_page_rows(&page),
            vec![
                "Why not?",
                "#1 Sneaky: A quiet observation.",
                "#2 ?: Seconding this.",
            ]
        );

        let json = topic_page_value(&page);
        assert_eq!(json["topic"]["title"], "Why not?");
        assert_eq!(json["posts"][0]["actor_ref"], "agent:user_b3ce");
        assert_eq!(json["posts"][0]["tip_sats"], 21);
        assert!(
            json["posts"][1]
                .as_object()
                .unwrap()
                .get("tip_sats")
                .is_none(),
            "invented a tip count the server never sent"
        );
        assert_eq!(json["pagination"]["total"], 14);
    }

    /// The post preview is cut on a character boundary, not a byte index.
    ///
    /// A body whose 120th byte lands inside a multi-byte character used to
    /// abort the whole listing rather than shorten one line.
    #[test]
    fn a_post_preview_does_not_split_a_character() {
        let mut body = "A".repeat(119);
        body.push('é');
        body.push_str(&"B".repeat(40));
        assert!(
            !body.is_char_boundary(BODY_PREVIEW),
            "the fixture proves nothing"
        );

        let page = ForumTopicPage {
            topic: parse_topic(&serde_json::json!({"title": "t"})),
            posts: vec![parse_post(&serde_json::json!({
                "id": "p1", "post_number": 1, "body_text": body,
                "author": {"display_name": "Whoever"}
            }))],
            pagination: None,
        };
        let rows = topic_page_rows(&page);
        assert_eq!(rows[1], format!("#1 Whoever: {}", "A".repeat(119)));
    }

    /// The page number reaches the query string, and only when asked for.
    #[test]
    fn a_page_is_sent_only_when_the_caller_named_one() {
        let mut unpaged = "forum/topics?forum=general".to_string();
        push_page(&mut unpaged, None);
        assert_eq!(unpaged, "forum/topics?forum=general");

        let mut paged = "forum/topics?forum=general".to_string();
        push_page(&mut paged, Some(3));
        assert_eq!(paged, "forum/topics?forum=general&page=3");
    }
}
