#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;

use crate::{
    authority::InMemoryRuntimeAuthority,
    config::Config,
    db::RuntimeDb,
    orchestration::RuntimeOrchestrator,
    projectors::InMemoryProjectionPipeline,
    server::{AppState, build_router},
    spacetime_publisher::SpacetimePublisher,
    workers::InMemoryWorkerRegistry,
};

pub mod adjudication;
pub mod artifacts;
pub mod authority;
pub mod bridge;
pub mod config;
pub mod credit;
pub mod db;
pub mod event_log;
pub mod fraud;
pub mod fx;
pub mod history_compat;
pub mod human_qa;
pub mod inference_tiering;
pub mod lightning_node;
pub mod liquidity;
pub mod liquidity_pool;
pub mod marketplace;
pub mod orchestration;
pub mod projectors;
pub mod route_ownership;
pub mod run_state_machine;
pub mod server;
pub mod shadow;
pub mod spacetime_publisher;
pub mod sync_auth;
pub mod treasury;
pub mod types;
pub mod verification;
pub mod workers;

pub async fn build_runtime_state(config: Config) -> Result<AppState> {
    let db = match config.db_url.clone() {
        Some(url) => Some(Arc::new(RuntimeDb::connect(url.as_str()).await?)),
        None => None,
    };
    let authority = InMemoryRuntimeAuthority::shared();
    let projectors = InMemoryProjectionPipeline::shared_from_env();
    let orchestrator = Arc::new(RuntimeOrchestrator::new(authority, projectors));
    let workers = Arc::new(InMemoryWorkerRegistry::new(
        orchestrator.projectors(),
        120_000,
    ));
    let spacetime_publisher = Arc::new(SpacetimePublisher::in_memory());
    Ok(AppState::new(
        config,
        orchestrator,
        workers,
        spacetime_publisher,
        db,
    ))
}

pub async fn build_app(config: Config) -> Result<axum::Router> {
    Ok(build_router(build_runtime_state(config).await?))
}

pub async fn serve(config: Config) -> Result<()> {
    let listener = TcpListener::bind(config.bind_addr).await?;
    info!(
        service = %config.service_name,
        bind_addr = %config.bind_addr,
        "runtime service listening"
    );
    axum::serve(listener, build_app(config).await?).await?;
    Ok(())
}
