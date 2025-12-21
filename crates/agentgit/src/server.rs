//! Actix-web server for AgentGit

use actix_web::{web, App, HttpResponse, HttpServer};
use std::sync::Arc;

use crate::nostr::NostrClient;
use crate::views::home_page_with_repos;
use crate::ws::{ws_handler, WsBroadcaster};

/// Application state shared across handlers
pub struct AppState {
    pub broadcaster: Arc<WsBroadcaster>,
    pub nostr_client: Arc<NostrClient>,
}

/// Starts server on 127.0.0.1:0, returns the assigned port
pub async fn start_server(
    broadcaster: Arc<WsBroadcaster>,
    nostr_client: Arc<NostrClient>,
) -> anyhow::Result<u16> {
    let state = web::Data::new(AppState {
        broadcaster,
        nostr_client,
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .route("/ws", web::get().to(ws_route))
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    tokio::spawn(server.run());

    Ok(port)
}

/// Home page
async fn index(state: web::Data<AppState>) -> HttpResponse {
    // Fetch repositories from cache
    let repositories = match state.nostr_client.get_cached_repositories(50).await {
        Ok(repos) => repos,
        Err(e) => {
            tracing::warn!("Failed to fetch repositories: {}", e);
            vec![]
        }
    };

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(home_page_with_repos(&repositories).into_string())
}

/// WebSocket upgrade
async fn ws_route(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    ws_handler(req, stream, state.broadcaster.clone()).await
}
