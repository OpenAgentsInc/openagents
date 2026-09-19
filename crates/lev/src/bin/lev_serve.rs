//! Serves the System One contract from Apple's on-device model.
//!
//! ```text
//! ./scripts/build-lev-bridge.sh
//! cargo run -p lev --features serve --bin lev-serve -- \
//!     --manifest crates/lev/manifests/lev-base-v1.json \
//!     --calibration crates/lev/calibration/lev-base
//! ```
//!
//! Then point any System One client at it:
//!
//! ```text
//! TYPESAFE_BASE_URL=http://127.0.0.1:11436 TYPESAFE_API_KEY=unused
//! ```
//!
//! `--manifest` is the way to start a door that serves a probability.
//! It names the artifact, the estimator, the sample count, and the
//! calibration records the release rests on, and every one of those is
//! checked before the port is bound. The separate `--adapter`, `--samples`,
//! and `--seed-base` flags still start a door for an unreleased run, and that
//! door admits nothing: a family without a measured `evalRef` does not admit,
//! and without a manifest there is no `evalRef` to have.

use std::sync::Arc;

use lev::bridge::Pool;
use lev::manifest::Manifest;
use lev::serve::{DEFAULT_SAMPLES, Door};

#[tokio::main]
async fn main() {
    let mut port = 11436_u16;
    let mut samples = DEFAULT_SAMPLES;
    let mut seed_base = 0_u64;
    let mut helpers = 4_usize;
    let mut adapter: Option<String> = None;
    let mut calibration: Option<String> = None;
    let mut manifest: Option<String> = None;
    let mut loose = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--port" => port = args.next().and_then(|value| value.parse().ok()).unwrap_or(port),
            // The release this door serves. It supplies the adapter, the
            // estimator, and the sample count, so those flags are refused
            // beside it rather than silently overriding the document.
            "--manifest" => manifest = args.next(),
            "--adapter" => {
                adapter = args.next();
                loose.push("--adapter");
            }
            // A directory of calibration records. Each one is checked against
            // what this door is running and against the manifest that says
            // which measurements the release rests on, and a record that does
            // not match is reported with the field that refused it.
            "--calibration" => calibration = args.next(),
            "--helpers" => {
                helpers = args.next().and_then(|value| value.parse().ok()).unwrap_or(helpers);
            }
            "--samples" => {
                samples = args.next().and_then(|value| value.parse().ok()).unwrap_or(samples);
                loose.push("--samples");
            }
            // Block 0 is the default and reproduces the recorded numbers.
            // Another block answers with seeds this door has not drawn, which
            // is what a confirmation run needs.
            "--seed-base" => {
                seed_base = args.next().and_then(|value| value.parse().ok()).unwrap_or(seed_base);
                loose.push("--seed-base");
            }
            other => {
                eprintln!("unknown flag {other}");
                std::process::exit(2);
            }
        }
    }
    if manifest.is_some() && !loose.is_empty() {
        eprintln!(
            "lev-serve: {} cannot be passed beside --manifest, which already names the artifact, \
             the estimator, and the sample count. One document decides, or none does.",
            loose.join(" and ")
        );
        std::process::exit(2);
    }

    let pool = match Pool::discover(helpers) {
        Ok(pool) => pool,
        Err(refusal) => {
            eprintln!("{refusal}");
            std::process::exit(2);
        }
    };

    // Check the release before serving it. Every one of these is a deployment
    // error rather than a caller error, so the door does not start, rather
    // than refusing every request at run time.
    let mut door = match &manifest {
        Some(path) => {
            let manifest = checked(path, &pool);
            eprintln!(
                "lev-serve: serving {} — {}",
                manifest.release(),
                match &manifest.artifact {
                    Some(artifact) => format!("artifact {}", &artifact.sha256[..16]),
                    None => "no artifact; the operating system ships the weights".to_string(),
                }
            );
            Door::new(pool, manifest.name.clone(), manifest.estimator.samples)
                .with_manifest(manifest)
        }
        None => {
            let model = if adapter.is_some() { "lev-adapted" } else { "lev-base" };
            let mut door = Door::new(pool, model, samples).with_seed_base(seed_base);
            if let Some(path) = adapter {
                match lev::adapter::Package::open(&path) {
                    Ok(package) => {
                        eprintln!(
                            "lev-serve: adapter {} pinned to base {}, and no manifest names it",
                            package.metadata.adapter_identifier,
                            package.metadata.base_model_signature
                        );
                        door = door.with_adapter(path);
                    }
                    Err(refusal) => {
                        eprintln!("lev-serve: {}", refusal.message);
                        std::process::exit(2);
                    }
                }
            }
            door
        }
    };

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
    let samples = door.samples();
    let seed_base = door.seed_base();
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

/// Reads a manifest and checks every claim it makes, or exits.
fn checked(path: &str, pool: &Pool) -> Manifest {
    let manifest = match Manifest::load(path) {
        Ok(manifest) => manifest,
        Err(fault) => {
            eprintln!("lev-serve: {path}: {fault}");
            std::process::exit(2);
        }
    };
    if manifest.artifact.is_some()
        && let Err(fault) = manifest.check_artifact()
    {
        eprintln!("lev-serve: {path}: {fault}");
        std::process::exit(2);
    }
    if let Err(fault) = manifest.check_eval_refs() {
        eprintln!("lev-serve: {path}: {fault}");
        std::process::exit(2);
    }
    let running = pool.base_signature_prefix().unwrap_or_default();
    if let Err(fault) = manifest.base.check(&running) {
        eprintln!("lev-serve: {path}: {fault}");
        std::process::exit(2);
    }
    manifest
}
