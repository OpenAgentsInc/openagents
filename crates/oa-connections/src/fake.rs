//! A fake Google for tests (feature `fake`): the OAuth token and revoke
//! endpoints, Drive's file listing, metadata, export, and media, and the
//! Sheets values API, over a small in-memory Drive. Every Drive and Sheets
//! call must carry the access token the token endpoint handed out.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Bytes;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Value, json};

use crate::google::Endpoints;
use crate::google::drive::{DOC, FOLDER, PDF, SHEET};

/// The code the fake's consent screen "returns".
pub const CODE: &str = "fake-code";
/// The refresh token it grants.
pub const REFRESH: &str = "1//fake-refresh-token";
/// The access token it grants; Drive calls must carry it.
pub const ACCESS: &str = "ya29.fake-access-token";
/// The Google account it signs in as.
pub const EMAIL: &str = "owner@example.com";

/// What a fake file holds.
#[derive(Clone, Debug)]
pub enum Content {
    Folder,
    /// A Google Doc's text.
    Doc(String),
    /// A Google Sheet's tabs.
    Sheet(Vec<(String, Vec<Vec<String>>)>),
    /// A PDF's bytes.
    Pdf(Vec<u8>),
    /// A plain file with its MIME type.
    Plain(String, String),
}

#[derive(Clone, Debug)]
pub struct FakeFile {
    pub id: String,
    pub name: String,
    pub parent: Option<String>,
    pub content: Content,
}

impl FakeFile {
    #[must_use]
    pub fn new(id: &str, name: &str, parent: Option<&str>, content: Content) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            parent: parent.map(str::to_owned),
            content,
        }
    }

    fn mime(&self) -> String {
        match &self.content {
            Content::Folder => FOLDER.into(),
            Content::Doc(_) => DOC.into(),
            Content::Sheet(_) => SHEET.into(),
            Content::Pdf(_) => PDF.into(),
            Content::Plain(mime, _) => mime.clone(),
        }
    }

    fn json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "mimeType": self.mime(),
            "webViewLink": format!("https://drive.example/{}", self.id),
            "modifiedTime": "2026-10-01T00:00:00Z",
        })
    }

    fn text(&self) -> String {
        match &self.content {
            Content::Doc(text) | Content::Plain(_, text) => text.clone(),
            _ => String::new(),
        }
    }
}

/// What the fake has seen and holds.
#[derive(Default)]
pub struct Inner {
    pub files: Vec<FakeFile>,
    /// Every request's method and path with query.
    pub requests: Vec<String>,
    /// Refresh tokens revoked through the revoke endpoint (or set to fail).
    pub revoked: Vec<String>,
    /// The scopes the token endpoint reports as granted.
    pub granted: Vec<String>,
}

/// A running fake Google.
#[derive(Clone)]
pub struct FakeGoogle {
    pub inner: Arc<Mutex<Inner>>,
    pub root: String,
}

impl FakeGoogle {
    /// Serve `files` on a local port, granting `granted` scopes.
    ///
    /// # Panics
    ///
    /// When it can't listen.
    pub async fn start(files: Vec<FakeFile>, granted: &[&str]) -> Self {
        let inner = Arc::new(Mutex::new(Inner {
            files,
            granted: granted.iter().map(|s| (*s).to_owned()).collect(),
            ..Inner::default()
        }));
        let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind");
        let root = format!("http://{}", listener.local_addr().expect("addr"));
        let app = Router::new().fallback(handle).with_state(inner.clone());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Self { inner, root }
    }

    #[must_use]
    pub fn endpoints(&self) -> Endpoints {
        Endpoints::at(&self.root)
    }

    /// Requests seen so far.
    ///
    /// # Panics
    ///
    /// On a poisoned lock.
    #[must_use]
    pub fn requests(&self) -> Vec<String> {
        self.inner.lock().unwrap().requests.clone()
    }
}

fn id_token() -> String {
    format!(
        "h.{}.s",
        URL_SAFE_NO_PAD.encode(json!({"email": EMAIL, "email_verified": true}).to_string())
    )
}

fn form(body: &[u8]) -> Vec<(String, String)> {
    url::form_urlencoded::parse(body).into_owned().collect()
}

fn get<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    pairs
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// The quoted literals in a Drive query, in order.
fn literals(q: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = q.chars();
    while let Some(c) = chars.next() {
        if c != '\'' {
            continue;
        }
        let mut literal = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => literal.extend(chars.next()),
                '\'' => break,
                other => literal.push(other),
            }
        }
        out.push(literal);
    }
    out
}

