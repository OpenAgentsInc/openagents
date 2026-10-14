//! Game Lab in the web chat (docs/game-lab/README.md, docs/game-lab/mvp.md).
//!
//! When this server is told where a Game Lab runs (`OPENAGENTS_GAME_LAB`,
//! a loopback URL: the Lab keeps its versions on the Mac it runs on), its
//! tools (`game_open`, `game_play`, `game_edit`, `game_versions`,
//! `game_compare`, `game_notes`, `game_list`; the Lab serves their
//! declarations at `/api/tools`) join Video Lab's in one chat turn
//! ([`crate::labs`], #11619): the model's typed function choice is the
//! route. No words of the message are matched here.
//!
//! A tool's result carries a card (`"card": "game_lab"`): the game, the
//! version and its address fragment (the version's numbers and words, as
//! data on one build of the game), and its bot-run stats. The last card of
//! an answer is kept on the reply and drawn under it by [`thread_entry`]:
//! the game itself in a frame (`/chat/{id}/game/play`), a Play button,
//! Compare, and a note at the time and place in the run.

use std::time::Duration;

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::App;

/// How long one Lab call may take (an edit plays 24 bot runs).
const LAB_DEADLINE: Duration = Duration::from_secs(90);
/// The card kind a Game Lab tool returns.
pub(crate) const CARD: &str = "game_lab";
/// The canvas the game draws in (`icecream-web` looks it up by this id).
const CANVAS: &str = "icecream-canvas";

/// The Game Lab this server talks to, when one is configured: loopback only.
pub(crate) fn lab_url() -> Option<String> {
    loopback(&std::env::var("OPENAGENTS_GAME_LAB").ok()?)
}

fn loopback(url: &str) -> Option<String> {
    let url = url.trim().trim_end_matches('/').to_owned();
    (url.starts_with("http://127.0.0.1:") || url.starts_with("http://localhost:")).then_some(url)
}

fn client(deadline: Duration) -> Option<reqwest::Client> {
    reqwest::Client::builder().timeout(deadline).build().ok()
}

