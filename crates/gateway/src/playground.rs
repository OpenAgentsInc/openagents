//! The playground — a bounded, session-signed surface for trying real
//! decision workflows, and a bounded chat demo that invokes the same
//! documented tool.
//!
//! A browser signs in with the same `sess_` session the dashboard uses;
//! every live run hands that token to the real `POST /v1/classify`
//! admission path in-process, so a playground call reserves quota,
//! forwards to the bound backend, and leaves the same sealed receipt
//! any API call would. The simulated lane never touches a backend: it
//! derives deterministic answers from the request's digest, marks every
//! result SIMULATED, and bills nothing.
//!
//! Nothing here persists inputs, answers, or transcripts — the results
//! page and the chat's hidden form state are the whole retention story.
//! The chat demo's only tool is the classify facade; its replies are
//! assembled from the tool's actual answers and it never invents an
//! outcome.

use std::sync::Arc;
use std::time::Instant;

use axum::Form;
use axum::body::Bytes;
use axum::extract::{Multipart, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::classify;
use crate::dashboard::{cookie_token, esc, page, page_error, principal_of};
use crate::serve::{self, ServeState};

/// The most items one playground run may carry.
const MAX_ITEMS: usize = 100;
/// The most bytes one pasted item may carry.
const MAX_ITEM_BYTES: usize = 4096;
/// The most labels a run may declare.
const MAX_LABELS: usize = 20;
/// The most bytes an uploaded dataset file may carry.
const MAX_UPLOAD_BYTES: usize = 65_536;
/// The most chat turns one demo conversation may hold.
const MAX_TURNS: usize = 10;
/// The most characters one chat message may carry.
const MAX_MESSAGE_CHARS: usize = 1024;

/// The playground's pages — mounted beside the dashboard only when
/// `accounts` is configured, because its sign-in is the session token.
pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        ("/playground", get(form_page)),
        ("/playground/session", post(crate::dashboard::session)),
        ("/playground/run", post(run)),
        ("/playground/upload", post(upload)),
        ("/playground/chat", get(chat_page).post(chat_turn)),
    ]
}

/// The session token for decision calls, or the sign-in page.
fn signed_in(state: &ServeState, headers: &HeaderMap) -> Result<String, Response> {
    principal_of(state, headers)?;
    cookie_token(headers).ok_or_else(|| {
        page_error(
            StatusCode::UNAUTHORIZED,
            "sign in required",
            "no session cookie — sign in with a session token",
        )
    })
}

/// Build the bearer + workspace headers a live internal call needs.
fn live_headers(token: &str, workspace: &str, idempotency: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}")) {
        headers.insert(AUTHORIZATION, value);
    }
    if let Ok(value) = HeaderValue::from_str(workspace) {
        headers.insert("x-workspace-id", value);
    }
    if let Ok(value) = HeaderValue::from_str(idempotency) {
        headers.insert("idempotency-key", value);
    }
    headers
}

/// A deterministic simulated answer: the SHA-256 of the seed mapped to
/// `[0, 1)` — stable across runs so a caller can compare.
fn simulated(seed: &str) -> f64 {
    let digest = Sha256::digest(seed.as_bytes());
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    (u64::from_be_bytes(bytes) % 1_000_000) as f64 / 1_000_000.0
}

/// A playground run's form fields.
#[derive(Debug, Deserialize)]
pub struct RunForm {
    /// The workspace the live call names (`x-workspace-id`).
    workspace: String,
    /// The door to call.
    model: String,
    /// The classification mode.
    mode: String,
    /// One item per line.
    items: String,
    /// Comma-separated label ids.
    #[serde(default)]
    labels: String,
    /// Comma-separated rubric levels for `score`.
    #[serde(default)]
    levels: String,
    /// The selection cut for `multi-label` and `binary`, or the
    /// `min_probability` for `single-label`.
    #[serde(default)]
    threshold: Option<f64>,
    /// The review cut that marks a unit `uncertain`.
    #[serde(default)]
    uncertain_below: Option<f64>,
    /// Shared instructions.
    #[serde(default)]
    instructions: Option<String>,
    /// `on` runs the deterministic simulated lane.
    #[serde(default)]
    simulate: Option<String>,
}

