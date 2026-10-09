//! Public synthetic conversations rendered from the shared Rust fixtures.
//!
//! The HTTP and SSE readers expose the same original records. A preview
//! submission creates presentation markup only and cannot execute a command.

use std::{borrow::Cow, convert::Infallible, time::Duration};

use axum::{
    Form, Router,
    extract::{DefaultBodyLimit, Path, Query},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use coder_ui::demo::{
    DemoState, Key, KeyCode,
    agents::{DEMOS, DemoMessage, MAIN_PLUGINS, MAIN_TOOLS},
    models, onboarding, plugin_definition,
    tools::{ToolKind, ToolState},
};
use futures_util::stream;
use maud::{DOCTYPE, Markup, html};
use serde::Deserialize;

use crate::App;

const POLICY: &str = "default-src 'none'; style-src 'self'; font-src 'self'; img-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'";
const MAX_PROMPT_CHARS: usize = 4_000;
const CHATS: [(usize, &str, &str); 6] = [
    (5, "Set up OpenAgents", "Environment onboarding · saved v1"),
    (0, "Coder workspace", "Four conversations in parallel"),
    (
        1,
        "Keyboard navigation",
        "Draft editing and cursor restoration",
    ),
    (2, "Agent rail layout", "Wide and narrow layouts"),
    (
        3,
        "Conversation switching",
        "Independent drafts and history",
    ),
    (4, "Preview colors", "Shared Coder Noir palette"),
];

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/demo", get(index))
        .route("/demo/message", post(message))
        .route("/demo/{chat}", get(show))
        .route("/demo/{chat}/workspace", get(workspace))
        .route("/demo/{chat}/replay", get(replay))
        .route("/demo/{chat}/events", get(events))
        .route("/demo/{chat}/output/{sequence}", get(output))
        .route("/demo/{chat}/export", get(export))
        .layer(DefaultBodyLimit::max(32 * 1024))
}

fn response(markup: Markup) -> Response {
    let mut response = Html(markup.into_string()).into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(POLICY),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn index() -> Response {
    page(5)
}

async fn show(Path(chat): Path<usize>) -> Response {
    if chat >= CHATS.len() {
        return StatusCode::NOT_FOUND.into_response();
    }
    page(chat)
}

fn title(chat: usize) -> &'static str {
    CHATS.iter().find(|row| row.0 == chat).unwrap().1
}

fn detail(chat: usize) -> &'static str {
    CHATS.iter().find(|row| row.0 == chat).unwrap().2
}

fn page(chat: usize) -> Response {
    response(html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                meta name="color-scheme" content="dark";
                meta name="htmx-config" content=r#"{"allowEval":false,"allowScriptTags":false,"historyCacheSize":0,"historyRestoreAsHxRequest":false,"refreshOnHistoryMiss":true,"selfRequestsOnly":true,"includeIndicatorStyles":false,"timeout":20000}"#;
                title { (title(chat)) " · OpenAgents demo" }
                link rel="icon" href="/favicon.svg";
                link rel="stylesheet" href="/static/legacy-demo.css";
                link rel="stylesheet" href="/static/demo-html.css";
                script src="/static/htmx.min.js" defer {}
                script src="/static/htmx-sse.js" defer {}
                script src="/static/chat-start.js" type="module" {}
            }
            body hx-history="false" {
                a class="skip" href="#chat-thread" { "Skip to conversation" }
                div id="demo-root" {
                    (sidebar(chat, false))
                    main id="demo-workspace" aria-label="Demo conversation" {
                        (content(chat))
                        (composer(chat))
                    }
                }
            }
        }
    })
}

