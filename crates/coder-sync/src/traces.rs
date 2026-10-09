//! Uploading agent traces to the signed-in account (#11109): the client
//! side of the website's `/api/traces` (`openagents-web` `traces`).
//!
//! A trace is a whole ATIF document, so before it leaves the computer every
//! string in it is redacted ([`prepare`]): credential shapes, the exact
//! values of this computer's credential variables, home folder names, and
//! email addresses. [`Prepared::left_out`] says what was taken out, so the
//! person sees it. The website screens again and refuses anything that
//! still looks like a credential.

use std::time::Duration;

use openagents_login::Saved;
use secret_screen::{Counts, Screen};
use serde::Deserialize;
use serde_json::Value;

/// The largest trace the website takes.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

/// A trace ready to send.
#[derive(Debug)]
pub struct Prepared {
    pub document: Value,
    pub bytes: Vec<u8>,
    /// What redaction took out, by rule.
    pub left_out: Counts,
}

/// Redact `document` for upload with `screen`.
///
/// # Errors
/// When it isn't a trace or is too large to send.
pub fn prepare(mut document: Value, screen: &Screen) -> Result<Prepared, String> {
    if !document.get("steps").is_some_and(Value::is_array) {
        return Err("That isn't an ATIF trace: it has no steps.".into());
    }
    let mut left_out = screen.redact_document(&mut document);
    // What the website would still refuse goes out whole, so a long
    // session is never refused for one string (#11154).
    let scrubbed = secret_screen::scrub_document(&mut document);
    if scrubbed > 0 {
        *left_out.entry("credential".to_owned()).or_default() += scrubbed;
    }
    let bytes =
        serde_json::to_vec(&document).map_err(|_| "Couldn't read the trace.".to_string())?;
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "This trace is {:.1} MB; the most you can upload is 8 MB.",
            bytes.len() as f64 / (1024.0 * 1024.0)
        ));
    }
    Ok(Prepared {
        document,
        bytes,
        left_out,
    })
}

/// What redaction took out, in plain words, or `None` when nothing was.
#[must_use]
pub fn left_out_text(counts: &Counts) -> Option<String> {
    let mut keys = 0;
    let mut folders = 0;
    let mut emails = 0;
    for (rule, count) in counts {
        match rule.as_str() {
            "home-directory" => folders += count,
            "email" => emails += count,
            _ => keys += count,
        }
    }
    let plural = |n: u32, one: &str, many: &str| {
        if n == 1 {
            format!("1 {one}")
        } else {
            format!("{n} {many}")
        }
    };
    let mut parts = Vec::new();
    if keys > 0 {
        parts.push(plural(keys, "password or key", "passwords or keys"));
    }
    if folders > 0 {
        parts.push(plural(folders, "home folder name", "home folder names"));
    }
    if emails > 0 {
        parts.push(plural(emails, "email address", "email addresses"));
    }
    (!parts.is_empty()).then(|| format!("Left out before upload: {}.", parts.join(", ")))
}

/// A saved trace, as the website lists it.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Trace {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub steps: usize,
    #[serde(default)]
    pub uploaded_unix: u64,
    #[serde(default)]
    pub shared: bool,
    pub url: String,
    #[serde(default)]
    pub share_url: Option<String>,
}

/// What an upload did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uploaded {
    pub trace: Trace,
    /// The same trace was already on the account.
    pub existing: bool,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|_| "Couldn't start the upload.".to_string())
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Couldn't start the upload.".to_string())
}

/// The website's words for a refusal.
fn refusal(status: u16, body: &Value) -> String {
    if status == 401 {
        return "Your sign-in has ended. Run coder login, then try again.".into();
    }
    body["error"]["message"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if status >= 500 {
                "openagents.com couldn't save it right now. Try again in a minute.".into()
            } else {
                format!("openagents.com refused the trace ({status}).")
            }
        })
}

