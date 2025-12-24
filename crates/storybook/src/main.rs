//! Storybook: Visual component explorer for OpenAgents UI
//!
//! Run with hot-reload:
//! ```bash
//! systemfd --no-pid -s http::3030 -- cargo watch -x 'run -p storybook'
//! ```
//!
//! Or simple (no hot-reload):
//! ```bash
//! cargo storybook
//! ```
//!
//! Configure port with STORYBOOK_PORT environment variable:
//! ```bash
//! STORYBOOK_PORT=8080 cargo storybook
//! ```

mod stories;

use actix_web::{App, HttpResponse, HttpServer, Responder, rt, web};
use actix_ws::Message;
use listenfd::ListenFd;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use std::time::Duration;

use stories::organisms::recorder::atoms::attempt_badge::attempt_badge_story;
use stories::organisms::recorder::atoms::blob_ref::blob_ref_story;
use stories::organisms::recorder::atoms::call_id_badge::call_id_badge_story;
use stories::organisms::recorder::atoms::cost_badge::cost_badge_story;
use stories::organisms::recorder::atoms::index::atoms_index_story;
use stories::organisms::recorder::atoms::latency_badge::latency_badge_story;
use stories::organisms::recorder::atoms::line_type_label::line_type_label_story;
use stories::organisms::recorder::atoms::redacted_value::redacted_value_story;
use stories::organisms::recorder::atoms::result_arrow::result_arrow_story;
use stories::organisms::recorder::atoms::status_dot::status_dot_story;
use stories::organisms::recorder::atoms::step_badge::step_badge_story;
use stories::organisms::recorder::atoms::tid_badge::tid_badge_story;
use stories::organisms::recorder::atoms::timestamp_badge::timestamp_badge_story;
use stories::organisms::recorder::atoms::token_badge::token_badge_story;
use stories::organisms::recorder::demo::recorder_demo_story;
use stories::organisms::recorder::index::recorder_index_story;
use stories::organisms::recorder::molecules::recorder_molecules_story;
use stories::organisms::recorder::organisms::recorder_organisms_story;
use stories::organisms::recorder::sections::recorder_sections_story;
use stories::atoms::base_document::base_document_story;
use stories::atoms::button::button_story;
use stories::atoms::claude_status::claude_status_story;
// ACP stories
use stories::acp::index::acp_index_story;
use stories::acp::demo::acp_demo_story;
use stories::acp::atoms::atoms_index_story as acp_atoms_index_story;
use stories::acp::atoms::tool_icon::tool_icon_story;
use stories::acp::atoms::tool_status_badge::tool_status_badge_story;
use stories::acp::atoms::permission_button::permission_button_story;
use stories::acp::atoms::mode_badge::mode_badge_story;
use stories::acp::atoms::model_badge::model_badge_story;
use stories::acp::atoms::thinking_toggle::thinking_toggle_story;
use stories::acp::atoms::checkpoint_badge::checkpoint_badge_story;
use stories::acp::atoms::feedback_button::feedback_button_story;
use stories::acp::atoms::content_type_icon::content_type_icon_story;
use stories::acp::atoms::entry_marker::entry_marker_story;
use stories::acp::atoms::keybinding_hint::keybinding_hint_story;
use stories::acp::atoms::streaming_indicator::streaming_indicator_story;
use stories::acp::molecules::molecules_index_story as acp_molecules_index_story;
use stories::acp::molecules::tool_header::tool_header_story as acp_molecule_tool_header_story;
use stories::acp::molecules::permission_bar::permission_bar_story as acp_molecule_permission_bar_story;
use stories::acp::molecules::mode_selector::mode_selector_story as acp_molecule_mode_selector_story;
use stories::acp::molecules::model_selector::model_selector_story as acp_molecule_model_selector_story;
use stories::acp::molecules::message_header::message_header_story as acp_molecule_message_header_story;
use stories::acp::molecules::thinking_block::thinking_block_story as acp_molecule_thinking_block_story;
use stories::acp::molecules::diff_header::diff_header_story as acp_molecule_diff_header_story;
use stories::acp::molecules::terminal_header::terminal_header_story as acp_molecule_terminal_header_story;
use stories::acp::molecules::checkpoint_restore::checkpoint_restore_story as acp_molecule_checkpoint_restore_story;
use stories::acp::molecules::entry_actions::entry_actions_story as acp_molecule_entry_actions_story;
use stories::acp::organisms::organisms_index_story as acp_organisms_index_story;
use stories::acp::sections::sections_index_story as acp_sections_index_story;
use ui::{TAILWIND_CDN, TAILWIND_THEME};

