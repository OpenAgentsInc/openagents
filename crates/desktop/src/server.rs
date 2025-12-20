//! Actix-web server for the desktop app

use actix_web::{App, HttpResponse, HttpServer, web};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::replay::replay_handler;
use crate::views::{autopilot_page, counter_fragment, home_page};
use crate::ws::{WsBroadcaster, ws_handler};

/// Application state shared across handlers
pub struct AppState {
    pub counter: AtomicU64,
    pub broadcaster: Arc<WsBroadcaster>,
}

/// Starts server on 127.0.0.1:0, returns the assigned port
pub async fn start_server(broadcaster: Arc<WsBroadcaster>) -> anyhow::Result<u16> {
    let state = web::Data::new(AppState {
        counter: AtomicU64::new(0),
        broadcaster: broadcaster.clone(),
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .route("/autopilot", web::get().to(autopilot))
            .route("/autopilot/replay", web::get().to(replay_handler))
            .route("/events", web::post().to(events))
            .route("/increment", web::post().to(increment))
            .route("/ws", web::get().to(ws_route))
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    tokio::spawn(server.run());

    Ok(port)
}

/// Home page
async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(home_page(0).into_string())
}

/// Autopilot live viewer page
async fn autopilot() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(autopilot_page().into_string())
}

/// Receive HTML fragment events and broadcast to WebSocket clients
async fn events(state: web::Data<AppState>, body: String) -> HttpResponse {
    // Broadcast the HTML fragment to all connected WebSocket clients
    state.broadcaster.broadcast(&body);
    HttpResponse::Ok().finish()
}

/// Increment counter and broadcast update
async fn increment(state: web::Data<AppState>) -> HttpResponse {
    let new_val = state.counter.fetch_add(1, Ordering::SeqCst) + 1;

    // Broadcast counter update via WebSocket (OOB swap)
    let fragment = counter_fragment(new_val);
    state.broadcaster.broadcast(&fragment);

    HttpResponse::Ok().finish()
}

/// WebSocket upgrade
async fn ws_route(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    ws_handler(req, stream, state.broadcaster.clone()).await
}
