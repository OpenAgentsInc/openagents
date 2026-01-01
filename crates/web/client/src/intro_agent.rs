//! IntroAgent - Browser-based agent that introduces itself and explores a GitHub repo.
//!
//! Note: The full BrowserRuntime uses Timestamp::now() which depends on native time APIs
//! that don't work in WASM. This simplified version uses direct async/fetch for exploration.

#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::state::AppState;
use wgpui::components::molecules::SectionStatus;

/// Exploration phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) enum IntroPhase {
    #[default]
    Greeting,
    FetchingMetadata,
    FetchingIssues,
    FetchingPRs,
    FetchingTree,
    FetchingReadme,
    FetchingCommits,
    FetchingContributors,
    Complete,
    Failed,
}

/// State for the intro agent.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct IntroAgentState {
    pub(crate) phase: IntroPhase,
    pub(crate) github_username: String,
    pub(crate) repo: String,
    pub(crate) repo_description: Option<String>,
    pub(crate) repo_language: Option<String>,
    pub(crate) repo_stars: Option<u64>,
    pub(crate) open_issues_count: Option<u64>,
    pub(crate) open_prs_count: Option<u64>,
    pub(crate) recent_issues: Vec<String>,
    pub(crate) recent_prs: Vec<String>,
    pub(crate) file_tree: Vec<String>,
    pub(crate) readme_excerpt: Option<String>,
    pub(crate) recent_commits: Vec<String>,
    pub(crate) contributors: Vec<String>,
    pub(crate) error: Option<String>,
}

/// GitHub explore API response.
#[derive(Debug, Deserialize)]
struct ExploreResponse {
    repo: Option<RepoInfo>,
    issues: Vec<IssueInfo>,
    pull_requests: Vec<PrInfo>,
    tree: Vec<TreeItem>,
    readme_excerpt: Option<String>,
    commits: Vec<CommitInfo>,
    contributors: Vec<ContributorInfo>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RepoInfo {
    description: Option<String>,
    language: Option<String>,
    stargazers_count: Option<u64>,
    open_issues_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct IssueInfo {
    number: u64,
    title: String,
}

#[derive(Debug, Deserialize)]
struct PrInfo {
    number: u64,
    title: String,
}

#[derive(Debug, Deserialize)]
struct TreeItem {
    path: String,
    #[serde(rename = "type")]
    item_type: String,
}

#[derive(Debug, Deserialize)]
struct CommitInfo {
    sha: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct ContributorInfo {
    login: String,
    contributions: u64,
}

/// Controller for the intro agent exploration.
pub(crate) struct IntroAgentController;

impl IntroAgentController {
    /// Start the intro agent exploration.
    pub(crate) fn start(
        state: Rc<RefCell<AppState>>,
        github_username: String,
        repo: String,
    ) {
        // Show the chat pane with greeting
        {
            let mut state_guard = state.borrow_mut();
            state_guard.autopilot_chat.show(&github_username);
        }

        // Start the exploration async
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            run_exploration(state_clone, github_username, repo).await;
        });
    }
}

/// Run the exploration asynchronously.
async fn run_exploration(
    state: Rc<RefCell<AppState>>,
    _github_username: String,
    repo: String,
) {
    // Small delay before starting
    sleep_ms(300).await;

    let explore_result = fetch_explore(&repo).await;

    match explore_result {
        Ok(response) => {
            // Check for API error
            if let Some(error) = response.error {
                push_error_message(&state, &format!("API error: {}", error));
                return;
            }

            // Build context summary from initial exploration
            let repo_desc = response.repo.as_ref()
                .and_then(|r| r.description.clone())
                .unwrap_or_else(|| "No description".to_string());
            let repo_lang = response.repo.as_ref()
                .and_then(|r| r.language.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let repo_stars = response.repo.as_ref()
                .and_then(|r| r.stargazers_count)
                .unwrap_or(0);

            let dirs: Vec<String> = response.tree.iter()
                .filter(|t| t.item_type == "tree")
                .take(10)
                .map(|t| t.path.clone())
                .collect();

            let files: Vec<String> = response.tree.iter()
                .filter(|t| t.item_type == "blob")
                .take(10)
                .map(|t| t.path.clone())
                .collect();

            // Get last 10 commits for better activity insight
            let recent_commits: Vec<String> = response.commits.iter()
                .take(10)
                .map(|c| c.message.clone())
                .collect();

            let issues: Vec<String> = response.issues.iter()
                .take(10)
                .map(|i| format!("#{}: {}", i.number, i.title))
                .collect();

            let prs: Vec<String> = response.pull_requests.iter()
                .take(5)
                .map(|p| format!("#{}: {}", p.number, p.title))
                .collect();

            // Build context string - emphasize recent activity
            let initial_context = format!(
                "**Repository**: {}\n\
                **Description**: {}\n\
                **Language**: {} | **Stars**: {}\n\n\
                **Root structure**: dirs=[{}] files=[{}]\n\n\
                **Recent commits (last 10)** - shows what's being worked on:\n{}\n\n\
                **Open issues** ({} total):\n{}\n\n\
                **Open PRs** ({} total):\n{}\n\n\
                **README excerpt**:\n{}",
                repo,
                repo_desc,
                repo_lang,
                repo_stars,
                if dirs.is_empty() { "none".to_string() } else { dirs.join(", ") },
                if files.is_empty() { "none".to_string() } else { files.join(", ") },
                if recent_commits.is_empty() { "none".to_string() } else { recent_commits.iter().enumerate().map(|(i, c)| format!("{}. {}", i+1, c)).collect::<Vec<_>>().join("\n") },
                response.issues.len(),
                if issues.is_empty() { "none".to_string() } else { issues.join("\n") },
                response.pull_requests.len(),
                if prs.is_empty() { "none".to_string() } else { prs.join("\n") },
                response.readme_excerpt.as_deref().unwrap_or("Not available"),
            );

            // Start the agent response placeholder
            {
                let mut state_guard = state.borrow_mut();
                state_guard.autopilot_chat.push_streaming_assistant();
            }

            // Run the agentic loop
            match run_agent_loop(&state, &repo, &initial_context).await {
                Ok(_) => {
                    let mut state_guard = state.borrow_mut();
                    state_guard.autopilot_chat.enable_claude_cta();
                    state_guard.autopilot_chat.push_system_message(
                        "Ready for a full local Claude run. Click Start Claude below.",
                    );
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("AI agent failed: {}", e).into());
                    push_error_message(&state, &format!("AI error: {}", e));
                }
            }
        }
        Err(error) => {
            push_error_message(&state, &format!("Failed to explore repository: {}", error));
        }
    }
}

/// Push an assistant message to the chat.
fn push_assistant_message(state: &Rc<RefCell<AppState>>, text: &str) {
    let mut state_guard = state.borrow_mut();
    state_guard.autopilot_chat.push_assistant_message(text);
}

/// Push a tool message to the chat.
fn push_tool_message(state: &Rc<RefCell<AppState>>, tool_name: &str, status: &str) {
    let mut state_guard = state.borrow_mut();
    state_guard.autopilot_chat.push_tool_message(tool_name, status);
}

/// Push an error message to the chat.
fn push_error_message(state: &Rc<RefCell<AppState>>, text: &str) {
    let mut state_guard = state.borrow_mut();
    state_guard.autopilot_chat.push_error_message(text);
}

/// Sleep for a number of milliseconds.
async fn sleep_ms(ms: u32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let window = web_sys::window().expect("no window");
        window
            .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms as i32)
            .expect("setTimeout failed");
    });
    JsFuture::from(promise).await.ok();
}