fn sidebar(chat: usize, oob: bool) -> Markup {
    html! {
        aside id="demo-sidebar" aria-label="Chat sidebar" hx-swap-oob=[oob.then_some("outerHTML")] {
            a class="demo-brand" href="/" { "OpenAgents" span { "Workspace" } }
            nav aria-label="Chats" {
                h2 { "Chats" }
                @for (index, title, detail) in CHATS {
                    a id=(format!("demo-chat-{index}")) class="demo-chat"
                        href=(format!("/demo/{index}"))
                        hx-get=(format!("/demo/{index}/workspace"))
                        hx-push-url=(format!("/demo/{index}"))
                        hx-target="#demo-content" hx-swap="outerHTML"
                        hx-sync="#demo-workspace:replace"
                        data-demo-chat=(index)
                        aria-current=[(index == chat).then_some("page")] {
                        span class="demo-chat-title" { (title) }
                        span class="demo-chat-detail" { (detail) }
                    }
                }
            }
            section class="demo-flow" aria-label="Onboarding stages" {
                h2 { "Environment setup" }
                ol {
                    li { "Discover the repository" }
                    li { "Install and repair" }
                    li { "Build a clean image" }
                    li { "Verify a fresh machine" }
                    li { "Review and save" }
                    li { "Run the first task" }
                }
            }
            p class="demo-disclosure" { "Demo conversations" br; "Machines and results are simulated." }
            a class="demo-catalog" href="/components" { "Component catalog" }
        }
    }
}

fn content(chat: usize) -> Markup {
    html! {
        section id="demo-content" class="demo-content" data-demo-selected=(chat) {
            header id="demo-header" {
                div {
                    p class="demo-eyebrow" { "OpenAgents / Engineering " span class="demo-badge" { "Demo" } }
                    h1 id="demo-chat-heading" { (title(chat)) }
                    p id="demo-chat-description" {
                        @if chat == 5 {
                            "From repository discovery to the first task on a saved environment."
                        } @else {
                            (detail(chat))
                        }
                    }
                }
                nav class="demo-history-controls" aria-label="Conversation navigation" {
                    a id="demo-history-start" href="#demo-beginning" data-chat-history="start" { "Beginning" }
                    a id="demo-history-end" href="#demo-latest" data-chat-history="end" { "Latest" }
                    a href=(format!("/demo/{chat}"))
                        hx-get=(format!("/demo/{chat}/replay"))
                        hx-target="#demo-transcript" hx-swap="outerHTML"
                        hx-sync="#demo-workspace:replace" { "Replay" }
                    a href=(format!("/demo/{chat}/export")) download { "Export" }
                }
            }
            div id="chat-thread" tabindex="0" aria-label="Conversation history" {
                div id="demo-beginning" {}
                div id="demo-transcript" {
                    @for (sequence, record) in records(chat).iter().enumerate() {
                        (block(chat, sequence, record))
                    }
                }
                div id="demo-latest" {}
            }
        }
    }
}

fn composer(chat: usize) -> Markup {
    html! {
        section class="demo-dock" aria-label="Demo composer" {
            form id="chat-form" action="/demo/message" method="post"
                hx-post="/demo/message" hx-target="#demo-transcript" hx-swap="beforeend"
                hx-sync="#demo-workspace:drop" data-chat-local="demo" {
                input id="demo-selected" name="chat" type="hidden" value=(chat);
                div id="chat-card" class="demo-compose-card" {
                    label class="unseen" for="chat-input" { "Message the demo" }
                    textarea id="chat-input" name="q" rows="2" maxlength=(MAX_PROMPT_CHARS)
                        required placeholder="Try a message, /plugins, /models, or /export" {}
                    div class="demo-compose-controls" {
                        details id="demo-plugins" {
                            summary { "Plugins" }
                            div class="demo-plugin-list" {
                                p { "Synthetic settings. No credentials or services are connected." }
                                @for plugin in plugin_definition::DEFINITIONS {
                                    label {
                                        input type="checkbox" value=(plugin.id) checked[plugin.default_enabled];
                                        span { (plugin.name) small { (plugin.description) } }
                                    }
                                }
                            }
                        }
                        label class="demo-model-label" for="demo-model" { "Model" }
                        select id="demo-model" name="model" aria-label="Synthetic model" {
                            option value="demo/local" { "Auto · demo" }
                            @for model in models::openrouter_catalog() {
                                option value=(model.id) { (model.name) }
                            }
                        }
                        button type="submit" { "Send ↑" }
                    }
                }
            }
            p id="demo-status" role="status" { "Synthetic preview · server-rendered Rust · no agent is connected" }
        }
    }
}

async fn workspace(Path(chat): Path<usize>) -> Response {
    if chat >= CHATS.len() {
        return StatusCode::NOT_FOUND.into_response();
    }
    response(html! { title {(title(chat)) " · OpenAgents demo"}
        (content(chat))
        (sidebar(chat, true))
        input id="demo-selected" name="chat" type="hidden" value=(chat) hx-swap-oob="outerHTML";
    })
}