/// Parse the textarea's non-empty lines into bounded item ids.
fn items_of(form: &RunForm) -> Result<Vec<(String, String)>, Response> {
    let items: Vec<(String, String)> = form
        .items
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(index, text)| (format!("item-{index}"), text.to_string()))
        .collect();
    if items.is_empty() {
        return Err(page_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "no items",
            "paste at least one item — one per line, or upload a dataset file",
        ));
    }
    if items.len() > MAX_ITEMS {
        return Err(page_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "too many items",
            &format!("the playground bounds a run at {MAX_ITEMS} items"),
        ));
    }
    if let Some((id, _)) = items.iter().find(|(_, text)| text.len() > MAX_ITEM_BYTES) {
        return Err(page_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "item too large",
            &format!("{id} exceeds the {MAX_ITEM_BYTES}-byte item bound"),
        ));
    }
    Ok(items)
}

/// The caller's label list as `Label` objects.
fn labels_of(form: &RunForm) -> Result<Vec<classify::Label>, Response> {
    let labels: Vec<classify::Label> = form
        .labels
        .split(',')
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .map(|id| classify::Label {
            id: id.to_string(),
            description: None,
        })
        .collect();
    if labels.len() > MAX_LABELS {
        return Err(page_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "too many labels",
            &format!("the playground bounds a label set at {MAX_LABELS}"),
        ));
    }
    Ok(labels)
}

/// Build the classify envelope the run sends — live or simulated, the
/// request is the same shape the API takes.
fn envelope(form: &RunForm, items: &[(String, String)]) -> Result<Value, Response> {
    let labels = labels_of(form)?;
    let mode = match form.mode.as_str() {
        "single-label" => "single-label",
        "multi-label" => "multi-label",
        "binary" => "binary",
        "score" => "score",
        _ => {
            return Err(page_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "unknown mode",
                "mode is one of single-label, multi-label, binary, score",
            ));
        }
    };
    let mut select = json!({});
    if let Some(threshold) = form.threshold {
        match mode {
            "single-label" => {
                select["single_label"] = json!({
                "ties": "first-declared", "min_probability": threshold,
                "no_match": {"kind": "null"}})
            }
            "multi-label" => {
                select["multi_label"] = json!({
                "threshold": threshold, "ties": "truncate",
                "no_match": "empty"})
            }
            "binary" => select["binary"] = json!({"threshold": threshold}),
            _ => {}
        }
    }
    if select.as_object().unwrap().is_empty() {
        // The contract requires a rule for the mode the request plans;
        // the playground declares the neutral rule per mode.
        select = match mode {
            "single-label" => json!({"single_label": {
                "ties": "first-declared", "no_match": {"kind": "null"}}}),
            "multi-label" => json!({"multi_label": {
                "threshold": 0.5, "ties": "truncate", "no_match": "empty"}}),
            "binary" => json!({"binary": {"threshold": 0.5}}),
            _ => json!({"score": {"order": "descending"}}),
        };
    }
    if let Some(cut) = form.uncertain_below
        && let Some(rule) = select.get_mut(mode.replace('-', "_").as_str())
    {
        rule["uncertain_below"] = json!(cut);
    }
    let mut request = json!({
        "v": classify::SCHEMA,
        "model": form.model,
        "capacity": "dedicated",
        "policy": {"v": classify::POLICY_SCHEMA, "name": "playground", "select": select},
        "inputs": items.iter().map(|(id, text)| json!({"id": id, "text": text}))
            .collect::<Vec<_>>(),
    });
    if let Some(instructions) = form.instructions.as_ref().filter(|s| !s.is_empty()) {
        request["instructions"] = json!(instructions);
    }
    if mode == "score" {
        let levels: Vec<&str> = form
            .levels
            .split(',')
            .map(str::trim)
            .filter(|level| !level.is_empty())
            .collect();
        if !(2..=10).contains(&levels.len()) {
            return Err(page_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "score needs a rubric",
                "a score run declares 2–10 comma-separated rubric levels",
            ));
        }
        request["mode"] = json!("score");
        request["levels"] = json!(levels);
    } else {
        if labels.is_empty() {
            return Err(page_error(
                StatusCode::UNPROCESSABLE_ENTITY,
                "no labels",
                "declare at least one comma-separated label",
            ));
        }
        request["mode"] = json!(mode);
        request["labels"] = json!(labels);
    }
    Ok(request)
}

