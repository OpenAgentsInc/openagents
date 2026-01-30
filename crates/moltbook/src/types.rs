//! Request and response types for the Moltbook API.

use serde::{Deserialize, Serialize};

// ---------- Registration ----------

/// Request body for agent registration (no auth).
#[derive(Debug, Clone, Serialize)]
pub struct RegisterRequest {
    pub name: String,
    pub description: String,
}

/// Response from agent registration.
#[derive(Debug, Clone, Deserialize)]
pub struct RegisterResponse {
    pub agent: RegisterAgent,
    #[serde(default)]
    pub important: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegisterAgent {
    pub api_key: String,
    pub claim_url: String,
    pub verification_code: String,
}

// ---------- Claim status ----------

/// Claim status response.
#[derive(Debug, Clone, Deserialize)]
pub struct ClaimStatusResponse {
    pub status: ClaimStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    PendingClaim,
    Claimed,
}

// ---------- Agent / profile ----------

/// Response envelope for GET /agents/me (API returns { success, data } or { agent }).
#[derive(Debug, Clone, Deserialize)]
pub(super) struct AgentsMeEnvelope {
    #[serde(default)]
    pub data: Option<Agent>,
    #[serde(default)]
    pub agent: Option<Agent>,
}

impl AgentsMeEnvelope {
    pub(super) fn into_agent(self) -> Option<Agent> {
        self.data.or(self.agent)
    }
}

/// Stats nested under agent (GET /agents/me returns agent.stats).
#[derive(Debug, Clone, Deserialize)]
pub struct AgentStats {
    #[serde(default)]
    pub posts: Option<u64>,
    #[serde(default)]
    pub comments: Option<u64>,
    #[serde(default)]
    pub subscriptions: Option<u64>,
}

/// Current agent (from GET /agents/me).
#[derive(Debug, Clone, Deserialize)]
pub struct Agent {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub karma: Option<i64>,
    #[serde(default)]
    pub follower_count: Option<u64>,
    #[serde(default)]
    pub following_count: Option<u64>,
    #[serde(default)]
    pub is_claimed: Option<bool>,
    #[serde(default)]
    pub is_active: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_active: Option<String>,
    #[serde(default)]
    pub owner: Option<AgentOwner>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// Post/comment/subscription counts (API returns agent.stats).
    #[serde(default)]
    pub stats: Option<AgentStats>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentOwner {
    #[serde(default, rename = "xHandle")]
    pub x_handle: Option<String>,
    #[serde(default, rename = "xName")]
    pub x_name: Option<String>,
    #[serde(default, rename = "xAvatar")]
    pub x_avatar: Option<String>,
    #[serde(default, rename = "xBio")]
    pub x_bio: Option<String>,
    #[serde(default, rename = "xFollowerCount")]
    pub x_follower_count: Option<u64>,
    #[serde(default, rename = "xFollowingCount")]
    pub x_following_count: Option<u64>,
    #[serde(default, rename = "xVerified")]
    pub x_verified: Option<bool>,
}

/// Request body for PATCH /agents/me.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateProfileRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Response from GET /agents/profile?name=...
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileResponse {
    #[serde(default)]
    pub success: Option<bool>,
    pub agent: Agent,
    #[serde(default, rename = "recentPosts")]
    pub recent_posts: Option<Vec<Post>>,
}

// ---------- Posts ----------

/// Post sort for feed listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostSort {
    Hot,
    New,
    Top,
    Rising,
}

impl PostSort {
    pub fn as_str(self) -> &'static str {
        match self {
            PostSort::Hot => "hot",
            PostSort::New => "new",
            PostSort::Top => "top",
            PostSort::Rising => "rising",
        }
    }
}

/// Comment sort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentSort {
    Top,
    New,
    Controversial,
}

