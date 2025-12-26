//! Route configuration for unified server

use actix_web::web;

pub mod acp;
mod agents;
mod autopilot;
mod claude;
mod daemon;
mod gitafter;
mod local_inference;
mod marketplace;
mod parallel;
mod wallet;

/// Configure all routes
pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // Root - dashboard with tab navigation
        .route("/", web::get().to(super::views::home))
        // API routes
        .service(web::scope("/api/acp").configure(acp::configure_api))
        .service(web::scope("/api/agents").configure(agents::configure_api))
        .service(web::scope("/api/autopilot").configure(autopilot::configure_api))
        .service(web::scope("/api/claude").configure(claude::configure_api))
        .service(web::scope("/api/daemon").configure(daemon::configure_api))
        .service(web::scope("/api/parallel").configure(parallel::configure_api))
        .service(web::scope("/api/local-inference").configure(local_inference::configure_api))
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
        // Parallel agents routes
        .service(web::scope("/parallel").configure(parallel::configure))
        // Local inference settings
        .service(web::scope("/local-inference").configure(local_inference::configure))
        // WebSocket
        .route("/ws", web::get().to(super::ws::ws_handler));
}
