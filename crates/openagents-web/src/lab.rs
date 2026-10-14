//! Video Lab in the web chat (docs/video-lab/README.md, docs/video-lab/mvp.md).
//!
//! When this server is told where a Video Lab runs (`OPENAGENTS_VIDEO_LAB`,
//! a loopback URL: the Lab renders with private assets and never leaves
//! the Mac it runs on), a signed-in owner's message goes to [`LabDoor`]:
//! Gemini 4 Argon (Vertex, global) with the Lab's tools declared as typed functions (`lab_open`,
//! `lab_show`, `lab_frame`, `lab_edit`, `lab_render`, `lab_compare`,
//! `lab_notes`, ...; the Lab serves their declarations at `/api/tools`) and
//! one more, `general_chat`. The model's typed function choice is the
//! route: it works on a film or hands the message to the chat behind it.
//! No words of the message are matched here.
//!
//! A tool's result carries a card (`"card": "video_lab"`): the document,
//! the version, the rendered video when there is one, a frame strip or a
//! side-by-side frame, and a compare pair. The last card of an answer is
//! kept on the reply ([`openagents_chat::router::Meta::cards`]) and drawn
//! under it by [`thread_entry`]: an inline `<video>` from the Lab, which
//! refreshes itself from [`card`] while a render is still running.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use inference::upstream::vertex;
use maud::{Markup, html};
use openagents_chat::basic_coder::{self, Door, Failure, Reply, Role as TurnRole, Turn};
use openagents_chat::router::Context;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;
use crate::chat_vision::{Gemini, gemini_text};
use crate::connections::chat::{HANDOFF, gemini_call};

/// The most model steps (tool rounds) one answer takes.
const MAX_STEPS: usize = 8;
/// How long one Lab call may take (a frame strip draws from the sources).
const LAB_DEADLINE: Duration = Duration::from_secs(90);
/// The card kind a Lab tool returns.
pub(crate) const CARD: &str = "video_lab";

/// The Lab this server talks to, when one is configured: loopback only.
pub(crate) fn lab_url() -> Option<String> {
    loopback(&std::env::var("OPENAGENTS_VIDEO_LAB").ok()?)
}

