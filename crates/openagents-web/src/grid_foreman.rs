//! The Grid Foreman's public status (#12444): `GET /v1/foreman/status`.
//!
//! The Grid homepage draws a giant Foreman beside the shared terminal with
//! a status line under its tag (`crates/everglade-web/src/foreman.rs`). The
//! line comes from here: how many issues are open on the repository and
//! how many of those someone is working, read from GitHub's issue search
//! and kept for [`grid_foreman::CACHE_SECONDS`]. The repository is public,
//! so the counts are public facts; the answer holds counts, a line, and a
//! time, never a token.
//!
//! The read uses `OPENAGENTS_FOREMAN_GITHUB_TOKEN` when the deployment
//! sets one (a fine-grained token that can only read the repository's
//! issues), and GitHub's anonymous limit otherwise. When GitHub can't be
//! read, the last good status answers; with none yet, the route says so.
//!
//! Plan: `docs/foreman/grid-foreman-plan.md`.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};

use crate::App;
use crate::coder_sync::{answer, refused};

/// The environment variable that holds the read-only GitHub token.
const TOKEN_ENV: &str = "OPENAGENTS_FOREMAN_GITHUB_TOKEN";
/// GitHub's API.
const GITHUB: &str = "https://api.github.com";
/// How long one GitHub read may take.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn routes() -> Router<App> {
    Router::new().route(grid_foreman::STATUS_PATH, get(status))
}

/// Whether the site answers `path` itself.
pub(crate) fn owns(path: &str) -> bool {
    path == "/v1/foreman" || path.starts_with("/v1/foreman/")
}

/// The counts the status reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Counts {
    pub open: u64,
    pub in_progress: u64,
}

/// The status document for `counts` read at `updated_at` (Unix seconds).
pub(crate) fn document(counts: Counts, updated_at: u64) -> Value {
    json!({
        "schema": grid_foreman::STATUS_SCHEMA,
        "open": counts.open,
        "in_progress": counts.in_progress.min(counts.open),
        "line": grid_foreman::status_line(counts.open, counts.in_progress),
        "updated_at": updated_at,
    })
}

/// The search query for `repository`'s open issues, all of them or only
/// those nobody is assigned to. A claim assigns the claimer, so the
/// difference is the work in progress.
fn query(repository: &str, unassigned: bool) -> String {
    let mut q = format!("repo:{repository} is:issue is:open");
    if unassigned {
        q.push_str(" no:assignee");
    }
    q
}

async fn total(
    client: &reqwest::Client,
    base: &str,
    token: Option<&str>,
    q: &str,
) -> Result<u64, String> {
    let mut request = client
        .get(format!("{base}/search/issues"))
        .query(&[("q", q), ("per_page", "1")])
        .header(header::ACCEPT, "application/vnd.github+json")
        .header(header::USER_AGENT, "openagents-grid-foreman")
        .timeout(READ_TIMEOUT);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|_| "unreachable".to_owned())?;
    if !response.status().is_success() {
        return Err(format!("http {}", response.status().as_u16()));
    }
    let body: Value = response.json().await.map_err(|_| "unreadable".to_owned())?;
    body["total_count"]
        .as_u64()
        .ok_or_else(|| "no total".to_owned())
}

/// Reads the counts for `repository` from the GitHub API at `base`.
pub(crate) async fn read(
    client: &reqwest::Client,
    base: &str,
    token: Option<&str>,
    repository: &str,
) -> Result<Counts, String> {
    let open = total(client, base, token, &query(repository, false)).await?;
    let unassigned = total(client, base, token, &query(repository, true)).await?;
    Ok(Counts {
        open,
        in_progress: open.saturating_sub(unassigned),
    })
}

struct Cached {
    at: Instant,
    counts: Counts,
    updated_at: u64,
}

static CACHE: Mutex<Option<Cached>> = Mutex::new(None);
/// One read at a time; others answer from the cache.
static READING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn cached(fresh_only: bool) -> Option<Value> {
    let cache = CACHE.lock().ok()?;
    let cached = cache.as_ref()?;
    if fresh_only && cached.at.elapsed() >= Duration::from_secs(grid_foreman::CACHE_SECONDS) {
        return None;
    }
    Some(document(cached.counts, cached.updated_at))
}

