//! The tracker client: issues, projects, comments, labels, assignees,
//! milestones, and prerequisites.
//!
//! This is the Rust port of `packages/openagents-cli/src/tracker-request.ts`,
//! `issue-client.ts`, and `project-client.ts`. It keeps the same three
//! properties those files were written for:
//!
//! 1. The server's own body is what gets returned. Every route here answers a
//!    GitHub-compatible shape, so the client parses nothing it does not have to
//!    and `--json` prints exactly what the server sent.
//! 2. A status outside the accepted set is an error carrying the server's own
//!    message, never an empty list. An earlier version of this file answered
//!    every non-2xx with `Ok(Vec::new())`, so `oa project list` reported no
//!    projects — with exit status 0 — against a repository that has four,
//!    because it was asking for `/projects` and the route is `/projectsV2`.
//! 3. The list route publishes no `per_page`, so a limit above one page is only
//!    reachable by paging until the server's own total is covered.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde_json::{json, Value};
use std::fmt;

/// The largest list the CLI will page for; the server holds 25 to a page.
pub const MAXIMUM_ISSUE_LIST_LIMIT: u32 = 1_000;

/// Why a tracker call did not produce data. Never a substitute for data.
#[derive(Debug)]
pub enum ApiError {
    /// The request never completed.
    Transport { operation: String, why: String },
    /// The server answered, and refused. Carries the status and its own message.
    Refused {
        operation: String,
        status: u16,
        message: String,
    },
    /// The server answered inside the accepted set with a body this cannot read.
    Malformed { operation: String, why: String },
    /// The caller asked for something the client will not send.
    Input(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport { operation, why } => {
                write!(f, "Could not reach the API to {}: {}", operation, why)
            }
            Self::Refused {
                operation,
                status,
                message,
            } => write!(
                f,
                "The API refused the request to {} (HTTP {}): {}",
                operation, status, message
            ),
            Self::Malformed { operation, why } => write!(
                f,
                "The API returned an unreadable body for {}: {}",
                operation, why
            ),
            Self::Input(message) => write!(f, "{}", message),
        }
    }
}

impl std::error::Error for ApiError {}

/// Turns the unified error envelope into one sentence.
///
/// The deployment answers refusals in more than one shape: `{"message": …,
/// "errors": {field: [messages]}}` from the tracker routes, `{"error":
/// {"code": …}}` from the token guard, and `{"errors": {"detail": …}}` from the
/// router. All three carry something worth printing, so all three are read.
/// Nothing is invented: with no readable field, the sentence is the status.
pub fn error_sentence(body: &str, status: u16) -> String {
    let parsed: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        // A non-JSON body is still the server's answer. Print it, bounded.
        Err(_) => {
            let trimmed = body.trim();
            return if trimmed.is_empty() {
                format!("The OpenAgents API returned HTTP {}.", status)
            } else {
                trimmed[..trimmed.len().min(400)].to_string()
            };
        }
    };

    let mut sentence = parsed
        .get("message")
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| {
            parsed
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map(String::from)
        })
        .or_else(|| {
            parsed
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(Value::as_str)
                .map(String::from)
        })
        .unwrap_or_else(|| format!("The OpenAgents API returned HTTP {}.", status));

    // `errors` is a field-to-messages map, so a rejected write names the field
    // it was rejected on instead of a bare status the caller has to reproduce.
    if let Some(fields) = parsed.get("errors").and_then(Value::as_object) {
        let rendered: Vec<String> = fields
            .iter()
            .map(|(field, messages)| format!("{}: {}", field, message_list(messages)))
            .collect();
        if !rendered.is_empty() {
            sentence = format!("{} ({})", sentence, rendered.join("; "));
        }
    }

    if let Some(request_id) = parsed.get("request_id").and_then(Value::as_str) {
        sentence = format!("{} [request {}]", sentence, request_id);
    }
    sentence
}