fn loopback(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/').to_owned();
    (url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")).then_some(url)
}

fn client(deadline: Duration) -> Option<reqwest::Client> {
    reqwest::Client::builder().timeout(deadline).build().ok()
}

/// The Lab's tool declarations and documents, when it answers.
async fn manifest(lab: &str) -> Result<(Vec<Value>, Vec<Value>), String> {
    let http = client(Duration::from_secs(3)).ok_or("no HTTP client")?;
    let tools: Value = http
        .get(format!("{lab}/api/tools"))
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let docs: Value = http
        .get(format!("{lab}/api/docs"))
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let tools = tools["tools"].as_array().cloned().unwrap_or_default();
    let docs = docs["documents"].as_array().cloned().unwrap_or_default();
    Ok((tools, docs))
}

/// One tool call on the Lab: its JSON result.
async fn run_tool(lab: &str, name: &str, arguments: &Value) -> Result<Value, String> {
    if !name.starts_with("lab_")
        || !name[4..]
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_')
    {
        return Err(format!("{name} isn't a Lab tool"));
    }
    let http = client(LAB_DEADLINE).ok_or("no HTTP client")?;
    let response = http
        .post(format!("{lab}/api/tool/{name}"))
        .json(arguments)
        .send()
        .await
        .map_err(|e| format!("the Lab didn't answer ({})", e.without_url()))?;
    response.json().await.map_err(|e| e.to_string())
}

/// A door that works on Video Lab documents, or hands the message to
/// `fallback`.
pub(crate) struct LabDoor {
    pub gemini: Gemini,
    pub lab: String,
    pub tools: Vec<Value>,
    pub documents: Vec<Value>,
    pub fallback: Option<Box<dyn Door>>,
}

/// The Lab door, when a Lab is configured and answering and this server
/// has Gemini. A Lab is only ever configured on the computer it runs on
/// (loopback, [`lab_url`]), so whoever uses this site there is its owner,
/// signed in or not: the person at the Mac.
pub(crate) async fn door(_app: &App, _owner: &str) -> Option<LabDoor> {
    let lab = lab_url()?;
    // Video Lab's turns run on Gemini 4 Argon (Vertex, global), the Labs' model
    // (Game Lab's door does the same); the chat's image path keeps its own.
    let gemini = Gemini::from_env_for(vertex::ARGON)?;
    let (tools, documents) = match manifest(&lab).await {
        Ok(found) => found,
        Err(why) => {
            eprintln!("openagents-web: video lab: not answering at {lab}: {why}");
            return None;
        }
    };
    Some(LabDoor {
        gemini,
        lab,
        tools,
        documents,
        fallback: None,
    })
}

/// What the model is told about the Lab.
fn guidance(documents: &[Value]) -> String {
    let mut out = String::from(
        "\n\nThis chat can see and edit the owner's films and scenes in Video Lab, on this Mac, with the lab_* tools. \
When the message is about a film, a cut, a shot, a scene or a video in the Lab (open it, show a time, look at a frame, \
change it, render it, compare versions, read the review notes), use the tools; otherwise call general_chat. \
The chat shows a card with the video under your answer, so don't paste paths or links; say in a sentence or two what \
you did and what to look at. Times are m:ss. \
To change a document, read it first (lab_open or lab_show), then call lab_edit with a JSON Patch on its JSON and a \
one-line summary in plain words. Address a cut's shot by name: /shots/@NAME/dur. In a cut at 120 BPM every shot length \
is a whole number of beats (a multiple of 0.5 s). If lab_edit refuses, read its reason, fix the patch and try again; \
tell the owner if it can't be done. After an edit a preview renders by itself; to compare, call lab_compare with the \
time asked for (a and b default to the version before and the latest). \
To use a new clip, bring it in with lab_import (a path or a file name like IMG_4135), read what's said and its pauses \
with lab_sources, then put a range into the cut with lab_splice: it keeps the clip's own speech with the music ducked, \
moves the in and out to pauses so no word is cut, and makes captions of what's said. Speak as I, never we.\
\n\nDocuments:\n",
    );
    for doc in documents {
        out.push_str(&format!(
            "- {} \"{}\" ({}, {} versions)\n",
            doc["id"].as_str().unwrap_or_default(),
            doc["title"].as_str().unwrap_or_default().replace('"', "'"),
            doc["kind"].as_str().unwrap_or_default(),
            doc["versions"].as_u64().unwrap_or_default(),
        ));
    }
    out
}

/// The function declarations Gemini gets: the Lab's tools and the hand-off.
fn declarations(tools: &[Value]) -> Vec<Value> {
    let mut out: Vec<Value> = tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "name": tool["name"],
                "description": tool["description"],
                "parameters": tool["parameters"],
            })
        })
        .collect();
    out.push(json!({
        "type": "function",
        "name": HANDOFF,
        "description": "Hand the message to OpenAgents' general chat, for anything that isn't about a film, shot or scene in Video Lab.",
        "parameters": {"type": "object", "properties": {}},
    }));
    out
}

fn request(instructions: &str, turns: &[Turn], tools: &[Value]) -> Value {
    let input: Vec<Value> = turns
        .iter()
        .map(|turn| {
            let role = match turn.role {
                TurnRole::User => "user",
                TurnRole::Assistant => "assistant",
            };
            json!({"type": "message", "role": role, "content": turn.text})
        })
        .collect();
    json!({
        "model": vertex::ARGON,
        "instructions": instructions,
        "input": input,
        "tools": declarations(tools),
        "stream": false,
        "store": false,
    })
}

/// How a run of the loop ended.
enum Ending {
    Answer { text: String, card: Option<Value> },
    Handoff,
}