async fn send(
    saved: &Saved,
    method: reqwest::Method,
    path: &str,
    body: Option<Vec<u8>>,
) -> Result<Value, String> {
    let http = client()?;
    let mut request = http
        .request(method, format!("{}{path}", saved.origin))
        .bearer_auth(saved.token());
    if let Some(body) = body {
        request = request
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);
    }
    let response = request
        .send()
        .await
        .map_err(|_| format!("Couldn't reach {}. Check your connection.", saved.origin))?;
    let status = response.status().as_u16();
    let body: Value = response.json().await.unwrap_or(Value::Null);
    if (200..300).contains(&status) {
        Ok(body)
    } else {
        Err(refusal(status, &body))
    }
}

fn trace_of(value: &Value) -> Result<Trace, String> {
    serde_json::from_value(value.clone())
        .map_err(|_| "openagents.com answered in a way Coder doesn't understand.".to_string())
}

/// Upload a prepared trace, shared at once when `share`.
///
/// # Errors
/// The website's words, or why it couldn't be reached.
pub fn upload(saved: &Saved, prepared: &Prepared, share: bool) -> Result<Uploaded, String> {
    let path = if share {
        "/api/traces?share=true"
    } else {
        "/api/traces"
    };
    let body = runtime()?.block_on(send(
        saved,
        reqwest::Method::POST,
        path,
        Some(prepared.bytes.clone()),
    ))?;
    Ok(Uploaded {
        trace: trace_of(&body["trace"])?,
        existing: body["existing"].as_bool().unwrap_or(false),
    })
}

/// A tree ready to send: each node's parent (an index into the list, the
/// main conversation first) and its prepared trajectory.
#[derive(Debug)]
pub struct PreparedTree {
    pub nodes: Vec<(Option<usize>, Prepared)>,
    /// What redaction took out across the tree, by rule.
    pub left_out: Counts,
}

/// Redact every node of `nodes` with `screen`, shortening any that is too
/// large to send. `left_out` is what an earlier pass (the converter's)
/// already took out, added to the total.
///
/// # Errors
/// When a node isn't a trace, or there are none.
pub fn prepare_tree(
    nodes: Vec<crate::claude_session::Node>,
    screen: &Screen,
    mut left_out: Counts,
) -> Result<PreparedTree, String> {
    let mut prepared = Vec::with_capacity(nodes.len());
    for mut node in nodes {
        crate::claude_session::fit(&mut node.document, crate::claude_session::NODE_BYTES);
        let one = prepare(node.document, screen)?;
        for (rule, count) in &one.left_out {
            *left_out.entry(rule.clone()).or_default() += count;
        }
        prepared.push((node.parent, one));
    }
    if prepared.is_empty() {
        return Err("There's nothing to upload.".into());
    }
    Ok(PreparedTree {
        nodes: prepared,
        left_out,
    })
}

/// What a tree upload did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeUploaded {
    /// The main conversation's trace; its page shows the whole tree.
    pub trace: Trace,
    pub existing: bool,
    /// Agents saved under it (new or already there).
    pub agents: usize,
    /// Agents the website refused, with its words.
    pub failed: Vec<String>,
}

