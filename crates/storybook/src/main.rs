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

mod stories;

use actix_web::{App, HttpResponse, HttpServer, Responder, rt, web};
use actix_ws::Message;
use listenfd::ListenFd;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use std::time::Duration;

use stories::atoms::attempt_badge::attempt_badge_story;
use stories::atoms::blob_ref::blob_ref_story;
use stories::atoms::call_id_badge::call_id_badge_story;
use stories::atoms::cost_badge::cost_badge_story;
use stories::atoms::index::atoms_index_story;
use stories::atoms::latency_badge::latency_badge_story;
use stories::atoms::line_type_label::line_type_label_story;
use stories::atoms::redacted_value::redacted_value_story;
use stories::atoms::result_arrow::result_arrow_story;
use stories::atoms::status_dot::status_dot_story;
use stories::atoms::step_badge::step_badge_story;
use stories::atoms::tid_badge::tid_badge_story;
use stories::atoms::timestamp_badge::timestamp_badge_story;
use stories::atoms::token_badge::token_badge_story;
use stories::button::button_story;
use ui::{TAILWIND_CDN, TAILWIND_THEME};

const PORT: u16 = 3030;

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
                h2 class="uppercase text-muted-foreground mb-1 mt-3 pl-1 tracking-wide text-xs" { "Components" }
                a href="/stories/button" class=(link_class("button")) { "Button" }
                h2 class="uppercase text-muted-foreground mb-1 mt-4 pl-1 tracking-wide text-xs" { "Atoms" }
                a href="/stories/atoms" class=(link_class("atoms")) { "Index" }
                a href="/stories/atoms/status-dot" class=(link_class("atoms/status-dot")) { "Status Dot" }
                a href="/stories/atoms/line-type-label" class=(link_class("atoms/line-type-label")) { "Line Type Label" }
                a href="/stories/atoms/step-badge" class=(link_class("atoms/step-badge")) { "Step Badge" }
                a href="/stories/atoms/timestamp-badge" class=(link_class("atoms/timestamp-badge")) { "Timestamp Badge" }
                a href="/stories/atoms/call-id-badge" class=(link_class("atoms/call-id-badge")) { "Call ID Badge" }
                a href="/stories/atoms/cost-badge" class=(link_class("atoms/cost-badge")) { "Cost Badge" }
                a href="/stories/atoms/token-badge" class=(link_class("atoms/token-badge")) { "Token Badge" }
                a href="/stories/atoms/latency-badge" class=(link_class("atoms/latency-badge")) { "Latency Badge" }
                a href="/stories/atoms/attempt-badge" class=(link_class("atoms/attempt-badge")) { "Attempt Badge" }
                a href="/stories/atoms/tid-badge" class=(link_class("atoms/tid-badge")) { "TID Badge" }
                a href="/stories/atoms/blob-ref" class=(link_class("atoms/blob-ref")) { "Blob Ref" }
                a href="/stories/atoms/redacted-value" class=(link_class("atoms/redacted-value")) { "Redacted Value" }
                a href="/stories/atoms/result-arrow" class=(link_class("atoms/result-arrow")) { "Result Arrow" }
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
async fn ws_reload(req: actix_web::HttpRequest, stream: web::Payload) -> impl Responder {
    let (res, mut session, mut msg_stream) = actix_ws::handle(&req, stream).unwrap();

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

    res
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
async fn button_story_page() -> impl Responder {
    let content = button_story();
    let html = base_layout("Button", "button", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_status_dot_page() -> impl Responder {
    let content = status_dot_story();
    let html = base_layout("Status Dot", "atoms/status-dot", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_line_type_label_page() -> impl Responder {
    let content = line_type_label_story();
    let html = base_layout("Line Type Label", "atoms/line-type-label", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_step_badge_page() -> impl Responder {
    let content = step_badge_story();
    let html = base_layout("Step Badge", "atoms/step-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_timestamp_badge_page() -> impl Responder {
    let content = timestamp_badge_story();
    let html = base_layout("Timestamp Badge", "atoms/timestamp-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_call_id_badge_page() -> impl Responder {
    let content = call_id_badge_story();
    let html = base_layout("Call ID Badge", "atoms/call-id-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_cost_badge_page() -> impl Responder {
    let content = cost_badge_story();
    let html = base_layout("Cost Badge", "atoms/cost-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_token_badge_page() -> impl Responder {
    let content = token_badge_story();
    let html = base_layout("Token Badge", "atoms/token-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_latency_badge_page() -> impl Responder {
    let content = latency_badge_story();
    let html = base_layout("Latency Badge", "atoms/latency-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_attempt_badge_page() -> impl Responder {
    let content = attempt_badge_story();
    let html = base_layout("Attempt Badge", "atoms/attempt-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_tid_badge_page() -> impl Responder {
    let content = tid_badge_story();
    let html = base_layout("TID Badge", "atoms/tid-badge", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_blob_ref_page() -> impl Responder {
    let content = blob_ref_story();
    let html = base_layout("Blob Ref", "atoms/blob-ref", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_redacted_value_page() -> impl Responder {
    let content = redacted_value_story();
    let html = base_layout("Redacted Value", "atoms/redacted-value", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_result_arrow_page() -> impl Responder {
    let content = result_arrow_story();
    let html = base_layout("Result Arrow", "atoms/result-arrow", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

async fn atoms_index_page() -> impl Responder {
    let content = atoms_index_story();
    let html = base_layout("Atoms", "atoms", content);
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html.into_string())
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut listenfd = ListenFd::from_env();

    let server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/stories/button", web::get().to(button_story_page))
            .route("/stories/atoms", web::get().to(atoms_index_page))
            .route("/stories/atoms/status-dot", web::get().to(atoms_status_dot_page))
            .route("/stories/atoms/line-type-label", web::get().to(atoms_line_type_label_page))
            .route("/stories/atoms/step-badge", web::get().to(atoms_step_badge_page))
            .route("/stories/atoms/timestamp-badge", web::get().to(atoms_timestamp_badge_page))
            .route("/stories/atoms/call-id-badge", web::get().to(atoms_call_id_badge_page))
            .route("/stories/atoms/cost-badge", web::get().to(atoms_cost_badge_page))
            .route("/stories/atoms/token-badge", web::get().to(atoms_token_badge_page))
            .route("/stories/atoms/latency-badge", web::get().to(atoms_latency_badge_page))
            .route("/stories/atoms/attempt-badge", web::get().to(atoms_attempt_badge_page))
            .route("/stories/atoms/tid-badge", web::get().to(atoms_tid_badge_page))
            .route("/stories/atoms/blob-ref", web::get().to(atoms_blob_ref_page))
            .route("/stories/atoms/redacted-value", web::get().to(atoms_redacted_value_page))
            .route("/stories/atoms/result-arrow", web::get().to(atoms_result_arrow_page))
            .route("/__ws_reload", web::get().to(ws_reload))
    });

    // Use systemfd socket if available, otherwise bind to port
    let server = if let Some(listener) = listenfd.take_tcp_listener(0)? {
        println!(
            "Storybook running (hot-reload) at http://localhost:{}",
            PORT
        );
        server.listen(listener)?
    } else {
        println!("Storybook running at http://localhost:{}", PORT);
        let _ = open::that(format!("http://localhost:{}", PORT));
        server.bind(("127.0.0.1", PORT))?
    };

    server.run().await
}