/// Fetch exploration data from the backend.
async fn fetch_explore(repo: &str) -> Result<ExploreResponse, String> {
    let window = web_sys::window().ok_or("No window")?;

    let url = format!("/api/github/explore?repo={}", js_sys::encode_uri_component(repo));

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_credentials(web_sys::RequestCredentials::Include);

    let request = web_sys::Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast error")?;

    if !resp.ok() {
        return Err(format!("HTTP error: {}", resp.status()));
    }

    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON error: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON parse error: {:?}", e))?;

    let response: ExploreResponse = serde_wasm_bindgen::from_value(json)
        .map_err(|e| format!("Deserialize error: {:?}", e))?;

    Ok(response)
}

/// Tool definitions for the AI agent (OpenAI format).
fn get_tools() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "function",
            "function": {
                "name": "recall_knowledge",
                "description": "Check what you previously learned about this repository. Returns cached file contents with SHA hashes and timestamps, plus any previous AI analysis. Use this FIRST before view_file to see if you already have the content cached. Files with unchanged SHA don't need re-fetching.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional: specific file paths to recall. If empty, returns all cached knowledge."
                        }
                    },
                    "required": []
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "view_folder",
                "description": "View the contents of a folder/directory in the repository. Returns a list of files and subdirectories.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The path to the folder, relative to repository root. Use empty string or '/' for root."
                        }
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "view_file",
                "description": "View the contents of a file in the repository. Returns the file content as text.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The path to the file, relative to repository root."
                        }
                    },
                    "required": ["path"]
                }
            }
        }
    ])
}

/// GitHub contents API response.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentsResponse {
    #[serde(rename = "file")]
    File { path: String, content: String, size: u64, sha: String },
    #[serde(rename = "directory")]
    Directory { path: String, entries: Vec<DirectoryEntry> },
    #[serde(rename = "error")]
    Error { message: String },
}

#[derive(Debug, Deserialize)]
struct DirectoryEntry {
    name: String,
    #[serde(rename = "type")]
    entry_type: String,
    size: Option<u64>,
}

/// Repo knowledge API response (cached agent memory).
#[derive(Debug, Deserialize)]
struct RecallResponse {
    files: Vec<FileKnowledgeEntry>,
    ai_insights: Option<AiInsightsEntry>,
    explored_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FileKnowledgeEntry {
    path: String,
    sha: String,
    content_preview: Option<String>,
    file_type: String,
    size: Option<i64>,
    viewed_at: String,
}

#[derive(Debug, Deserialize)]
struct AiInsightsEntry {
    summary: Option<String>,
    suggestions: Option<serde_json::Value>,
    analyzed_at: Option<String>,
}

/// Fetch cached knowledge from the backend.
async fn fetch_repo_knowledge(repo: &str) -> Result<RecallResponse, String> {
    let window = web_sys::window().ok_or("No window")?;

    // Parse owner/repo
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        return Err("Invalid repo format".to_string());
    }
    let (owner, name) = (parts[0], parts[1]);

    let url = format!("/api/repo-knowledge/{}/{}", owner, name);

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_credentials(web_sys::RequestCredentials::Include);

    let request = web_sys::Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast error")?;

    if resp.status() == 404 {
        // No cached knowledge - return empty
        return Ok(RecallResponse {
            files: Vec::new(),
            ai_insights: None,
            explored_at: None,
        });
    }

    if !resp.ok() {
        return Err(format!("HTTP error: {}", resp.status()));
    }

    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON error: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON parse error: {:?}", e))?;

    serde_wasm_bindgen::from_value(json)
        .map_err(|e| format!("Deserialize error: {:?}", e))
}

