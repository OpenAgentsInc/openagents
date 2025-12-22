//! Actix-web server for Wallet GUI

use actix_web::{web, App, HttpResponse, HttpServer, Result as ActixResult};
use serde::Deserialize;
use std::sync::Arc;

use crate::core::identity::UnifiedIdentity;
use super::views::{dashboard_page, send_page, history_page, settings_page};

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
            let profile = identity.get_profile().await.unwrap_or_default();

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
#[allow(dead_code)]
struct SendPaymentForm {
    address: String,
    amount: u64,
}

/// Handle send payment form submission
async fn send_payment(
    _state: web::Data<AppState>,
    _form: web::Form<SendPaymentForm>,
) -> ActixResult<HttpResponse> {
    // Payment functionality not yet available - requires Spark SDK integration (see d-001)
    Ok(HttpResponse::ServiceUnavailable()
        .content_type("text/html; charset=utf-8")
        .body(r#"<!DOCTYPE html>
<html>
<head><title>Payment Not Available</title></head>
<body>
<h1>Payment Functionality Not Available</h1>
<p>Lightning payment sending is not yet implemented. This requires Spark SDK integration (directive d-001).</p>
<p>The wallet GUI currently only supports viewing identity and balance information.</p>
<p><a href="/">Return to Home</a></p>
</body>
</html>"#))
}

/// Receive page
async fn receive_page_route(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(_identity) => {
            // Receive functionality not yet available - requires Spark SDK integration (see d-001)
            Ok(HttpResponse::ServiceUnavailable()
                .content_type("text/html; charset=utf-8")
                .body(r#"<!DOCTYPE html>
<html>
<head><title>Receive Not Available</title></head>
<body>
<h1>Receive Functionality Not Available</h1>
<p>Lightning address generation is not yet implemented. This requires Spark SDK integration (directive d-001).</p>
<p>The wallet GUI currently only supports viewing identity information.</p>
<p><strong>Do not share any placeholder addresses - they will not work and funds sent to them will be lost.</strong></p>
<p><a href="/">Return to Home</a></p>
</body>
</html>"#))
        }
        None => {
            Ok(HttpResponse::Unauthorized().body("No wallet identity"))
        }
    }
}

/// History page
///
/// Per d-012, transaction history requires Breez SDK integration (d-001).
/// Shows empty state with clear message until implemented.
async fn history_page_route(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(_identity) => {
            // Transaction history requires Breez SDK integration (d-001)
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
///
/// Per d-012, loads actual relay configuration from WalletConfig.
async fn settings_page_route(state: web::Data<AppState>) -> ActixResult<HttpResponse> {
    match &state.identity {
        Some(_identity) => {
            // Load relay configuration from WalletConfig
            let relays = crate::storage::config::WalletConfig::load()
                .ok()
                .map(|c| c.nostr.relays)
                .unwrap_or_else(|| vec![
                    "wss://relay.damus.io".to_string(),
                    "wss://nos.lol".to_string(),
                ]);

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
///
/// Per d-012, actually updates relay configuration.
async fn update_relays(
    _state: web::Data<AppState>,
    form: web::Form<RelayForm>,
) -> ActixResult<HttpResponse> {
    // Load config, add relay, save
    if let Ok(mut config) = crate::storage::config::WalletConfig::load() {
        let url = form.relay_url.trim().to_string();

        // Validate URL format
        if url.starts_with("wss://") || url.starts_with("ws://") {
            if !config.nostr.relays.contains(&url) {
                config.nostr.relays.push(url);
                let _ = config.save();
            }
        }
    }

    Ok(HttpResponse::SeeOther()
        .insert_header(("Location", "/settings"))
        .finish())
}