async fn converse(
    gemini: &Gemini,
    lab: &str,
    tools: &[Value],
    instructions: &str,
    turns: &[Turn],
) -> Result<Ending, String> {
    let mut body = gemini.body(&request(instructions, turns, tools))?;
    let mut card: Option<Value> = None;
    for step in 0..=MAX_STEPS {
        if step == MAX_STEPS {
            body["toolConfig"] = json!({"functionCallingConfig": {"mode": "NONE"}});
        }
        let reply = gemini_call(gemini, &body).await?;
        let content = reply["candidates"][0]["content"].clone();
        let calls: Vec<Value> = content["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|part| part["functionCall"].is_object())
            .map(|part| part["functionCall"].clone())
            .collect();
        if calls.is_empty() {
            let text = gemini_text(&reply).ok_or("Gemini answered nothing")?;
            return Ok(Ending::Answer { text, card });
        }
        if calls.iter().any(|call| call["name"] == HANDOFF) && card.is_none() {
            return Ok(Ending::Handoff);
        }
        let mut responses = Vec::new();
        for call in &calls {
            let name = call["name"].as_str().unwrap_or_default().to_owned();
            let arguments = match &call["args"] {
                Value::Object(_) => call["args"].clone(),
                _ => json!({}),
            };
            let response = match run_tool(lab, &name, &arguments).await {
                Ok(mut result) => {
                    if let Some(found) = result.get("card").filter(|c| c["card"] == CARD) {
                        card = Some(found.clone());
                    }
                    // The model reads the words; the card is for the person.
                    if let Some(object) = result.as_object_mut() {
                        object.remove("card");
                        object.remove("documents");
                    }
                    json!({"result": result})
                }
                Err(why) => json!({"error": why}),
            };
            eprintln!("openagents-web: video lab: {name}");
            let mut part = json!({"functionResponse": {"name": name, "response": response}});
            if let Some(id) = call["id"].as_str() {
                part["functionResponse"]["id"] = json!(id);
            }
            responses.push(part);
        }
        let contents = body["contents"]
            .as_array_mut()
            .ok_or("the request has no contents")?;
        contents.push(json!({"role": "model", "parts": content["parts"].clone()}));
        contents.push(json!({"role": "user", "parts": responses}));
    }
    Err("Gemini kept calling tools".into())
}

impl Door for LabDoor {
    fn ask(
        &self,
        turns: Vec<Turn>,
        context: Context,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let instructions = format!(
            "{}{}",
            basic_coder::instructions(&context),
            guidance(&self.documents)
        );
        let fallback = self
            .fallback
            .as_ref()
            .map(|door| door.ask(turns.clone(), context.clone(), reply.clone()));
        let gemini = self.gemini.clone();
        let lab = self.lab.clone();
        let tools = self.tools.clone();
        Box::pin(async move {
            match converse(&gemini, &lab, &tools, &instructions, &turns).await {
                Ok(Ending::Answer { text, card }) => {
                    let mut reply = basic_coder::lock(&reply);
                    reply.text = text;
                    reply.model = Some(gemini.row.id.clone());
                    reply.meta.tier = Some("model".into());
                    reply.meta.route = Some("video_lab".into());
                    reply.meta.cards = card.into_iter().collect();
                    reply.done = true;
                }
                other => {
                    if let Err(why) = &other {
                        eprintln!("openagents-web: video lab: {why}; the general chat answers");
                    }
                    match fallback {
                        Some(job) => job.await,
                        None => {
                            basic_coder::lock(&reply).failure = Some(Failure::Transport(
                                other.err().unwrap_or_else(|| "no general chat".into()),
                            ));
                        }
                    }
                }
            }
        })
    }
}

/// The Lab cards a reply keeps (others are the worker's and aren't kept).
pub(crate) fn cards(meta: &openagents_chat::router::Meta) -> Vec<Value> {
    meta.cards
        .iter()
        .filter(|card| card["card"] == CARD)
        .take(1)
        .cloned()
        .collect()
}

// ---- the card -------------------------------------------------------------------

fn s<'a>(card: &'a Value, key: &str) -> &'a str {
    card[key].as_str().unwrap_or_default()
}

fn clock(t: f64) -> String {
    let t = t.max(0.0);
    let m = (t / 60.0).floor();
    let s = t - m * 60.0;
    if (s - s.round()).abs() < 0.05 {
        format!("{}:{:02}", m as u64, s.round() as u64)
    } else {
        format!("{}:{:04.1}", m as u64, s)
    }
}