/// Render one classify response body as result rows.
fn result_rows(body: &Value) -> String {
    let mut rows = String::new();
    let empty = Vec::new();
    for result in body["results"].as_array().unwrap_or(&empty) {
        let input = esc(result["input"].as_str().unwrap_or("?"));
        let outcome = result["outcome"].as_str().unwrap_or("?");
        let mut units = String::new();
        for unit in result["units"].as_array().unwrap_or(&empty) {
            let selected = match &unit["selected"] {
                Value::Null => "—".to_string(),
                value => esc(&value.to_string()),
            };
            let mut detail = format!("selected <code>{selected}</code>");
            if let Some(distribution) = unit["raw"]["probabilities"].as_object() {
                let parts: Vec<String> = distribution
                    .iter()
                    .map(|(label, p)| format!("{} {:.2}", esc(label), p.as_f64().unwrap_or(0.0)))
                    .collect();
                detail.push_str(&format!(" ({})", parts.join(", ")));
            }
            if let Some(score) = unit["raw"]["score"].as_f64() {
                detail.push_str(&format!(" score {:.2}", score));
            }
            if unit["uncertain"].as_bool() == Some(true) {
                detail.push_str(" <strong>review</strong>");
            }
            if let Some(review) = unit.get("review") {
                detail.push_str(&format!(" reviewed: {}", esc(&review.to_string())));
            }
            units.push_str(&format!("<br>{detail}"));
        }
        let review_status = result["review_status"]
            .as_str()
            .map(|status| format!(" · {}", esc(status)))
            .unwrap_or_default();
        let cause = result["cause"]
            .as_str()
            .map(|c| format!(" <span class=\"err\">{}</span>", esc(c)))
            .unwrap_or_default();
        rows.push_str(&format!(
            "<tr><td><code>{input}</code></td><td>{outcome}{review_status}{cause}</td><td>{units}</td></tr>"
        ));
    }
    if rows.is_empty() {
        rows.push_str("<tr><td colspan=\"3\" class=\"dim\">no results</td></tr>");
    }
    rows
}

/// The results page's footer: model, usage, timing, receipt, export.
fn run_footer(
    body: &Value,
    elapsed_ms: u128,
    request: &Value,
    simulated: bool,
    receipt: Option<&str>,
) -> String {
    let usage = body.get("usage").cloned().unwrap_or(json!({}));
    let receipt = receipt
        .map(|digest| format!("<p>receipt <code>{}</code></p>", esc(digest)))
        .unwrap_or_default();
    let served = body["served"]["model"]
        .as_str()
        .map(|model| format!("served by <code>{}</code>", esc(model)))
        .unwrap_or_default();
    let badge = if simulated {
        r#"<p class="err"><strong>SIMULATED</strong> — deterministic synthetic answers; nothing was forwarded and nothing was billed.</p>"#
    } else {
        ""
    };
    let request_json = esc(&serde_json::to_string_pretty(request).unwrap_or_default());
    format!(
        r#"{badge}
        <h2>call</h2>
        <p class="dim">model <code>{}</code> {served} · {} ms · usage <code>{}</code></p>
        {receipt}
        <h2>export</h2>
        <p class="dim">Equivalent request — <code>POST /v1/classify</code>:</p>
        <pre><code>{request_json}</code></pre>"#,
        esc(body["model"].as_str().unwrap_or("simulated")),
        elapsed_ms,
        esc(&usage.to_string()),
    )
}

