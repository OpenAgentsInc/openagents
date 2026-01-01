//! GitHub exploration endpoint for the intro agent.
//!
//! Uses a single GraphQL query to fetch all repository data efficiently.

use crate::db::users;
use crate::middleware::auth::authenticate;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::*;

/// Repository metadata.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct RepoInfo {
    pub description: Option<String>,
    pub language: Option<String>,
    pub stargazers_count: Option<u64>,
    pub open_issues_count: Option<u64>,
    pub forks_count: Option<u64>,
    pub default_branch: Option<String>,
}

/// Issue info.
#[derive(Debug, Serialize, Deserialize)]
pub struct IssueInfo {
    pub number: u64,
    pub title: String,
    pub state: String,
}

/// Pull request info.
#[derive(Debug, Serialize, Deserialize)]
pub struct PrInfo {
    pub number: u64,
    pub title: String,
    pub state: String,
}

/// Tree entry.
#[derive(Debug, Serialize, Deserialize)]
pub struct TreeEntry {
    pub path: String,
    #[serde(rename = "type")]
    pub entry_type: String,
}

/// Commit info.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub message: String,
}

/// Contributor info.
#[derive(Debug, Serialize, Deserialize)]
pub struct ContributorInfo {
    pub login: String,
    pub contributions: u64,
}

/// Full exploration response.
#[derive(Debug, Serialize)]
pub struct ExploreResponse {
    pub repo: Option<RepoInfo>,
    pub issues: Vec<IssueInfo>,
    pub pull_requests: Vec<PrInfo>,
    pub tree: Vec<TreeEntry>,
    pub readme_excerpt: Option<String>,
    pub commits: Vec<CommitInfo>,
    pub contributors: Vec<ContributorInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// GraphQL query to fetch all repo data in one call.
const GRAPHQL_QUERY: &str = r#"
query RepoExplore($owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) {
    description
    stargazerCount
    forkCount
    primaryLanguage { name }
    defaultBranchRef { name }
    issues(first: 10, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
      totalCount
      nodes { number title state }
    }
    pullRequests(first: 10, states: OPEN, orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes { number title state }
    }
    object(expression: "HEAD:") {
      ... on Tree {
        entries { name type }
      }
    }
    readme: object(expression: "HEAD:README.md") {
      ... on Blob { text }
    }
    defaultBranchRef {
      target {
        ... on Commit {
          history(first: 5) {
            nodes {
              oid
              messageHeadline
              author { user { login } }
            }
          }
        }
      }
    }
  }
}
"#;

/// GET /api/github/explore?repo=owner/name
///
/// Fetches comprehensive repository information using a single GraphQL query.
pub async fn explore(req: Request, env: Env) -> Result<Response> {
    // Require authentication
    let user = authenticate(&req, &env).await?;

    // Get repo parameter
    let url = req.url()?;
    let repo_param = url
        .query_pairs()
        .find(|(k, _)| k == "repo")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| Error::RustError("Missing repo parameter".to_string()))?;

    // Parse owner/name
    let parts: Vec<&str> = repo_param.split('/').collect();
    if parts.len() != 2 {
        return Response::error("Invalid repo format, expected owner/name", 400);
    }
    let (owner, name) = (parts[0], parts[1]);

    // Get GitHub access token
    let db = env.d1("DB")?;
    let session_secret = env.secret("SESSION_SECRET")?.to_string();
    let access_token = users::get_github_access_token(&db, &user.user_id, &session_secret).await?;

    // Execute GraphQL query
    let graphql_response = execute_graphql(&access_token, owner, name).await?;

    // Parse and transform response
    let response = parse_graphql_response(&graphql_response)?;

    Response::from_json(&response)
}

/// Execute GraphQL query against GitHub API.
async fn execute_graphql(access_token: &str, owner: &str, name: &str) -> Result<String> {
    let body = serde_json::json!({
        "query": GRAPHQL_QUERY,
        "variables": {
            "owner": owner,
            "name": name
        }
    });

    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", access_token))?;
    headers.set("User-Agent", "OpenAgents-Worker/1.0")?;
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body.to_string())));

    let request = Request::new_with_init("https://api.github.com/graphql", &init)?;
    let mut response = Fetch::Request(request).send().await?;

    if !response.status_code().to_string().starts_with('2') {
        let status = response.status_code();
        let body = response.text().await.unwrap_or_default();
        return Err(Error::RustError(format!("GitHub GraphQL error {}: {}", status, body)));
    }

    response.text().await
}

