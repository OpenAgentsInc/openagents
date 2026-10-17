//! Robot Lab's typed tools and private, owner/project-bound player routes.
use crate::App;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

pub(crate) const CARD: &str = "robot_lab";
pub(crate) const START_JS: &str = "/chat/robot/build/start.js";
pub(crate) fn configured() -> bool {
    lab_url().is_some()
        && std::env::var("OPENAGENTS_ROBOT_LAB_OWNER").is_ok_and(|owner| !owner.is_empty())
}
fn lab_url() -> Option<String> {
    let value = std::env::var("OPENAGENTS_ROBOT_LAB").ok()?;
    loopback(&value)
}
fn owner_matches(configured: Option<&str>, caller: Option<&str>) -> bool {
    configured.is_some_and(|owner| !owner.is_empty() && caller == Some(owner))
}
fn project_matches(project: Option<&str>, lab: Option<&str>) -> bool {
    project.is_some() && project == lab
}
fn loopback(value: &str) -> Option<String> {
    let url = reqwest::Url::parse(value.trim()).ok()?;
    (url.scheme() == "http"
        && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"))
        && url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.port().is_some_and(|p| p > 0))
    .then(|| url.to_string().trim_end_matches('/').into())
}
fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|_| "Robot Lab couldn't connect".into())
}
async fn get_json(url: &str) -> Result<Value, String> {
    let reply = client()?
        .get(url)
        .send()
        .await
        .map_err(|_| "Robot Lab isn't answering")?;
    if reply.content_length().is_some_and(|n| n > 18 << 20) {
        return Err("This robot reply is too large".into());
    }
    let bytes = reply
        .bytes()
        .await
        .map_err(|_| "Robot Lab couldn't answer")?;
    if bytes.len() > 18 << 20 {
        return Err("This robot reply is too large".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "This robot reply couldn't be read".into())
}
pub(crate) async fn manifest_here(owner: &str, project: Option<&str>) -> Option<crate::labs::Lab> {
    if owner.is_empty() {
        return None;
    }
    if !owner_matches(
        std::env::var("OPENAGENTS_ROBOT_LAB_OWNER").ok().as_deref(),
        Some(owner),
    ) {
        return None;
    }
    let url = lab_url()?;
    let docs = get_json(&format!("{url}/api/docs")).await.ok()?;
    if !project_matches(project, docs["project"].as_str()) {
        return None;
    }
    let tools = get_json(&format!("{url}/api/tools")).await.ok()?;
    Some(crate::labs::Lab {
        kind: crate::labs::Kind::Robot,
        url,
        tools: tools["tools"].as_array()?.clone(),
        documents: docs["documents"].as_array()?.clone(),
        hosted: None,
        platform: None,
    })
}
pub(crate) fn guidance(documents: &[Value]) -> String {
    let mut text = String::from(
        "\nRobot Lab uses robot_* tools to import SO-101, inspect and pose it, edit its table scene, open saved attempts, compare exact versions, and save notes. Use these typed tools when asked; use general_chat for other topics. Speak as I, briefly. The card carries the result. Read a scene before editing, use its exact version hash as base, and report only what the tools returned. A demonstration does not prove a physical grasp. The supported SO-101 source is so101 at revision a758567c3978dfeefe282ede0500085a48fe8f78.\n",
    );
    for doc in documents {
        text.push_str(&format!(
            "{}: {} ({})\n",
            doc["id"].as_str().unwrap_or(""),
            doc["title"].as_str().unwrap_or(""),
            doc["reference"]["version"].as_str().unwrap_or("")
        ));
    }
    text
}
pub(crate) async fn run_tool(lab: &str, name: &str, arguments: &Value) -> Result<Value, String> {
    if !name.starts_with("robot_") || !name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_') {
        return Err("Choose a declared Robot Lab tool".into());
    }
    let reply = client()?
        .post(format!("{lab}/api/tool/{name}"))
        .json(arguments)
        .send()
        .await
        .map_err(|_| "Robot Lab isn't answering")?;
    if reply.content_length().is_some_and(|n| n > 18 << 20) {
        return Err("This robot reply is too large".into());
    }
    let bytes = reply
        .bytes()
        .await
        .map_err(|_| "Robot Lab couldn't answer")?;
    if bytes.len() > 18 << 20 {
        return Err("This robot reply is too large".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "This robot reply couldn't be read")?;
    if let Some(error) = value["error"].as_str() {
        return Err(error.into());
    }
    Ok(value)
}
pub(crate) fn cards(meta: &openagents_chat::router::Meta) -> Vec<Value> {
    meta.cards
        .iter()
        .filter(|c| c["card"] == CARD)
        .cloned()
        .collect()
}

fn reference(card: &Value) -> Option<robot_lab::Reference> {
    let r: robot_lab::Reference = serde_json::from_value(card["reference"].clone()).ok()?;
    r.validate().ok()?;
    Some(r)
}
pub(crate) fn card_markup(chat: &str, card: &Value) -> Markup {
    let Some(subject) = reference(card) else {
        return html! {};
    };
    let key = format!("robot-{}", subject.version);
    let mut source = format!(
        "/chat/{chat}/robot/play?document={}&version={}&chat={chat}&key={key}",
        subject.document, subject.version
    );
    if let Ok(compare) = serde_json::from_value::<robot_lab::Reference>(card["comparison"].clone())
    {
        if compare.validate().is_ok() {
            source.push_str(&format!(
                "&compare_document={}&compare_version={}",
                compare.document, compare.version
            ));
        }
    }
    html! {div.oa-robot-card style="margin:12px 0;border:1px solid #555;border-radius:10px;overflow:hidden"{
        iframe src=(source) title={ (card["title"].as_str().unwrap_or("Robot Lab")) ", version " (card["version"].as_u64().unwrap_or(1)) }
            loading="lazy" data-robot-card=(key) data-robot-save=(format!("/chat/{chat}/robot/saved"))
            style="width:100%;height:660px;border:0;display:block" {}
        a href=(source) target="_blank" rel="noopener" style="display:block;padding:8px" {"Open Robot Lab"}
    }}
}
pub(crate) fn thread_entry(chat: &str, meta: Option<&openagents_chat::router::Meta>) -> Markup {
    meta.and_then(|m| m.cards.iter().rev().find(|c| c["card"] == CARD))
        .map_or_else(|| html! {}, |c| card_markup(chat, c))
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/robot/build/{file}", get(manager_build))
        .route("/chat/{id}/robot/play", get(play))
        .route("/chat/{id}/robot/build/{file}", get(build))
        .route("/chat/{id}/robot/api/view", get(view))
        .route("/chat/{id}/robot/api/poster", get(poster))
        .route("/chat/{id}/robot/api/asset/{hash}", get(asset))
        .route("/chat/{id}/robot/api/tool/{name}", post(tool))
        .route("/chat/{id}/robot/saved", post(saved))
}
fn unavailable() -> Response {
    (
        StatusCode::FORBIDDEN,
        "Open Robot Lab from its owner's project",
    )
        .into_response()
}
async fn access(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    write: bool,
) -> Result<(String, crate::chat_store::Loaded), Response> {
    let who = crate::chat_owner::who(app, headers).await;
    let owner = if write { who.writer() } else { who.reader() }.ok_or_else(unavailable)?;
    if !owner_matches(
        std::env::var("OPENAGENTS_ROBOT_LAB_OWNER").ok().as_deref(),
        Some(owner),
    ) {
        return Err(unavailable());
    }
    let loaded = crate::pages::chat::load(app, headers, id).await?;
    if loaded.conversation.owner != owner {
        return Err(unavailable());
    }
    let lab = lab_url().ok_or_else(unavailable)?;
    let docs = get_json(&format!("{lab}/api/docs"))
        .await
        .map_err(|_| unavailable())?;
    if !project_matches(
        loaded.conversation.project.as_deref(),
        docs["project"].as_str(),
    ) {
        return Err(unavailable());
    }
    Ok((lab, loaded))
}
async fn proxy(lab: &str, path: &str, kind: &str, limit: usize) -> Response {
    let result = async {
        let reply = client()?
            .get(format!("{lab}{path}"))
            .send()
            .await
            .map_err(|_| "Robot Lab isn't answering")?;
        if reply.content_length().is_some_and(|n| n > limit as u64) {
            return Err("This robot file is too large".into());
        }
        let status =
            StatusCode::from_u16(reply.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        let bytes = reply
            .bytes()
            .await
            .map_err(|_| "The robot file couldn't be opened")?;
        if bytes.len() > limit {
            return Err("This robot file is too large".into());
        }
        Ok::<_, String>((status, bytes))
    }
    .await;
    match result {
        Ok((status, bytes)) => (
            status,
            [
                (header::CONTENT_TYPE, HeaderValue::from_str(kind).unwrap()),
                (
                    header::X_CONTENT_TYPE_OPTIONS,
                    HeaderValue::from_static("nosniff"),
                ),
                (
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("private, no-cache"),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (
            StatusCode::BAD_GATEWAY,
            "Robot Lab isn't answering. Try again",
        )
            .into_response(),
    }
}
async fn play(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let (lab, _) = match access(&app, &headers, &id, false).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut response = proxy(&lab, "/play", "text/html; charset=utf-8", 65536).await;
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY,HeaderValue::from_static("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' blob:; connect-src 'self'; frame-ancestors 'self'; base-uri 'none'; form-action 'self'"));
    response
        .headers_mut()
        .insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
    response
}
async fn manager_build(
    State(app): State<App>,
    headers: HeaderMap,
    Path(file): Path<String>,
) -> Response {
    let who = crate::chat_owner::who(&app, &headers).await;
    let Some(owner) = std::env::var("OPENAGENTS_ROBOT_LAB_OWNER")
        .ok()
        .filter(|s| !s.is_empty())
    else {
        return unavailable();
    };
    if who.reader() != Some(owner.as_str()) {
        return unavailable();
    }
    build_response(&file).await
}
async fn build(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
) -> Response {
    if let Err(response) = access(&app, &headers, &id, false).await {
        return response;
    }
    build_response(&file).await
}
async fn build_response(file: &str) -> Response {
    let kind = match file {
        "robot_web.js" | "start.js" => "text/javascript",
        "robot_web_bg.wasm" => "application/wasm",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let Some(lab) = lab_url() else {
        return unavailable();
    };
    proxy(&lab, &format!("/build/{file}"), kind, 128 << 20).await
}
#[derive(Deserialize)]
struct Subject {
    document: String,
    version: String,
}
impl Subject {
    fn query(&self) -> Option<String> {
        robot_lab::Reference {
            document: self.document.clone(),
            version: self.version.clone(),
        }
        .validate()
        .ok()?;
        Some(format!(
            "?document={}&version={}",
            self.document, self.version
        ))
    }
}
async fn view(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(subject): Query<Subject>,
) -> Response {
    let (lab, _) = match access(&app, &headers, &id, false).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(query) = subject.query() else {
        return unavailable();
    };
    proxy(
        &lab,
        &format!("/api/view{query}"),
        "application/json",
        18 << 20,
    )
    .await
}
async fn poster(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(subject): Query<Subject>,
) -> Response {
    let (lab, _) = match access(&app, &headers, &id, false).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(query) = subject.query() else {
        return unavailable();
    };
    proxy(&lab, &format!("/api/poster{query}"), "image/png", 8 << 20).await
}
async fn asset(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, hash)): Path<(String, String)>,
) -> Response {
    let (lab, _) = match access(&app, &headers, &id, false).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return unavailable();
    }
    proxy(
        &lab,
        &format!("/api/asset/{hash}"),
        "application/octet-stream",
        64 << 20,
    )
    .await
}
async fn tool(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
    Json(mut arguments): Json<Value>,
) -> Response {
    let read_only = matches!(
        name.as_str(),
        "robot_open" | "robot_versions" | "robot_notes" | "robot_inspect" | "robot_compare"
    );
    let (lab, _) = match access(&app, &headers, &id, !read_only).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if name == "robot_select" || name == "robot_restore" {
        arguments["chat"] = json!(id);
    }
    match run_tool(&lab, &name, &arguments).await {
        Ok(value) => Json(value).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(json!({"error":error}))).into_response(),
    }
}
async fn saved(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Response {
    let (lab, loaded) = match access(&app, &headers, &id, true).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Some(reference) = reference(&body["card"]) else {
        return unavailable();
    };
    let value = match run_tool(
        &lab,
        "robot_open",
        &json!({"document":reference.document,"version":reference.version}),
    )
    .await
    {
        Ok(v) => v,
        Err(_) => return unavailable(),
    };
    let mut next = loaded.conversation.clone();
    // A replayed callback cannot append the same version twice.
    if next
        .requests
        .iter()
        .filter_map(|r| r.reply.as_ref())
        .flat_map(|m| &m.cards)
        .any(|c| c["card"] == CARD && c["reference"] == value["card"]["reference"])
    {
        return Json(json!({"reload":format!("/chat/{id}")})).into_response();
    }
    if next.pending.is_some() {
        return (
            StatusCode::CONFLICT,
            "Wait for this answer before adding the new robot card",
        )
            .into_response();
    }
    let request_id = format!("robot-{}", reference.version);
    let mut meta = openagents_chat::router::Meta::default();
    meta.cards = vec![value["card"].clone()];
    meta.route = Some(CARD.into());
    next.messages.push(crate::chat_store::Message {
        role: crate::chat_store::Role::Assistant,
        text: "Joint pose saved".into(),
        request_id: Some(request_id.clone()),
    });
    next.requests.push(crate::chat_store::Request {
        turn_error: None,
        id: request_id,
        digest: reference.version.clone(),
        outcome: crate::chat_store::Outcome::Answered,
        selection: next.selection.clone(),
        cloud: None,
        reply: Some(meta),
        files: vec![],
    });
    next.revision += 1;
    match app.config.chat_store.compare_and_swap(&loaded, &next).await {
        Ok(_) => {
            let frame = (value["card"]["mode"] != "Inspecting").then_some(0);
            let _=run_tool(&lab,"robot_select",&json!({"chat":id,"document":reference.document,"version":reference.version,"frame":frame,"camera":"orbit"})).await;
            Json(json!({"reload":format!("/chat/{id}")})).into_response()
        }
        Err(_) => (
            StatusCode::CONFLICT,
            "The chat changed. Open the new version from Robot Lab",
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_lab_requires_its_named_owner_and_project() {
        assert!(!owner_matches(None, None));
        assert!(!owner_matches(Some(""), Some("")));
        assert!(!owner_matches(
            Some("account:owner"),
            Some("account:member")
        ));
        assert!(owner_matches(Some("account:owner"), Some("account:owner")));
        assert!(!project_matches(None, None));
        assert!(!project_matches(Some("prj_other"), Some("prj_lab")));
        assert!(project_matches(Some("prj_lab"), Some("prj_lab")));
    }
    #[test]
    fn card_keeps_exact_subjects_and_uses_only_authenticated_player_routes() {
        let card = json!({"card":CARD,"doc":"table","reference":{"document":"table","version":"a".repeat(64)},"version":2,"title":"SO-101 table","comparison":{"document":"table","version":"b".repeat(64)},"player":"https://untrusted.invalid/anything"});
        let markup = card_markup("chat-one", &card).into_string();
        assert!(markup.contains("/chat/chat-one/robot/play?document=table"));
        assert!(markup.contains(&format!("compare_version={}", "b".repeat(64))));
        assert!(!markup.contains("untrusted"));
        assert!(markup.contains("Open Robot Lab"));
        assert!(oa_copy::violations(&oa_copy::visible_text(&markup), &[]).is_empty());
    }
    #[test]
    fn robot_address_cannot_escape_loopback() {
        for value in [
            "https://127.0.0.1:8794",
            "http://127.0.0.1:8794@evil.invalid",
            "http://localhost:8794/path",
            "http://localhost:8794?key=x",
        ] {
            assert!(loopback(value).is_none());
        }
        assert_eq!(
            loopback("http://127.0.0.1:8794"),
            Some("http://127.0.0.1:8794".into())
        );
    }
}
