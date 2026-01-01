//! Repo knowledge persistence for agent memory.
//!
//! Stores and retrieves cached exploration results and AI insights per repo.

use crate::middleware::auth::authenticate;
use serde::{Deserialize, Serialize};
use worker::*;

/// Repo-level knowledge cache.
#[derive(Debug, Serialize, Deserialize)]
pub struct RepoKnowledge {
    pub id: String,
    pub repo: String,
    pub repo_metadata: Option<serde_json::Value>,
    pub recent_commits: Option<serde_json::Value>,
    pub file_tree: Option<serde_json::Value>,
    pub readme_excerpt: Option<String>,
    pub ai_summary: Option<String>,
    pub ai_suggestions: Option<serde_json::Value>,
    pub files_viewed: Option<serde_json::Value>,
    pub explored_at: String,
    pub insights_at: Option<String>,
    pub commit_sha: Option<String>,
}

/// File-level knowledge with SHA tracking.
#[derive(Debug, Serialize, Deserialize)]
pub struct FileKnowledge {
    pub path: String,
    pub sha: String,
    pub content_preview: Option<String>,
    pub file_type: String,
    pub size: Option<i64>,
    pub viewed_at: String,
    #[serde(skip_deserializing)]
    pub current_sha: Option<String>,
    #[serde(skip_deserializing)]
    pub changed: bool,
}