async fn get_json(url: &str, deadline: Duration) -> Result<Value, String> {
    client(deadline)
        .ok_or("no HTTP client")?
        .get(url)
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// The Lab's tool declarations and games, when it answers.
async fn manifest(lab: &str) -> Result<(Vec<Value>, Vec<Value>), String> {
    let tools = get_json(&format!("{lab}/api/tools"), Duration::from_secs(3)).await?;
    let docs = get_json(&format!("{lab}/api/docs"), Duration::from_secs(3)).await?;
    Ok((
        tools["tools"].as_array().cloned().unwrap_or_default(),
        docs["documents"].as_array().cloned().unwrap_or_default(),
    ))
}

/// One tool call on the Lab: its JSON result.
pub(crate) async fn run_tool(lab: &str, name: &str, arguments: &Value) -> Result<Value, String> {
    if !name.starts_with("game_")
        || !name[5..]
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_')
    {
        return Err(format!("{name} isn't a Game Lab tool"));
    }
    client(LAB_DEADLINE)
        .ok_or("no HTTP client")?
        .post(format!("{lab}/api/tool/{name}"))
        .json(arguments)
        .send()
        .await
        .map_err(|e| format!("Game Lab didn't answer ({})", e.without_url()))?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// The Game Lab on this Mac, when one is configured and answering: its
/// tools and games ([`crate::labs`] puts them in the chat's turn).
pub(crate) async fn manifest_here() -> Option<crate::labs::Lab> {
    let lab = lab_url()?;
    match manifest(&lab).await {
        Ok((tools, documents)) => Some(crate::labs::Lab {
            kind: crate::labs::Kind::Game,
            url: lab,
            tools,
            documents,
        }),
        Err(why) => {
            eprintln!("openagents-web: game lab: not answering at {lab}: {why}");
            None
        }
    }
}

/// What the model is told about Game Lab.
pub(crate) fn guidance(documents: &[Value]) -> String {
    let mut out = String::from(
        "\n\nThis chat can play and change the owner's games in Game Lab, on this Mac, with the game_* tools. \
When the message is about one of these games (open it, play it, change how it plays, its rules, items, the Ice Cream Man's \
behaviour, its layout or its words, compare versions, notes), use the game_* tools, always naming the game. The chat shows a card with the game under your answer, so don't paste links or fragments; say in a sentence or \
two, as \"I\", what you did and what changed. \
To change a game, read its document first (game_open), then call game_edit with a JSON Patch on the document and a \
one-line summary in plain words. Paths are /SECTION/NAME, e.g. /man/man_chase. Speeds are in man_wander, \
man_investigate, man_search and man_chase; hiding spots are closets, added with /layout/extra_closets. Some things are switched on by a number: the earned weapons \
(/weapons/earned_weapons 1, which needs minions), minions (/minions/minion_count), play after a win (/rules/free_roam 1), \
the Backrooms house (/layout/backrooms 1, /layout/loops) and its look (/look/yellow, fog_near, fog_far, flicker, buzz, \
first_person). Work numbers out \
from the document's current values. game_edit checks the edit with the game's rules and 24 seeded bot runs. If it \
refuses, tell the owner why in plain words and don't save a different change instead without asking. If a request needs \
the game's code changed rather than its settings (game_edit says so), say so and offer to have Coder make that change as a \
later step; when they agree, record it with game_propose_code (it records the request, it doesn't make it), and \
game_code_requests says where each request stands and links the commit that made it. When a request is made, its new \
settings are in the document: read it again with game_open before editing them. When asked to play, call game_play. To compare with the original, call game_compare with a=1; then say what \
the bot runs measured.\n\nGames:\n",
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

/// The Game Lab cards a reply keeps.
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
    let t = t.max(0.0).round() as u64;
    format!("{}:{:02}", t / 60, t % 60)
}

/// The fragment as the page's address carries it: only the characters
/// the Lab writes (its words come percent-encoded).
fn clean_fragment(fragment: &str) -> String {
    fragment
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || "=&._-~%".contains(*c))
        .collect()
}

/// Where a version plays: the game's page in this chat, with the
/// version's numbers and words after the `#`.
fn play_src(chat: &str, version: u64, fragment: &str, extra: &str) -> String {
    format!(
        "/chat/{chat}/game/play?v={version}#{}{extra}",
        clean_fragment(fragment)
    )
}

/// The bot runs, in a line.
fn stats_line(stats: &Value) -> String {
    let Some(runs) = stats["runs"].as_u64() else {
        return String::new();
    };
    let melted = stats["melted"].as_u64().unwrap_or(0);
    let mut line = if melted > 0 {
        format!(
            "Bot runs: {} of {runs} escaped, {melted} melted him ({}% won)",
            stats["escaped"].as_u64().unwrap_or(0),
            stats["win_rate"].as_u64().unwrap_or(0)
        )
    } else {
        format!(
            "Bot runs: {} of {runs} escaped ({}%)",
            stats["escaped"].as_u64().unwrap_or(0),
            stats["win_rate"].as_u64().unwrap_or(0)
        )
    };
    if let Some(t) = stats["median_escape"].as_f64() {
        line.push_str(&format!(" in {} on average", clock(t)));
    }
    line.push_str(&format!(
        " · {:.1} hits taken · spotted {:.1} times · {} hiding spots",
        stats["hits"].as_f64().unwrap_or(0.0),
        stats["spotted"].as_f64().unwrap_or(0.0),
        stats["hiding_spots"].as_u64().unwrap_or(0)
    ));
    line
}

fn refresh_href(chat: &str, card: &Value) -> String {
    let mut href = format!(
        "/chat/{chat}/game/card?doc={}&v={}",
        clean_fragment(s(card, "doc")),
        card["version"].as_u64().unwrap_or(1)
    );
    if let Some(a) = card["compare"]["a"]["version"].as_u64() {
        href.push_str(&format!("&a={a}"));
    }
    if card["autoplay"] == true {
        href.push_str("&play=1");
    }
    href
}

fn building(card: &Value) -> bool {
    card["build"] == "building"
}

/// The card for a Game Lab result: the game in a frame (or two versions
/// side by side), Play, the bot-run stats, Compare, and a note at a time
/// and place.
pub(crate) fn card_markup(chat: &str, card: &Value) -> Markup {
    let waiting = building(card);
    let version = card["version"].as_u64().unwrap_or(1);
    let compare = card["compare"].is_object().then_some(&card["compare"]);
    let failed = card["build"]["failed"].as_str();
    let doc = s(card, "doc");
    let autoplay = card["autoplay"] == true;
    html! {
        div.oa-game-card
            hx-get=[waiting.then(|| refresh_href(chat, card))]
            hx-trigger=[waiting.then_some("every 3s")]
            hx-swap=[waiting.then_some("outerHTML")] {
            link rel="stylesheet" href=(ASSET_CSS);
            script src=(ASSET_JS) defer {}
            p.oa-game-title {
                strong { (s(card, "title")) " · v" (version) }
                " " span.oa-game-dim { (s(card, "summary")) }
            }
            @if waiting {
                p.oa-game-dim { "Getting the game ready to play here. The first time takes a few minutes." }
            } @else if let Some(why) = failed {
                p { "The game couldn't be got ready to play: " (why) }
            } @else if let Some(compare) = compare {
                div.oa-game-pair {
                    @for side in ["a", "b"] {
                        @let n = compare[side]["version"].as_u64().unwrap_or(0);
                        div {
                            div.oa-game-frame.oa-game-small {
                                iframe src=(play_src(chat, n, compare[side]["fragment"].as_str().unwrap_or_default(), "&bot&speed=2"))
                                    title={ "Version " (n) ", the bot playing" } loading="lazy" {}
                            }
                            p.oa-game-caption { "v" (n) ": " (compare[side]["summary"].as_str().unwrap_or_default()) }
                            p.oa-game-caption { (stats_line(&compare[side]["stats"])) }
                        }
                    }
                }
                p.oa-game-dim { "The bot plays both on the same house. Measured over the same seeded runs:" }
                ul.oa-game-list {
                    @for line in compare["differences"].as_array().into_iter().flatten() {
                        li { (line.as_str().unwrap_or_default()) }
                    }
                }
                @if let Some(changes) = compare["changes"].as_array().filter(|c| !c.is_empty()) {
                    details.oa-game-changes {
                        summary { "What changed (" (changes.len()) ")" }
                        ul.oa-game-list {
                            @for c in changes {
                                li { (c[0].as_str().unwrap_or_default()) ": " (shown(&c[1])) " → " (shown(&c[2])) }
                            }
                        }
                    }
                }
            } @else {
                div.oa-game-frame {
                    iframe src=(play_src(chat, version, s(card, "fragment"), if autoplay { "&skip" } else { "" }))
                        title={ (s(card, "title")) ", version " (version) } data-game-frame allow="fullscreen" {}
                }
                div.oa-game-actions {
                    button.oa-game-button type="button" data-game-play
                        data-src=(play_src(chat, version, s(card, "fragment"), "&skip")) { "Play" }
                    @if version > 1 {
                        button.oa-game-button type="button"
                            hx-get=(format!("/chat/{chat}/game/card?doc={}&v={version}&a=1", clean_fragment(doc)))
                            hx-target="closest .oa-game-card" hx-swap="outerHTML" { "Compare with the original" }
                    }
                }
                p.oa-game-meta { (stats_line(&card["stats"])) }
                form.oa-game-note hx-post=(format!("/chat/{chat}/game/note")) hx-swap="outerHTML" data-game-note {
                    input type="hidden" name="doc" value=(doc);
                    input type="hidden" name="v" value=(version);
                    input type="hidden" name="seconds" value="";
                    input type="hidden" name="room" value="";
                    input type="text" name="text" placeholder="A note at this time and place in the run" required;
                    button.oa-game-button type="submit" { "Note at this time and place" }
                }
            }
            p.oa-game-meta {
                @if let Some(player) = card["player"].as_str().filter(|p| loopback(p).is_some()) {
                    a href=(player) target="_blank" rel="noopener" { "Play in its own window" }
                    " · "
                }
                @if let Some(versions) = card["versions"].as_array() {
                    (versions.len()) @if versions.len() == 1 { " version" } @else { " versions" }
                }
                @if card["notes"].as_u64().unwrap_or(0) > 0 {
                    " · " (card["notes"].as_u64().unwrap_or(0)) " notes"
                }
            }
        }
    }
}

fn shown(value: &Value) -> String {
    match value {
        Value::String(s) => format!("\"{s}\""),
        Value::Null => "nothing".into(),
        other => other.to_string(),
    }
}

/// Where a reply with a Game Lab card shows it.
pub(crate) fn thread_entry(chat: &str, reply: Option<&openagents_chat::router::Meta>) -> Markup {
    match reply.and_then(|meta| meta.cards.iter().find(|card| card["card"] == CARD)) {
        Some(card) => card_markup(chat, card),
        None => html! {},
    }
}

const ASSET_CSS: &str = "/chat/game/card.css";
pub(crate) const ASSET_JS: &str = "/chat/game/card.js";

const CSS: &str = r".oa-game-card{margin:12px 0;padding:12px;border:1px solid rgba(127,127,127,.28);border-radius:12px;max-width:960px}
.oa-game-frame{position:relative;width:100%;aspect-ratio:16/10;border-radius:8px;overflow:hidden;background:#000}
.oa-game-frame iframe{position:absolute;inset:0;width:100%;height:100%;border:0;display:block}
.oa-game-title{margin:0 0 8px}.oa-game-dim{opacity:.7}
.oa-game-pair{display:grid;grid-template-columns:1fr 1fr;gap:8px}
.oa-game-caption{margin:4px 0 0;opacity:.75;font-size:13px}
.oa-game-actions{display:flex;gap:6px;flex-wrap:wrap}
.oa-game-meta{margin:8px 0 0;font-size:13px;opacity:.8}
.oa-game-list{margin:6px 0 0;padding-left:20px;font-size:13px}
.oa-game-changes{margin-top:6px;font-size:13px}
.oa-game-note{display:flex;gap:6px;margin-top:8px}
.oa-game-note input[type=text]{flex:1;min-width:0;padding:6px 10px;border-radius:8px;border:1px solid rgba(127,127,127,.35);background:transparent;color:inherit;font:inherit}
.oa-game-button{margin-top:8px;padding:6px 12px;border-radius:8px;border:1px solid rgba(127,127,127,.35);background:transparent;color:inherit;font:inherit}
.oa-game-note .oa-game-button{margin-top:0}
";

const JS: &str = r"(function () {
  if (window.__oaGame) return;
  window.__oaGame = 1;
  var plays = 0;
  document.addEventListener('click', function (e) {
    var b = e.target.closest && e.target.closest('[data-game-play]');
    if (!b) return;
    var f = b.closest('.oa-game-card').querySelector('iframe[data-game-frame]');
    if (!f) return;
    plays += 1;
    var src = b.getAttribute('data-src');
    var at = src.indexOf('#');
    f.src = src.slice(0, at) + '&play=' + plays + src.slice(at);
    f.focus();
  });
  document.addEventListener('htmx:configRequest', function (e) {
    var form = e.detail.elt;
    if (!form.matches || !form.matches('form[data-game-note]')) return;
    var f = form.closest('.oa-game-card').querySelector('iframe[data-game-frame]');
    var c = null;
    try { c = f && f.contentDocument && f.contentDocument.getElementById('icecream-canvas'); } catch (x) {}
    if (!c) return;
    e.detail.parameters.seconds = c.getAttribute('data-seconds') || '';
    e.detail.parameters.room = c.getAttribute('data-room') || '';
  });
})();
";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/chat/{id}/game/card", get(card))
        .route("/chat/{id}/game/note", post(note))
        .route("/chat/{id}/game/play", get(play))
        .route("/chat/game/build/{file}", get(build_file))
        .route(
            ASSET_CSS,
            get(|| async { ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], CSS) }),
        )
        .route(
            ASSET_JS,
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    JS,
                )
            }),
        )
}