fn ok(body: Value) -> Response {
    let mut response = answer(StatusCode::OK, body);
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("public, max-age=60"),
    );
    response
}

async fn status() -> Response {
    if let Some(body) = cached(true) {
        return ok(body);
    }
    let _reading = READING.lock().await;
    if let Some(body) = cached(true) {
        return ok(body);
    }
    let token = std::env::var(TOKEN_ENV)
        .ok()
        .filter(|t| !t.trim().is_empty());
    let client = reqwest::Client::new();
    match read(
        &client,
        GITHUB,
        token.as_deref(),
        crate::github_refs::DEFAULT_REPOSITORY,
    )
    .await
    {
        Ok(counts) => {
            let updated_at = crate::chat_store::now_unix();
            if let Ok(mut cache) = CACHE.lock() {
                *cache = Some(Cached {
                    at: Instant::now(),
                    counts,
                    updated_at,
                });
            }
            ok(document(counts, updated_at))
        }
        Err(error) => {
            eprintln!("grid foreman: GitHub status read failed: {error}");
            match cached(false) {
                Some(body) => ok(body),
                None => refused(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "foreman_status_unavailable",
                    "The Foreman's status isn't available right now. Try again soon.",
                ),
            }
        }
    }
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_document_holds_counts_a_line_and_a_time() {
        let body = document(
            Counts {
                open: 14,
                in_progress: 3,
            },
            1_800_000_000,
        );
        assert_eq!(body["schema"], grid_foreman::STATUS_SCHEMA);
        assert_eq!(body["open"], 14);
        assert_eq!(body["in_progress"], 3);
        assert_eq!(body["line"], "14 OPEN ISSUES, 3 IN PROGRESS");
        assert_eq!(body["updated_at"], 1_800_000_000u64);
        assert_eq!(body.as_object().map(serde_json::Map::len), Some(5));
    }

    #[test]
    fn it_owns_only_its_routes() {
        assert!(owns(grid_foreman::STATUS_PATH));
        assert!(owns("/v1/foreman/commands"));
        assert!(!owns("/v1/foremanx"));
        assert!(!owns("/agent/foreman"));
    }

    #[tokio::test]
    async fn it_counts_open_and_assigned_issues_and_sends_the_token() {
        use axum::extract::Query;
        use std::collections::HashMap;
        use std::sync::Arc;
        let seen: Arc<Mutex<Vec<(String, Option<String>)>>> = Arc::default();
        let into = seen.clone();
        let app = axum::Router::new().route(
            "/search/issues",
            get(
                move |Query(q): Query<HashMap<String, String>>, headers: axum::http::HeaderMap| {
                    let into = into.clone();
                    async move {
                        let query = q.get("q").cloned().unwrap_or_default();
                        let auth = headers
                            .get(header::AUTHORIZATION)
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned);
                        into.lock().unwrap().push((query.clone(), auth));
                        let total = if query.contains("no:assignee") {
                            11
                        } else {
                            14
                        };
                        axum::Json(json!({ "total_count": total, "items": [] }))
                    }
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let counts = read(
            &reqwest::Client::new(),
            &base,
            Some("test-token"),
            "OpenAgentsInc/openagents",
        )
        .await
        .unwrap();
        assert_eq!(
            counts,
            Counts {
                open: 14,
                in_progress: 3
            }
        );
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, "repo:OpenAgentsInc/openagents is:issue is:open");
        assert!(seen[1].0.ends_with("no:assignee"));
        assert!(
            seen.iter()
                .all(|(_, auth)| auth.as_deref() == Some("Bearer test-token"))
        );
    }

    #[tokio::test]
    async fn a_github_refusal_is_an_error() {
        let app = axum::Router::new().route(
            "/search/issues",
            get(|| async { (StatusCode::FORBIDDEN, "no") }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let error = read(&reqwest::Client::new(), &base, None, "a/b")
            .await
            .unwrap_err();
        assert_eq!(error, "http 403");
    }

    #[test]
    fn the_refusal_is_plain() {
        let text = "The Foreman's status isn't available right now. Try again soon.";
        assert!(oa_copy::violations(text, &[]).is_empty());
    }
}