/// `GET /playground` — the run form, or the sign-in prompt.
async fn form_page(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if principal_of(&state, &headers).is_err() {
        return page(
            "playground",
            None,
            r#"<h1>playground</h1>
            <p>Sign in with a session token to try decision workflows — the same
            <code>sess_…</code> the dashboard uses.</p>
            <form method="post" action="/playground/session">
            <label>session token <input name="token" type="password" size="48" required autocomplete="off"></label>
            <button type="submit">sign in</button></form>
            <p class="dim">Runs against your own workspaces and doors; the simulated lane needs
            no backend and bills nothing.</p>"#,
        )
        .into_response();
    }
    // Populate the model select from the doors this session can name.
    let mut forwarded = HeaderMap::new();
    if let Some(token) = cookie_token(&headers)
        && let Ok(value) = HeaderValue::from_str(&format!("Bearer {token}"))
    {
        forwarded.insert(AUTHORIZATION, value);
    }
    let mut options = String::new();
    if let Ok(models) = serve::models(State(state.clone()), forwarded).await
        && let Some(cards) = models.0["models"].as_array()
    {
        for card in cards {
            if let (Some(id), Some(model)) = (card["id"].as_str(), card["model"].as_str()) {
                options.push_str(&format!(
                    "<option value=\"{}\">{} ({})</option>",
                    esc(id),
                    esc(id),
                    esc(model)
                ));
            }
        }
    }
    page(
        "playground",
        None,
        &format!(
            r#"<h1>playground</h1>
            <p>Run a bounded classification and inspect the exact request, answers,
            uncertainty, usage, and receipt. Check <strong>simulate</strong> for a
            deterministic synthetic run that bills nothing.</p>
            <form method="post" action="/playground/run">
            <p><label>workspace <input name="workspace" size="28" required></label>
            <label>model <input name="model" list="doors" size="20" required>
            <datalist id="doors">{options}</datalist></label>
            <label>mode <select name="mode">
            <option>single-label</option><option>multi-label</option>
            <option>binary</option><option>score</option></select></label></p>
            <p><label>labels <input name="labels" size="40" placeholder="a,b,c"></label>
            <label>score rubric <input name="levels" size="30" placeholder="low,medium,high"></label></p>
            <p><label>threshold <input name="threshold" size="6" placeholder="0.5"></label>
            <label>review below <input name="uncertain_below" size="6" placeholder="0.7"></label>
            <label>instructions <input name="instructions" size="30"></label></p>
            <p><label>items, one per line<br>
            <textarea name="items" rows="8" cols="72" required></textarea></label></p>
            <p><label>or upload a file
            <input type="file" form="upload" disabled title="use POST /playground/upload"></label>
            <label><input type="checkbox" name="simulate"> simulate — deterministic, unbilled</label></p>
            <p><button type="submit">run</button>
            <a href="/playground/chat">chat demo</a></p>
            </form>
            <form method="post" action="/playground/upload" enctype="multipart/form-data" id="upload">
            <p class="dim">dataset file (one item per line, ≤64 KiB):
            <input type="file" name="file" required>
            <input type="hidden" name="workspace" value="">
            <button type="submit">upload</button></p></form>
            <p class="dim">Bounds: {MAX_ITEMS} items, {MAX_ITEM_BYTES} bytes each,
            {MAX_LABELS} labels. Nothing is persisted — the results page is the whole record.</p>"#
        ),
    )
    .into_response()
}

