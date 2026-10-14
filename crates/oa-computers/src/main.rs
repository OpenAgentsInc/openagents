//! `oa-computers`: the API and the reconcile loop in one process.
//!
//! Environment:
//! - `OA_COMPUTERS_DATABASE_URL` (required): the account database; this
//!   service keeps its tables in the `computers` schema.
//! - `OA_COMPUTERS_TOKEN` (required): the service bearer token callers send.
//! - `OA_COMPUTERS_DRIVER`: `boat` (default; `BOAT_API_BASE`, `BOAT_API_KEY`)
//!   or `fake` (`OA_COMPUTERS_FAKE_DIR`, local development only).
//! - `OA_COMPUTERS_LISTEN` (default `0.0.0.0:$PORT`, else `127.0.0.1:8790`).
//! - `OA_COMPUTERS_RECONCILE=0` serves the API without a reconciler.
//! - `OA_COMPUTERS_LEASE_SECONDS` (60), `OA_COMPUTERS_REVISIT_SECONDS` (30),
//!   `OA_COMPUTERS_BACKOFF_MS` (5000), `OA_COMPUTERS_DRIVER_POLL_MS` (3000).
//!
//! No secret is ever printed.

use computer_driver::Driver;
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
    s
}

async fn serve<D: Driver + Clone + Send + 'static>(driver: D, dsn: String, token: String) {
    let caps = driver.capabilities();
    if let Err(e) = coder_working_computer::on_driver::check(&caps) {
        eprintln!("oa-computers: {e}");
        std::process::exit(2);
    }
    let db = match Db::connect(&dsn).await {
        Ok(db) => db,
        Err(e) => {
            eprintln!("oa-computers: {e}");
            std::process::exit(1);
        }
    };
    if env("OA_COMPUTERS_RECONCILE").as_deref() != Some("0") {
        oa_computers::spawn_reconciler(dsn, driver, settings());
    }
    let app = oa_computers::app(Store::new(db), caps.clone(), &token);
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
        "oa-computers: listening on {listen}, driver {}",
        caps.driver
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
    match env("OA_COMPUTERS_DRIVER").as_deref().unwrap_or("boat") {
        "fake" => {
            let dir =
                env("OA_COMPUTERS_FAKE_DIR").unwrap_or_else(|| "/tmp/oa-computers-fake".into());
            serve(computer_driver::fake::FakeDriver::new(dir), dsn, token).await;
        }
        "boat" => {
            let client = match boat::Client::from_env().await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("oa-computers: boat client: {e}");
                    std::process::exit(2);
                }
            };
            let driver = match computer_driver::boat::BoatDriver::connect(client).await {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("oa-computers: {e}");
                    std::process::exit(2);
                }
            };
            serve(driver, dsn, token).await;
        }
        other => {
            eprintln!("oa-computers: unknown driver {other}");
            std::process::exit(2);
        }
    }
}