enum Record {
    Said {
        user: bool,
        text: &'static str,
    },
    Call {
        name: String,
        input: Cow<'static, str>,
        output: Cow<'static, str>,
        state: ToolState,
    },
}

fn records(chat: usize) -> Vec<Record> {
    let mut result = Vec::new();
    if chat == 0 {
        result.push(Record::Said {
            user: true,
            text: "Review the terminal with four agents.",
        });
        for call in MAIN_TOOLS {
            result.push(Record::Call {
                name: tool_name(call.kind).into(),
                input: call.input.into(),
                output: call.output.into(),
                state: call.state,
            });
        }
        for call in MAIN_PLUGINS {
            result.push(Record::Call {
                name: format!("{}.{}", call.plugin, call.operation),
                input: call.input.into(),
                output: call.output.into(),
                state: call.state,
            });
        }
        for agent in &DEMOS {
            result.push(Record::Call {
                name: "delegate".into(),
                input: serde_json::json!({"agent":agent.name,"task":agent.task})
                    .to_string()
                    .into(),
                output: "null".into(),
                state: ToolState::Running,
            });
        }
        return result;
    }
    let agent = if chat == 5 {
        &onboarding::DEMO
    } else {
        &DEMOS[chat - 1]
    };
    for message in agent.conversation {
        result.push(match message {
            DemoMessage::User(text) => Record::Said { user: true, text },
            DemoMessage::Assistant(text) => Record::Said { user: false, text },
            DemoMessage::Tool(call) => Record::Call {
                name: tool_name(call.kind).into(),
                input: call.input.into(),
                output: call.output.into(),
                state: call.state,
            },
            DemoMessage::Plugin(call) => Record::Call {
                name: format!("{}.{}", call.plugin, call.operation),
                input: call.input.into(),
                output: call.output.into(),
                state: call.state,
            },
        });
    }
    result
}

fn tool_name(kind: ToolKind) -> &'static str {
    match kind {
        ToolKind::Read => "Read",
        ToolKind::Search => "Search",
        ToolKind::Edit => "Edit",
        ToolKind::Run => "Run",
    }
}

fn block(chat: usize, sequence: usize, record: &Record) -> Markup {
    let id = format!("demo-{chat}-record-{sequence}");
    html! {
        @match record {
            Record::Said { user, text } => {
                article id=(id) class=(if *user { "demo-message demo-user" } else { "demo-message demo-assistant" }) {
                    h2 { @if *user { "You" } @else { "OpenAgents" } }
                    p { (text) }
                }
            }
            Record::Call { name, input, output, state } => {
                details id=(id) class="demo-tool" {
                    summary {
                        span class="demo-tool-name" { (name) }
                        span class="demo-tool-state" {
                            @match state {
                                ToolState::Complete => { "Complete" }
                                ToolState::Running => { "Running · fixture" }
                                ToolState::Failed => { "Failed · retained" }
                            }
                        }
                    }
                    div class="demo-tool-body" {
                        h3 { "Input" }
                        pre { code { (input.as_ref()) } }
                        h3 { "Output" }
                        pre { code { (output.as_ref()) } }
                        a href=(format!("/demo/{chat}/output/{sequence}")) target="_blank" rel="noopener" { "Original output" }
                        " · "
                        a href=(format!("/demo/{chat}/output/{sequence}?part=input")) target="_blank" rel="noopener" { "Original input" }
                    }
                }
            }
        }
    }
}

async fn replay(Path(chat): Path<usize>) -> Response {
    if chat >= CHATS.len() {
        return StatusCode::NOT_FOUND.into_response();
    }
    response(html! {
        div id="demo-transcript" hx-ext="sse"
            sse-connect=(format!("/demo/{chat}/events?after=0"))
            sse-swap="message" sse-close="done" hx-swap="beforeend" {
            p class="demo-replay-status" sse-swap="done" hx-swap="outerHTML" { "Replaying the synthetic conversation…" }
        }
    })
}

#[derive(Default, Deserialize)]
struct Cursor {
    #[serde(default)]
    after: usize,
}