fn sidebar_nav(active_story: &str) -> Markup {
    let link_class = |name: &str| {
        if active_story == name {
            "block px-2 py-1 text-foreground bg-accent"
        } else {
            "block px-2 py-1 text-muted-foreground hover:text-foreground hover:bg-secondary"
        }
    };

    html! {
        aside class="fixed top-0 left-0 bottom-0 w-48 border-r border-border overflow-y-auto p-3 bg-background z-50" {
            h1 class="font-bold mb-3 text-foreground" { "Storybook" }
            nav {
                h2 class="uppercase text-muted-foreground mb-1 mt-3 pl-1 tracking-wide text-xs" { "Atoms" }
                a href="/stories/base-document" class=(link_class("base-document")) { "Base Document" }
                a href="/stories/button" class=(link_class("button")) { "Button" }
                a href="/stories/claude-status" class=(link_class("claude-status")) { "Claude Status" }
                h2 class="uppercase text-muted-foreground mb-1 mt-4 pl-1 tracking-wide text-xs" { "Molecules" }
                p class="text-muted-foreground text-xs pl-1 py-1" { "No stories yet" }
                h2 class="uppercase text-muted-foreground mb-1 mt-4 pl-1 tracking-wide text-xs" { "Organisms" }
                a href="/stories/recorder" class=(link_class("recorder")) { "Recorder Index" }
                a href="/stories/recorder/atoms" class=(link_class("recorder/atoms")) { "Recorder Atoms" }
                a href="/stories/recorder/molecules" class=(link_class("recorder/molecules")) { "Recorder Molecules" }
                a href="/stories/recorder/organisms" class=(link_class("recorder/organisms")) { "Recorder Organisms" }
                a href="/stories/recorder/sections" class=(link_class("recorder/sections")) { "Recorder Sections" }
                a href="/stories/recorder/demo" class=(link_class("recorder/demo")) { "Recorder Demo" }
                h2 class="uppercase text-muted-foreground mb-1 mt-3 pl-1 tracking-wide text-xs" { "Recorder Atoms" }
                a href="/stories/recorder/atoms/status-dot" class=(link_class("recorder/atoms/status-dot")) { "Status Dot" }
                a href="/stories/recorder/atoms/line-type-label" class=(link_class("recorder/atoms/line-type-label")) { "Line Type Label" }
                a href="/stories/recorder/atoms/step-badge" class=(link_class("recorder/atoms/step-badge")) { "Step Badge" }
                a href="/stories/recorder/atoms/timestamp-badge" class=(link_class("recorder/atoms/timestamp-badge")) { "Timestamp Badge" }
                a href="/stories/recorder/atoms/call-id-badge" class=(link_class("recorder/atoms/call-id-badge")) { "Call ID Badge" }
                a href="/stories/recorder/atoms/cost-badge" class=(link_class("recorder/atoms/cost-badge")) { "Cost Badge" }
                a href="/stories/recorder/atoms/token-badge" class=(link_class("recorder/atoms/token-badge")) { "Token Badge" }
                a href="/stories/recorder/atoms/latency-badge" class=(link_class("recorder/atoms/latency-badge")) { "Latency Badge" }
                a href="/stories/recorder/atoms/attempt-badge" class=(link_class("recorder/atoms/attempt-badge")) { "Attempt Badge" }
                a href="/stories/recorder/atoms/tid-badge" class=(link_class("recorder/atoms/tid-badge")) { "TID Badge" }
                a href="/stories/recorder/atoms/blob-ref" class=(link_class("recorder/atoms/blob-ref")) { "Blob Ref" }
                a href="/stories/recorder/atoms/redacted-value" class=(link_class("recorder/atoms/redacted-value")) { "Redacted Value" }
                a href="/stories/recorder/atoms/result-arrow" class=(link_class("recorder/atoms/result-arrow")) { "Result Arrow" }
                h2 class="uppercase text-muted-foreground mb-1 mt-4 pl-1 tracking-wide text-xs" { "ACP" }
                a href="/stories/acp" class=(link_class("acp")) { "ACP Index" }
                a href="/stories/acp/atoms" class=(link_class("acp/atoms")) { "ACP Atoms" }
                a href="/stories/acp/molecules" class=(link_class("acp/molecules")) { "ACP Molecules" }
                a href="/stories/acp/organisms" class=(link_class("acp/organisms")) { "ACP Organisms" }
                a href="/stories/acp/sections" class=(link_class("acp/sections")) { "ACP Sections" }
                a href="/stories/acp/demo" class=(link_class("acp/demo")) { "ACP Demo" }
                h2 class="uppercase text-muted-foreground mb-1 mt-3 pl-1 tracking-wide text-xs" { "ACP Atoms" }
                a href="/stories/acp/atoms/tool-icon" class=(link_class("acp/atoms/tool-icon")) { "Tool Icon" }
                a href="/stories/acp/atoms/tool-status-badge" class=(link_class("acp/atoms/tool-status-badge")) { "Tool Status Badge" }
                a href="/stories/acp/atoms/permission-button" class=(link_class("acp/atoms/permission-button")) { "Permission Button" }
                a href="/stories/acp/atoms/mode-badge" class=(link_class("acp/atoms/mode-badge")) { "Mode Badge" }
                a href="/stories/acp/atoms/model-badge" class=(link_class("acp/atoms/model-badge")) { "Model Badge" }
                a href="/stories/acp/atoms/thinking-toggle" class=(link_class("acp/atoms/thinking-toggle")) { "Thinking Toggle" }
                a href="/stories/acp/atoms/checkpoint-badge" class=(link_class("acp/atoms/checkpoint-badge")) { "Checkpoint Badge" }
                a href="/stories/acp/atoms/feedback-button" class=(link_class("acp/atoms/feedback-button")) { "Feedback Button" }
                a href="/stories/acp/atoms/content-type-icon" class=(link_class("acp/atoms/content-type-icon")) { "Content Type Icon" }
                a href="/stories/acp/atoms/entry-marker" class=(link_class("acp/atoms/entry-marker")) { "Entry Marker" }
                a href="/stories/acp/atoms/keybinding-hint" class=(link_class("acp/atoms/keybinding-hint")) { "Keybinding Hint" }
                a href="/stories/acp/atoms/streaming-indicator" class=(link_class("acp/atoms/streaming-indicator")) { "Streaming Indicator" }
                h2 class="uppercase text-muted-foreground mb-1 mt-3 pl-1 tracking-wide text-xs" { "ACP Molecules" }
                a href="/stories/acp/molecules/tool-header" class=(link_class("acp/molecules/tool-header")) { "Tool Header" }
                a href="/stories/acp/molecules/permission-bar" class=(link_class("acp/molecules/permission-bar")) { "Permission Bar" }
                a href="/stories/acp/molecules/mode-selector" class=(link_class("acp/molecules/mode-selector")) { "Mode Selector" }
                a href="/stories/acp/molecules/model-selector" class=(link_class("acp/molecules/model-selector")) { "Model Selector" }
                a href="/stories/acp/molecules/message-header" class=(link_class("acp/molecules/message-header")) { "Message Header" }
                a href="/stories/acp/molecules/thinking-block" class=(link_class("acp/molecules/thinking-block")) { "Thinking Block" }
                a href="/stories/acp/molecules/diff-header" class=(link_class("acp/molecules/diff-header")) { "Diff Header" }
                a href="/stories/acp/molecules/terminal-header" class=(link_class("acp/molecules/terminal-header")) { "Terminal Header" }
                a href="/stories/acp/molecules/checkpoint-restore" class=(link_class("acp/molecules/checkpoint-restore")) { "Checkpoint Restore" }
                a href="/stories/acp/molecules/entry-actions" class=(link_class("acp/molecules/entry-actions")) { "Entry Actions" }
                h2 class="uppercase text-muted-foreground mb-1 mt-4 pl-1 tracking-wide text-xs" { "Screens" }
                p class="text-muted-foreground text-xs pl-1 py-1" { "No stories yet" }
                h2 class="uppercase text-muted-foreground mb-1 mt-4 pl-1 tracking-wide text-xs" { "Layouts" }
                p class="text-muted-foreground text-xs pl-1 py-1" { "No stories yet" }
            }
        }
    }
}

