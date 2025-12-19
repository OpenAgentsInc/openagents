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
