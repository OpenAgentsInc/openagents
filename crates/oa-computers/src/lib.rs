//! `oa-computers`: one lifecycle layer for every computer (#11624).
//!
//! One binary: an HTTP API that records intent in Postgres and returns at
//! once, and a reconcile loop that drives each computer toward its intent
//! through a `computer-driver` driver (our `oa-boat` on GCE in production,
//! the in-memory fake in tests), using `coder-working-computer`'s pure
//! decide/apply core. Desired and observed state live in the `computers`
//! schema of the account database. See `docs/cloud/computers.md`.

pub mod api;
pub mod commands;
pub mod db;
pub mod files;
pub mod reconcile;
pub mod store;
pub mod turns;

pub use db::Db;
pub use store::Store;

use computer_driver::Driver;
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
    D: Driver + Clone + Send + 'static,
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
                let r = Rc::new(reconcile::Reconciler {
                    store: Store::new(db),
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
        token: api::Api::token_digest(token),
        turns,
    }))
}
