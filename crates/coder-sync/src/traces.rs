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
}