/// Save file knowledge to the backend (fire-and-forget).
fn save_file_knowledge(repo: &str, path: &str, sha: &str, content: &str, size: u64) {
    // Parse owner/repo
    let parts: Vec<&str> = repo.split('/').collect();
    if parts.len() != 2 {
        web_sys::console::error_1(&"Invalid repo format for save_file_knowledge".into());
        return;
    }
    let (owner, name) = (parts[0].to_string(), parts[1].to_string());

    // Create content preview (first 4KB)
    let content_preview = if content.len() > 4000 {
        let mut end = 4000;
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        content[..end].to_string()
    } else {
        content.to_string()
    };

    // Convert to owned types for the async block
    let path_owned = path.to_string();
    let sha_owned = sha.to_string();

    let url = format!("/api/file-knowledge/{}/{}", owner, name);
    let body = serde_json::json!({
        "path": &path_owned,
        "sha": &sha_owned,
        "content_preview": content_preview,
        "file_type": "file",
        "size": size
    });

    // Fire and forget - spawn async task to save
    wasm_bindgen_futures::spawn_local(async move {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        let opts = web_sys::RequestInit::new();
        opts.set_method("POST");
        opts.set_credentials(web_sys::RequestCredentials::Include);

        let headers = match web_sys::Headers::new() {
            Ok(h) => h,
            Err(_) => return,
        };
        let _ = headers.set("Content-Type", "application/json");
        opts.set_headers(&headers);
        opts.set_body(&wasm_bindgen::JsValue::from_str(&body.to_string()));

        let request = match web_sys::Request::new_with_str_and_init(&url, &opts) {
            Ok(r) => r,
            Err(e) => {
                web_sys::console::error_1(&format!("save_file_knowledge request error: {:?}", e).into());
                return;
            }
        };

        match JsFuture::from(window.fetch_with_request(&request)).await {
            Ok(_) => {
                web_sys::console::log_1(&format!("Saved file knowledge: {}", path_owned).into());
            }
            Err(e) => {
                web_sys::console::error_1(&format!("save_file_knowledge fetch error: {:?}", e).into());
            }
        }
    });
}

/// Fetch contents from GitHub API.
async fn fetch_contents(repo: &str, path: &str) -> Result<ContentsResponse, String> {
    let window = web_sys::window().ok_or("No window")?;

    let url = format!(
        "/api/github/contents?repo={}&path={}",
        js_sys::encode_uri_component(repo),
        js_sys::encode_uri_component(path)
    );

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_credentials(web_sys::RequestCredentials::Include);

    let request = web_sys::Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast error")?;

    if !resp.ok() {
        return Err(format!("HTTP error: {}", resp.status()));
    }

    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON error: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON parse error: {:?}", e))?;

    serde_wasm_bindgen::from_value(json)
        .map_err(|e| format!("Deserialize error: {:?}", e))
}

/// Execute a tool and return the result as a string.
/// Also updates UI with collapsible display for recall_knowledge.
async fn execute_tool(
    state: &Rc<RefCell<AppState>>,
    repo: &str,
    tool_name: &str,
    tool_input: &serde_json::Value,
) -> String {
    let path = tool_input.get("path")
        .and_then(|p| p.as_str())
        .unwrap_or("");

    web_sys::console::log_1(&format!("execute_tool: {} for repo {} path {}", tool_name, repo, path).into());

    match tool_name {
        "recall_knowledge" => {
            web_sys::console::log_1(&"Fetching cached knowledge...".into());

            // Get optional paths filter
            let paths_filter: Vec<String> = tool_input.get("paths")
                .and_then(|p| p.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            match fetch_repo_knowledge(repo).await {
                Ok(knowledge) => {
                    let mut result = String::new();

                    // Show AI insights if available
                    if let Some(insights) = &knowledge.ai_insights {
                        if let Some(summary) = &insights.summary {
                            result.push_str("## Previous AI Analysis\n\n");
                            result.push_str(summary);
                            result.push_str("\n\n");
                        }
                        if let Some(analyzed_at) = &insights.analyzed_at {
                            result.push_str(&format!("*Analyzed: {}*\n\n", analyzed_at));
                        }
                    }

                    // Filter and show cached files
                    let files: Vec<&FileKnowledgeEntry> = if paths_filter.is_empty() {
                        knowledge.files.iter().collect()
                    } else {
                        knowledge.files.iter()
                            .filter(|f| paths_filter.iter().any(|p| f.path.contains(p)))
                            .collect()
                    };

                    if files.is_empty() && knowledge.ai_insights.is_none() {
                        result.push_str("No cached knowledge found for this repository. This is your first exploration.");
                    } else if !files.is_empty() {
                        result.push_str(&format!("## Cached Files ({} total)\n\n", files.len()));
                        for file in files.iter().take(20) {
                            result.push_str(&format!(
                                "- **{}** (SHA: {}, viewed: {})\n",
                                file.path,
                                &file.sha[..8.min(file.sha.len())],
                                file.viewed_at
                            ));
                            if let Some(preview) = &file.content_preview {
                                if !preview.is_empty() {
                                    // Show first 200 chars of preview
                                    let short_preview = if preview.len() > 200 {
                                        format!("{}...", &preview[..200])
                                    } else {
                                        preview.clone()
                                    };
                                    result.push_str(&format!("  ```\n  {}\n  ```\n", short_preview.replace('\n', "\n  ")));
                                }
                            }
                        }
                        if files.len() > 20 {
                            result.push_str(&format!("\n*...and {} more files*\n", files.len() - 20));
                        }
                    }

                    if let Some(explored_at) = &knowledge.explored_at {
                        result.push_str(&format!("\n*Last exploration: {}*", explored_at));
                    }

                    // Push collapsible UI for user visibility
                    let summary = if files.is_empty() {
                        "No cached files".to_string()
                    } else {
                        let time_ago = knowledge.explored_at.as_ref()
                            .map(|t| t.clone())
                            .unwrap_or_else(|| "unknown".to_string());
                        format!("Recalled {} files (last seen: {})", files.len(), time_ago)
                    };

                    let details: Vec<String> = files.iter().take(10).map(|f| {
                        format!("  {} (sha: {})", f.path, &f.sha[..8.min(f.sha.len())])
                    }).collect();

                    let status = if files.is_empty() && knowledge.ai_insights.is_none() {
                        SectionStatus::Pending
                    } else {
                        SectionStatus::Success
                    };

                    state.borrow_mut().autopilot_chat.push_collapsible_tool(&summary, details, status);

                    result
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("Recall knowledge error: {}", e).into());
                    let err_msg = format!("Error recalling knowledge: {}", e);
                    state.borrow_mut().autopilot_chat.push_collapsible_tool(
                        "Failed to recall knowledge",
                        vec![err_msg.clone()],
                        SectionStatus::Error,
                    );
                    err_msg
                }
            }
        }
        "view_folder" | "view_file" => {
            web_sys::console::log_1(&"Fetching contents...".into());
            match fetch_contents(repo, path).await {
                Ok(ContentsResponse::File { path, content, size, sha }) => {
                    web_sys::console::log_1(&format!("Got file: {} ({} bytes, sha: {})", path, size, &sha[..8.min(sha.len())]).into());

                    // Auto-save file knowledge for future recall (fire and forget)
                    save_file_knowledge(repo, &path, &sha, &content, size);

                    // Truncate large files at a valid UTF-8 boundary (4KB limit to control context size)
                    let truncated = if content.len() > 4000 {
                        // Find a valid char boundary at or before 4000 bytes
                        let mut end = 4000;
                        while end > 0 && !content.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{}...\n[truncated, showing first {} bytes of {} total]", &content[..end], end, size)
                    } else {
                        content
                    };
                    format!("File: {} (sha: {})\n```\n{}\n```", path, &sha[..8.min(sha.len())], truncated)
                }
                Ok(ContentsResponse::Directory { path, entries }) => {
                    web_sys::console::log_1(&format!("Got directory: {} ({} entries)", path, entries.len()).into());
                    let listing: Vec<String> = entries.iter().map(|e| {
                        let icon = if e.entry_type == "tree" { "📁" } else { "📄" };
                        format!("{} {}", icon, e.name)
                    }).collect();
                    format!("Directory: {}\n{}", path, listing.join("\n"))
                }
                Ok(ContentsResponse::Error { message }) => {
                    web_sys::console::error_1(&format!("Contents error: {}", message).into());
                    format!("Error: {}", message)
                }
                Err(e) => {
                    web_sys::console::error_1(&format!("Fetch error: {}", e).into());
                    format!("Error fetching contents: {}", e)
                }
            }
        }
        _ => {
            web_sys::console::error_1(&format!("Unknown tool: {}", tool_name).into());
            format!("Unknown tool: {}", tool_name)
        }
    }
}