/// The game page a card frames: The Ice Cream Man's page, its build from
/// the Lab, framed only by this site.
pub(crate) const PLAY_POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; \
img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; \
base-uri 'none'; form-action 'self'; frame-ancestors 'self'";

fn play_body() -> String {
    format!(
        "<h1 class=\"unseen\">The Ice Cream Man</h1>\
<div class=\"glade\" id=\"icecream\">\
<canvas id=\"{CANVAS}\" tabindex=\"0\" aria-label=\"The Airbnb, where the Ice Cream Man is\"></canvas>\
<p class=\"glade-status\" id=\"icecream-status\" aria-live=\"polite\">Loading\u{2026}</p>\
<noscript><p class=\"glade-status\">Turn on JavaScript to play.</p></noscript></div>\
<script type=\"module\" src=\"/chat/game/build/start.js\"></script>"
    )
}

async fn play(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    if let Err(response) = crate::pages::chat::load(&app, &headers, &id).await {
        return response;
    }
    if lab_url().is_none() {
        return Html("<p>Game Lab isn't running on this Mac.</p>").into_response();
    }
    let mut response = crate::layout::fullscreen("The Ice Cream Man", &play_body());
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(PLAY_POLICY),
    );
    headers.insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
    response
}

/// The game's build, from the Lab (versions are data on one build).
async fn build_file(Path(file): Path<String>) -> Response {
    let Some(lab) = lab_url() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if !matches!(
        file.as_str(),
        "icecream_web.js" | "icecream_web_bg.wasm" | "start.js"
    ) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Some(http) = client(Duration::from_secs(60)) else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let Ok(upstream) = http.get(format!("{lab}/build/{file}")).send().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let kind = upstream
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or(HeaderValue::from_static("application/octet-stream"));
    let Ok(bytes) = upstream.bytes().await else {
        return StatusCode::BAD_GATEWAY.into_response();
    };
    (
        status,
        [
            (header::CONTENT_TYPE, kind),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        bytes,
    )
        .into_response()
}

#[derive(Deserialize, Debug)]
pub(crate) struct CardQuery {
    doc: String,
    v: u64,
    #[serde(default)]
    a: Option<u64>,
    #[serde(default)]
    play: Option<u8>,
}

/// The card again, as the Lab has it now (the build finishing, or
/// Compare).
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
        return Html("<p>Game Lab isn't running on this Mac.</p>").into_response();
    };
    let mut url = format!(
        "{lab}/api/card?doc={}&v={}",
        clean_fragment(&query.doc),
        query.v
    );
    if let Some(a) = query.a {
        url.push_str(&format!("&a={a}"));
    }
    match get_json(&url, Duration::from_secs(10)).await {
        Ok(mut fresh) if fresh["card"] == CARD => {
            if query.play == Some(1) {
                fresh["autoplay"] = json!(true);
            }
            Html(card_markup(&id, &fresh).into_string()).into_response()
        }
        Ok(other) => Html(
            html! { p { (other["error"].as_str().unwrap_or("Game Lab has no card for that.")) } }
                .into_string(),
        )
        .into_response(),
        Err(_) => Html("<p>Game Lab isn't answering on this Mac.</p>").into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct NoteForm {
    doc: String,
    v: u64,
    #[serde(default)]
    seconds: String,
    #[serde(default)]
    room: String,
    text: String,
}

/// A note at the time and place the run in the card is at.
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
        return Html("<p>Game Lab isn't running on this Mac.</p>").into_response();
    };
    let seconds = form.seconds.trim().parse::<f64>().ok();
    let room = form.room.trim().to_owned();
    let sent = async {
        client(Duration::from_secs(10))
            .ok_or("no HTTP client".to_owned())?
            .post(format!("{lab}/api/note"))
            .json(
                &json!({"doc": form.doc, "version": form.v, "seconds": seconds,
                          "room": room, "text": form.text}),
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
        Ok(result) if result["saved"] == true => {
            let place = match (seconds, room.is_empty()) {
                (Some(t), false) => format!(" at {} in the {}", clock(t), room.to_lowercase()),
                (Some(t), true) => format!(" at {}", clock(t)),
                (None, false) => format!(" in the {}", room.to_lowercase()),
                (None, true) => String::new(),
            };
            format!("Note saved{place}.")
        }
        Ok(result) => result["error"]
            .as_str()
            .unwrap_or("The note wasn't saved.")
            .to_owned(),
        Err(_) => "Game Lab isn't answering, so the note wasn't saved.".to_owned(),
    };
    Html(html! { p.oa-game-meta { (line) } }.into_string()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        json!({
            "card": "game_lab", "doc": "the-ice-cream-man", "title": "The Ice Cream Man",
            "version": 2, "summary": "He's 20% slower; two more closets",
            "fragment": "seed=7&t.man_chase=4.48&t.extra_closets=2",
            "stats": {"runs": 24, "escaped": 15, "win_rate": 63, "median_escape": 150.0,
                      "hits": 3.2, "spotted": 4.0, "hiding_spots": 7},
            "versions": [{"n": 1}, {"n": 2}], "notes": 0, "build": "ready"
        })
    }

    #[test]
    fn a_card_plays_the_version_in_a_frame() {
        let html = card_markup("c1", &sample()).into_string();
        assert!(
            html.contains(
                "src=\"/chat/c1/game/play?v=2#seed=7&amp;t.man_chase=4.48&amp;t.extra_closets=2\""
            ),
            "{html}"
        );
        assert!(html.contains(">Play<"));
        assert!(html.contains("Compare with the original"));
        assert!(html.contains("Note at this time and place"));
        assert!(html.contains("15 of 24 escaped (63%) in 2:30"));
        assert!(!html.contains("style=") && !html.contains("onclick"));
        assert!(!html.contains("hx-trigger"), "a ready card doesn't poll");
    }

    #[test]
    fn a_card_waits_for_the_build_and_compares_two_versions() {
        let mut card = sample();
        card["build"] = json!("building");
        let html = card_markup("c1", &card).into_string();
        assert!(html.contains("every 3s") && html.contains("Getting the game ready"));
        let mut card = sample();
        card["compare"] = json!({
            "a": {"version": 1, "summary": "The original", "fragment": "seed=7", "stats": card["stats"].clone()},
            "b": {"version": 2, "summary": "Slower", "fragment": "seed=7&t.man_chase=4.48", "stats": card["stats"].clone()},
            "changes": [["man.man_chase", 5.6, 4.48]],
            "differences": ["Hiding spots: 5 → 7"],
        });
        let html = card_markup("c1", &card).into_string();
        assert_eq!(html.matches("<iframe").count(), 2, "{html}");
        assert!(html.contains("&amp;bot&amp;speed=2"));
        assert!(html.contains("Hiding spots: 5 → 7"));
        assert!(html.contains("man.man_chase: 5.6 → 4.48"));
    }

    #[test]
    fn fragments_carry_only_address_characters() {
        assert_eq!(
            clean_fragment("seed=7&c.title=A%20B\"><script>"),
            "seed=7&c.title=A%20Bscript"
        );
        assert_eq!(loopback("https://lab.example.com"), None);
    }

    #[test]
    fn the_play_page_runs_only_the_lab_build_and_is_framed_only_here() {
        let html = play_body();
        assert_eq!(html.matches("<script").count(), 1);
        assert!(html.contains("src=\"/chat/game/build/start.js\""));
        assert!(PLAY_POLICY.contains("frame-ancestors 'self'"));
    }
}
