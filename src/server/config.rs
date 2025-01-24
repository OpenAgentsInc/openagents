use super::services::{DeepSeekService, GitHubService, ModelRouter, RepomapService};
use super::ws::handlers::chat::ChatHandler;
use super::ws::transport::WebSocketState;
use actix_web::web;
use std::sync::Arc;

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Create shared services
    let deepseek_service = Arc::new(DeepSeekService::new(
        std::env::var("DEEPSEEK_API_KEY").expect("DEEPSEEK_API_KEY must be set"),
    ));
    let github_service = Arc::new(GitHubService::new(
        std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set"),
    ));
    let repomap_service = Arc::new(RepomapService::new(
        std::env::var("FIRECRAWL_API_KEY").expect("FIRECRAWL_API_KEY must be set"),
    ));

    // Create WebSocket state
    let ws_state = Arc::new(WebSocketState::new());

    // Create chat handler
    let chat_handler = web::Data::new(ChatHandler::new(
        ws_state.clone(),
        deepseek_service.clone(),
        github_service.clone(),
    ));

    // Add services to app data
    cfg.app_data(web::Data::new(deepseek_service.clone()))
        .app_data(web::Data::new(github_service.clone()))
        .app_data(web::Data::new(repomap_service.clone()))
        .app_data(chat_handler);
}