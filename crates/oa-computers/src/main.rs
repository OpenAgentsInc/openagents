//! `oa-computers`: the API and the reconcile loop in one process.
//!
//! Environment:
//! - `OA_COMPUTERS_DATABASE_URL` (required): the account database; this
//!   service keeps its tables in the `computers` schema.
//! - `OA_COMPUTERS_TOKEN` (required): the service bearer token callers send.
//! - `OA_COMPUTERS_DRIVER`: `boat` (default; `BOAT_API_BASE`, `BOAT_API_KEY`)
//!   `fake` (`OA_COMPUTERS_FAKE_DIR`, local development only), or `space`
//!   (feature `space`; `SPACE_API_URL`, `SPACE_DATA_PLANE_URL`,
//!   `SPACE_API_TOKEN`, and `SPACE_TEMPLATE`).
//! - `OA_COMPUTERS_REGISTRY_FILE`: explicit deployment configuration. Old rows
//!   require an explicit `legacy` mapping. With the old single-driver variables,
//!   `OA_COMPUTERS_LEGACY_DEPLOYMENT` identifies that previous deployment.
//! - `OA_COMPUTERS_LISTEN` (default `0.0.0.0:$PORT`, else `127.0.0.1:8790`).
//! - `OA_COMPUTERS_RECONCILE=0` serves the API without a reconciler.
//! - `OA_COMPUTERS_KEEP_STOPPED_DAYS` (7): a computer stopped this long
//!   with nothing asked of it is deleted.
//! - `OA_COMPUTERS_LEASE_SECONDS` (60), `OA_COMPUTERS_REVISIT_SECONDS` (30),
//!   `OA_COMPUTERS_BACKOFF_MS` (5000), `OA_COMPUTERS_DRIVER_POLL_MS` (3000).
//!
//! No secret is ever printed.

use oa_computers::registry::{Registry, runtime::RuntimeDriver};
use oa_computers::{Db, Store, reconcile::Settings};
use std::time::Duration;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn number(name: &str, default: u64) -> u64 {
    env(name).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn settings() -> Settings {
    let mut holder = [0u8; 6];
    let _ = getrandom::fill(&mut holder);
    let mut s = Settings::new(format!(
        "{}-{}",
        std::process::id(),
        holder
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ));
    s.lease = Duration::from_secs(number("OA_COMPUTERS_LEASE_SECONDS", 60));
    s.revisit = Duration::from_secs(number("OA_COMPUTERS_REVISIT_SECONDS", 30));
    s.backoff = Duration::from_millis(number("OA_COMPUTERS_BACKOFF_MS", 5000));
    s.poll = Duration::from_millis(number("OA_COMPUTERS_POLL_MS", 1000));
    s.driver_poll = Duration::from_millis(number("OA_COMPUTERS_DRIVER_POLL_MS", 3000));
    s.heartbeat = Duration::from_millis(number("OA_COMPUTERS_HEARTBEAT_MS", 30_000));
    s.keep_stopped = Duration::from_secs(number("OA_COMPUTERS_KEEP_STOPPED_DAYS", 7) * 86_400);
    s
}

async fn serve(mut driver: Registry<RuntimeDriver>, dsn: String, token: String) {
    let catalog = driver.catalog().clone();
    let db = match Db::connect(&dsn).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("oa-computers: {e}");
            std::process::exit(1);
        }
    };
    oa_computers::registry::runtime::retain_lifecycle_operations(&mut driver, db.clone());
    if let Err(error) = Store::new(db.clone()).pin_legacy(&catalog).await {
        eprintln!("oa-computers: {error}");
        std::process::exit(1);
    }
    // The free cloud allowance (#11981): recorded, or removed, at every start.
    let free = match oa_computers::usage::FreeAllowance::from_env() {
        Ok(free) => free,
        Err(error) => {
            eprintln!("oa-computers: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = Store::new(db.clone()).set_free_allowance(free).await {
        eprintln!("oa-computers: {error}");
        std::process::exit(1);
    }
    let files = oa_computers::files::spawn(Store::new(db.clone()), driver.clone());
    let turns = oa_computers::turns::Secrets::default();
    if env("OA_COMPUTERS_RECONCILE").as_deref() != Some("0") {
        oa_computers::spawn_reconciler(dsn, driver, settings(), turns.clone());
    }
    let app = oa_computers::app_with_registry(
        Store::new(db),
        catalog.clone(),
        &token,
        turns,
        Some(files),
    );
    let listen = env("OA_COMPUTERS_LISTEN").unwrap_or_else(|| match env("PORT") {
        Some(port) => format!("0.0.0.0:{port}"),
        None => "127.0.0.1:8790".into(),
    });
    let listener = match tokio::net::TcpListener::bind(&listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("oa-computers: listen {listen}: {e}");
            std::process::exit(1);
        }
    };
    eprintln!(
        "oa-computers: listening on {listen}, {} configured services",
        catalog.targets.len()
    );
    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("oa-computers: {e}");
    }
}

#[tokio::main]
async fn main() {
    let (Some(dsn), Some(token)) = (env("OA_COMPUTERS_DATABASE_URL"), env("OA_COMPUTERS_TOKEN"))
    else {
        eprintln!("oa-computers: OA_COMPUTERS_DATABASE_URL and OA_COMPUTERS_TOKEN are required");
        std::process::exit(2);
    };
    let registry = match oa_computers::registry::runtime::from_environment().await {
        Ok(registry) => registry,
        Err(error) => {
            eprintln!("oa-computers: {error}");
            std::process::exit(2);
        }
    };
    serve(registry, dsn, token).await;
}
