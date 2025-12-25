//! Benchmark local inference backends against live servers.
//!
//! Run with:
//!   GPT_OSS_BENCH_URL=http://localhost:8000 \
//!   FM_BRIDGE_BENCH_URL=http://localhost:3030 \
//!   cargo bench -p local-inference --bench backend_overhead

use std::env;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use local_inference::{CompletionRequest, LocalModelBackend};
use tokio::runtime::Runtime;

fn bench_backend_overhead(c: &mut Criterion) {
    let gpt_url = env::var("GPT_OSS_BENCH_URL")
        .or_else(|_| env::var("GPT_OSS_URL"))
        .ok();
    let fm_url = env::var("FM_BRIDGE_BENCH_URL")
        .or_else(|_| env::var("FM_BRIDGE_URL"))
        .ok();

    if gpt_url.is_none() && fm_url.is_none() {
        eprintln!("Set GPT_OSS_BENCH_URL or FM_BRIDGE_BENCH_URL to run benchmarks.");
        return;
    }

    let rt = Runtime::new().expect("Failed to create Tokio runtime");
    let prompt = "Summarize this in one sentence.";

    let mut group = c.benchmark_group("local_inference_overhead");
    group.sample_size(10);

    if let Some(url) = gpt_url {
        let model = env::var("GPT_OSS_BENCH_MODEL").unwrap_or_else(|_| "gpt-oss-20b".to_string());
        let mut client = gpt_oss::GptOssClient::builder()
            .base_url(url)
            .default_model(&model)
            .build()
            .expect("Failed to build GPT-OSS client");

        if let Err(err) = rt.block_on(LocalModelBackend::initialize(&mut client)) {
            eprintln!("Skipping GPT-OSS benchmark: {}", err);
        } else {
            group.bench_function(BenchmarkId::new("gpt-oss", &model), |b| {
                b.iter(|| {
                    let request = CompletionRequest::new(&model, prompt)
                        .with_max_tokens(32)
                        .with_temperature(0.2);
                    let response = rt
                        .block_on(LocalModelBackend::complete(&client, request))
                        .expect("GPT-OSS completion failed");
                    black_box(response);
                })
            });
        }
    }

    if let Some(url) = fm_url {
        let model = env::var("FM_BRIDGE_BENCH_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini-2024-07-18".to_string());
        let mut client = fm_bridge::FMClient::builder()
            .base_url(url)
            .default_model(&model)
            .build()
            .expect("Failed to build FM bridge client");

        if let Err(err) = rt.block_on(LocalModelBackend::initialize(&mut client)) {
            eprintln!("Skipping FM bridge benchmark: {}", err);
        } else {
            group.bench_function(BenchmarkId::new("fm-bridge", &model), |b| {
                b.iter(|| {
                    let request = CompletionRequest::new(&model, prompt)
                        .with_max_tokens(32)
                        .with_temperature(0.2);
                    let response = rt
                        .block_on(LocalModelBackend::complete(&client, request))
                        .expect("FM bridge completion failed");
                    black_box(response);
                })
            });
        }
    }

    group.finish();
}

criterion_group!(benches, bench_backend_overhead);
criterion_main!(benches);
