//! `oa-computers`: one lifecycle layer for every computer (#11624).
//!
//! One binary: an HTTP API that records intent in Postgres and returns at
//! once, and a reconcile loop that drives each computer toward its intent
//! through a `computer-driver` driver (our `oa-boat` on GCE in production,
//! the in-memory fake in tests), using `coder-working-computer`'s pure
//! decide/apply core. Desired and observed state live in the `computers`
//! schema of the account database. See `docs/cloud/computers.md`.

pub mod api;
pub mod command_credentials;
pub mod commands;
pub mod credential_broker;
pub mod db;
pub mod economics;
pub mod files;
#[cfg(feature = "space")]
pub mod lifecycle_operations;
pub mod reconcile;
pub mod registry;
pub mod storage;
pub mod store;
pub mod turns;
pub mod usage;

pub use db::Db;
pub use store::Store;

use registry::Drivers;
use std::rc::Rc;
use std::sync::Arc;

/// Run the reconcile loop on its own thread (a current-thread runtime with
/// a `LocalSet`, since driver futures need not be `Send`). Returns at once.
pub fn spawn_reconciler<D>(
    dsn: String,
    driver: D,
    settings: reconcile::Settings,
    turns: turns::Secrets,
) -> std::thread::JoinHandle<()>
where
    D: Drivers + Send + 'static,
{
    std::thread::Builder::new()
        .name("oa-computers-reconcile".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("a runtime for the reconciler");
            let local = tokio::task::LocalSet::new();
            local.block_on(&rt, async move {
                let db = loop {
                    match Db::connect(&dsn).await {
                        Ok(db) => break db,
                        Err(e) => {
                            eprintln!("oa-computers: reconciler database: {e}");
                            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        }
                    }
                };
                let store = Store::new(db);
                let observer = storage::DeletedStorageObserver {
                    store: store.clone(),
                    driver: driver.clone(),
                    holder: format!("{}:storage", settings.holder),
                };
                tokio::task::spawn_local(observer.run());
                let r = Rc::new(reconcile::Reconciler {
                    store,
                    driver,
                    settings,
                    turns,
                });
                r.run().await;
            });
        })
        .expect("the reconciler thread starts")
}

/// The API router over `store`, for a driver declaring `caps`.
pub fn app(
    store: Store,
    caps: computer_driver::Capabilities,
    token: &str,
    turns: turns::Secrets,
) -> axum::Router {
    app_with_files(store, caps, token, turns, None)
}

/// The API with a bounded protected-file worker.
pub fn app_with_files(
    store: Store,
    mut caps: computer_driver::Capabilities,
    token: &str,
    turns: turns::Secrets,
    files: Option<files::Client>,
) -> axum::Router {
    if files.is_none() {
        caps.workspace_file_bytes = None;
    }
    api::router(Arc::new(api::Api {
        files,
        store,
        caps,
        catalog: None,
        token: api::Api::token_digest(token),
        turns,
    }))
}

/// The registry-backed API keeps the original capability fields for callers
/// that do not use placement. All new allocation decisions use the catalog.
pub fn app_with_registry(
    store: Store,
    mut catalog: registry::Catalog,
    token: &str,
    turns: turns::Secrets,
    files: Option<files::Client>,
) -> axum::Router {
    if files.is_none() {
        for target in catalog.targets.values_mut() {
            target.caps.workspace_file_bytes = None;
        }
    }
    let caps = catalog
        .targets
        .get(&catalog.default)
        .map(|target| target.caps.clone())
        .unwrap_or_else(|| computer_driver::Capabilities::new("unavailable", [], &[]));
    api::router(Arc::new(api::Api {
        files,
        store,
        caps,
        catalog: Some(catalog),
        token: api::Api::token_digest(token),
        turns,
    }))
}

/// Private, account-bound clean Environment image operations.
pub mod artifacts;