fn base_layout(title: &str, active_story: &str, content: Markup) -> Markup {
    let body_content = html! {
        (sidebar_nav(active_story))

        main class="ml-48 min-h-screen overflow-y-auto p-8" {
            (content)
        }

        // Hot reload WebSocket - reconnects on server restart
        script {
            (PreEscaped(r#"
            (function() {
                var wasConnected = false;
                function connect() {
                    var ws = new WebSocket('ws://' + location.host + '/__ws_reload');
                    ws.onopen = function() {
                        if (wasConnected) location.reload();
                        wasConnected = true;
                    };
                    ws.onclose = function() {
                        setTimeout(connect, 500);
                    };
                }
                connect();
            })();
            "#))
        }
    };

    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " - Storybook" }
                // Inline Tailwind CSS (Play CDN)
                script { (PreEscaped(TAILWIND_CDN)) }
                // Custom theme
                style type="text/tailwindcss" { (PreEscaped(TAILWIND_THEME)) }
            }
            body class="bg-background text-foreground font-mono antialiased" {
                (body_content)
            }
        }
    }
}

/// WebSocket hot reload endpoint
async fn ws_reload(req: actix_web::HttpRequest, stream: web::Payload) -> actix_web::Result<HttpResponse> {
    let (res, mut session, mut msg_stream) = actix_ws::handle(&req, stream)?;

    rt::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if session.ping(b"").await.is_err() {
                        break;
                    }
                }
                msg = msg_stream.recv() => {
                    match msg {
                        Some(Ok(Message::Pong(_))) => {}
                        Some(Ok(Message::Ping(data))) => {
                            let _ = session.pong(&data).await;
                        }
                        _ => break,
                    }
                }
            }
        }
    });

    Ok(res)
}

