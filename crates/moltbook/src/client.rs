//! HTTP client for the Moltbook API.

use crate::error::{MoltbookError, Result};
use crate::types::*;
use reqwest::Client;
use std::time::Duration;

/// Default Moltbook API base URL (must use www to avoid redirect stripping auth).
const DEFAULT_BASE_URL: &str = "https://www.moltbook.com/api/v1";
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Moltbook API client.
///
/// All methods except `register` require a valid API key (obtained from registration).
/// Use `https://www.moltbook.com` (with www); redirects from moltbook.com can strip the auth header.
#[derive(Clone)]
pub struct MoltbookClient {
    base_url: String,
    api_key: Option<String>,
    http: Client,
}

impl MoltbookClient {
    /// Create a new client with the given API key (for all authenticated endpoints).
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url(DEFAULT_BASE_URL, Some(api_key.into()))
    }

    /// Create a client without an API key (only `register` will work).
    pub fn unauthenticated() -> Result<Self> {
        Self::with_base_url(DEFAULT_BASE_URL, None)
    }

    /// Create a client with a custom base URL and optional API key.
    pub fn with_base_url(
        base_url: impl Into<String>,
        api_key: Option<String>,
    ) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .map_err(MoltbookError::Http)?;
        Ok(Self {
            base_url: base_url.into(),
            api_key,
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        let base = self.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        format!("{base}/{path}")
    }

    fn auth_header(&self) -> Result<reqwest::header::HeaderValue> {
        let key = self
            .api_key
            .as_deref()
            .ok_or_else(|| MoltbookError::Api {
                status: 401,
                error: "API key required".to_string(),
                hint: Some("Use MoltbookClient::new(api_key) or register first.".to_string()),
            })?;
        let value = format!("Bearer {key}");
        reqwest::header::HeaderValue::from_str(&value).map_err(|_| MoltbookError::Api {
            status: 401,
            error: "Invalid API key".to_string(),
            hint: None,
        })
    }

    async fn check_response(&self, response: reqwest::Response) -> Result<reqwest::Response> {
        let status = response.status();
        if status.as_u16() == 429 {
            let body = response.text().await.map_err(MoltbookError::Http)?;
            let parsed: ApiErrorBody = serde_json::from_str(&body).unwrap_or(ApiErrorBody {
                _success: false,
                error: Some(body),
                hint: None,
                retry_after_minutes: None,
            });
            return Err(MoltbookError::RateLimited {
                retry_after_minutes: parsed.retry_after_minutes.unwrap_or(30),
            });
        }
        if !status.is_success() {
            let body = response.text().await.map_err(MoltbookError::Http)?;
            let parsed: ApiErrorBody = serde_json::from_str(&body).unwrap_or(ApiErrorBody {
                _success: false,
                error: Some(body.clone()),
                hint: None,
                retry_after_minutes: None,
            });
            return Err(MoltbookError::Api {
                status: status.as_u16(),
                error: parsed.error.unwrap_or(body),
                hint: parsed.hint,
            });
        }
        Ok(response)
    }

    // ---------- Registration (no auth) ----------

    /// Register a new agent. Returns API key and claim URL; save the API key for all other requests.
    pub async fn register(&self, request: RegisterRequest) -> Result<RegisterResponse> {
        let url = self.url("agents/register");
        let response = self
            .http
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    // ---------- Agents ----------

    /// Get the current agent's profile (requires API key).
    pub async fn agents_me(&self) -> Result<Agent> {
        let url = self.url("agents/me");
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let envelope: crate::types::AgentsMeEnvelope = response.json().await.map_err(MoltbookError::Http)?;
        envelope
            .into_agent()
            .ok_or_else(|| MoltbookError::Api {
                status: 200,
                error: "agents/me response missing 'data' and 'agent'".to_string(),
                hint: None,
            })
    }

    /// Check claim status (pending_claim or claimed).
    pub async fn agents_status(&self) -> Result<ClaimStatusResponse> {
        let url = self.url("agents/status");
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// View another agent's profile by name.
    pub async fn agents_profile(&self, name: &str) -> Result<ProfileResponse> {
        let url = self.url("agents/profile");
        let response = self
            .http
            .get(&url)
            .query(&[("name", name)])
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Update current agent profile (PATCH). Only send fields you want to change.
    pub async fn agents_me_update(&self, request: UpdateProfileRequest) -> Result<Agent> {
        let url = self.url("agents/me");
        let response = self
            .http
            .patch(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .json(&request)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Upload avatar (multipart; max 500 KB; JPEG/PNG/GIF/WebP).
    pub async fn agents_me_avatar_upload(&self, image_bytes: &[u8], filename: &str) -> Result<Agent> {
        let url = self.url("agents/me/avatar");
        let part = reqwest::multipart::Part::bytes(image_bytes.to_vec())
            .file_name(filename.to_string())
            .mime_str("image/png")
            .map_err(MoltbookError::Http)?;
        let form = reqwest::multipart::Form::new().part("file", part);
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .multipart(form)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Remove current agent's avatar.
    pub async fn agents_me_avatar_remove(&self) -> Result<Agent> {
        let url = self.url("agents/me/avatar");
        let response = self
            .http
            .delete(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Follow another agent by name.
    pub async fn agents_follow(&self, molty_name: &str) -> Result<ActionResponse> {
        let url = self.url(&format!("agents/{molty_name}/follow"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Unfollow an agent by name.
    pub async fn agents_unfollow(&self, molty_name: &str) -> Result<ActionResponse> {
        let url = self.url(&format!("agents/{molty_name}/follow"));
        let response = self
            .http
            .delete(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    // ---------- Posts ----------

    /// Create a text or link post.
    pub async fn posts_create(&self, request: CreatePostRequest) -> Result<Post> {
        let url = self.url("posts");
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .json(&request)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Get global feed or filter by submolt. Sort: hot, new, top, rising.
    pub async fn posts_feed(
        &self,
        sort: PostSort,
        limit: Option<u32>,
        submolt: Option<&str>,
    ) -> Result<Vec<Post>> {
        let url = self.url("posts");
        let mut query: Vec<(&str, String)> = vec![("sort", sort.as_str().to_string())];
        if let Some(n) = limit {
            query.push(("limit", n.to_string()));
        }
        if let Some(s) = submolt {
            query.push(("submolt", s.to_string()));
        }
        let response = self
            .http
            .get(&url)
            .query(&query)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body: PostsResponse = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body.into_posts())
    }

    /// Get feed for a specific submolt (convenience endpoint).
    pub async fn submolt_feed(
        &self,
        submolt_name: &str,
        sort: PostSort,
        limit: Option<u32>,
    ) -> Result<Vec<Post>> {
        let url = self.url(&format!("submolts/{submolt_name}/feed"));
        let mut query: Vec<(&str, String)> = vec![("sort", sort.as_str().to_string())];
        if let Some(n) = limit {
            query.push(("limit", n.to_string()));
        }
        let response = self
            .http
            .get(&url)
            .query(&query)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body: PostsResponse = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body.into_posts())
    }

    /// Get a single post by ID.
    pub async fn posts_get(&self, post_id: &str) -> Result<Post> {
        let url = self.url(&format!("posts/{post_id}"));
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Delete your own post.
    pub async fn posts_delete(&self, post_id: &str) -> Result<()> {
        let url = self.url(&format!("posts/{post_id}"));
        let response = self
            .http
            .delete(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Upvote a post.
    pub async fn posts_upvote(&self, post_id: &str) -> Result<ActionResponse> {
        let url = self.url(&format!("posts/{post_id}/upvote"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Downvote a post.
    pub async fn posts_downvote(&self, post_id: &str) -> Result<ActionResponse> {
        let url = self.url(&format!("posts/{post_id}/downvote"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Pin a post (mod only; max 3 per submolt).
    pub async fn posts_pin(&self, post_id: &str) -> Result<()> {
        let url = self.url(&format!("posts/{post_id}/pin"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Unpin a post (mod only).
    pub async fn posts_unpin(&self, post_id: &str) -> Result<()> {
        let url = self.url(&format!("posts/{post_id}/pin"));
        let response = self
            .http
            .delete(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        self.check_response(response).await?;
        Ok(())
    }

    // ---------- Comments ----------

    /// Add a comment or reply to a post.
    pub async fn comments_create(&self, post_id: &str, request: CreateCommentRequest) -> Result<Comment> {
        let url = self.url(&format!("posts/{post_id}/comments"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .json(&request)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Get comments on a post (sort: top, new, controversial).
    pub async fn comments_list(
        &self,
        post_id: &str,
        sort: CommentSort,
        limit: Option<u32>,
    ) -> Result<Vec<Comment>> {
        let url = self.url(&format!("posts/{post_id}/comments"));
        let mut query: Vec<(&str, String)> = vec![("sort", sort.as_str().to_string())];
        if let Some(n) = limit {
            query.push(("limit", n.to_string()));
        }
        let response = self
            .http
            .get(&url)
            .query(&query)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body: CommentsResponse = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body.into_comments())
    }

    /// Upvote a comment.
    pub async fn comments_upvote(&self, comment_id: &str) -> Result<ActionResponse> {
        let url = self.url(&format!("comments/{comment_id}/upvote"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    // ---------- Feed (personalized) ----------

    /// Get personalized feed (subscribed submolts + followed moltys). Sort: hot, new, top.
    pub async fn feed(&self, sort: PostSort, limit: Option<u32>) -> Result<Vec<Post>> {
        let url = self.url("feed");
        let mut query: Vec<(&str, String)> = vec![("sort", sort.as_str().to_string())];
        if let Some(n) = limit {
            query.push(("limit", n.to_string()));
        }
        let response = self
            .http
            .get(&url)
            .query(&query)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body: FeedResponse = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body.into_posts())
    }

    // ---------- Search ----------

    /// Search posts, agents, and submolts.
    pub async fn search(&self, q: &str, limit: Option<u32>) -> Result<SearchResponse> {
        let url = self.url("search");
        let mut query: Vec<(&str, String)> = vec![("q", q.to_string())];
        if let Some(n) = limit {
            query.push(("limit", n.to_string()));
        }
        let response = self
            .http
            .get(&url)
            .query(&query)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    // ---------- Submolts ----------

    /// Create a submolt (community).
    pub async fn submolts_create(&self, request: CreateSubmoltRequest) -> Result<Submolt> {
        let url = self.url("submolts");
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .json(&request)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// List all submolts.
    pub async fn submolts_list(&self) -> Result<Vec<Submolt>> {
        let url = self.url("submolts");
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body: SubmoltsResponse = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body.into_submolts())
    }

    /// Get a single submolt by name.
    pub async fn submolts_get(&self, name: &str) -> Result<Submolt> {
        let url = self.url(&format!("submolts/{name}"));
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Subscribe to a submolt.
    pub async fn submolts_subscribe(&self, submolt_name: &str) -> Result<()> {
        let url = self.url(&format!("submolts/{submolt_name}/subscribe"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Unsubscribe from a submolt.
    pub async fn submolts_unsubscribe(&self, submolt_name: &str) -> Result<()> {
        let url = self.url(&format!("submolts/{submolt_name}/subscribe"));
        let response = self
            .http
            .delete(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Update submolt settings (mod). JSON only; use submolts_settings_upload_avatar/banner for files.
    pub async fn submolts_settings_update(
        &self,
        submolt_name: &str,
        request: SubmoltSettingsRequest,
    ) -> Result<Submolt> {
        let url = self.url(&format!("submolts/{submolt_name}/settings"));
        let response = self
            .http
            .patch(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .json(&request)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Upload submolt avatar (multipart; max 500 KB).
    pub async fn submolts_settings_upload_avatar(
        &self,
        submolt_name: &str,
        image_bytes: &[u8],
        filename: &str,
    ) -> Result<Submolt> {
        let url = self.url(&format!("submolts/{submolt_name}/settings"));
        let part = reqwest::multipart::Part::bytes(image_bytes.to_vec())
            .file_name(filename.to_string())
            .mime_str("image/png")
            .map_err(MoltbookError::Http)?;
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("type", "avatar");
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .multipart(form)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Upload submolt banner (multipart; max 2 MB).
    pub async fn submolts_settings_upload_banner(
        &self,
        submolt_name: &str,
        image_bytes: &[u8],
        filename: &str,
    ) -> Result<Submolt> {
        let url = self.url(&format!("submolts/{submolt_name}/settings"));
        let part = reqwest::multipart::Part::bytes(image_bytes.to_vec())
            .file_name(filename.to_string())
            .mime_str("image/jpeg")
            .map_err(MoltbookError::Http)?;
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("type", "banner");
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .multipart(form)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }

    /// Add a moderator (owner only).
    pub async fn submolts_moderators_add(
        &self,
        submolt_name: &str,
        request: ModeratorRequest,
    ) -> Result<()> {
        let url = self.url(&format!("submolts/{submolt_name}/moderators"));
        let response = self
            .http
            .post(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .json(&request)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        self.check_response(response).await?;
        Ok(())
    }

    /// Remove a moderator (owner only).
    pub async fn submolts_moderators_remove(
        &self,
        submolt_name: &str,
        agent_name: &str,
    ) -> Result<()> {
        let url = self.url(&format!("submolts/{submolt_name}/moderators"));
        let response = self
            .http
            .delete(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .json(&ModeratorRequest {
                agent_name: agent_name.to_string(),
                role: None,
            })
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        self.check_response(response).await?;
        Ok(())
    }

    /// List moderators of a submolt.
    pub async fn submolts_moderators_list(&self, submolt_name: &str) -> Result<Vec<Moderator>> {
        let url = self.url(&format!("submolts/{submolt_name}/moderators"));
        let response = self
            .http
            .get(&url)
            .header(reqwest::header::AUTHORIZATION, self.auth_header()?)
            .send()
            .await
            .map_err(MoltbookError::Http)?;
        let response = self.check_response(response).await?;
        let body = response.json().await.map_err(MoltbookError::Http)?;
        Ok(body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_builds_with_api_key() {
        let _ = MoltbookClient::new("moltbook_test_key").expect("client with key");
    }

    #[test]
    fn client_builds_unauthenticated() {
        let _ = MoltbookClient::unauthenticated().expect("unauthenticated client");
    }

    #[test]
    fn post_sort_as_str() {
        assert_eq!(PostSort::Hot.as_str(), "hot");
        assert_eq!(PostSort::New.as_str(), "new");
        assert_eq!(PostSort::Top.as_str(), "top");
        assert_eq!(PostSort::Rising.as_str(), "rising");
    }

    #[test]
    fn comment_sort_as_str() {
        assert_eq!(CommentSort::Top.as_str(), "top");
        assert_eq!(CommentSort::New.as_str(), "new");
        assert_eq!(CommentSort::Controversial.as_str(), "controversial");
    }
}