/// Where the page loads a `/media/...` file the card names: through this
/// site (`/chat/{id}/lab/media/...`), since the page only loads its own
/// origin's files, and the Lab only answers on this computer.
fn media(chat: &str, path: &str) -> Option<String> {
    path.strip_prefix("/media/")
        .filter(|rest| !rest.split('/').any(|part| part == ".." || part.is_empty()))
        .map(|rest| format!("/chat/{chat}/lab/media/{rest}"))
}

/// What the card's refresh asks the Lab for.
#[derive(Deserialize, Default, Debug)]
pub(crate) struct CardQuery {
    doc: String,
    v: u64,
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    from: Option<f64>,
    #[serde(default)]
    to: Option<f64>,
    #[serde(default)]
    a: Option<u64>,
    #[serde(default)]
    at: Option<f64>,
    #[serde(default)]
    image: Option<String>,
}

fn refresh_href(chat: &str, card: &Value) -> String {
    let mut query = vec![
        ("doc", s(card, "doc").to_owned()),
        ("v", card["version"].as_u64().unwrap_or(1).to_string()),
        (
            "q",
            card["quality"].as_str().unwrap_or("preview").to_owned(),
        ),
    ];
    if let Some(from) = card["from"].as_f64() {
        query.push(("from", from.to_string()));
    }
    if let Some(to) = card["to"].as_f64() {
        query.push(("to", to.to_string()));
    }
    if let Some(a) = card["compare"]["a"]["version"].as_u64() {
        query.push(("a", a.to_string()));
        query.push((
            "at",
            card["compare"]["at"].as_f64().unwrap_or(0.0).to_string(),
        ));
    }
    if let Some(image) = card["image"].as_str() {
        query.push(("image", image.to_owned()));
    }
    let encoded: Vec<String> = query
        .into_iter()
        .map(|(k, v)| format!("{k}={}", urlencoding(&v)))
        .collect();
    format!("/chat/{chat}/lab/card?{}", encoded.join("&"))
}