/// GET /api/github/contents?repo=owner/name&path=path
///
/// Fetches file or directory contents from a repository.
pub async fn contents(req: Request, env: Env) -> Result<Response> {
    // Require authentication
    let user = authenticate(&req, &env).await?;

    let url = req.url()?;

    // Get repo parameter
    let repo_param = url
        .query_pairs()
        .find(|(k, _)| k == "repo")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| Error::RustError("Missing repo parameter".to_string()))?;

    // Get path parameter (defaults to root)
    let path_param = url
        .query_pairs()
        .find(|(k, _)| k == "path")
        .map(|(_, v)| v.to_string())
        .unwrap_or_default();

    // Parse owner/name
    let parts: Vec<&str> = repo_param.split('/').collect();
    if parts.len() != 2 {
        return Response::error("Invalid repo format, expected owner/name", 400);
    }
    let (owner, name) = (parts[0], parts[1]);

    // Get GitHub access token
    let db = env.d1("DB")?;
    let session_secret = env.secret("SESSION_SECRET")?.to_string();
    let access_token = users::get_github_access_token(&db, &user.user_id, &session_secret).await?;

    // Fetch contents using GraphQL
    let response = fetch_contents(&access_token, owner, name, &path_param).await?;

    Response::from_json(&response)
}

/// Contents response - can be a file or directory listing.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum ContentsResponse {
    #[serde(rename = "file")]
    File {
        path: String,
        content: String,
        size: u64,
        sha: String,
    },
    #[serde(rename = "directory")]
    Directory {
        path: String,
        entries: Vec<DirectoryEntry>,
    },
    #[serde(rename = "error")]
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize)]
pub struct DirectoryEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub size: Option<u64>,
}

/// GraphQL query for file/directory contents.
const CONTENTS_QUERY: &str = r#"
query GetContents($owner: String!, $name: String!, $expression: String!) {
  repository(owner: $owner, name: $name) {
    object(expression: $expression) {
      oid
      ... on Blob {
        text
        byteSize
      }
      ... on Tree {
        entries {
          name
          type
          oid
          object {
            ... on Blob { byteSize }
          }
        }
      }
    }
  }
}
"#;

/// Fetch file or directory contents.
async fn fetch_contents(access_token: &str, owner: &str, name: &str, path: &str) -> Result<ContentsResponse> {
    // Build expression for HEAD:path
    let expression = if path.is_empty() {
        "HEAD:".to_string()
    } else {
        format!("HEAD:{}", path)
    };

    let body = serde_json::json!({
        "query": CONTENTS_QUERY,
        "variables": {
            "owner": owner,
            "name": name,
            "expression": expression
        }
    });

    let headers = Headers::new();
    headers.set("Accept", "application/json")?;
    headers.set("Authorization", &format!("Bearer {}", access_token))?;
    headers.set("User-Agent", "OpenAgents-Worker/1.0")?;
    headers.set("Content-Type", "application/json")?;

    let mut init = RequestInit::new();
    init.with_method(Method::Post);
    init.with_headers(headers);
    init.with_body(Some(JsValue::from_str(&body.to_string())));

    let request = Request::new_with_init("https://api.github.com/graphql", &init)?;
    let mut response = Fetch::Request(request).send().await?;

    if !response.status_code().to_string().starts_with('2') {
        let status = response.status_code();
        let body = response.text().await.unwrap_or_default();
        return Ok(ContentsResponse::Error {
            message: format!("GitHub API error {}: {}", status, body)
        });
    }

    let json_str = response.text().await?;
    parse_contents_response(&json_str, path)
}

