//! Route configuration for unified server

use actix_web::web;

pub mod acp;
mod autopilot;
mod claude;
mod daemon;
mod gitafter;
mod marketplace;
mod wallet;

/// Configure all routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // Root - dashboard with tab navigation
        .route("/", web::get().to(super::views::home))
        // API routes
        .service(web::scope("/api/acp").configure(acp::configure_api))
        .service(web::scope("/api/autopilot").configure(autopilot::configure_api))
        .service(web::scope("/api/claude").configure(claude::configure_api))
        .service(web::scope("/api/daemon").configure(daemon::configure_api))
        // Wallet routes
        .service(web::scope("/wallet").configure(wallet::configure))
        // Marketplace routes
        .service(web::scope("/marketplace").configure(marketplace::configure))
        // Autopilot routes
        .service(web::scope("/autopilot").configure(autopilot::configure))
        // GitAfter routes
        .service(web::scope("/git").configure(gitafter::configure))
        // Daemon routes
        .service(web::scope("/daemon").configure(daemon::configure))
        // WebSocket
        .route("/ws", web::get().to(super::ws::ws_handler));
}
