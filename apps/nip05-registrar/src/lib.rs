pub mod config;
pub mod error;
pub mod routes;
pub mod store;
pub mod validation;

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::TcpListener;

pub use config::Config;
pub use error::RegistrarError;
pub use routes::{AppState, router};
pub use store::{NostrJson, Store};

pub async fn run(config: Config) -> Result<()> {
    let store = Store::load(config.data_file.clone(), config.reserved.clone())
        .map_err(|err| anyhow::anyhow!("failed to load store: {err}"))?;
    let state = AppState {
        store: Arc::new(store),
        admin_token: Arc::new(config.admin_token.clone()),
    };
    let app = router(state);

    let listener = TcpListener::bind(config.listen_addr)
        .await
        .with_context(|| format!("failed to bind {}", config.listen_addr))?;
    let bound = listener.local_addr().unwrap_or(config.listen_addr);
    tracing::info!(addr = %bound, data_file = %config.data_file.display(), "nip05-registrar listening");
    axum::serve(listener, app)
        .await
        .context("nip05-registrar server failed")
}
