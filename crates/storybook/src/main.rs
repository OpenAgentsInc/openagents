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
use maud::{DOCTYPE, Markup, html};
use std::time::Duration;

use stories::button::button_story;

const PORT: u16 = 3030;

fn sidebar_nav(active_story: &str) -> Markup {
    html! {
        aside style="position: fixed; top: 0; left: 0; bottom: 0; width: 12rem; border-right: 1px solid #333; overflow-y: auto; padding: 0.75rem; background: #111; z-index: 100;" {
            h1 style="font-weight: bold; margin-bottom: 0.75rem; color: #fff;" { "Storybook" }
            nav {
                h2 style="text-transform: uppercase; color: #888; margin-bottom: 0.25rem; margin-top: 0.75rem; padding-left: 0.25rem; letter-spacing: 0.05em; font-size: 0.75rem;" { "Components" }
                a href="/stories/button" style=(format!("display: block; padding: 0.25rem 0.5rem; color: {}; text-decoration: none; background: {};",
                    if active_story == "button" { "#fff" } else { "#888" },
                    if active_story == "button" { "#222" } else { "transparent" }
                )) { "Button" }
            }
        }
    }
}

fn base_layout(title: &str, active_story: &str, content: Markup) -> Markup {
    let body_content = html! {
        (sidebar_nav(active_story))

        main style="margin-left: 12rem; height: 100vh; overflow-y: auto; padding: 2rem;" {
            (content)
        }

        // Hot reload WebSocket - reconnects on server restart
        script {
            (maud::PreEscaped(r#"
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
                style {
                    r#"
                    * { box-sizing: border-box; margin: 0; padding: 0; }
                    html, body { height: 100%; overflow: hidden; background: #0a0a0a; color: #e0e0e0; font-family: 'Berkeley Mono', 'SF Mono', 'Monaco', monospace; }
                    "#
                }
            }
            body { (body_content) }
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

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let mut listenfd = ListenFd::from_env();

    let server = HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/stories/button", web::get().to(button_story_page))
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
