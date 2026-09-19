//! Serves the System One contract from Apple's on-device model.
//!
//! ```text
//! ./scripts/build-lev-bridge.sh
//! cargo run -p lev --features serve --bin lev-serve -- --port 11436
//! ```
//!
//! Then point any System One client at it:
//!
//! ```text
//! TYPESAFE_BASE_URL=http://127.0.0.1:11436 TYPESAFE_API_KEY=unused
//! ```

use std::sync::Arc;

use lev::bridge::Pool;
use lev::serve::{DEFAULT_SAMPLES, Door};

#[tokio::main]
async fn main() {
    let mut port = 11436_u16;
    let mut samples = DEFAULT_SAMPLES;
    let mut helpers = 4_usize;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--port" => port = args.next().and_then(|value| value.parse().ok()).unwrap_or(port),
            "--helpers" => {
                helpers = args.next().and_then(|value| value.parse().ok()).unwrap_or(helpers);
            }
            "--samples" => {
                samples = args.next().and_then(|value| value.parse().ok()).unwrap_or(samples);
            }
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
    }

    let pool = match Pool::discover(helpers) {
        Ok(pool) => pool,
        Err(refusal) => {
            eprintln!("{refusal}");
            std::process::exit(2);
        }
    };
    let door = Arc::new(Door::new(pool, "lev-base", samples));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("the port is free");
    eprintln!(
        "lev-serve on http://127.0.0.1:{port}, {samples} samples per question across {} helpers",
        door.pool_width()
    );
    axum::serve(listener, door.router()).await.expect("the server runs");
}