/// AI completion response (non-streaming).
#[derive(Debug, Deserialize)]
struct AiResponse {
    content: String,
    finish_reason: Option<String>,
    tool_use: Option<Vec<ToolUseBlock>>,
}

#[derive(Debug, Deserialize, Clone)]
struct ToolUseBlock {
    id: String,
    name: String,
    input: serde_json::Value,
}

/// SSE streaming delta chunk
#[derive(Debug, Deserialize)]
struct StreamDelta {
    choices: Option<Vec<StreamChoice>>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Option<DeltaContent>,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeltaContent {
    content: Option<String>,
    tool_use: Option<ToolUseBlock>,
}

/// Message for the conversation (OpenAI format)
#[derive(Debug, Clone, Serialize)]
struct Message {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

/// Error classification for recovery strategies.
#[derive(Debug, Clone, PartialEq)]
enum ErrorClass {
    /// Transient error - should retry with backoff (5xx, network issues)
    Transient,
    /// Rate limit - should retry after delay (429)
    RateLimit,
    /// Token/context limit - should truncate and retry
    TokenLimit,
    /// Format error - should sanitize messages and retry
    Format,
    /// Unrecoverable - give up (4xx except above)
    Unrecoverable,
}

/// Classify an error for recovery strategy.
fn classify_error(error: &str) -> ErrorClass {
    let lower = error.to_lowercase();

    // Rate limit errors
    if lower.contains("429") || lower.contains("rate") || lower.contains("too many") {
        return ErrorClass::RateLimit;
    }

    // Token/context limit errors
    if lower.contains("token") || lower.contains("limit") || lower.contains("too long")
        || lower.contains("context") || lower.contains("maximum")
    {
        return ErrorClass::TokenLimit;
    }

    // Format errors
    if lower.contains("content") || lower.contains("format") || lower.contains("invalid")
        || lower.contains("missing field") || lower.contains("parse")
    {
        return ErrorClass::Format;
    }

    // Server errors are transient
    if lower.contains("500") || lower.contains("502") || lower.contains("503")
        || lower.contains("504") || lower.contains("network") || lower.contains("timeout")
        || lower.contains("fetch error")
    {
        return ErrorClass::Transient;
    }

    // Everything else is unrecoverable
    ErrorClass::Unrecoverable
}

/// Truncate tool results in messages to reduce context size.
fn truncate_tool_results(messages: &mut [Message], max_len: usize) {
    for msg in messages.iter_mut() {
        if msg.role == "tool" {
            if let Some(content) = &mut msg.content {
                if let Some(s) = content.as_str() {
                    if s.len() > max_len {
                        // Find valid UTF-8 boundary
                        let mut end = max_len;
                        while end > 0 && !s.is_char_boundary(end) {
                            end -= 1;
                        }
                        *content = serde_json::json!(format!("{}...[truncated to {} chars]", &s[..end], end));
                    }
                }
            }
        }
    }
}

/// Estimate total context size in bytes.
fn estimate_context_size(messages: &[Message]) -> usize {
    messages.iter().map(|msg| {
        let content_size = msg.content.as_ref()
            .map(|c| c.to_string().len())
            .unwrap_or(0);
        let tool_calls_size = msg.tool_calls.as_ref()
            .map(|tc| serde_json::to_string(tc).map(|s| s.len()).unwrap_or(0))
            .unwrap_or(0);
        content_size + tool_calls_size + 50 // 50 bytes overhead for role, etc.
    }).sum()
}

/// Compact old tool results to reduce context size.
/// Keeps the most recent `keep_recent` tool results intact, compacts older ones to summaries.
fn compact_old_context(messages: &mut [Message], keep_recent: usize, max_context_bytes: usize) {
    let current_size = estimate_context_size(messages);
    if current_size <= max_context_bytes {
        return; // Context is within limits
    }

    web_sys::console::log_1(&format!(
        "Context size {} bytes exceeds limit {}, compacting old tool results...",
        current_size, max_context_bytes
    ).into());

    // Find all tool message indices
    let tool_indices: Vec<usize> = messages.iter()
        .enumerate()
        .filter(|(_, m)| m.role == "tool")
        .map(|(i, _)| i)
        .collect();

    // Keep the most recent `keep_recent` tool results, compact the rest
    let compact_count = tool_indices.len().saturating_sub(keep_recent);

    for &idx in tool_indices.iter().take(compact_count) {
        if let Some(content) = &mut messages[idx].content {
            if let Some(s) = content.as_str() {
                // Extract just the first line (usually "File: path" or "Directory: path")
                let summary = s.lines().next().unwrap_or("[tool result]");
                let summary = if summary.len() > 100 {
                    format!("{}...", &summary[..100])
                } else {
                    summary.to_string()
                };
                *content = serde_json::json!(format!("[Previously viewed: {}]", summary));
            }
        }
    }

    let new_size = estimate_context_size(messages);
    web_sys::console::log_1(&format!(
        "Context compacted: {} bytes -> {} bytes (saved {} bytes)",
        current_size, new_size, current_size - new_size
    ).into());
}

/// Sanitize messages to fix common format issues.
fn sanitize_messages(messages: &mut Vec<Message>) {
    for msg in messages.iter_mut() {
        // Ensure assistant messages with tool_calls have proper content
        if msg.role == "assistant" {
            if msg.tool_calls.is_some() && msg.content.is_none() {
                // OpenAI format allows null content for tool calls, but some backends don't
                // Leave as None - our backend fix should handle this now
            }
        }

        // Ensure tool messages have content
        if msg.role == "tool" && msg.content.is_none() {
            msg.content = Some(serde_json::json!("[No content]"));
        }
    }
}

/// Run the agentic AI loop with tools.
async fn run_agent_loop(
    state: &Rc<RefCell<AppState>>,
    repo: &str,
    initial_context: &str,
) -> Result<(), String> {
    let mut messages: Vec<Message> = vec![
        Message {
            role: "user".to_string(),
            content: Some(serde_json::json!(format!(
                "Repository: {}\n\n{}\n\n---\n\n\
                **IMPORTANT - Memory System:**\n\
                You have a memory system! Before exploring with view_file/view_folder, ALWAYS call `recall_knowledge` FIRST to check what you already know about this repo. \n\
                - If you have cached knowledge, use it instead of re-fetching unchanged files\n\
                - Only use view_file for files you haven't seen before or need to verify\n\
                - This saves time and tokens\n\n\
                Your task:\n\
                1. **Check memory first** - call recall_knowledge to see what you already know\n\
                2. **Analyze recent commits** - understand what's actively being worked on\n\
                3. **Find roadmap/next steps** - look for ROADMAP.md, TODO.md, docs/, CONTRIBUTING.md, or any planning docs\n\
                4. **Check open issues** - if any, note what's prioritized\n\
                5. **Explore key code areas** - briefly understand the architecture (skip files you already have cached)\n\n\
                Then give me:\n\
                - One paragraph: what this project does\n\
                - **3 specific suggestions** of tasks I could work on, based on recent activity, open issues, or roadmap items. Be concrete (e.g., \"Implement the missing X feature mentioned in issue #Y\" not \"help with development\").\n\n\
                Be direct. No fluff.",
                repo,
                initial_context
            ))),
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let max_iterations = 50;
    let mut iteration = 0;
    let mut consecutive_errors = 0;
    let max_consecutive_errors = 3;
    let max_retries = 2;

    // Context management limits
    const MAX_CONTEXT_BYTES: usize = 50_000; // ~12.5K tokens - keep under control
    const KEEP_RECENT_TOOLS: usize = 6; // Keep last 6 tool results intact

    loop {
        iteration += 1;
        web_sys::console::log_1(&format!("Agent loop iteration {}", iteration).into());

        if iteration > max_iterations {
            web_sys::console::log_1(&"Max iterations reached".into());
            break;
        }

        // Proactively compact context if it's getting large
        compact_old_context(&mut messages, KEEP_RECENT_TOOLS, MAX_CONTEXT_BYTES);

        // Log context size for monitoring
        let context_size = estimate_context_size(&messages);
        web_sys::console::log_1(&format!("Calling AI with {} messages (~{} bytes, ~{} tokens)",
            messages.len(), context_size, context_size / 4).into());

        let response = match stream_ai_with_retry(state, &messages, max_retries).await {
            Ok(r) => {
                consecutive_errors = 0; // Reset on success
                r
            }
            Err(e) => {
                consecutive_errors += 1;
                web_sys::console::error_1(&format!("AI call failed (attempt {}): {}", consecutive_errors, e).into());

                let error_class = classify_error(&e);
                web_sys::console::log_1(&format!("Error class: {:?}", error_class).into());

                // Attempt recovery based on error type
                let recovery_msg = match error_class {
                    ErrorClass::TokenLimit => {
                        // First try aggressive compaction
                        compact_old_context(&mut messages, 3, 30_000);
                        // Then truncate any remaining large tool results
                        truncate_tool_results(&mut messages, 1000);
                        Some("Context was compacted and truncated to fit token limits.".to_string())
                    }
                    ErrorClass::Format => {
                        // Sanitize messages
                        sanitize_messages(&mut messages);
                        Some("Message format was corrected.".to_string())
                    }
                    ErrorClass::RateLimit => {
                        // Already retried with backoff
                        Some("Hit rate limit, waiting before retry.".to_string())
                    }
                    ErrorClass::Transient => {
                        // Already retried
                        Some("Transient error occurred, retrying.".to_string())
                    }
                    ErrorClass::Unrecoverable => None,
                };

                if let Some(recovery) = recovery_msg {
                    if consecutive_errors < max_consecutive_errors {
                        // Add system message about the error for AI context
                        messages.push(Message {
                            role: "user".to_string(),
                            content: Some(serde_json::json!(format!(
                                "[System: Previous request encountered an issue: {}. {}. Please continue.]",
                                e.chars().take(200).collect::<String>(),
                                recovery
                            ))),
                            tool_calls: None,
                            tool_call_id: None,
                        });
                        continue;
                    }
                }

                // Too many errors or unrecoverable
                if consecutive_errors >= max_consecutive_errors {
                    web_sys::console::error_1(&"Max consecutive errors reached, giving up".into());

                    // Show partial results if any
                    {
                        let mut state_guard = state.borrow_mut();
                        state_guard.autopilot_chat.update_last_assistant(
                            &format!("⚠️ I encountered repeated errors and couldn't complete the exploration. Last error: {}\n\nHere's what I gathered so far from the repository...", e)
                        );
                    }
                    return Err(format!("Max errors reached: {}", e));
                }

                return Err(e);
            }
        };
        web_sys::console::log_1(&format!("AI response - content len: {}, tool_use: {:?}", response.content.len(), response.tool_use.as_ref().map(|t| t.len())).into());

        // Check if AI wants to use tools
        if let Some(tool_uses) = &response.tool_use {
            if !tool_uses.is_empty() {
                web_sys::console::log_1(&format!("Processing {} tool calls", tool_uses.len()).into());
                // Content already streamed to UI, just process tool calls

                // Build tool_calls array in OpenAI format
                let tool_calls: Vec<serde_json::Value> = tool_uses.iter().map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "arguments": serde_json::to_string(&t.input).unwrap_or_default()
                        }
                    })
                }).collect();

                // Add assistant message with tool_calls
                messages.push(Message {
                    role: "assistant".to_string(),
                    content: if response.content.is_empty() { None } else { Some(serde_json::json!(response.content)) },
                    tool_calls: Some(tool_calls),
                    tool_call_id: None,
                });

                // Execute each tool and add tool result messages
                for tool_use in tool_uses {
                    web_sys::console::log_1(&format!("Executing tool: {} with path: {:?}", tool_use.name, tool_use.input.get("path")).into());

                    // Show tool execution in UI
                    {
                        let mut state_guard = state.borrow_mut();
                        let path = tool_use.input.get("path").and_then(|p| p.as_str()).unwrap_or("/");
                        state_guard.autopilot_chat.push_tool_message(&tool_use.name, path);
                    }

                    let result = execute_tool(&state, repo, &tool_use.name, &tool_use.input).await;
                    web_sys::console::log_1(&format!("Tool result length: {}", result.len()).into());

                    // Add tool result message (OpenAI format: role=tool, tool_call_id)
                    messages.push(Message {
                        role: "tool".to_string(),
                        content: Some(serde_json::json!(result)),
                        tool_calls: None,
                        tool_call_id: Some(tool_use.id.clone()),
                    });
                }

                web_sys::console::log_1(&"Continuing loop after tool execution".into());
                continue;
            }
        }