/// Parse GraphQL contents response.
fn parse_contents_response(json_str: &str, path: &str) -> Result<ContentsResponse> {
    #[derive(Deserialize)]
    struct GraphQLResponse {
        data: Option<GraphQLData>,
        errors: Option<Vec<GraphQLError>>,
    }

    #[derive(Deserialize)]
    struct GraphQLError {
        message: String,
    }

    #[derive(Deserialize)]
    struct GraphQLData {
        repository: Option<GraphQLRepo>,
    }

    #[derive(Deserialize)]
    struct GraphQLRepo {
        object: Option<GraphQLObject>,
    }

    #[derive(Deserialize)]
    struct GraphQLObject {
        // SHA (object ID)
        oid: Option<String>,
        // Blob fields
        text: Option<String>,
        #[serde(rename = "byteSize")]
        byte_size: Option<u64>,
        // Tree fields
        entries: Option<Vec<GraphQLEntry>>,
    }

    #[derive(Deserialize)]
    struct GraphQLEntry {
        name: String,
        #[serde(rename = "type")]
        entry_type: String,
        object: Option<GraphQLEntryObject>,
    }

    #[derive(Deserialize)]
    struct GraphQLEntryObject {
        #[serde(rename = "byteSize")]
        byte_size: Option<u64>,
    }

    let gql_response: GraphQLResponse = serde_json::from_str(json_str)
        .map_err(|e| Error::RustError(format!("GraphQL parse error: {}", e)))?;

    // Check for errors
    if let Some(errors) = gql_response.errors {
        let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        return Ok(ContentsResponse::Error { message: msgs.join("; ") });
    }

    let object = gql_response.data
        .and_then(|d| d.repository)
        .and_then(|r| r.object);

    let Some(obj) = object else {
        return Ok(ContentsResponse::Error {
            message: format!("Path not found: {}", path)
        });
    };

    // Check if it's a file (has text) or directory (has entries)
    if let Some(entries) = obj.entries {
        // It's a directory
        let dir_entries: Vec<DirectoryEntry> = entries.into_iter().map(|e| {
            DirectoryEntry {
                name: e.name,
                entry_type: e.entry_type.to_lowercase(),
                size: e.object.and_then(|o| o.byte_size),
            }
        }).collect();

        Ok(ContentsResponse::Directory {
            path: if path.is_empty() { "/".to_string() } else { path.to_string() },
            entries: dir_entries,
        })
    } else if let Some(text) = obj.text {
        // It's a file
        Ok(ContentsResponse::File {
            path: path.to_string(),
            content: text,
            size: obj.byte_size.unwrap_or(0),
            sha: obj.oid.unwrap_or_default(),
        })
    } else {
        Ok(ContentsResponse::Error {
            message: format!("Unknown object type at path: {}", path)
        })
    }
}