/// `POST /playground/run` — one bounded classification, live or
/// simulated, rendered with its request, answers, usage, and receipt.
async fn run(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Form(form): Form<RunForm>,
) -> Response {
    let token = match signed_in(&state, &headers) {
        Ok(token) => token,
        Err(response) => return response,
    };
    let items = match items_of(&form) {
        Ok(items) => items,
        Err(response) => return response,
    };
    let request = match envelope(&form, &items) {
        Ok(request) => request,
        Err(response) => return response,
    };
    let started = Instant::now();
    if form.simulate.is_some() {
        // The deterministic lane: synthesize the response the envelope
        // would produce, from the request's own digest.
        let seed = serde_json::to_string(&request).unwrap_or_default();
        let mut results = Vec::new();
        for (id, _) in &items {
            let mut units = Vec::new();
            match form.mode.as_str() {
                "multi-label" | "binary" => {
                    let labels = labels_of(&form).unwrap_or_default();
                    let cut = form.threshold.unwrap_or(0.5);
                    let picked: Vec<String> = labels
                        .iter()
                        .filter(|label| simulated(&format!("{seed}:{id}:{}", label.id)) >= cut)
                        .map(|label| label.id.clone())
                        .collect();
                    units.push(json!({"selected": picked, "uncertain": false}));
                }
                "score" => {
                    units.push(json!({"score": simulated(&format!("{seed}:{id}:score"))}));
                }
                _ => {
                    let labels = labels_of(&form).unwrap_or_default();
                    let best = labels
                        .iter()
                        .map(|label| {
                            (
                                simulated(&format!("{seed}:{id}:{}", label.id)),
                                label.id.clone(),
                            )
                        })
                        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
                        .map(|(_, id)| id);
                    units.push(json!({"selected": best, "uncertain": false}));
                }
            }
            results.push(json!({"input": id, "outcome": "answered", "units": units}));
        }
        let body = json!({
            "v": "openagents.classify-result.v1", "outcome": "answered",
            "model": "simulated", "results": results,
            "usage": {"input_tokens": 0, "output_tokens": 0},
        });
        let rows = result_rows(&body);
        return page(
            "simulated run",
            None,
            &format!(
                r#"<h1>results</h1>
                <table><tr><th>input</th><th>outcome</th><th>units</th></tr>{rows}</table>
                {}"#,
                run_footer(&body, started.elapsed().as_millis(), &request, true, None)
            ),
        )
        .into_response();
    }
    // Live: the session's own credential, the real admission path.
    let idempotency = format!(
        "playground-{:x}",
        Sha256::digest(
            serde_json::to_string(&request)
                .unwrap_or_default()
                .as_bytes()
        )
        .iter()
        .take(8)
        .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
    );
    let response = serve::classify(
        State(state.clone()),
        live_headers(&token, &form.workspace, &idempotency),
        Bytes::from(serde_json::to_vec(&request).unwrap_or_default()),
    )
    .await;
    let status = response.status();
    let receipt = response
        .headers()
        .get("x-receipt")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    let body: Value =
        serde_json::from_slice(&bytes).unwrap_or(json!({"raw": String::from_utf8_lossy(&bytes)}));
    if !status.is_success() {
        let detail = body["error"]["message"]
            .as_str()
            .or_else(|| body["error"]["code"].as_str())
            .unwrap_or("the call was refused");
        return page_error(
            status,
            "call refused",
            &format!("{} — see the equivalent request below", detail),
        );
    }
    let rows = result_rows(&body);
    page(
        "run results",
        None,
        &format!(
            r#"<h1>results</h1>
            <table><tr><th>input</th><th>outcome</th><th>units</th></tr>{rows}</table>
            {}"#,
            run_footer(
                &body,
                started.elapsed().as_millis(),
                &request,
                false,
                receipt.as_deref(),
            )
        ),
    )
    .into_response()
}

