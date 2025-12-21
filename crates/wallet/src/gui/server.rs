//! Actix-web server for Wallet GUI

use actix_web::{web, App, HttpResponse, HttpServer, Result as ActixResult};
use serde::Deserialize;
use std::sync::Arc;

use crate::core::identity::UnifiedIdentity;
use super::views::{dashboard_page, send_page, receive_page, history_page, settings_page};

/// Application state shared across handlers
pub struct AppState {
    pub identity: Option<Arc<UnifiedIdentity>>,
}

/// Starts server on 127.0.0.1:0, returns the assigned port
pub async fn start_server(
    identity: Option<Arc<UnifiedIdentity>>,
) -> anyhow::Result<u16> {
    let state = web::Data::new(AppState { identity });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(index))
            .route("/send", web::get().to(send_page_route))
            .route("/send", web::post().to(send_payment))
            .route("/receive", web::get().to(receive_page_route))
            .route("/history", web::get().to(history_page_route))
            .route("/settings", web::get().to(settings_page_route))
            .route("/settings/relays", web::post().to(update_relays))
    })
    .bind("127.0.0.1:0")?;

    let port = server.addrs().first().unwrap().port();

    tokio::spawn(server.run());

    Ok(port)
}

/// Home page - Dashboard
async fn index(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(identity) => {
            // Get npub
            let npub = identity.npub();

            // Get balance (placeholder - will integrate with Spark)
            let balance_sats = 0u64;

            // Get profile
            let profile = match identity.get_profile().await {
                Ok(p) => p,
                Err(_) => None,
            };

            let display_name = profile
                .as_ref()
                .and_then(|p| p.name.clone())
                .unwrap_or_else(|| "Anonymous".to_string());

            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(dashboard_page(&npub, &display_name, balance_sats)))
        }
        None => {
            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(r#"
                    <!DOCTYPE html>
                    <html>
                    <head>
                        <title>Wallet - No Identity</title>
                    </head>
                    <body>
                        <h1>No Wallet Found</h1>
                        <p>Please run <code>cargo wallet init</code> to create a new wallet.</p>
                    </body>
                    </html>
                "#))
        }
    }
}

/// Send page
async fn send_page_route(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(_identity) => {
            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(send_page()))
        }
        None => {
            Ok(HttpResponse::Unauthorized().body("No wallet identity"))
        }
    }
}

#[derive(Deserialize)]
struct SendPaymentForm {
    address: String,
    amount: u64,
}

/// Handle send payment form submission
async fn send_payment(
    _state: web::Data<AppState>,
    form: web::Form<SendPaymentForm>,
) -> ActixResult<HttpResponse> {
    // TODO: Implement actual payment sending
    tracing::info!("Send payment: {} sats to {}", form.amount, form.address);

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/"))
        .finish())
}

/// Receive page
async fn receive_page_route(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(_identity) => {
            // TODO: Generate actual Spark address
            let address = "spark1...placeholder".to_string();

            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(receive_page(&address)))
        }
        None => {
            Ok(HttpResponse::Unauthorized().body("No wallet identity"))
        }
    }
}

/// History page
async fn history_page_route(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(_identity) => {
            // TODO: Fetch actual transaction history
            let transactions = vec![];

            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(history_page(&transactions)))
        }
        None => {
            Ok(HttpResponse::Unauthorized().body("No wallet identity"))
        }
    }
}

/// Settings page
async fn settings_page_route(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(_identity) => {
            // TODO: Load actual relay configuration
            let relays = vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
            ];

            Ok(HttpResponse::Ok()
                .content_type("text/html; charset=utf-8")
                .body(settings_page(&relays)))
        }
        None => {
            Ok(HttpResponse::Unauthorized().body("No wallet identity"))
        }
    }
}

#[derive(Deserialize)]
struct RelayForm {
    #[allow(dead_code)]
    relay_url: String,
}

/// Update relays
async fn update_relays(
    _state: web::Data<AppState>,
    _form: web::Form<RelayForm>,
) -> ActixResult<HttpResponse> {
    // TODO: Implement relay updates

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/settings"))
        .finish())
}
