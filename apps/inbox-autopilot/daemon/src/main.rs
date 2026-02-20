mod auth;
mod config;
mod db;
mod error;
mod gmail;
mod pipeline;
mod routes;
mod types;
mod vault;

use crate::auth::AuthManager;
use crate::config::Config;
use crate::db::Database;
use crate::gmail::GmailClient;
use crate::pipeline::DraftPipeline;
use crate::routes::AppState;
use crate::vault::Vault;
use anyhow::Context;
use serde_json::json;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::broadcast;
use tokio::time::Duration;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    let config = Config::from_env();
    let vault = Vault::load_or_create(config.data_dir.clone()).context("failed loading vault")?;
    let db = Database::open(config.db_path.clone(), vault).context("failed opening database")?;

    let auth = Arc::new(AuthManager::new(config.session_ttl_seconds));
    let gmail = GmailClient::new(config.clone());
    let pipeline = DraftPipeline::new(config.clone());
    let (event_tx, _) = broadcast::channel(1_024);

    let state = AppState {
        config: config.clone(),
        db,
        auth,
        gmail,
        pipeline,
        event_tx,
    };

    state
        .emit_event(
            None,
            "daemon_started",
            json!({
                "bind_addr": config.bind_addr,
            }),
        )
        .context("failed writing daemon_started event")?;

    let _sync_worker = spawn_periodic_sync(state.clone());

    let app = routes::router(state);

    let listener = tokio::net::TcpListener::bind(&config.bind_addr)
        .await
        .with_context(|| format!("failed binding {}", config.bind_addr))?;

    info!("daemon listening on {}", config.bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("axum server exited with error")?;

    Ok(())
}

fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("install ctrl-c handler should not fail");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install signal handler should not fail")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    error!("shutdown signal received");
}

fn spawn_periodic_sync(state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let interval_seconds = state
                .db
                .settings()
                .map(|settings| settings.sync_interval_seconds.clamp(15, 3600))
                .unwrap_or(60);

            tokio::time::sleep(Duration::from_secs(interval_seconds)).await;

            let gmail_connected = state
                .db
                .oauth_status("gmail")
                .map(|(connected, _)| connected)
                .unwrap_or(false);
            if !gmail_connected {
                continue;
            }

            let backfill_days = state.db.settings().map(|s| s.backfill_days).unwrap_or(90);

            match state.gmail.backfill(&state.db, backfill_days).await {
                Ok(sync) => {
                    if let Err(err) = state.emit_event(
                        None,
                        "sync_incremental_completed",
                        json!({
                            "imported_threads": sync.imported_threads,
                            "imported_messages": sync.imported_messages
                        }),
                    ) {
                        tracing::warn!("failed to emit sync event: {}", err);
                    }

                    if let Err(err) = state.generate_drafts_for_recent_threads().await {
                        tracing::warn!("periodic draft generation failed: {}", err);
                    }
                }
                Err(err) => {
                    tracing::warn!("periodic sync failed: {}", err);
                    let _ = state.emit_event(
                        None,
                        "sync_incremental_failed",
                        json!({ "error": err.to_string() }),
                    );
                }
            }
        }
    })
}