/// `POST /playground/upload` — a bounded dataset file becomes the
/// form's items: one item per line, the same bounds as paste.
async fn upload(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    if signed_in(&state, &headers).is_err() {
        return page_error(
            StatusCode::UNAUTHORIZED,
            "sign in required",
            "no session cookie — sign in with a session token",
        );
    }
    let mut text = String::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let bytes = match field.bytes().await {
                Ok(bytes) => bytes,
                Err(trouble) => {
                    return page_error(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "upload failed",
                        &trouble.to_string(),
                    );
                }
            };
            if bytes.len() > MAX_UPLOAD_BYTES {
                return page_error(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "upload too large",
                    &format!("dataset files are bounded at {MAX_UPLOAD_BYTES} bytes"),
                );
            }
            text = String::from_utf8_lossy(&bytes).to_string();
        }
    }
    let count = text.lines().filter(|line| !line.trim().is_empty()).count();
    page(
        "uploaded",
        None,
        &format!(
            r#"<h1>dataset uploaded</h1>
            <p>{count} items parsed. Copy them into the run form:</p>
            <form method="get" action="/playground"><button type="submit">back to playground</button></form>
            <pre><code>{}</code></pre>"#,
            esc(&text)
        ),
    )
    .into_response()
}

/// The chat demo's form state: turns ride the page, not the server.
#[derive(Debug, Deserialize)]
pub struct ChatForm {
    /// Accumulated `user|assistant` lines — the whole retention story.
    #[serde(default)]
    history: String,
    /// This turn's message.
    #[serde(default)]
    message: String,
    /// The workspace a live chat names.
    #[serde(default)]
    workspace: String,
    /// The door a live chat calls.
    #[serde(default)]
    model: String,
    /// `on` runs the simulated lane.
    #[serde(default)]
    simulate: Option<String>,
}

/// Render the accumulated history as the transcript.
fn transcript(history: &str) -> String {
    let mut rendered = String::new();
    for line in history.lines() {
        if let Some(message) = line.strip_prefix("user|") {
            rendered.push_str(&format!("<p><strong>you:</strong> {}</p>", esc(message)));
        } else if let Some(message) = line.strip_prefix("assistant|") {
            rendered.push_str(&format!("<p><strong>demo:</strong> {}</p>", esc(message)));
        }
    }
    rendered
}

/// `GET /playground/chat` — the bounded chat form.
async fn chat_page(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if principal_of(&state, &headers).is_err() {
        return page_error(
            StatusCode::UNAUTHORIZED,
            "sign in required",
            "no session cookie — sign in with a session token",
        );
    }
    page(
        "chat demo",
        None,
        &format!(
            r#"<h1>chat demo</h1>
            <p>A bounded conversation whose only tool is the classify facade. Every
            turn shows the actual call and answer — the demo invents nothing.</p>
            <form method="post" action="/playground/chat">
            <p><label>workspace <input name="workspace" size="28" required></label>
            <label>model <input name="model" size="20" required></label>
            <label><input type="checkbox" name="simulate"> simulate</label></p>
            <p><label>message <input name="message" size="64" maxlength="{MAX_MESSAGE_CHARS}" required></label>
            <button type="submit">send</button></p>
            <input type="hidden" name="history" value="">
            </form>
            <p class="dim">Caps: {MAX_TURNS} turns, 1 tool call per turn, each bounded by the
            facade's own limits. History lives in this page only — closing it deletes the
            transcript. No fetching or external content: everything the tool sees is typed here.</p>
            <p><a href="/playground">back to playground</a></p>"#
        ),
    )
    .into_response()
}