        // No tool use - final response already streamed
        web_sys::console::log_1(&"Final response (no tool use), done.".into());
        break;
    }

    web_sys::console::log_1(&"Agent loop complete".into());
    Ok(())
}

/// Stream AI with retry and exponential backoff.
async fn stream_ai_with_retry(
    state: &Rc<RefCell<AppState>>,
    messages: &[Message],
    max_retries: u32,
) -> Result<AiResponse, String> {
    let mut last_error = String::new();
    let base_delay_ms = 500u32;

    for attempt in 0..=max_retries {
        if attempt > 0 {
            // Exponential backoff: 500ms, 1000ms, 2000ms...
            let delay = base_delay_ms * (1 << (attempt - 1));
            web_sys::console::log_1(&format!("Retry attempt {} after {}ms delay", attempt, delay).into());
            sleep_ms(delay).await;
        }

        match stream_ai_with_tools(state, messages).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                last_error = e.clone();
                let error_class = classify_error(&e);

                // Only retry for transient and rate limit errors
                match error_class {
                    ErrorClass::Transient | ErrorClass::RateLimit => {
                        web_sys::console::log_1(&format!("Retryable error: {}", e).into());
                        continue;
                    }
                    _ => {
                        // Don't retry non-transient errors
                        return Err(e);
                    }
                }
            }
        }
    }

    Err(format!("Max retries exceeded. Last error: {}", last_error))
}