/// Home page - shows button story by default
async fn index() -> impl Responder {
    let content = button_story();
    let html = base_layout("Button", "button", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

/// Button story page
async fn base_document_story_page() -> impl Responder {
    let content = base_document_story();
    let html = base_layout("Base Document", "base-document", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn button_story_page() -> impl Responder {
    let content = button_story();
    let html = base_layout("Button", "button", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn claude_status_story_page() -> impl Responder {
    let content = claude_status_story();
    let html = base_layout("Claude Status", "claude-status", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_status_dot_page() -> impl Responder {
    let content = status_dot_story();
    let html = base_layout("Status Dot", "recorder/atoms/status-dot", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_line_type_label_page() -> impl Responder {
    let content = line_type_label_story();
    let html = base_layout("Line Type Label", "recorder/atoms/line-type-label", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_step_badge_page() -> impl Responder {
    let content = step_badge_story();
    let html = base_layout("Step Badge", "recorder/atoms/step-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_timestamp_badge_page() -> impl Responder {
    let content = timestamp_badge_story();
    let html = base_layout("Timestamp Badge", "recorder/atoms/timestamp-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_call_id_badge_page() -> impl Responder {
    let content = call_id_badge_story();
    let html = base_layout("Call ID Badge", "recorder/atoms/call-id-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_cost_badge_page() -> impl Responder {
    let content = cost_badge_story();
    let html = base_layout("Cost Badge", "recorder/atoms/cost-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_token_badge_page() -> impl Responder {
    let content = token_badge_story();
    let html = base_layout("Token Badge", "recorder/atoms/token-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_latency_badge_page() -> impl Responder {
    let content = latency_badge_story();
    let html = base_layout("Latency Badge", "recorder/atoms/latency-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_attempt_badge_page() -> impl Responder {
    let content = attempt_badge_story();
    let html = base_layout("Attempt Badge", "recorder/atoms/attempt-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_tid_badge_page() -> impl Responder {
    let content = tid_badge_story();
    let html = base_layout("TID Badge", "recorder/atoms/tid-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_blob_ref_page() -> impl Responder {
    let content = blob_ref_story();
    let html = base_layout("Blob Ref", "recorder/atoms/blob-ref", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_redacted_value_page() -> impl Responder {
    let content = redacted_value_story();
    let html = base_layout("Redacted Value", "recorder/atoms/redacted-value", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_result_arrow_page() -> impl Responder {
    let content = result_arrow_story();
    let html = base_layout("Result Arrow", "recorder/atoms/result-arrow", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_index_page() -> impl Responder {
    let content = atoms_index_story();
    let html = base_layout("Atoms", "recorder/atoms", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn recorder_index_page() -> impl Responder {
    let content = recorder_index_story();
    let html = base_layout("Recorder", "recorder", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn recorder_molecules_page() -> impl Responder {
    let content = recorder_molecules_story();
    let html = base_layout("Recorder Molecules", "recorder/molecules", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn recorder_organisms_page() -> impl Responder {
    let content = recorder_organisms_story();
    let html = base_layout("Recorder Organisms", "recorder/organisms", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn recorder_sections_page() -> impl Responder {
    let content = recorder_sections_story();
    let html = base_layout("Recorder Sections", "recorder/sections", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn recorder_demo_page() -> impl Responder {
    let content = recorder_demo_story();
    let html = base_layout("Recorder Demo", "recorder/demo", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

// ACP handlers
async fn acp_index_page() -> impl Responder {
    let content = acp_index_story();
    let html = base_layout("ACP", "acp", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atoms_page() -> impl Responder {
    let content = acp_atoms_index_story();
    let html = base_layout("ACP Atoms", "acp/atoms", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecules_page() -> impl Responder {
    let content = acp_molecules_index_story();
    let html = base_layout("ACP Molecules", "acp/molecules", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_organisms_page() -> impl Responder {
    let content = acp_organisms_index_story();
    let html = base_layout("ACP Organisms", "acp/organisms", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_sections_page() -> impl Responder {
    let content = acp_sections_index_story();
    let html = base_layout("ACP Sections", "acp/sections", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_demo_page() -> impl Responder {
    let content = acp_demo_story();
    let html = base_layout("ACP Demo", "acp/demo", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_tool_icon_page() -> impl Responder {
    let content = tool_icon_story();
    let html = base_layout("Tool Icon", "acp/atoms/tool-icon", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_tool_status_badge_page() -> impl Responder {
    let content = tool_status_badge_story();
    let html = base_layout("Tool Status Badge", "acp/atoms/tool-status-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_permission_button_page() -> impl Responder {
    let content = permission_button_story();
    let html = base_layout("Permission Button", "acp/atoms/permission-button", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_mode_badge_page() -> impl Responder {
    let content = mode_badge_story();
    let html = base_layout("Mode Badge", "acp/atoms/mode-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_model_badge_page() -> impl Responder {
    let content = model_badge_story();
    let html = base_layout("Model Badge", "acp/atoms/model-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_thinking_toggle_page() -> impl Responder {
    let content = thinking_toggle_story();
    let html = base_layout("Thinking Toggle", "acp/atoms/thinking-toggle", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_checkpoint_badge_page() -> impl Responder {
    let content = checkpoint_badge_story();
    let html = base_layout("Checkpoint Badge", "acp/atoms/checkpoint-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_feedback_button_page() -> impl Responder {
    let content = feedback_button_story();
    let html = base_layout("Feedback Button", "acp/atoms/feedback-button", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_content_type_icon_page() -> impl Responder {
    let content = content_type_icon_story();
    let html = base_layout("Content Type Icon", "acp/atoms/content-type-icon", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_entry_marker_page() -> impl Responder {
    let content = entry_marker_story();
    let html = base_layout("Entry Marker", "acp/atoms/entry-marker", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_keybinding_hint_page() -> impl Responder {
    let content = keybinding_hint_story();
    let html = base_layout("Keybinding Hint", "acp/atoms/keybinding-hint", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_atom_streaming_indicator_page() -> impl Responder {
    let content = streaming_indicator_story();
    let html = base_layout("Streaming Indicator", "acp/atoms/streaming-indicator", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

// ACP molecule handlers
async fn acp_molecule_tool_header_page() -> impl Responder {
    let content = acp_molecule_tool_header_story();
    let html = base_layout("Tool Header", "acp/molecules/tool-header", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_permission_bar_page() -> impl Responder {
    let content = acp_molecule_permission_bar_story();
    let html = base_layout("Permission Bar", "acp/molecules/permission-bar", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_mode_selector_page() -> impl Responder {
    let content = acp_molecule_mode_selector_story();
    let html = base_layout("Mode Selector", "acp/molecules/mode-selector", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_model_selector_page() -> impl Responder {
    let content = acp_molecule_model_selector_story();
    let html = base_layout("Model Selector", "acp/molecules/model-selector", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_message_header_page() -> impl Responder {
    let content = acp_molecule_message_header_story();
    let html = base_layout("Message Header", "acp/molecules/message-header", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_thinking_block_page() -> impl Responder {
    let content = acp_molecule_thinking_block_story();
    let html = base_layout("Thinking Block", "acp/molecules/thinking-block", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_diff_header_page() -> impl Responder {
    let content = acp_molecule_diff_header_story();
    let html = base_layout("Diff Header", "acp/molecules/diff-header", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_terminal_header_page() -> impl Responder {
    let content = acp_molecule_terminal_header_story();
    let html = base_layout("Terminal Header", "acp/molecules/terminal-header", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_checkpoint_restore_page() -> impl Responder {
    let content = acp_molecule_checkpoint_restore_story();
    let html = base_layout("Checkpoint Restore", "acp/molecules/checkpoint-restore", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn acp_molecule_entry_actions_page() -> impl Responder {
    let content = acp_molecule_entry_actions_story();
    let html = base_layout("Entry Actions", "acp/molecules/entry-actions", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // Read port from environment variable or use default
    let port: u16 = std::env::var("STORYBOOK_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3030);

    let mut listenfd = ListenFd::from_env();

    let server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/stories/base-document", web::get().to(base_document_story_page))
            .route("/stories/button", web::get().to(button_story_page))
            .route("/stories/claude-status", web::get().to(claude_status_story_page))
            .route("/stories/recorder", web::get().to(recorder_index_page))
            .route("/stories/recorder/molecules", web::get().to(recorder_molecules_page))
            .route("/stories/recorder/organisms", web::get().to(recorder_organisms_page))
            .route("/stories/recorder/sections", web::get().to(recorder_sections_page))
            .route("/stories/recorder/demo", web::get().to(recorder_demo_page))
            .route("/stories/recorder/atoms", web::get().to(atoms_index_page))
            .route("/stories/recorder/atoms/status-dot", web::get().to(atoms_status_dot_page))
            .route("/stories/recorder/atoms/line-type-label", web::get().to(atoms_line_type_label_page))
            .route("/stories/recorder/atoms/step-badge", web::get().to(atoms_step_badge_page))
            .route("/stories/recorder/atoms/timestamp-badge", web::get().to(atoms_timestamp_badge_page))
            .route("/stories/recorder/atoms/call-id-badge", web::get().to(atoms_call_id_badge_page))
            .route("/stories/recorder/atoms/cost-badge", web::get().to(atoms_cost_badge_page))
            .route("/stories/recorder/atoms/token-badge", web::get().to(atoms_token_badge_page))
            .route("/stories/recorder/atoms/latency-badge", web::get().to(atoms_latency_badge_page))
            .route("/stories/recorder/atoms/attempt-badge", web::get().to(atoms_attempt_badge_page))
            .route("/stories/recorder/atoms/tid-badge", web::get().to(atoms_tid_badge_page))
            .route("/stories/recorder/atoms/blob-ref", web::get().to(atoms_blob_ref_page))
            .route("/stories/recorder/atoms/redacted-value", web::get().to(atoms_redacted_value_page))
            .route("/stories/recorder/atoms/result-arrow", web::get().to(atoms_result_arrow_page))
            // ACP routes
            .route("/stories/acp", web::get().to(acp_index_page))
            .route("/stories/acp/atoms", web::get().to(acp_atoms_page))
            .route("/stories/acp/molecules", web::get().to(acp_molecules_page))
            .route("/stories/acp/organisms", web::get().to(acp_organisms_page))
            .route("/stories/acp/sections", web::get().to(acp_sections_page))
            .route("/stories/acp/demo", web::get().to(acp_demo_page))
            .route("/stories/acp/atoms/tool-icon", web::get().to(acp_atom_tool_icon_page))
            .route("/stories/acp/atoms/tool-status-badge", web::get().to(acp_atom_tool_status_badge_page))
            .route("/stories/acp/atoms/permission-button", web::get().to(acp_atom_permission_button_page))
            .route("/stories/acp/atoms/mode-badge", web::get().to(acp_atom_mode_badge_page))
            .route("/stories/acp/atoms/model-badge", web::get().to(acp_atom_model_badge_page))
            .route("/stories/acp/atoms/thinking-toggle", web::get().to(acp_atom_thinking_toggle_page))
            .route("/stories/acp/atoms/checkpoint-badge", web::get().to(acp_atom_checkpoint_badge_page))
            .route("/stories/acp/atoms/feedback-button", web::get().to(acp_atom_feedback_button_page))
            .route("/stories/acp/atoms/content-type-icon", web::get().to(acp_atom_content_type_icon_page))
            .route("/stories/acp/atoms/entry-marker", web::get().to(acp_atom_entry_marker_page))
            .route("/stories/acp/atoms/keybinding-hint", web::get().to(acp_atom_keybinding_hint_page))
            .route("/stories/acp/atoms/streaming-indicator", web::get().to(acp_atom_streaming_indicator_page))
            // ACP molecule routes
            .route("/stories/acp/molecules/tool-header", web::get().to(acp_molecule_tool_header_page))
            .route("/stories/acp/molecules/permission-bar", web::get().to(acp_molecule_permission_bar_page))
            .route("/stories/acp/molecules/mode-selector", web::get().to(acp_molecule_mode_selector_page))
            .route("/stories/acp/molecules/model-selector", web::get().to(acp_molecule_model_selector_page))
            .route("/stories/acp/molecules/message-header", web::get().to(acp_molecule_message_header_page))
            .route("/stories/acp/molecules/thinking-block", web::get().to(acp_molecule_thinking_block_page))
            .route("/stories/acp/molecules/diff-header", web::get().to(acp_molecule_diff_header_page))
            .route("/stories/acp/molecules/terminal-header", web::get().to(acp_molecule_terminal_header_page))
            .route("/stories/acp/molecules/checkpoint-restore", web::get().to(acp_molecule_checkpoint_restore_page))
            .route("/stories/acp/molecules/entry-actions", web::get().to(acp_molecule_entry_actions_page))
            .route("/__ws_reload", web::get().to(ws_reload))
    });

    // Use systemfd socket if available, otherwise bind to port
    let server = if let Some(listener) = listenfd.take_tcp_listener(0)? {
        println!(
            "Storybook running (hot-reload) at http://localhost:{}",
            port
        );
        server.listen(listener)?
    } else {
        println!("Storybook running at http://localhost:{}", port);
        let _ = open::that(format!("http://localhost:{}", port));
        server.bind(("127.0.0.1", port))?
    };

    server.run().await
}
