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
    let mut seed_base = 0_u64;
    let mut helpers = 4_usize;
    let mut adapter: Option<String> = None;
    let mut calibration: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--port" => port = args.next().and_then(|value| value.parse().ok()).unwrap_or(port),
            "--adapter" => adapter = args.next(),
            // A directory of calibration records. Each one is checked against
            // what this door is running before it may serve, and a record
            // that does not match is reported with the field that refused it.
            "--calibration" => calibration = args.next(),
            "--helpers" => {
                helpers = args.next().and_then(|value| value.parse().ok()).unwrap_or(helpers);
            }
            "--samples" => {
                samples = args.next().and_then(|value| value.parse().ok()).unwrap_or(samples);
            }
            // Block 0 is the default and reproduces the recorded numbers.
            // Another block answers with seeds this door has not drawn, which
            // is what a confirmation run needs.
            "--seed-base" => {
                seed_base = args.next().and_then(|value| value.parse().ok()).unwrap_or(seed_base);
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
    // Check the package before serving with it. A signature mismatch is a
    // deployment error and the door should not start, rather than refusing
    // every request at run time.
    let model = if adapter.is_some() { "lev-adapted" } else { "lev-base" };
    let mut door = Door::new(pool, model, samples).with_seed_base(seed_base);
    if let Some(path) = adapter {
        match lev::adapter::Package::open(&path) {
            Ok(package) => {
                eprintln!(
                    "lev-serve: adapter {} pinned to base {}",
                    package.metadata.adapter_identifier, package.metadata.base_model_signature
                );
                door = door.with_adapter(path);
            }
            Err(refusal) => {
                eprintln!("lev-serve: {}", refusal.message);
                std::process::exit(2);
            }
        }
    }
    if let Some(dir) = calibration {
        door = door.with_calibration(&dir);
        let held = door.calibration();
        if held.is_empty() {
            eprintln!("lev-serve: no record in {dir} may serve this door");
        } else {
            eprintln!("lev-serve: serving fitted maps for {}", held.families().join(", "));
        }
        if let Some(trouble) = held.trouble() {
            eprintln!("lev-serve: {dir} could not be read: {trouble}");
        }
        for (named, reason) in held.refusals() {
            eprintln!("lev-serve: {named} refused — {reason}");
        }
    }
    let door = Arc::new(door);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("the port is free");
    eprintln!(
        "lev-serve on http://127.0.0.1:{port}, {samples} samples per question from seed block \
         {seed_base} across {} helpers",
        door.pool_width()
    );
    axum::serve(listener, door.router()).await.expect("the server runs");
}
