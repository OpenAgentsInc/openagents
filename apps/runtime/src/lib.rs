#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;

use crate::{
    authority::InMemoryRuntimeAuthority,
    config::Config,
    orchestration::RuntimeOrchestrator,
    projectors::InMemoryProjectionPipeline,
    server::{AppState, build_router},
    workers::InMemoryWorkerRegistry,
};

pub mod artifacts;
pub mod authority;
pub mod config;
pub mod event_log;
pub mod orchestration;
pub mod projectors;
pub mod run_state_machine;
pub mod server;
pub mod types;
pub mod workers;

pub fn build_runtime_state(config: Config) -> AppState {
    let authority = InMemoryRuntimeAuthority::shared();
    let projectors = InMemoryProjectionPipeline::shared_from_env();
    let orchestrator = Arc::new(RuntimeOrchestrator::new(authority, projectors));
    let workers = Arc::new(InMemoryWorkerRegistry::new(
        orchestrator.projectors(),
        120_000,
    ));
    AppState::new(config, orchestrator, workers)
}

pub fn build_app(config: Config) -> axum::Router {
    build_router(build_runtime_state(config))
}

pub async fn serve(config: Config) -> Result<()> {
    let listener = TcpListener::bind(config.bind_addr).await?;
    info!(
        service = %config.service_name,
        bind_addr = %config.bind_addr,
        "runtime service listening"
    );
    axum::serve(listener, build_app(config)).await?;
    Ok(())
}