/// Response for recall_knowledge tool.
#[derive(Debug, Serialize)]
pub struct RecallResponse {
    pub files: Vec<FileKnowledge>,
    pub ai_insights: Option<AiInsights>,
    pub explored_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AiInsights {
    pub summary: Option<String>,
    pub suggestions: Option<serde_json::Value>,
    pub analyzed_at: Option<String>,
}

/// GET /api/repo-knowledge/:owner/:repo
///
/// Returns cached knowledge for a repo, including file SHAs for freshness checking.
pub async fn get_knowledge(req: Request, env: Env, owner: &str, repo_name: &str) -> Result<Response> {
    let user = authenticate(&req, &env).await?;
    let db = env.d1("DB")?;
    let repo = format!("{}/{}", owner, repo_name);

    // Get repo-level knowledge
    let repo_knowledge: Option<RepoKnowledge> = db
        .prepare("SELECT * FROM repo_knowledge WHERE user_id = ? AND repo = ?")
        .bind(&[user.user_id.clone().into(), repo.clone().into()])?
        .first(None)
        .await?;

    // Get file-level knowledge
    let file_results = db
        .prepare("SELECT path, sha, content_preview, file_type, size, viewed_at FROM file_knowledge WHERE user_id = ? AND repo = ? ORDER BY viewed_at DESC")
        .bind(&[user.user_id.clone().into(), repo.clone().into()])?
        .all()
        .await?;

    let files: Vec<FileKnowledge> = file_results
        .results::<FileKnowledge>()?
        .into_iter()
        .map(|mut f| {
            // Mark as not changed by default (client will check current SHA)
            f.changed = false;
            f
        })
        .collect();

    let ai_insights = repo_knowledge.as_ref().map(|rk| AiInsights {
        summary: rk.ai_summary.clone(),
        suggestions: rk.ai_suggestions.clone(),
        analyzed_at: rk.insights_at.clone(),
    });

    let response = RecallResponse {
        files,
        ai_insights,
        explored_at: repo_knowledge.map(|rk| rk.explored_at),
    };

    Response::from_json(&response)
}

/// Request body for saving repo knowledge.
#[derive(Debug, Deserialize)]
pub struct SaveKnowledgeRequest {
    pub repo_metadata: Option<serde_json::Value>,
    pub recent_commits: Option<serde_json::Value>,
    pub file_tree: Option<serde_json::Value>,
    pub readme_excerpt: Option<String>,
    pub ai_summary: Option<String>,
    pub ai_suggestions: Option<serde_json::Value>,
    pub files_viewed: Option<serde_json::Value>,
    pub commit_sha: Option<String>,
}

/// POST /api/repo-knowledge/:owner/:repo
///
/// Saves or updates repo knowledge cache.
pub async fn save_knowledge(mut req: Request, env: Env, owner: &str, repo_name: &str) -> Result<Response> {
    let user = authenticate(&req, &env).await?;
    let db = env.d1("DB")?;
    let repo = format!("{}/{}", owner, repo_name);

    let body: SaveKnowledgeRequest = req.json().await?;
    let now = chrono::Utc::now().to_rfc3339();
    let id = format!("rk_{}_{}", user.user_id.chars().take(8).collect::<String>(), chrono::Utc::now().timestamp_millis());

    // Upsert repo knowledge
    db.prepare(
        "INSERT INTO repo_knowledge (id, user_id, repo, repo_metadata, recent_commits, file_tree, readme_excerpt, ai_summary, ai_suggestions, files_viewed, explored_at, insights_at, commit_sha, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, repo) DO UPDATE SET
            repo_metadata = COALESCE(excluded.repo_metadata, repo_metadata),
            recent_commits = COALESCE(excluded.recent_commits, recent_commits),
            file_tree = COALESCE(excluded.file_tree, file_tree),
            readme_excerpt = COALESCE(excluded.readme_excerpt, readme_excerpt),
            ai_summary = COALESCE(excluded.ai_summary, ai_summary),
            ai_suggestions = COALESCE(excluded.ai_suggestions, ai_suggestions),
            files_viewed = COALESCE(excluded.files_viewed, files_viewed),
            explored_at = CASE WHEN excluded.repo_metadata IS NOT NULL THEN excluded.explored_at ELSE explored_at END,
            insights_at = CASE WHEN excluded.ai_summary IS NOT NULL THEN excluded.insights_at ELSE insights_at END,
            commit_sha = COALESCE(excluded.commit_sha, commit_sha),
            updated_at = excluded.updated_at"
    )
    .bind(&[
        id.into(),
        user.user_id.clone().into(),
        repo.into(),
        body.repo_metadata.map(|v| v.to_string()).unwrap_or_default().into(),
        body.recent_commits.map(|v| v.to_string()).unwrap_or_default().into(),
        body.file_tree.map(|v| v.to_string()).unwrap_or_default().into(),
        body.readme_excerpt.unwrap_or_default().into(),
        body.ai_summary.clone().unwrap_or_default().into(),
        body.ai_suggestions.map(|v| v.to_string()).unwrap_or_default().into(),
        body.files_viewed.map(|v| v.to_string()).unwrap_or_default().into(),
        now.clone().into(),
        if body.ai_summary.is_some() { now.clone().into() } else { "".into() },
        body.commit_sha.unwrap_or_default().into(),
        now.clone().into(),
        now.into(),
    ])?
    .run()
    .await?;

    Response::ok("Knowledge saved")
}

/// Request body for saving file knowledge.
#[derive(Debug, Deserialize)]
pub struct SaveFileRequest {
    pub path: String,
    pub sha: String,
    pub content_preview: Option<String>,
    pub file_type: String,
    pub size: Option<i64>,
}

/// POST /api/file-knowledge/:owner/:repo
///
/// Saves or updates file knowledge with SHA for freshness tracking.
pub async fn save_file_knowledge(mut req: Request, env: Env, owner: &str, repo_name: &str) -> Result<Response> {
    let user = authenticate(&req, &env).await?;
    let db = env.d1("DB")?;
    let repo = format!("{}/{}", owner, repo_name);

    let body: SaveFileRequest = req.json().await?;
    let now = chrono::Utc::now().to_rfc3339();
    let id = format!("fk_{}", chrono::Utc::now().timestamp_millis());

    // D1 doesn't support bigint, cast to i32
    let size_i32 = body.size.unwrap_or(0) as i32;

    db.prepare(
        "INSERT INTO file_knowledge (id, user_id, repo, path, sha, content_preview, file_type, size, viewed_at, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, repo, path) DO UPDATE SET
            sha = excluded.sha,
            content_preview = excluded.content_preview,
            file_type = excluded.file_type,
            size = excluded.size,
            viewed_at = excluded.viewed_at,
            updated_at = excluded.updated_at"
    )
    .bind(&[
        id.into(),
        user.user_id.into(),
        repo.into(),
        body.path.into(),
        body.sha.into(),
        body.content_preview.unwrap_or_default().into(),
        body.file_type.into(),
        size_i32.into(),
        now.clone().into(),
        now.clone().into(),
        now.into(),
    ])?
    .run()
    .await?;

    Response::ok("File knowledge saved")
}