impl CommentSort {
    pub fn as_str(self) -> &'static str {
        match self {
            CommentSort::Top => "top",
            CommentSort::New => "new",
            CommentSort::Controversial => "controversial",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Post {
    pub id: String,
    #[serde(default)]
    pub submolt: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub author: Option<PostAuthor>,
    #[serde(default)]
    pub score: Option<i64>,
    #[serde(default)]
    pub comment_count: Option<u64>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub is_pinned: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostAuthor {
    pub name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Request body for creating a post.
#[derive(Debug, Clone, Serialize)]
pub struct CreatePostRequest {
    pub submolt: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Request body for adding a comment (or reply).
#[derive(Debug, Clone, Serialize)]
pub struct CreateCommentRequest {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Comment {
    pub id: String,
    #[serde(default)]
    pub post_id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub author: Option<PostAuthor>,
    #[serde(default)]
    pub score: Option<i64>,
    #[serde(default)]
    pub created_at: Option<String>,
}

// ---------- Voting (optional follow suggestion in response) ----------

/// Common voting/action response with optional follow suggestion.
#[derive(Debug, Clone, Deserialize)]
pub struct ActionResponse {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub author: Option<PostAuthor>,
    #[serde(default)]
    pub already_following: Option<bool>,
    #[serde(default)]
    pub suggestion: Option<String>,
}

// ---------- Submolts ----------

#[derive(Debug, Clone, Deserialize)]
pub struct Submolt {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub subscriber_count: Option<u64>,
    #[serde(default)]
    pub your_role: Option<SubmoltRole>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub banner_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubmoltRole {
    Owner,
    Moderator,
}

/// Request body for creating a submolt.
#[derive(Debug, Clone, Serialize)]
pub struct CreateSubmoltRequest {
    pub name: String,
    pub display_name: String,
    pub description: String,
}

/// Request body for PATCH submolt settings.
#[derive(Debug, Clone, Serialize)]
pub struct SubmoltSettingsRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_color: Option<String>,
}

/// Request body for add/remove moderator.
#[derive(Debug, Clone, Serialize)]
pub struct ModeratorRequest {
    pub agent_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Moderator {
    #[serde(default)]
    pub agent_name: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

// ---------- Feed / list responses ----------

/// Response from GET /posts (global feed or by submolt).
#[derive(Debug, Clone, Deserialize)]
pub struct PostsResponse {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub posts: Option<Vec<Post>>,
    /// Some APIs return `data` with posts array.
    #[serde(default)]
    pub data: Option<Vec<Post>>,
}

impl PostsResponse {
    /// Returns the list of posts from the response.
    pub fn into_posts(self) -> Vec<Post> {
        self.posts
            .or(self.data)
            .unwrap_or_default()
    }
}

/// Response from GET /feed (personalized feed).
pub type FeedResponse = PostsResponse;

/// Response from GET /posts/:id/comments.
#[derive(Debug, Clone, Deserialize)]
pub struct CommentsResponse {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub comments: Option<Vec<Comment>>,
    #[serde(default)]
    pub data: Option<Vec<Comment>>,
}

impl CommentsResponse {
    pub fn into_comments(self) -> Vec<Comment> {
        self.comments.or(self.data).unwrap_or_default()
    }
}

/// Response from GET /submolts.
#[derive(Debug, Clone, Deserialize)]
pub struct SubmoltsResponse {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub submolts: Option<Vec<Submolt>>,
    #[serde(default)]
    pub data: Option<Vec<Submolt>>,
}

impl SubmoltsResponse {
    pub fn into_submolts(self) -> Vec<Submolt> {
        self.submolts.or(self.data).unwrap_or_default()
    }
}

/// Response from GET /search.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    #[serde(default)]
    pub success: Option<bool>,
    #[serde(default)]
    pub posts: Option<Vec<Post>>,
    #[serde(default)]
    pub agents: Option<Vec<Agent>>,
    #[serde(default)]
    pub submolts: Option<Vec<Submolt>>,
}

// ---------- API envelope (success/error) ----------

#[derive(Debug, Clone, Deserialize)]
pub(super) struct ApiErrorBody {
    #[serde(rename = "success")]
    pub _success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(default)]
    pub retry_after_minutes: Option<u32>,
}
