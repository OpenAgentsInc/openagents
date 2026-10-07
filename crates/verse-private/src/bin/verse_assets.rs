//! `verse-assets`: the private asset broker on Cloud Run.
//!
//! Environment:
//!
//! - `VERSE_ASSETS_PUBLIC_URL`: the service's public origin, which every
//!   request's NIP-98 event must name. Required.
//! - `VERSE_ASSETS_BUCKET`: the private bucket (default
//!   `openagentsgemini-verse-private-assets`).
//! - `PORT`: the listening port (default 8080).
//!
//! The service account comes from the metadata server; there is no key.

use std::sync::Arc;

use verse_private::broker::gcp::{Gcs, IamSigner, Metadata};
use verse_private::broker::{Broker, router};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("verse-assets: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let origin = std::env::var("VERSE_ASSETS_PUBLIC_URL")
        .map_err(|_| "set VERSE_ASSETS_PUBLIC_URL to the service's https:// origin")?;
    if !origin.starts_with("https://") {
        return Err("VERSE_ASSETS_PUBLIC_URL must be an https:// origin".into());
    }
    let bucket =
        std::env::var("VERSE_ASSETS_BUCKET").unwrap_or_else(|_| verse_private::BUCKET.into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let metadata = Arc::new(Metadata::new()?);
    let broker = Broker::new(
        &bucket,
        &origin,
        Gcs {
            metadata: metadata.clone(),
            bucket: bucket.clone(),
        },
        IamSigner { metadata },
    );
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .map_err(|e| format!("cannot listen on {port}: {e}"))?;
    eprintln!("verse-assets: serving {} for {bucket}", broker.url());
    axum::serve(listener, router(Arc::new(broker)))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|e| e.to_string())
}