fn urlencoding(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Whether the card is waiting on a render.
fn pending(card: &Value) -> bool {
    let job = card["job"]["state"].as_str();
    let waiting = matches!(job, Some("queued" | "running"));
    let compare_waiting = card["compare"].is_object()
        && (card["compare"]["a"]["video"].is_null() || card["compare"]["b"]["video"].is_null());
    (card["video"].is_null() && waiting) || compare_waiting
}

/// One video in the card.
fn video(
    chat: &str,
    path: &str,
    poster: Option<&str>,
    from: Option<f64>,
    to: Option<f64>,
) -> Markup {
    let fragment = match (from, to) {
        (Some(a), Some(b)) => format!("#t={a:.2},{b:.2}"),
        (Some(a), None) => format!("#t={a:.2}"),
        _ => String::new(),
    };
    let src = media(chat, path).map(|url| format!("{url}{fragment}"));
    let poster = poster.and_then(|p| media(chat, p));
    html! {
        @if let Some(src) = src {
            video.oa-lab-video controls preload="metadata" playsinline poster=[poster] src=(src) {}
        }
    }
}

/// The card for a Lab result: the video inline (or the compare pair), the
/// frame strip, the version, and the Lab player link.
pub(crate) fn card_markup(chat: &str, card: &Value) -> Markup {
    let waiting = pending(card);
    let version = card["version"].as_u64().unwrap_or(1);
    let from = card["from"].as_f64();
    let to = card["to"].as_f64();
    let image = card["image"].as_str().and_then(|p| media(chat, p));
    let compare = card["compare"].is_object().then_some(&card["compare"]);
    let state = card["job"]["state"].as_str().unwrap_or_default();
    let estimate = card["job"]["estimate"].as_f64();
    html! {
        div.oa-lab-card
            hx-get=[waiting.then(|| refresh_href(chat, card))]
            hx-trigger=[waiting.then_some("every 3s")]
            hx-swap=[waiting.then_some("outerHTML")] {
            link rel="stylesheet" href=(ASSET_CSS);
            script src=(ASSET_JS) defer {}
            p.oa-lab-title {
                strong { (s(card, "title")) " · v" (version) }
                " " span.oa-lab-dim { (s(card, "summary")) }
            }
            @if let Some(compare) = compare {
                div.oa-lab-pair {
                    @for side in ["a", "b"] {
                        div {
                            @if let Some(path) = compare[side]["video"].as_str() {
                                (video(chat, path, None, compare["at"].as_f64(), None))
                            } @else {
                                p.oa-lab-dim { "Rendering v" (compare[side]["version"].as_u64().unwrap_or(0)) "…" }
                            }
                            p.oa-lab-caption {
                                "v" (compare[side]["version"].as_u64().unwrap_or(0)) ": " (compare[side]["summary"].as_str().unwrap_or_default())
                            }
                        }
                    }
                }
                @if compare["a"]["video"].is_string() && compare["b"]["video"].is_string() {
                    button.oa-lab-button type="button" data-lab-play-both {
                        "Play both from " (clock(compare["at"].as_f64().unwrap_or(0.0)))
                    }
                }
            } @else if let Some(path) = card["video"].as_str() {
                (video(chat, path, card["poster"].as_str(), from, to))
            } @else if state == "failed" {
                p { "The render failed: " (s(&card["job"], "error")) }
            } @else if waiting {
                p.oa-lab-dim {
                    "Rendering the " (s(card, "quality")) "…"
                    @if let Some(seconds) = estimate {
                        " The last one took about " (((seconds / 60.0).round() as u64).max(1)) " min."
                    }
                }
            }
            @if let Some(image) = image {
                img.oa-lab-strip src=(image) alt="Frames from the video" loading="lazy";
            }
            p.oa-lab-meta {
                a href=(s(card, "player")) target="_blank" rel="noopener" { "Open in the Lab player" }
                @if let Some(versions) = card["versions"].as_array() {
                    " · " (versions.len()) " versions"
                }
                @if card["notes"].as_u64().unwrap_or(0) > 0 {
                    " · " (card["notes"].as_u64().unwrap_or(0)) " notes"
                }
            }
            @if card["video"].is_string() && compare.is_none() {
                form.oa-lab-note hx-post=(format!("/chat/{chat}/lab/note")) hx-swap="outerHTML" data-lab-note {
                    input type="hidden" name="doc" value=(s(card, "doc"));
                    input type="hidden" name="v" value=(version);
                    input type="hidden" name="q" value=(s(card, "quality"));
                    input type="hidden" name="t" value="0";
                    input type="text" name="text" placeholder="A note at this time" required;
                    button.oa-lab-button type="submit" { "Note at this time" }
                }
            }
        }
    }
}

/// Where a reply with a Lab card shows it.
pub(crate) fn thread_entry(chat: &str, reply: Option<&openagents_chat::router::Meta>) -> Markup {
    match reply.and_then(|meta| meta.cards.iter().find(|card| card["card"] == CARD)) {
        Some(card) => card_markup(chat, card),
        None => html! {},
    }
}

/// The card's look and its two behaviors (Play both; a note at the playing
/// time), as files: the chat page runs only its own origin's scripts and
/// styles.
const ASSET_CSS: &str = "/chat/lab/card.css";
pub(crate) const ASSET_JS: &str = "/chat/lab/card.js";

const CSS: &str = r".oa-lab-card{margin:12px 0;padding:12px;border:1px solid rgba(127,127,127,.28);border-radius:12px;max-width:960px}
.oa-lab-card video{width:100%;aspect-ratio:16/9;display:block;border-radius:8px;background:#000}
.oa-lab-title{margin:0 0 8px}.oa-lab-dim{opacity:.7}
.oa-lab-pair{display:grid;grid-template-columns:1fr 1fr;gap:8px}
.oa-lab-caption{margin:4px 0 0;opacity:.7;font-size:13px}
.oa-lab-strip{width:100%;margin-top:8px;border-radius:8px;display:block}
.oa-lab-meta{margin:8px 0 0;font-size:13px;opacity:.8}

.oa-lab-note{display:flex;gap:6px;margin-top:8px}
.oa-lab-note input[type=text]{flex:1;min-width:0;padding:6px 10px;border-radius:8px;border:1px solid rgba(127,127,127,.35);background:transparent;color:inherit;font:inherit}
.oa-lab-button{margin-top:8px;padding:6px 12px;border-radius:8px;border:1px solid rgba(127,127,127,.35);background:transparent;color:inherit;font:inherit}
.oa-lab-note .oa-lab-button{margin-top:0}
";

const JS: &str = r"(function () {
  if (window.__oaLab) return;
  window.__oaLab = 1;
  document.addEventListener('click', function (e) {
    var b = e.target.closest && e.target.closest('[data-lab-play-both]');
    if (!b) return;
    var vs = b.closest('.oa-lab-card').querySelectorAll('video');
    var t = vs[vs.length - 1].currentTime;
    vs.forEach(function (v) { v.currentTime = t; v.play(); });
  });
  document.addEventListener('htmx:configRequest', function (e) {
    var f = e.detail.elt;
    if (!f.matches || !f.matches('form[data-lab-note]')) return;
    var v = f.closest('.oa-lab-card').querySelector('video');
    e.detail.parameters.t = v ? v.currentTime : 0;
  });
})();
";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/{id}/lab/card", get(card))
        .route("/chat/{id}/lab/note", post(note))
        .route("/chat/{id}/lab/media/{*path}", get(media_file))
        .route(
            ASSET_CSS,
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    CSS,
                )
            }),
        )
        .route(
            ASSET_JS,
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    JS,
                )
            }),
        )
}

