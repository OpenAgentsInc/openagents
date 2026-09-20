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
//!
//! A door started from a manifest also runs under the release's policy
//! snapshot. It fetches one at startup, refetches every `--policy-refresh`
//! (15 minutes by default, `off` to stop), and asks the cached snapshot on
//! every question. Two consequences are the point of the mechanism: a
//! revocation published while this door is running stops it without a
//! restart, and a door that stops reaching the service stops serving its
//! release once the cached snapshot passes its freshness window. Unlike the
//! checks above, a stale or revoked policy does not stop the door from
//! starting — the door starts, says so, and refuses, because "refuse while
//! running" is exactly the case a startup check cannot cover.

use std::sync::Arc;

use lev::admission::{Floor, Gate};
use lev::bridge::Pool;
use lev::manifest::Manifest;
use lev::policy::Policy;
use lev::serve::{Calibration, DEFAULT_SAMPLES, Door};

#[tokio::main]
async fn main() {
    let mut port = 11436_u16;
    let mut samples = DEFAULT_SAMPLES;
    let mut seed_base = 0_u64;
    let mut helpers = 4_usize;
    let mut adapter: Option<String> = None;
    let mut calibration: Option<String> = None;
    let mut manifest: Option<String> = None;
    let mut admission_record: Option<String> = None;
    let mut refresh = Some(15 * 60_u64);
    let mut loose = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--port" => {
                port = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(port)
            }
            // The release this door serves. It supplies the adapter, the
            // estimator, and the sample count, so those flags are refused
            // beside it rather than silently overriding the document.
            "--manifest" => manifest = args.next(),
            "--admission-record" => {
                admission_record = Some(args.next().unwrap_or_else(|| {
                    eprintln!("--admission-record requires a new output path");
                    std::process::exit(2);
                }));
            }
            "--adapter" => {
                adapter = args.next();
                loose.push("--adapter");
            }
            // A directory of calibration records. Each one is checked against
            // what this door is running and against the manifest that says
            // which measurements the release rests on, and a record that does
            // not match is reported with the field that refused it.
            "--calibration" => calibration = args.next(),
            // How often this door refetches the policy snapshot that decides
            // whether its release may still serve. `off` leaves the cache to
            // whatever else fetches it, and the door still goes stale on
            // schedule, because that guarantee is the client's and not the
            // fetcher's.
            "--policy-refresh" => {
                refresh = match args.next().as_deref() {
                    Some("off") => None,
                    Some(value) => match lev::policy::duration(value) {
                        Some(seconds) if seconds > 0 => Some(seconds),
                        _ => {
                            eprintln!("--policy-refresh takes off, or a duration such as 15m");
                            std::process::exit(2);
                        }
                    },
                    None => refresh,
                };
            }
            "--helpers" => {
                helpers = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(helpers);
            }
            "--samples" => {
                samples = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(samples);
                loose.push("--samples");
            }
            // Block 0 is the default and reproduces the recorded numbers.
            // Another block answers with seeds this door has not drawn, which
            // is what a confirmation run needs.
            "--seed-base" => {
                seed_base = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(seed_base);
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

    if admission_record.is_some() && manifest.is_none() {
        eprintln!("--admission-record requires --manifest");
        std::process::exit(2);
    }
    let pool = match Pool::discover(helpers) {
        Ok(pool) => pool,
        Err(refusal) => {
            eprintln!("{refusal}");
            std::process::exit(2);
        }
    };

    // The admission floor, before the port is bound. Every step it runs is a
    // deployment fact rather than a caller error, so a release that fails one
    // does not start, and the step it failed is on the console.
    let mut door = match &manifest {
        Some(path) => {
            let floor = admitted(
                path,
                &pool,
                calibration.as_deref(),
                admission_record.as_deref(),
            );
            let manifest = floor.manifest();
            eprintln!(
                "lev-serve: serving {} — {}",
                manifest.release(),
                match &manifest.artifact {
                    Some(artifact) => format!("artifact {}", &artifact.sha256[..16]),
                    None => "no artifact; the operating system ships the weights".to_string(),
                }
            );
            eprintln!("lev-serve: isolation probe passed: {}", floor.isolation());
            Door::new(pool, manifest.name.clone(), manifest.estimator.samples).with_floor(floor)
        }
        None => {
            let model = if adapter.is_some() {
                "lev-adapted"
            } else {
                "lev-base"
            };
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

    if let Some(dir) = &calibration {
        if manifest.is_none() {
            door = door.with_calibration(dir);
        }
        report_calibration(dir, door.calibration());
    }
    // The policy, before the port is bound: one fetch, then whatever the
    // cache says. A door that cannot reach the service starts anyway and
    // refuses, which is the state its refusals describe.
    if let Some(policy) = door.policy() {
        announce(policy);
        if let Some(every) = refresh {
            tokio::spawn(refresher(policy.clone(), every));
        } else {
            eprintln!(
                "lev-serve: policy refresh is off; this door serves until its snapshot goes stale"
            );
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
    axum::serve(listener, door.router())
        .await
        .expect("the server runs");
}

/// Fetches the snapshot and says where this door stands.
fn announce(policy: &Policy) {
    match policy.fetch() {
        Ok(digest) => eprintln!(
            "lev-serve: policy {} from {} ({})",
            &digest[..16],
            policy.source().display(),
            policy.cache().display()
        ),
        Err(trouble) => eprintln!("lev-serve: the policy did not fetch — {trouble}"),
    }
    let report = policy.report();
    for revocation in &report.revoked {
        eprintln!(
            "lev-serve: {} is revoked for {} — {}",
            policy.release(),
            revocation.scope(),
            revocation.reason
        );
    }
    match policy.admits("") {
        Ok(()) => eprintln!(
            "lev-serve: {} serves for another {} seconds unless the snapshot is refreshed",
            policy.release(),
            report.expires_in_seconds
        ),
        Err(refusal) => {
            eprintln!("lev-serve: {}", refusal.message);
            eprintln!(
                "lev-serve: starting anyway, and refusing every question, so the reason is on the wire"
            );
        }
    }
}

/// Refetches the snapshot forever.
///
/// A fetch that fails changes nothing: the door keeps the snapshot it last
/// confirmed and goes stale on its own clock. That is what makes the window a
/// guarantee rather than a request.
async fn refresher(policy: Policy, every: u64) {
    let mut ticks = tokio::time::interval(std::time::Duration::from_secs(every));
    ticks.tick().await;
    loop {
        ticks.tick().await;
        let before = policy.standing().label();
        match policy.fetch() {
            Ok(_) => {
                let after = policy.standing().label();
                if before != after {
                    eprintln!("lev-serve: policy {before} -> {after}");
                    if let Err(refusal) = policy.admits("") {
                        eprintln!("lev-serve: {}", refusal.message);
                    }
                }
            }
            Err(trouble) => eprintln!("lev-serve: the policy did not refresh — {trouble}"),
        }
    }
}

/// Runs the admission floor over the release at `path`, or exits.
///
/// [`Gate::run`] is the one function that says whether a door may serve.
/// Its denial names the step that failed, and that line is the whole
/// diagnosis: the console says `admission step 2 (base signature) failed`
/// rather than leaving an operator to work out which of four checks refused.
fn admitted(path: &str, pool: &Pool, calibration: Option<&str>, record: Option<&str>) -> Floor {
    let manifest = match Manifest::load(path) {
        Ok(manifest) => manifest,
        Err(fault) => {
            eprintln!("lev-serve: {path}: {fault}");
            std::process::exit(2);
        }
    };
    let mut gate = Gate::new(manifest);
    if let Some(dir) = calibration {
        gate = gate.with_calibration(dir);
    }
    let result = if let Some(path) = record {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap_or_else(|error| {
                eprintln!("lev-serve: cannot create admission record {path}: {error}");
                std::process::exit(2);
            });
        let mut rows = Vec::new();
        let result = gate.run_observed(pool, &mut |arm, call, outcome| {
            let outcome = match outcome {
                Ok(value) => serde_json::json!({"choice": value.choice, "band": value.band, "latency_ms": value.latency_ms}),
                Err(error) => serde_json::json!({"refusal": error.to_string()}),
            };
            rows.push(serde_json::json!({"schema": "openagents.lev.admission_probe_row.v1", "arm": arm, "call": call, "outcome": outcome}));
        });
        for row in rows {
            if let Err(error) = writeln!(file, "{row}") {
                eprintln!("lev-serve: cannot write admission record {path}: {error}");
                std::process::exit(2);
            }
        }
        result
    } else {
        gate.run(pool)
    };
    match result {
        Ok(floor) => floor,
        Err(denied) => {
            eprintln!("lev-serve: {path}: {denied}");
            std::process::exit(2);
        }
    }
}

/// Says which records in `dir` this door serves, and why it refused the rest.
fn report_calibration(dir: &str, held: &Calibration) {
    if held.is_empty() {
        eprintln!("lev-serve: no record in {dir} may serve this door");
    } else {
        eprintln!(
            "lev-serve: serving fitted maps for {}",
            held.families().join(", ")
        );
    }
    if let Some(trouble) = held.trouble() {
        eprintln!("lev-serve: {dir} could not be read: {trouble}");
    }
    for (named, reason) in held.refusals() {
        eprintln!("lev-serve: {named} refused — {reason}");
    }
}