/// Parse GraphQL response into our ExploreResponse format.
fn parse_graphql_response(json_str: &str) -> Result<ExploreResponse> {
    #[derive(Deserialize)]
    struct GraphQLResponse {
        data: Option<GraphQLData>,
        errors: Option<Vec<GraphQLError>>,
    }

    #[derive(Deserialize)]
    struct GraphQLError {
        message: String,
    }

    #[derive(Deserialize)]
    struct GraphQLData {
        repository: Option<GraphQLRepo>,
    }

    #[derive(Deserialize)]
    struct GraphQLRepo {
        description: Option<String>,
        #[serde(rename = "stargazerCount")]
        stargazer_count: Option<u64>,
        #[serde(rename = "forkCount")]
        fork_count: Option<u64>,
        #[serde(rename = "primaryLanguage")]
        primary_language: Option<GraphQLLanguage>,
        #[serde(rename = "defaultBranchRef")]
        default_branch_ref: Option<GraphQLBranchRef>,
        issues: Option<GraphQLIssues>,
        #[serde(rename = "pullRequests")]
        pull_requests: Option<GraphQLPRs>,
        object: Option<GraphQLTreeObject>,
        readme: Option<GraphQLReadme>,
    }

    #[derive(Deserialize)]
    struct GraphQLLanguage {
        name: String,
    }

    #[derive(Deserialize)]
    struct GraphQLBranchRef {
        name: Option<String>,
        target: Option<GraphQLCommitTarget>,
    }

    #[derive(Deserialize)]
    struct GraphQLCommitTarget {
        history: Option<GraphQLHistory>,
    }

    #[derive(Deserialize)]
    struct GraphQLHistory {
        nodes: Vec<GraphQLCommit>,
    }

    #[derive(Deserialize)]
    struct GraphQLCommit {
        oid: String,
        #[serde(rename = "messageHeadline")]
        message_headline: String,
    }

    #[derive(Deserialize)]
    struct GraphQLIssues {
        #[serde(rename = "totalCount")]
        total_count: u64,
        nodes: Vec<GraphQLIssue>,
    }

    #[derive(Deserialize)]
    struct GraphQLIssue {
        number: u64,
        title: String,
        state: String,
    }

    #[derive(Deserialize)]
    struct GraphQLPRs {
        nodes: Vec<GraphQLPR>,
    }

    #[derive(Deserialize)]
    struct GraphQLPR {
        number: u64,
        title: String,
        state: String,
    }

    #[derive(Deserialize)]
    struct GraphQLTreeObject {
        entries: Option<Vec<GraphQLTreeEntry>>,
    }

    #[derive(Deserialize)]
    struct GraphQLTreeEntry {
        name: String,
        #[serde(rename = "type")]
        entry_type: String,
    }

    #[derive(Deserialize)]
    struct GraphQLReadme {
        text: Option<String>,
    }

    let gql_response: GraphQLResponse = serde_json::from_str(json_str)
        .map_err(|e| Error::RustError(format!("GraphQL parse error: {}", e)))?;

    // Check for errors
    if let Some(errors) = gql_response.errors {
        let msgs: Vec<String> = errors.iter().map(|e| e.message.clone()).collect();
        return Ok(ExploreResponse {
            repo: None,
            issues: Vec::new(),
            pull_requests: Vec::new(),
            tree: Vec::new(),
            readme_excerpt: None,
            commits: Vec::new(),
            contributors: Vec::new(),
            error: Some(msgs.join("; ")),
        });
    }

    let repo = gql_response.data
        .and_then(|d| d.repository)
        .ok_or_else(|| Error::RustError("No repository data".to_string()))?;

    // Build response
    let repo_info = RepoInfo {
        description: repo.description,
        language: repo.primary_language.map(|l| l.name),
        stargazers_count: repo.stargazer_count,
        open_issues_count: repo.issues.as_ref().map(|i| i.total_count),
        forks_count: repo.fork_count,
        default_branch: repo.default_branch_ref.as_ref().and_then(|b| b.name.clone()),
    };

    let issues: Vec<IssueInfo> = repo.issues
        .map(|i| i.nodes.into_iter().map(|n| IssueInfo {
            number: n.number,
            title: n.title,
            state: n.state.to_lowercase(),
        }).collect())
        .unwrap_or_default();

    let pull_requests: Vec<PrInfo> = repo.pull_requests
        .map(|p| p.nodes.into_iter().map(|n| PrInfo {
            number: n.number,
            title: n.title,
            state: n.state.to_lowercase(),
        }).collect())
        .unwrap_or_default();

    let tree: Vec<TreeEntry> = repo.object
        .and_then(|o| o.entries)
        .map(|entries| entries.into_iter().map(|e| TreeEntry {
            path: e.name,
            entry_type: e.entry_type.to_lowercase(),
        }).collect())
        .unwrap_or_default();

    let readme_excerpt = repo.readme
        .and_then(|r| r.text)
        .map(|text| {
            if text.len() > 500 {
                format!("{}...", &text[..500])
            } else {
                text
            }
        });

    let commits: Vec<CommitInfo> = repo.default_branch_ref
        .and_then(|b| b.target)
        .and_then(|t| t.history)
        .map(|h| h.nodes.into_iter().map(|c| CommitInfo {
            sha: c.oid[..7].to_string(),
            message: c.message_headline,
        }).collect())
        .unwrap_or_default();

    // Note: GraphQL doesn't have a direct contributors query like REST
    // We'd need additional queries or use REST for this
    let contributors = Vec::new();

    Ok(ExploreResponse {
        repo: Some(repo_info),
        issues,
        pull_requests,
        tree,
        readme_excerpt,
        commits,
        contributors,
        error: None,
    })
}
