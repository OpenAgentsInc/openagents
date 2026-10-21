//! A small HTTP load generator for measuring the host while it swaps
//! manifests.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct LoadTarget {
    pub url: String,
    /// A `Host` header, to reach a tenant by name on a loopback address.
    pub host: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadReport {
    pub requests: usize,
    pub errors: usize,
    /// Responses whose body named a different manifest than the response
    /// header: a request that mixed two versions.
    pub mixed: usize,
    pub manifests_seen: usize,
    pub seconds: f64,
    pub rps: f64,
    pub p50_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

struct Sample {
    latency: Duration,
    ok: bool,
    mixed: bool,
    manifest: Option<String>,
}

/// Sends requests from `concurrency` workers, round-robin over `targets`,
/// until `duration` passes.
pub async fn run(
    targets: Vec<LoadTarget>,
    concurrency: usize,
    duration: Duration,
) -> Result<LoadReport> {
    let client = reqwest::Client::builder()
        .pool_max_idle_per_host(concurrency)
        .timeout(Duration::from_secs(10))
        .build()?;
    let targets = Arc::new(targets);
    let stop = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let mut workers = Vec::new();
    for worker in 0..concurrency {
        let (client, targets, stop) = (client.clone(), targets.clone(), stop.clone());
        workers.push(tokio::spawn(async move {
            let mut samples = Vec::new();
            let mut next = worker;
            while !stop.load(Ordering::Relaxed) {
                let target = &targets[next % targets.len()];
                next += 1;
                samples.push(request(&client, target).await);
            }
            samples
        }));
    }
    tokio::time::sleep(duration).await;
    stop.store(true, Ordering::Relaxed);
    let mut samples = Vec::new();
    for worker in workers {
        samples.extend(worker.await?);
    }
    let seconds = started.elapsed().as_secs_f64();
    let mut latencies: Vec<f64> = samples
        .iter()
        .map(|sample| sample.latency.as_secs_f64() * 1000.0)
        .collect();
    latencies.sort_by(f64::total_cmp);
    let manifests: BTreeSet<_> = samples.iter().filter_map(|s| s.manifest.clone()).collect();
    Ok(LoadReport {
        requests: samples.len(),
        errors: samples.iter().filter(|sample| !sample.ok).count(),
        mixed: samples.iter().filter(|sample| sample.mixed).count(),
        manifests_seen: manifests.len(),
        seconds,
        rps: samples.len() as f64 / seconds,
        p50_ms: percentile(&latencies, 0.50),
        p99_ms: percentile(&latencies, 0.99),
        max_ms: latencies.last().copied().unwrap_or(0.0),
    })
}

async fn request(client: &reqwest::Client, target: &LoadTarget) -> Sample {
    let started = Instant::now();
    let mut builder = client.get(&target.url);
    if let Some(host) = &target.host {
        builder = builder.header(reqwest::header::HOST, host);
    }
    let result = async {
        let response = builder.send().await?;
        let ok = response.status().is_success();
        let manifest = response
            .headers()
            .get("x-livehost-manifest")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.text().await?;
        Ok::<_, reqwest::Error>((ok, manifest, body))
    }
    .await;
    let latency = started.elapsed();
    match result {
        Ok((ok, manifest, body)) => {
            // Pages print their manifest in a meta tag; it must be the one
            // the header names.
            let mixed = match (&manifest, body_manifest(&body)) {
                (Some(header), Some(body)) => header != body,
                _ => false,
            };
            Sample {
                latency,
                ok,
                mixed,
                manifest,
            }
        }
        Err(_) => Sample {
            latency,
            ok: false,
            mixed: false,
            manifest: None,
        },
    }
}

fn body_manifest(body: &str) -> Option<&str> {
    let marker = "name=\"livehost-manifest\" content=\"";
    let start = body.find(marker)? + marker.len();
    body.get(start..start + 64)
}

pub fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() as f64 - 1.0) * fraction).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}