/// Fetch AI response with tools (non-streaming).
async fn fetch_ai_with_tools(messages: &[Message]) -> Result<AiResponse, String> {
    let window = web_sys::window().ok_or("No window")?;

    // NOTE: Model MUST be anthropic/claude-sonnet-4.5 - DO NOT CHANGE
    let body = serde_json::json!({
        "messages": messages,
        "model": "anthropic/claude-sonnet-4.5",
        "max_tokens": 4096,
        "tools": get_tools(),
        "tool_choice": "auto"
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_credentials(web_sys::RequestCredentials::Include);

    let headers = web_sys::Headers::new().map_err(|e| format!("Headers error: {:?}", e))?;
    headers.set("Content-Type", "application/json").map_err(|e| format!("Header set error: {:?}", e))?;
    opts.set_headers(&headers);
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body.to_string()));

    let request = web_sys::Request::new_with_str_and_init("/api/ai/chat", &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast error")?;

    if !resp.ok() {
        let status = resp.status();
        let text = JsFuture::from(resp.text().map_err(|e| format!("Text error: {:?}", e))?)
            .await
            .map_err(|e| format!("Text parse error: {:?}", e))?;
        return Err(format!("HTTP {}: {:?}", status, text));
    }

    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON error: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON parse error: {:?}", e))?;

    serde_wasm_bindgen::from_value(json)
        .map_err(|e| format!("Deserialize error: {:?}", e))
}

/// Stream AI response with tools - streams content to UI and returns tool_calls if any.
async fn stream_ai_with_tools(
    state: &Rc<RefCell<AppState>>,
    messages: &[Message],
) -> Result<AiResponse, String> {
    let window = web_sys::window().ok_or("No window")?;

    // NOTE: Model MUST be anthropic/claude-sonnet-4.5 - DO NOT CHANGE
    let body = serde_json::json!({
        "messages": messages,
        "model": "anthropic/claude-sonnet-4.5",
        "max_tokens": 4096,
        "tools": get_tools(),
        "tool_choice": "auto"
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_credentials(web_sys::RequestCredentials::Include);

    let headers = web_sys::Headers::new().map_err(|e| format!("Headers error: {:?}", e))?;
    headers.set("Content-Type", "application/json").map_err(|e| format!("Header set error: {:?}", e))?;
    opts.set_headers(&headers);
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body.to_string()));

    let request = web_sys::Request::new_with_str_and_init("/api/ai/chat/stream", &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast error")?;

    if !resp.ok() {
        let status = resp.status();
        let text = JsFuture::from(resp.text().map_err(|e| format!("Text error: {:?}", e))?)
            .await
            .map_err(|e| format!("Text parse error: {:?}", e))?;
        return Err(format!("HTTP {}: {:?}", status, text));
    }

    // Get the body as a readable stream
    let body_stream = resp.body().ok_or("No response body")?;
    let reader: web_sys::ReadableStreamDefaultReader = body_stream
        .get_reader()
        .dyn_into()
        .map_err(|_| "Not a ReadableStreamDefaultReader")?;

    let mut accumulated_content = String::new();
    let mut tool_calls: Vec<ToolUseBlock> = Vec::new();
    let mut current_tool_call: Option<(String, String, String)> = None; // (id, name, arguments)
    let mut finish_reason: Option<String> = None;

    loop {
        let result = JsFuture::from(reader.read())
            .await
            .map_err(|e| format!("Read error: {:?}", e))?;

        let done = js_sys::Reflect::get(&result, &"done".into())
            .map_err(|_| "No done property")?
            .as_bool()
            .unwrap_or(true);

        if done {
            break;
        }

        let value = js_sys::Reflect::get(&result, &"value".into())
            .map_err(|_| "No value property")?;

        if value.is_undefined() {
            continue;
        }

        let array: js_sys::Uint8Array = value.dyn_into().map_err(|_| "Not a Uint8Array")?;
        let chunk = String::from_utf8_lossy(&array.to_vec()).to_string();

        // Parse SSE data lines
        for line in chunk.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data == "[DONE]" {
                    continue;
                }

                // Parse the JSON chunk
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(choices) = parsed.get("choices").and_then(|c| c.as_array()) {
                        if let Some(choice) = choices.first() {
                            // Check finish_reason
                            if let Some(fr) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                                finish_reason = Some(fr.to_string());
                            }

                            if let Some(delta) = choice.get("delta") {
                                // Extract content
                                if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                    accumulated_content.push_str(content);
                                    // Update UI with streamed content
                                    let mut state_guard = state.borrow_mut();
                                    state_guard.autopilot_chat.update_last_assistant(&accumulated_content);
                                }

                                // Extract tool_calls (OpenAI streaming format)
                                if let Some(tc_array) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                    for tc in tc_array {
                                        let _index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;

                                        // New tool call starting
                                        if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                                            // Save previous tool call if any
                                            if let Some((prev_id, prev_name, prev_args)) = current_tool_call.take() {
                                                if let Ok(input) = serde_json::from_str::<serde_json::Value>(&prev_args) {
                                                    tool_calls.push(ToolUseBlock {
                                                        id: prev_id,
                                                        name: prev_name,
                                                        input,
                                                    });
                                                }
                                            }

                                            let name = tc.get("function")
                                                .and_then(|f| f.get("name"))
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            current_tool_call = Some((id.to_string(), name, String::new()));
                                        }

                                        // Accumulate arguments
                                        if let Some(func) = tc.get("function") {
                                            if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                                if let Some((_, _, ref mut accumulated_args)) = current_tool_call {
                                                    accumulated_args.push_str(args);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Finalize any remaining tool call
    if let Some((id, name, args)) = current_tool_call.take() {
        if let Ok(input) = serde_json::from_str::<serde_json::Value>(&args) {
            tool_calls.push(ToolUseBlock { id, name, input });
        }
    }

    Ok(AiResponse {
        content: accumulated_content,
        finish_reason,
        tool_use: if tool_calls.is_empty() { None } else { Some(tool_calls) },
    })
}

/// Start the intro agent (public entry point).
pub(crate) fn start_intro_agent(
    state: Rc<RefCell<AppState>>,
    github_username: String,
    repo: String,
) {
    IntroAgentController::start(state, github_username, repo);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Error Classification Tests
    // ============================================

    #[test]
    fn test_classify_rate_limit_errors() {
        // 429 status
        assert_eq!(classify_error("HTTP 429: Too Many Requests"), ErrorClass::RateLimit);
        assert_eq!(classify_error("rate limit exceeded"), ErrorClass::RateLimit);
        assert_eq!(classify_error("too many requests"), ErrorClass::RateLimit);
    }

    #[test]
    fn test_classify_token_limit_errors() {
        // Token/context limits
        assert_eq!(classify_error("token limit exceeded"), ErrorClass::TokenLimit);
        assert_eq!(classify_error("context too long"), ErrorClass::TokenLimit);
        assert_eq!(classify_error("maximum context length"), ErrorClass::TokenLimit);
        assert_eq!(classify_error("request too long"), ErrorClass::TokenLimit);
    }

    #[test]
    fn test_classify_format_errors() {
        // Format/parse errors
        assert_eq!(classify_error("missing field 'content'"), ErrorClass::Format);
        assert_eq!(classify_error("invalid request format"), ErrorClass::Format);
        assert_eq!(classify_error("parse error at line 1"), ErrorClass::Format);
        assert_eq!(classify_error("content must be a string"), ErrorClass::Format);
    }

    #[test]
    fn test_classify_transient_errors() {
        // Server errors
        assert_eq!(classify_error("HTTP 500: Internal Server Error"), ErrorClass::Transient);
        assert_eq!(classify_error("HTTP 502: Bad Gateway"), ErrorClass::Transient);
        assert_eq!(classify_error("HTTP 503: Service Unavailable"), ErrorClass::Transient);
        assert_eq!(classify_error("HTTP 504: Gateway Timeout"), ErrorClass::Transient);
        assert_eq!(classify_error("network error"), ErrorClass::Transient);
        assert_eq!(classify_error("fetch error: connection refused"), ErrorClass::Transient);
        assert_eq!(classify_error("request timeout"), ErrorClass::Transient);
    }

    #[test]
    fn test_classify_unrecoverable_errors() {
        // Client errors that aren't special cases
        assert_eq!(classify_error("HTTP 400: Bad Request"), ErrorClass::Unrecoverable);
        assert_eq!(classify_error("HTTP 401: Unauthorized"), ErrorClass::Unrecoverable);
        assert_eq!(classify_error("HTTP 403: Forbidden"), ErrorClass::Unrecoverable);
        assert_eq!(classify_error("HTTP 404: Not Found"), ErrorClass::Unrecoverable);
        assert_eq!(classify_error("unknown error"), ErrorClass::Unrecoverable);
    }

    // ============================================
    // Message Truncation Tests
    // ============================================

    #[test]
    fn test_truncate_short_content() {
        let mut messages = vec![
            Message {
                role: "tool".to_string(),
                content: Some(serde_json::json!("short content")),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];

        truncate_tool_results(&mut messages, 1000);

        // Should not be truncated
        assert_eq!(
            messages[0].content.as_ref().unwrap().as_str().unwrap(),
            "short content"
        );
    }

    #[test]
    fn test_truncate_long_content() {
        let long_content = "a".repeat(2000);
        let mut messages = vec![
            Message {
                role: "tool".to_string(),
                content: Some(serde_json::json!(long_content)),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];

        truncate_tool_results(&mut messages, 500);

        let truncated = messages[0].content.as_ref().unwrap().as_str().unwrap();
        assert!(truncated.contains("...[truncated to"));
        assert!(truncated.len() < 600); // Should be around 500 + suffix
    }

    #[test]
    fn test_truncate_only_tool_messages() {
        let long_content = "b".repeat(2000);
        let mut messages = vec![
            Message {
                role: "user".to_string(),
                content: Some(serde_json::json!(long_content.clone())),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "tool".to_string(),
                content: Some(serde_json::json!(long_content.clone())),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];

        truncate_tool_results(&mut messages, 500);

        // User message should NOT be truncated
        assert_eq!(
            messages[0].content.as_ref().unwrap().as_str().unwrap().len(),
            2000
        );

        // Tool message SHOULD be truncated
        let tool_content = messages[1].content.as_ref().unwrap().as_str().unwrap();
        assert!(tool_content.contains("...[truncated to"));
    }

    #[test]
    fn test_truncate_utf8_boundary() {
        // Test with multi-byte UTF-8 characters (emoji)
        let content_with_emoji = format!("{}🔥🔥🔥", "a".repeat(495));
        let mut messages = vec![
            Message {
                role: "tool".to_string(),
                content: Some(serde_json::json!(content_with_emoji)),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];

        truncate_tool_results(&mut messages, 500);

        // Should not panic, and should be valid UTF-8
        let truncated = messages[0].content.as_ref().unwrap().as_str().unwrap();
        assert!(truncated.is_ascii() || truncated.chars().count() > 0);
    }

    // ============================================
    // Message Sanitization Tests
    // ============================================

    #[test]
    fn test_sanitize_tool_message_without_content() {
        let mut messages = vec![
            Message {
                role: "tool".to_string(),
                content: None,
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];

        sanitize_messages(&mut messages);

        // Should have content now
        assert!(messages[0].content.is_some());
        assert_eq!(
            messages[0].content.as_ref().unwrap().as_str().unwrap(),
            "[No content]"
        );
    }

    #[test]
    fn test_sanitize_preserves_valid_messages() {
        let mut messages = vec![
            Message {
                role: "user".to_string(),
                content: Some(serde_json::json!("Hello")),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: "assistant".to_string(),
                content: Some(serde_json::json!("Hi there")),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        sanitize_messages(&mut messages);

        // Should be unchanged
        assert_eq!(
            messages[0].content.as_ref().unwrap().as_str().unwrap(),
            "Hello"
        );
        assert_eq!(
            messages[1].content.as_ref().unwrap().as_str().unwrap(),
            "Hi there"
        );
    }

    // ============================================
    // Integration-Style Tests (without actual HTTP)
    // ============================================

    #[test]
    fn test_error_recovery_sequence() {
        // Simulate the recovery flow: error -> classify -> recover

        // Token limit error
        let error = "token limit exceeded: context too long";
        let class = classify_error(error);
        assert_eq!(class, ErrorClass::TokenLimit);

        // Should truncate messages
        let mut messages = vec![
            Message {
                role: "tool".to_string(),
                content: Some(serde_json::json!("x".repeat(5000))),
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];
        truncate_tool_results(&mut messages, 1000);
        let len = messages[0].content.as_ref().unwrap().as_str().unwrap().len();
        assert!(len < 1200, "Should be truncated, but was {}", len);
    }

    #[test]
    fn test_format_error_recovery() {
        // Format error
        let error = "missing field 'content' at line 1 column 18135";
        let class = classify_error(error);
        assert_eq!(class, ErrorClass::Format);

        // Should sanitize messages
        let mut messages = vec![
            Message {
                role: "tool".to_string(),
                content: None,
                tool_calls: None,
                tool_call_id: Some("call_1".to_string()),
            },
        ];
        sanitize_messages(&mut messages);
        assert!(messages[0].content.is_some());
    }

    #[test]
    fn test_transient_error_is_retryable() {
        let errors = [
            "HTTP 500: Internal Server Error",
            "HTTP 502: Bad Gateway",
            "network error",
            "fetch error: connection refused",
        ];

        for error in errors {
            let class = classify_error(error);
            assert_eq!(class, ErrorClass::Transient, "Error '{}' should be transient", error);
        }
    }

    #[test]
    fn test_rate_limit_is_retryable() {
        let errors = [
            "HTTP 429: Too Many Requests",
            "rate limit exceeded",
        ];

        for error in errors {
            let class = classify_error(error);
            assert_eq!(class, ErrorClass::RateLimit, "Error '{}' should be rate limit", error);
        }
    }
}