/// A render, poster or frame from the Lab, streamed through this site with
/// its byte ranges (so the player can seek).
async fn media_file(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, path)): Path<(String, String)>,
) -> Response {
    use axum::http::{StatusCode, header};
    if let Err(response) = crate::pages::chat::load(&app, &headers, &id).await {
        return response;
    }
    let Some(lab) = lab_url() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if path.split('/').any(|part| part == ".." || part.is_empty()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(http) = client(Duration::from_secs(3600)) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let mut request = http.get(format!("{lab}/media/{path}"));
    if let Some(range) = headers.get(header::RANGE) {
        request = request.header(header::RANGE, range.as_bytes());
    }
    let Ok(upstream) = request.send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = Response::builder().status(status);
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
        "x-robots-tag",
        "x-openagents-no-training",
    ] {
        if let Some(value) = upstream.headers().get(name) {
            response = response.header(name, value.as_bytes());
        }
    }
    response = response.header(header::CACHE_CONTROL, "private, max-age=3600");
    response
        .body(axum::body::Body::from_stream(upstream.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

/// The card again, as the Lab has it now (a render finishing).
async fn card(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<CardQuery>,
) -> Response {
    if let Err(response) = crate::pages::chat::load(&app, &headers, &id).await {
        return response;
    }
    let Some(lab) = lab_url() else {
        return Html("<p>Video Lab isn't running on this Mac.</p>").into_response();
    };
    let mut url = format!(
        "{lab}/api/card?doc={}&v={}&q={}",
        urlencoding(&query.doc),
        query.v,
        urlencoding(query.q.as_deref().unwrap_or("preview"))
    );
    for (key, value) in [("from", query.from), ("to", query.to), ("at", query.at)] {
        if let Some(value) = value {
            url.push_str(&format!("&{key}={value}"));
        }
    }
    if let Some(a) = query.a {
        url.push_str(&format!("&a={a}"));
    }
    let fetched = async {
        client(Duration::from_secs(10))
            .ok_or("no HTTP client".to_owned())?
            .get(&url)
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?
            .json::<Value>()
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    match fetched {
        Ok(mut fresh) if fresh["card"] == CARD => {
            if let Some(image) = query.image.filter(|i| i.starts_with("/media/")) {
                fresh["image"] = json!(image);
            }
            Html(card_markup(&id, &fresh).into_string()).into_response()
        }
        Ok(other) => Html(
            html! { p { (other["error"].as_str().unwrap_or("The Lab has no card for that.")) } }
                .into_string(),
        )
        .into_response(),
        Err(_) => Html("<p>Video Lab isn't answering on this Mac.</p>").into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct NoteForm {
    doc: String,
    v: u64,
    q: String,
    t: f64,
    text: String,
}

/// A note at a time on the card's video: the same notes the review page keeps.
async fn note(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<NoteForm>,
) -> Response {
    if let Err(response) = crate::pages::chat::load(&app, &headers, &id).await {
        return response;
    }
    let Some(lab) = lab_url() else {
        return Html("<p>Video Lab isn't running on this Mac.</p>").into_response();
    };
    let sent = async {
        client(Duration::from_secs(10))
            .ok_or("no HTTP client".to_owned())?
            .post(format!("{lab}/api/note"))
            .json(
                &json!({"doc": form.doc, "v": form.v, "q": form.q, "t": form.t, "text": form.text}),
            )
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?
            .json::<Value>()
            .await
            .map_err(|e| e.to_string())
    }
    .await;
    let line = match sent {
        Ok(result) if result["saved"] == true => format!("Note saved at {}.", clock(form.t)),
        Ok(result) => result["error"]
            .as_str()
            .unwrap_or("The note wasn't saved.")
            .to_owned(),
        Err(_) => "Video Lab isn't answering, so the note wasn't saved.".to_owned(),
    };
    Html(html! { p.oa-lab-meta { (line) } }.into_string()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({
            "card": "video_lab", "lab": "http://127.0.0.1:8792", "doc": "episode-290",
            "title": "Episode 290", "kind": "cut.v1", "version": 3, "summary": "Delegation plays through",
            "quality": "preview", "versions": [{"n": 1}, {"n": 2}, {"n": 3}],
            "video": "/media/films/episode-290/renders/abc-preview.mp4",
            "poster": "/media/films/episode-290/renders/abc-preview.jpg",
            "from": 80.0, "to": 95.0, "image": null, "private": ["uses Berkeley Mono"], "notes": 0,
            "player": "http://127.0.0.1:8792/player?doc=episode-290&v=3"
        })
    }

    #[test]
    fn a_rendered_card_plays_the_range_inline() {
        let html = card_markup("c1", &sample()).into_string();
        assert!(html.contains("<video"), "{html}");
        assert!(
            html.contains(
                "/chat/c1/lab/media/films/episode-290/renders/abc-preview.mp4#t=80.00,95.00"
            ),
            "{html}"
        );
        assert!(
            !html.contains("style=") && !html.contains("onclick"),
            "the page runs no inline style or script"
        );
        assert!(html.contains("Open in the Lab player"));
        assert!(html.contains("Note at this time"));
        assert!(!html.contains("hx-trigger"), "a finished card doesn't poll");
    }

    #[test]
    fn a_card_waiting_on_a_render_refreshes_itself() {
        let mut card = sample();
        card["video"] = Value::Null;
        card["job"] = json!({"state": "running", "estimate": 120.0});
        let html = card_markup("c1", &card).into_string();
        assert!(html.contains("every 3s"), "{html}");
        assert!(
            html.contains("/chat/c1/lab/card?doc=episode-290&amp;v=3"),
            "{html}"
        );
        assert!(html.contains("Rendering the preview"));
    }

    #[test]
    fn a_compare_card_shows_both_versions() {
        let mut card = sample();
        card["compare"] = json!({"at": 86.0,
            "a": {"version": 2, "summary": "Take 2", "video": "/media/a.mp4"},
            "b": {"version": 3, "summary": "Delegation plays through", "video": "/media/b.mp4"}});
        let html = card_markup("c1", &card).into_string();
        assert!(
            html.contains("/media/a.mp4#t=86.00") && html.contains("/media/b.mp4#t=86.00"),
            "{html}"
        );
        assert!(html.contains("Play both from 1:26"));
    }

    #[test]
    fn only_a_loopback_lab_is_used() {
        assert_eq!(loopback("https://lab.example.com"), None);
        assert_eq!(loopback("http://10.0.0.2:8792"), None);
        assert_eq!(
            loopback("http://127.0.0.1:8792/").as_deref(),
            Some("http://127.0.0.1:8792")
        );
    }

    #[test]
    fn only_lab_cards_are_kept_on_a_reply() {
        let meta = openagents_chat::router::Meta {
            cards: vec![json!({"card": "tool"}), sample()],
            ..Default::default()
        };
        assert_eq!(cards(&meta).len(), 1);
        assert_eq!(cards(&meta)[0]["doc"], "episode-290");
    }
}