async fn events(
    Path(chat): Path<usize>,
    Query(cursor): Query<Cursor>,
    headers: HeaderMap,
) -> Response {
    if chat >= CHATS.len() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let total = records(chat).len();
    let mut after = cursor.after;
    if let Some(value) = headers.get("last-event-id") {
        let Some((selected, sequence)) = value.to_str().ok().and_then(|s| s.split_once(':')) else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        if selected.parse::<usize>().ok() != Some(chat) {
            return StatusCode::CONFLICT.into_response();
        }
        let Ok(sequence) = sequence.parse::<usize>() else {
            return StatusCode::BAD_REQUEST.into_response();
        };
        after = sequence;
    }
    if after > total {
        return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    }
    let stream = stream::unfold((chat, after, false), |(chat, after, ended)| async move {
        if ended {
            return None;
        }
        let records = records(chat);
        if after == records.len() {
            return Some((
                Ok::<_, Infallible>(Event::default().event("done").id(format!("{chat}:{after}")).data(
                    html! { p class="demo-replay-status" { "Replay complete · every original fixture record is available." } }.into_string(),
                )),
                (chat, after, true),
            ));
        }
        tokio::time::sleep(Duration::from_millis(180)).await;
        Some((
            Ok::<_, Infallible>(
                Event::default()
                    .event("message")
                    .id(format!("{chat}:{}", after + 1))
                    .data(block(chat, after, &records[after]).into_string()),
            ),
            (chat, after + 1, false),
        ))
    });
    let mut response = Sse::new(stream).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response
}

#[derive(Deserialize)]
struct Prompt {
    chat: usize,
    q: String,
    #[serde(default)]
    model: String,
}

async fn message(headers: HeaderMap, Form(prompt): Form<Prompt>) -> Response {
    let text = prompt.q.trim();
    if prompt.chat >= CHATS.len() || text.is_empty() || text.chars().count() > MAX_PROMPT_CHARS {
        return StatusCode::BAD_REQUEST.into_response();
    }
    let markup = html! {
        article class="demo-message demo-user" { h2 { "You" } p { (text) } }
        article class="demo-message demo-assistant" {
            h2 { "OpenAgents" }
            @match text {
                "/export" => {
                    p { "Download the complete original fixture, including every tool input and output: "
                        a href=(format!("/demo/{}/export", prompt.chat)) download { "ATIF conversation" } "."
                    }
                }
                "/plugins" => {
                    p { "The native Plugins control below lets you explore synthetic selections." }
                    ul {
                        @for plugin in plugin_definition::DEFINITIONS {
                            li { (plugin.name) " — " (plugin.description) }
                        }
                    }
                }
                "/models" => {
                    p { "The Model control below contains the shared Rust model fixture. Choosing a model makes no provider request." }
                }
                _ => {
                    p { "Preview message added. No agent is connected, and this text is not saved." }
                    @if prompt.model == "demo/local" || models::SHORTLIST.contains(&prompt.model.as_str()) {
                        p class="demo-meta" { "Synthetic model: " (prompt.model) }
                    }
                }
            }
        }
    };
    if headers.get("hx-request").is_some_and(|h| h == "true") {
        response(markup)
    } else {
        // A plain form submission stays readable when HTMX is unavailable.
        response(html! {
            (DOCTYPE)
            html lang="en" {
                head { title { "Demo message · OpenAgents" } link rel="stylesheet" href="/static/legacy-demo.css"; }
                body { div class="scroller" { main { (markup) a href=(format!("/demo/{}", prompt.chat)) { "Back to the demo" } } } }
            }
        })
    }
}

#[derive(Default, Deserialize)]
struct Output {
    part: Option<String>,
}

async fn output(
    Path((chat, sequence)): Path<(usize, usize)>,
    Query(query): Query<Output>,
) -> Response {
    if chat >= CHATS.len() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let records = records(chat);
    let Some(Record::Call { input, output, .. }) = records.get(sequence) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let text = match query.part.as_deref() {
        Some("input") => input.as_ref().to_owned(),
        None | Some("output") => output.as_ref().to_owned(),
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let mut response =
        ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], text).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

async fn export(Path(chat): Path<usize>) -> Response {
    if chat >= CHATS.len() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut state = DemoState::default();
    if chat == 5 {
        state.select_onboarding();
    } else if chat > 0 {
        state.select_agent(Some(chat - 1));
    }
    state.paste("/export");
    state.key(Key::new(KeyCode::Enter));
    let Some(download) = state.take_download() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    (
        [
            (header::CONTENT_TYPE, "application/json; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"coder-demo.atif.json\"",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        download.bytes,
    )
        .into_response()
}