/// `POST /playground/chat` — one bounded turn: the message becomes the
/// tool's input, the reply is assembled from the actual answer.
async fn chat_turn(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Form(form): Form<ChatForm>,
) -> Response {
    let token = match signed_in(&state, &headers) {
        Ok(token) => token,
        Err(response) => return response,
    };
    let turns = form
        .history
        .lines()
        .filter(|line| line.starts_with("user|"))
        .count();
    if turns >= MAX_TURNS {
        return page_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "conversation complete",
            &format!("the demo bounds a conversation at {MAX_TURNS} turns"),
        );
    }
    if form.message.is_empty() || form.message.chars().count() > MAX_MESSAGE_CHARS {
        return page_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "message out of bounds",
            &format!("messages run 1–{MAX_MESSAGE_CHARS} characters"),
        );
    }
    // The tool call: classify the message's intent over a fixed set —
    // the same facade, the same bounds, as the playground's run.
    let request = json!({
        "v": classify::SCHEMA,
        "model": form.model,
        "capacity": "dedicated",
        "policy": {"v": classify::POLICY_SCHEMA, "name": "playground-chat",
            "select": {"single_label": {"ties": "first-declared", "no_match": {"kind": "null"}}}},
        "instructions": "Classify the caller's chat message by what it asks the demo to do.",
        "inputs": [{"id": "message", "text": form.message}],
        "mode": "single-label",
        "labels": [{"id": "classification"}, {"id": "question"}, {"id": "other"}],
    });
    let (answer, note) = if form.simulate.is_some() {
        let seed = serde_json::to_string(&request).unwrap_or_default();
        let intents = ["classification", "question", "other"];
        let picked = intents
            .iter()
            .map(|intent| (simulated(&format!("{seed}:{intent}")), *intent))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, intent)| intent.to_string());
        (
            json!({"selected": picked, "uncertain": false}),
            "SIMULATED — no backend was called and nothing was billed".to_string(),
        )
    } else {
        let idempotency = format!(
            "chat-{:x}",
            Sha256::digest(format!("{}{}", form.history, form.message).as_bytes())
                .iter()
                .take(8)
                .fold(0u64, |acc, b| (acc << 8) | u64::from(*b))
        );
        let response = serve::classify(
            State(state.clone()),
            live_headers(&token, &form.workspace, &idempotency),
            Bytes::from(serde_json::to_vec(&request).unwrap_or_default()),
        )
        .await;
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap_or_default();
        let body: Value =
            serde_json::from_slice(&bytes).unwrap_or(json!({"error": {"code": "unavailable"}}));
        if body["error"].is_object() {
            (
                json!({"refused": body["error"]["code"]}),
                "the tool refused — the refusal is the answer".to_string(),
            )
        } else {
            let unit = body["results"][0]["units"][0].clone();
            (
                unit,
                format!(
                    "live call · outcome {}",
                    body["outcome"].as_str().unwrap_or("?")
                ),
            )
        }
    };
    // The reply is composed of the tool's answer — nothing else.
    let reply = match &answer["selected"] {
        Value::String(intent) => format!(
            "The tool classified your message as <code>{intent}</code> ({note}). \
             To run it as a classification, paste it into the <a href=\"/playground\">playground</a>.",
            intent = esc(intent),
        ),
        _ => format!("The tool did not select an intent ({note})."),
    };
    let history = format!(
        "{}user|{}\nassistant|{}",
        if form.history.is_empty() {
            String::new()
        } else {
            format!("{}\n", form.history)
        },
        form.message.replace(['\n', '|'], " "),
        reply.replace(['\n', '|'], " "),
    );
    page(
        "chat demo",
        None,
        &format!(
            r#"<h1>chat demo</h1>
            {}
            <form method="post" action="/playground/chat">
            <p><label>message <input name="message" size="64" maxlength="{MAX_MESSAGE_CHARS}" required></label>
            <button type="submit">send</button></p>
            <input type="hidden" name="workspace" value="{}">
            <input type="hidden" name="model" value="{}">
            <input type="hidden" name="history" value="{}">
            {}
            </form>
            <p class="dim">turn {}/{MAX_TURNS} · the transcript is this page's form state — nothing is stored server-side</p>
            <p><a href="/playground">back to playground</a></p>"#,
            transcript(&history),
            esc(&form.workspace),
            esc(&form.model),
            esc(&history),
            if form.simulate.is_some() { r#"<input type="hidden" name="simulate" value="on">"# } else { "" },
            turns + 1,
        ),
    )
    .into_response()
}