async fn handle(State(inner): State<Arc<Mutex<Inner>>>, request: Request) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let query: Vec<(String, String)> = request
        .uri()
        .query()
        .map(|q| form(q.as_bytes()))
        .unwrap_or_default();
    let auth = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body: Bytes = axum::body::to_bytes(request.into_body(), 1 << 20)
        .await
        .unwrap_or_default();
    let mut state = inner.lock().unwrap();
    state.requests.push(format!(
        "{method} {path}{}",
        if query.is_empty() {
            String::new()
        } else {
            format!(
                "?{}",
                url::form_urlencoded::Serializer::new(String::new())
                    .extend_pairs(&query)
                    .finish()
            )
        }
    ));
    if path == "/token" {
        let pairs = form(&body);
        let granted = state.granted.join(" ");
        return match get(&pairs, "grant_type") {
            Some("authorization_code") if get(&pairs, "code") == Some(CODE) => {
                if get(&pairs, "code_verifier").is_none_or(str::is_empty) {
                    return (
                        StatusCode::BAD_REQUEST,
                        axum::Json(json!({"error": "invalid_request"})),
                    )
                        .into_response();
                }
                axum::Json(json!({
                    "access_token": ACCESS,
                    "refresh_token": REFRESH,
                    "expires_in": 3599,
                    "scope": format!("openid {granted} https://www.googleapis.com/auth/userinfo.email"),
                    "id_token": id_token(),
                }))
                .into_response()
            }
            Some("refresh_token")
                if get(&pairs, "refresh_token")
                    .is_some_and(|t| !state.revoked.iter().any(|r| r == t)) =>
            {
                axum::Json(json!({"access_token": ACCESS, "expires_in": 3599, "scope": granted}))
                    .into_response()
            }
            _ => (
                StatusCode::BAD_REQUEST,
                axum::Json(json!({"error": "invalid_grant"})),
            )
                .into_response(),
        };
    }
    if path == "/revoke" {
        if let Some(token) = get(&form(&body), "token") {
            state.revoked.push(token.to_owned());
        }
        return StatusCode::OK.into_response();
    }
    if auth.as_deref() != Some(&format!("Bearer {ACCESS}")) {
        return (StatusCode::UNAUTHORIZED, "no token").into_response();
    }
    if path == "/drive/v3/files" {
        let q = get(&query, "q").unwrap_or_default();
        let words = literals(q);
        let parent = q
            .contains(" in parents")
            .then(|| words.last().cloned())
            .flatten();
        let needle = q
            .contains("contains")
            .then(|| words.first().cloned())
            .flatten();
        let files: Vec<Value> = state
            .files
            .iter()
            .filter(|f| parent.as_ref().is_none_or(|p| f.parent.as_ref() == Some(p)))
            .filter(|f| {
                needle.as_ref().is_none_or(|n| {
                    let n = n.to_lowercase();
                    f.name.to_lowercase().contains(&n) || f.text().to_lowercase().contains(&n)
                })
            })
            .map(FakeFile::json)
            .collect();
        return axum::Json(json!({"files": files})).into_response();
    }
    if let Some(rest) = path.strip_prefix("/drive/v3/files/") {
        let (id, export) = match rest.strip_suffix("/export") {
            Some(id) => (id, true),
            None => (rest, false),
        };
        let Some(file) = state.files.iter().find(|f| f.id == id).cloned() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if export {
            let text = match (&file.content, get(&query, "mimeType")) {
                (Content::Doc(text), Some("text/plain")) => text.clone(),
                (Content::Sheet(tabs), Some("text/csv")) => tabs
                    .first()
                    .map(|(_, rows)| rows.iter().map(|r| r.join(",") + "\n").collect())
                    .unwrap_or_default(),
                _ => return StatusCode::BAD_REQUEST.into_response(),
            };
            return text.into_response();
        }
        if get(&query, "alt") == Some("media") {
            return match &file.content {
                Content::Pdf(bytes) => bytes.clone().into_response(),
                Content::Plain(_, text) => text.clone().into_response(),
                _ => StatusCode::FORBIDDEN.into_response(),
            };
        }
        return axum::Json(file.json()).into_response();
    }
    if let Some(rest) = path.strip_prefix("/v4/spreadsheets/") {
        if !state
            .granted
            .iter()
            .any(|s| s == crate::google::SHEETS_READONLY)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        let (id, values) = match rest.strip_suffix("/values:batchGet") {
            Some(id) => (id, true),
            None => (rest, false),
        };
        let Some(Content::Sheet(tabs)) = state
            .files
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.content.clone())
        else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if !values {
            let sheets: Vec<Value> = tabs
                .iter()
                .map(|(title, _)| json!({"properties": {"title": title}}))
                .collect();
            return axum::Json(json!({"sheets": sheets})).into_response();
        }
        let ranges: Vec<Value> = query
            .iter()
            .filter(|(k, _)| k == "ranges")
            .filter_map(|(_, range)| {
                let title = range
                    .split('!')
                    .next()?
                    .trim_matches('\'')
                    .replace("''", "'");
                let rows = &tabs.iter().find(|(t, _)| *t == title)?.1;
                Some(json!({"range": range, "values": rows}))
            })
            .collect();
        return axum::Json(json!({"valueRanges": ranges})).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}
