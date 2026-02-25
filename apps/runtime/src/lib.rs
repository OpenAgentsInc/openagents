#![forbid(unsafe_code)]

use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tracing::info;

use crate::{
    authority::InMemoryRuntimeAuthority,
    config::Config,
    db::RuntimeDb,
    fanout::FanoutHub,
    orchestration::RuntimeOrchestrator,
    projectors::InMemoryProjectionPipeline,
    server::{AppState, build_router},
    sync_auth::{SyncAuthConfig, SyncAuthorizer},
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
pub mod fanout;
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
pub mod shadow_control_khala;
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
    let fanout = Arc::new(FanoutHub::memory_with_limits(
        config.fanout_queue_capacity,
        config.khala_fanout_limits(),
    ));
    let sync_auth = Arc::new(SyncAuthorizer::from_config(SyncAuthConfig {
        signing_key: config.sync_token_signing_key.clone(),
        issuer: config.sync_token_issuer.clone(),
        audience: config.sync_token_audience.clone(),
        require_jti: config.sync_token_require_jti,
        max_token_age_seconds: config.sync_token_max_age_seconds,
        revoked_jtis: config.sync_revoked_jtis.clone(),
    }));
    Ok(AppState::new(
        config,
        orchestrator,
        workers,
        fanout,
        sync_auth,
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