fn message_list(value: &Value) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| match item {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Percent-encode one path segment or query value.
pub fn urlencode(value: &str) -> String {
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

/// An owner and a repository, already split so neither is guessed downstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoTarget {
    pub owner: String,
    pub repo: String,
}

impl RepoTarget {
    /// Reads `owner/repo`. Anything else is refused rather than half-parsed.
    pub fn parse(slug: &str) -> Result<Self, ApiError> {
        let trimmed = slug.trim().trim_end_matches(".git");
        let mut parts = trimmed.split('/').filter(|part| !part.is_empty());
        match (parts.next(), parts.next(), parts.next()) {
            (Some(owner), Some(repo), None) => Ok(Self {
                owner: owner.to_string(),
                repo: repo.to_string(),
            }),
            _ => Err(ApiError::Input(format!(
                "`{}` is not a repository. Pass -R owner/repo, such as -R OpenAgentsInc/openagents.",
                slug
            ))),
        }
    }

    fn path(&self) -> String {
        format!(
            "repos/{}/{}",
            urlencode(&self.owner),
            urlencode(&self.repo)
        )
    }
}

/// The repository a tracker command runs against.
///
/// `-R` wins. With no flag the checkout names it, the same way the TypeScript
/// CLI resolves it: whichever remote points at a forge or GitHub URL. With no
/// flag and no readable remote the command refuses — it does not fall back to
/// a repository the caller never named.
pub fn resolve_repo_target(flag: Option<&str>) -> Result<RepoTarget, ApiError> {
    if let Some(slug) = flag {
        return RepoTarget::parse(slug);
    }
    if let Some(slug) = repo_slug_from_git_remote() {
        return RepoTarget::parse(&slug);
    }
    Err(ApiError::Input(
        "No repository named. Pass -R owner/repo, or run inside a checkout whose \
         `openagents` or `origin` remote points at one."
            .to_string(),
    ))
}

/// Reads `owner/repo` out of this checkout's remotes, preferring the forge.
fn repo_slug_from_git_remote() -> Option<String> {
    for remote in ["openagents", "origin"] {
        let output = std::process::Command::new("git")
            .args(["remote", "get-url", remote])
            .output()
            .ok()?;
        if !output.status.success() {
            continue;
        }
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if let Some(slug) = slug_from_remote_url(&url) {
            return Some(slug);
        }
    }
    None
}

/// `https://openagents.com/Owner/repo.git` and `git@github.com:Owner/repo.git`
/// both name `Owner/repo`.
pub fn slug_from_remote_url(url: &str) -> Option<String> {
    // Strip the scheme, then any `user@`, so what is left starts at the host
    // in both forms. Each step falls back to what it was handed, not to the
    // original string.
    let after_scheme = match url.split_once("://") {
        Some((_, rest)) => rest,
        None => url,
    };
    let after_user = match after_scheme.rsplit_once('@') {
        Some((_, rest)) => rest,
        None => after_scheme,
    };
    // `host:owner/repo` for SSH, `host/owner/repo` for HTTPS.
    let path = match after_user.split_once(':') {
        Some((_, rest)) => rest,
        None => after_user.split_once('/').map(|(_, rest)| rest)?,
    };
    let cleaned = path.trim_matches('/').trim_end_matches(".git");
    let parts: Vec<&str> = cleaned.split('/').filter(|p| !p.is_empty()).collect();
    if parts.len() == 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

/// What `oa issue list` asks for.
#[derive(Debug, Clone, Default)]
pub struct IssueListOptions {
    pub limit: u32,
    pub state: Option<String>,
    pub label: Option<String>,
    pub assignee: Option<String>,
    pub milestone: Option<String>,
    pub search: Option<String>,
    pub blocked: Option<bool>,
}

/// The rows the server sent, and the server's own pagination object.
#[derive(Debug, Clone)]
pub struct IssueListResult {
    pub pagination: Value,
    pub issues: Vec<Value>,
}

pub struct TrackerClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl TrackerClient {
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
        map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.token {
            if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", tok)) {
                map.insert(AUTHORIZATION, val);
            }
        }
        map
    }

    /// One request, one accepted-status check, one failure translation.
    ///
    /// Everything below goes through here, which is what makes "a non-2xx is an
    /// error" a property of the client rather than a habit each method has to
    /// remember.
    pub async fn request(
        &self,
        operation: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
        accepted: &[u16],
    ) -> Result<Value, ApiError> {
        let url = format!("{}/{}", self.api_base, path.trim_start_matches('/'));
        let mut builder = match method {
            "GET" => self.http.get(&url),
            "POST" => self.http.post(&url),
            "PATCH" => self.http.patch(&url),
            "PUT" => self.http.put(&url),
            "DELETE" => self.http.delete(&url),
            other => {
                return Err(ApiError::Input(format!(
                    "{} is not an HTTP method this client sends.",
                    other
                )))
            }
        }
        .headers(self.headers());
        if let Some(payload) = body {
            builder = builder.json(&payload);
        }

        let response = builder.send().await.map_err(|e| ApiError::Transport {
            operation: operation.to_string(),
            why: e.to_string(),
        })?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|e| ApiError::Transport {
            operation: operation.to_string(),
            why: e.to_string(),
        })?;

        if !accepted.contains(&status) {
            return Err(ApiError::Refused {
                operation: operation.to_string(),
                status,
                message: error_sentence(&text, status),
            });
        }
        if text.trim().is_empty() {
            // A 204 carries no body, and that is the server's answer, not a
            // stand-in for one.
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|e| ApiError::Malformed {
            operation: operation.to_string(),
            why: e.to_string(),
        })
    }

    // ---------------------------------------------------------------- issues

    fn issues_path(target: &RepoTarget) -> String {
        format!("{}/issues", target.path())
    }

    fn issue_path(target: &RepoTarget, number: u64) -> String {
        format!("{}/issues/{}", target.path(), number)
    }

    /// Lists issues, paging until the limit or the server's own total is met.
    pub async fn list_issues(
        &self,
        target: &RepoTarget,
        options: &IssueListOptions,
    ) -> Result<IssueListResult, ApiError> {
        if options.limit < 1 {
            return Err(ApiError::Input(
                "--limit must be a positive integer.".to_string(),
            ));
        }
        if options.limit > MAXIMUM_ISSUE_LIST_LIMIT {
            return Err(ApiError::Input(format!(
                "--limit must be at most {}.",
                MAXIMUM_ISSUE_LIST_LIMIT
            )));
        }

        let mut collected: Vec<Value> = Vec::new();
        let mut pagination = Value::Null;
        let mut page = 1u32;

        while (collected.len() as u32) < options.limit {
            let query = list_query(options, page);
            let body = self
                .request(
                    "list issues",
                    "GET",
                    &format!("{}?{}", Self::issues_path(target), query),
                    None,
                    &[200],
                )
                .await?;
            pagination = body.get("pagination").cloned().unwrap_or(Value::Null);
            let rows = body
                .get("issues")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let received = rows.len();
            collected.extend(rows);
            if received == 0 {
                break;
            }
            if let Some(total) = pagination.get("total").and_then(Value::as_u64) {
                if collected.len() as u64 >= total {
                    break;
                }
            }
            if let Some(total_pages) = pagination.get("total_pages").and_then(Value::as_u64) {
                if page as u64 >= total_pages {
                    break;
                }
            }
            page += 1;
        }

        collected.truncate(options.limit as usize);
        Ok(IssueListResult {
            pagination,
            issues: collected,
        })
    }

    pub async fn view_issue(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "view an issue",
            "GET",
            &Self::issue_path(target, number),
            None,
            &[200],
        )
        .await
    }

    pub async fn create_issue(
        &self,
        target: &RepoTarget,
        title: &str,
        body: Option<&str>,
        labels: &[String],
        assignees: &[String],
        milestone: Option<u64>,
    ) -> Result<Value, ApiError> {
        let mut payload = json!({ "title": title });
        if let Some(text) = body {
            payload["body"] = json!(text);
        }
        if !labels.is_empty() {
            payload["labels"] = json!(labels);
        }
        if !assignees.is_empty() {
            payload["assignees"] = json!(assignees);
        }
        if let Some(number) = milestone {
            payload["milestone"] = json!(number);
        }
        self.request(
            "create an issue",
            "POST",
            &Self::issues_path(target),
            Some(payload),
            &[201],
        )
        .await
    }

    /// A `PATCH` carrying `body` replaces the issue text, so a state change
    /// sends `state` and nothing else.
    pub async fn set_issue_state(
        &self,
        target: &RepoTarget,
        number: u64,
        state: &str,
    ) -> Result<Value, ApiError> {
        self.request(
            "change issue state",
            "PATCH",
            &Self::issue_path(target, number),
            Some(json!({ "state": state })),
            &[200],
        )
        .await
    }

    pub async fn list_comments(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "list issue comments",
            "GET",
            &format!("{}/comments", Self::issue_path(target, number)),
            None,
            &[200],
        )
        .await
    }

    pub async fn comment_issue(
        &self,
        target: &RepoTarget,
        number: u64,
        body: &str,
    ) -> Result<Value, ApiError> {
        self.request(
            "comment on an issue",
            "POST",
            &format!("{}/comments", Self::issue_path(target, number)),
            Some(json!({ "body": body })),
            &[201],
        )
        .await
    }

    pub async fn list_labels(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "list issue labels",
            "GET",
            &format!("{}/labels", Self::issue_path(target, number)),
            None,
            &[200],
        )
        .await
    }

    pub async fn add_labels(
        &self,
        target: &RepoTarget,
        number: u64,
        labels: &[String],
    ) -> Result<Value, ApiError> {
        self.request(
            "label an issue",
            "POST",
            &format!("{}/labels", Self::issue_path(target, number)),
            Some(json!({ "labels": labels })),
            &[200, 201],
        )
        .await
    }

    pub async fn remove_label(
        &self,
        target: &RepoTarget,
        number: u64,
        label: &str,
    ) -> Result<Value, ApiError> {
        self.request(
            "remove an issue label",
            "DELETE",
            &format!(
                "{}/labels/{}",
                Self::issue_path(target, number),
                urlencode(label)
            ),
            None,
            &[200],
        )
        .await
    }

    pub async fn list_assignees(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "list issue assignees",
            "GET",
            &format!("{}/assignees", Self::issue_path(target, number)),
            None,
            &[200],
        )
        .await
    }

    pub async fn add_assignees(
        &self,
        target: &RepoTarget,
        number: u64,
        assignees: &[String],
    ) -> Result<Value, ApiError> {
        self.request(
            "assign an issue",
            "POST",
            &format!("{}/assignees", Self::issue_path(target, number)),
            Some(json!({ "assignees": assignees })),
            &[200, 201],
        )
        .await
    }

    /// The route reads the logins from a body rather than the path, so this
    /// `DELETE` carries one.
    pub async fn remove_assignees(
        &self,
        target: &RepoTarget,
        number: u64,
        assignees: &[String],
    ) -> Result<Value, ApiError> {
        self.request(
            "unassign an issue",
            "DELETE",
            &format!("{}/assignees", Self::issue_path(target, number)),
            Some(json!({ "assignees": assignees })),
            &[200],
        )
        .await
    }

    pub async fn dependencies(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "read issue prerequisites",
            "GET",
            &format!("{}/dependencies", Self::issue_path(target, number)),
            None,
            &[200],
        )
        .await
    }

    pub async fn add_dependencies(
        &self,
        target: &RepoTarget,
        number: u64,
        blocked_by: &[u64],
    ) -> Result<Value, ApiError> {
        self.request(
            "add issue prerequisites",
            "POST",
            &format!("{}/dependencies", Self::issue_path(target, number)),
            Some(json!({ "blocked_by": blocked_by })),
            &[200, 201],
        )
        .await
    }

    /// The prerequisite is a path segment here, not a body key.
    pub async fn remove_dependency(
        &self,
        target: &RepoTarget,
        number: u64,
        blocked_by: u64,
    ) -> Result<Value, ApiError> {
        self.request(
            "remove an issue prerequisite",
            "DELETE",
            &format!(
                "{}/dependencies/{}",
                Self::issue_path(target, number),
                blocked_by
            ),
            None,
            &[200],
        )
        .await
    }

    // ------------------------------------------------------------ milestones

    pub async fn list_milestones(&self, target: &RepoTarget) -> Result<Value, ApiError> {
        self.request(
            "list milestones",
            "GET",
            &format!("{}/milestones", target.path()),
            None,
            &[200],
        )
        .await
    }

    pub async fn create_milestone(
        &self,
        target: &RepoTarget,
        title: &str,
        description: Option<&str>,
        due_on: Option<&str>,
    ) -> Result<Value, ApiError> {
        let mut payload = json!({ "title": title });
        if let Some(text) = description {
            payload["description"] = json!(text);
        }
        if let Some(when) = due_on {
            payload["due_on"] = json!(when);
        }
        self.request(
            "create a milestone",
            "POST",
            &format!("{}/milestones", target.path()),
            Some(payload),
            &[201],
        )
        .await
    }

    pub async fn delete_milestone(
        &self,
        target: &RepoTarget,
        number: u64,
    ) -> Result<Value, ApiError> {
        self.request(
            "delete a milestone",
            "DELETE",
            &format!("{}/milestones/{}", target.path(), number),
            None,
            &[200, 204],
        )
        .await
    }

    // -------------------------------------------------------------- projects

    /// The route is `projectsV2`. `projects` does not exist, and asking for it
    /// is what made this command report an empty board list for years.
    fn projects_path(target: &RepoTarget) -> String {
        format!("{}/projectsV2", target.path())
    }

    fn project_path(target: &RepoTarget, number: u64) -> String {
        format!("{}/{}", Self::projects_path(target), number)
    }

    fn item_path(target: &RepoTarget, number: u64, item: &str) -> String {
        format!(
            "{}/items/{}",
            Self::project_path(target, number),
            urlencode(item)
        )
    }

    pub async fn list_projects(
        &self,
        target: &RepoTarget,
        archived: bool,
    ) -> Result<Value, ApiError> {
        let path = if archived {
            format!("{}?archived=true", Self::projects_path(target))
        } else {
            Self::projects_path(target)
        };
        self.request("list projects", "GET", &path, None, &[200])
            .await
    }

    pub async fn view_project(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "view a project",
            "GET",
            &Self::project_path(target, number),
            None,
            &[200],
        )
        .await
    }

    pub async fn create_project(
        &self,
        target: &RepoTarget,
        title: &str,
        description: Option<&str>,
    ) -> Result<Value, ApiError> {
        let mut payload = json!({ "title": title });
        if let Some(text) = description {
            payload["description"] = json!(text);
        }
        self.request(
            "create a project",
            "POST",
            &Self::projects_path(target),
            Some(payload),
            &[201],
        )
        .await
    }

    pub async fn project_fields(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "list project fields",
            "GET",
            &format!("{}/fields", Self::project_path(target, number)),
            None,
            &[200],
        )
        .await
    }

    pub async fn project_items(&self, target: &RepoTarget, number: u64) -> Result<Value, ApiError> {
        self.request(
            "list project items",
            "GET",
            &format!("{}/items", Self::project_path(target, number)),
            None,
            &[200],
        )
        .await
    }

    /// A repeated add answers 200 with the membership the board already has, so
    /// both statuses are the same success.
    pub async fn project_add_item(
        &self,
        target: &RepoTarget,
        number: u64,
        issue_number: u64,
    ) -> Result<Value, ApiError> {
        self.request(
            "add a project item",
            "POST",
            &format!("{}/items", Self::project_path(target, number)),
            Some(json!({ "issue_number": issue_number })),
            &[200, 201],
        )
        .await
    }

    pub async fn project_set_item_values(
        &self,
        target: &RepoTarget,
        number: u64,
        item: &str,
        values: &Value,
    ) -> Result<Value, ApiError> {
        self.request(
            "set project item values",
            "PATCH",
            &Self::item_path(target, number, item),
            Some(json!({ "values": values })),
            &[200],
        )
        .await
    }

    pub async fn project_move_item(
        &self,
        target: &RepoTarget,
        number: u64,
        item: &str,
        values: &Value,
        position: Option<u64>,
    ) -> Result<Value, ApiError> {
        let mut payload = json!({ "values": values });
        if let Some(rank) = position {
            payload["position"] = json!(rank);
        }
        self.request(
            "move a project item",
            "POST",
            &format!("{}/move", Self::item_path(target, number, item)),
            Some(payload),
            &[200],
        )
        .await
    }

    pub async fn project_remove_item(
        &self,
        target: &RepoTarget,
        number: u64,
        item: &str,
    ) -> Result<Value, ApiError> {
        self.request(
            "remove a project item",
            "DELETE",
            &Self::item_path(target, number, item),
            None,
            &[200, 204],
        )
        .await
    }
}

/// The list route names its search parameter `q` and its label parameter
/// `labels`; the flags read the way a person says them.
fn list_query(options: &IssueListOptions, page: u32) -> String {
    let mut parts = vec![
        format!(
            "state={}",
            urlencode(options.state.as_deref().unwrap_or("open"))
        ),
        format!("page={}", page),
    ];
    if let Some(label) = &options.label {
        parts.push(format!("labels={}", urlencode(label)));
    }
    if let Some(assignee) = &options.assignee {
        parts.push(format!("assignee={}", urlencode(assignee)));
    }
    if let Some(milestone) = &options.milestone {
        parts.push(format!("milestone={}", urlencode(milestone)));
    }
    if let Some(search) = &options.search {
        parts.push(format!("q={}", urlencode(search)));
    }
    if let Some(blocked) = options.blocked {
        parts.push(format!("blocked={}", blocked));
    }
    parts.join("&")
}