/// Upload a tree: the main conversation as a trace, then every agent under
/// its parent. Sending the same tree again saves nothing twice, so an
/// interrupted upload can simply be run again. `progress` hears (done,
/// total) after each node.
///
/// # Errors
/// When the main conversation can't be saved. An agent that can't be is
/// counted in [`TreeUploaded::failed`], and its own agents go under the
/// nearest saved one.
pub fn upload_tree(
    saved: &Saved,
    tree: &PreparedTree,
    share: bool,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<TreeUploaded, String> {
    let total = tree.nodes.len();
    runtime()?.block_on(async {
        let (_, root) = &tree.nodes[0];
        let path = if share {
            "/api/traces?share=true"
        } else {
            "/api/traces"
        };
        let body = send(saved, reqwest::Method::POST, path, Some(root.bytes.clone())).await?;
        let trace = trace_of(&body["trace"])?;
        let existing = body["existing"].as_bool().unwrap_or(false);
        progress(1, total);
        // Each node's id on the website; None for the main conversation
        // or a node that wasn't saved.
        let mut ids: Vec<Option<String>> = vec![None; total];
        let mut agents = 0;
        let mut failed = Vec::new();
        for index in 1..total {
            let (parent, node) = &tree.nodes[index];
            let mut at = *parent;
            let parent_id = loop {
                match at {
                    Some(0) | None => break None,
                    Some(i) => match &ids[i] {
                        Some(id) => break Some(id.clone()),
                        None => at = tree.nodes[i].0,
                    },
                }
            };
            let path = match &parent_id {
                Some(parent) => format!("/api/traces/{}/agents?parent={parent}", trace.id),
                None => format!("/api/traces/{}/agents", trace.id),
            };
            match send(
                saved,
                reqwest::Method::POST,
                &path,
                Some(node.bytes.clone()),
            )
            .await
            {
                Ok(body) => {
                    ids[index] = body["agent"]["id"].as_str().map(str::to_owned);
                    agents += 1;
                }
                Err(error) => {
                    let title = node.document["extra"]["title"]
                        .as_str()
                        .unwrap_or("An agent");
                    failed.push(format!("{title}: {error}"));
                }
            }
            progress(index + 1, total);
        }
        Ok(TreeUploaded {
            trace,
            existing,
            agents,
            failed,
        })
    })
}

/// The account's traces, newest first.
///
/// # Errors
/// The website's words, or why it couldn't be reached.
pub fn list(saved: &Saved) -> Result<Vec<Trace>, String> {
    let body = runtime()?.block_on(send(saved, reqwest::Method::GET, "/api/traces", None))?;
    body["traces"]
        .as_array()
        .into_iter()
        .flatten()
        .map(trace_of)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::body::Bytes;
    use axum::http::{HeaderMap, StatusCode};
    use axum::routing::post;
    use serde_json::json;

    use super::*;

    const TOKEN: &str = "sess_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn saved(origin: &str) -> Saved {
        serde_json::from_value(json!({
            "origin": origin, "account": "acct_1", "label": "Octo",
            "expires_at": u64::MAX, "token": TOKEN,
        }))
        .unwrap()
    }

    #[test]
    fn a_trace_is_redacted_and_says_what_was_left_out() {
        let github = format!("ghp_{}", "Z9".repeat(18));
        let document = json!({
            "schema_version": "ATIF-v1.8",
            "steps": [{"source": "user", "message": format!("{github} in /Users/octo/x")}],
        });
        let prepared = prepare(document, &Screen::shapes()).unwrap();
        let text = String::from_utf8(prepared.bytes.clone()).unwrap();
        assert!(!text.contains(&github) && !text.contains("octo"), "{text}");
        assert_eq!(
            left_out_text(&prepared.left_out).as_deref(),
            Some("Left out before upload: 1 password or key, 1 home folder name.")
        );
        assert_eq!(left_out_text(&Counts::new()), None);
        assert!(prepare(json!({"x": 1}), &Screen::shapes()).is_err());
        let huge = json!({"steps": [{"message": "x".repeat(MAX_BYTES)}]});
        assert!(
            prepare(huge, &Screen::shapes())
                .unwrap_err()
                .contains("8 MB")
        );
    }

    #[test]
    fn upload_and_list_speak_to_the_website() {
        let seen: Arc<Mutex<Vec<(String, String, Value)>>> = Arc::default();
        let kept = seen.clone();
        let router = Router::new().route(
            "/api/traces",
            post(
                move |headers: HeaderMap,
                      axum::extract::RawQuery(query): axum::extract::RawQuery,
                      body: Bytes| {
                    let kept = kept.clone();
                    async move {
                        let auth = headers["authorization"].to_str().unwrap().to_owned();
                        let document: Value = serde_json::from_slice(&body).unwrap();
                        kept.lock()
                            .unwrap()
                            .push((auth, query.unwrap_or_default(), document));
                        (
                            StatusCode::CREATED,
                            axum::Json(json!({"trace": {
                                "id": "t1", "title": "Fix", "steps": 1, "uploaded_unix": 5,
                                "shared": true, "url": "http://x/settings/traces/t1",
                                "share_url": "http://x/trace/t1"
                            }, "existing": false})),
                        )
                    }
                },
            )
            .get(|| async {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(json!({"error": {"code": "signed_out", "message": "x"}})),
                )
            }),
        );
        let (send, address) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    send.send(format!("http://{}", listener.local_addr().unwrap()))
                        .unwrap();
                    axum::serve(listener, router).await.unwrap();
                });
        });
        let account = saved(&address.recv().unwrap());
        let prepared = prepare(
            json!({"schema_version": "ATIF-v1.8", "steps": [{"source": "user", "message": "Fix"}]}),
            &Screen::shapes(),
        )
        .unwrap();
        let uploaded = upload(&account, &prepared, true).unwrap();
        assert_eq!(
            uploaded.trace.share_url.as_deref(),
            Some("http://x/trace/t1")
        );
        assert!(!uploaded.existing);
        let (auth, query, document) = seen.lock().unwrap()[0].clone();
        assert_eq!(auth, format!("Bearer {TOKEN}"));
        assert_eq!(query, "share=true");
        assert_eq!(document["steps"][0]["message"], "Fix");
        assert!(list(&account).unwrap_err().contains("coder login"));
    }

    /// A Claude Code session with three agents (one with its own agent)
    /// goes up as a trace and four agents, each under its parent, and an
    /// agent the website refuses doesn't stop the rest.
    #[test]
    fn a_session_tree_goes_up_parent_first() {
        let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
        let kept = seen.clone();
        let router = Router::new().fallback(move |uri: axum::http::Uri, body: Bytes| {
            let kept = kept.clone();
            async move {
                let document: Value = serde_json::from_slice(&body).unwrap();
                let path = uri.to_string();
                let mut kept = kept.lock().unwrap();
                kept.push((path.clone(), document.clone()));
                let n = kept.len();
                if path == "/api/traces" {
                    return (
                        StatusCode::CREATED,
                        axum::Json(json!({"trace": {
                                "id": "root", "title": "Fan out demo", "steps": 6,
                                "url": "http://x/settings/traces/root"}, "existing": false})),
                    );
                }
                if document["extra"]["title"] == "Agent b2" {
                    return (
                        StatusCode::UNPROCESSABLE_ENTITY,
                        axum::Json(json!({"error": {"code": "secret", "message": "No."}})),
                    );
                }
                (
                    StatusCode::CREATED,
                    axum::Json(json!({"agent": {"id": format!("agent{n}")}, "existing": false})),
                )
            }
        });
        let (send, address) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    send.send(format!("http://{}", listener.local_addr().unwrap()))
                        .unwrap();
                    axum::serve(listener, router).await.unwrap();
                });
        });
        let account = saved(&address.recv().unwrap());
        let temp = tempfile::tempdir().unwrap();
        let main = crate::claude_session::tests::fixture(temp.path());
        let (nodes, counts) = crate::claude_session::convert(&main, &Screen::shapes()).unwrap();
        let tree = prepare_tree(nodes, &Screen::shapes(), counts).unwrap();
        assert!(
            left_out_text(&tree.left_out)
                .unwrap()
                .contains("1 password or key")
        );
        let mut heard = Vec::new();
        let uploaded = upload_tree(&account, &tree, false, &mut |done, total| {
            heard.push((done, total));
        })
        .unwrap();
        assert_eq!(heard.last(), Some(&(5, 5)));
        assert_eq!(uploaded.trace.id, "root");
        assert_eq!(uploaded.agents, 3);
        assert_eq!(uploaded.failed, ["Agent b2: No."]);
        let seen = seen.lock().unwrap();
        let paths: Vec<&str> = seen.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "/api/traces",
                "/api/traces/root/agents",
                "/api/traces/root/agents",
                "/api/traces/root/agents",
                // d4 under a1, which the website called agent2.
                "/api/traces/root/agents?parent=agent2",
            ]
        );
        let all = serde_json::to_string(&*seen).unwrap();
        assert!(!all.contains("ghp_") && !all.contains("/Users/octo"));
    }
}
