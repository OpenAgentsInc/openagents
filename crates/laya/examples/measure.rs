//! The numbers the measurement record cites: checkpoint load time, then
//! warm per-request `system_one` latency over a fixture's cases.
//!
//! ```text
//! cargo run --release --example measure -- ~/work/laya-artifacts/english \
//!     crates/laya/fixtures/requests-english.json 20
//! ```
//!
//! Run under `/usr/bin/time -l` (macOS) or `/usr/bin/time -v` (Linux) for
//! the peak resident set the record reports.

use std::process::ExitCode;
use std::time::Instant;

use laya::api::SystemOneRequest;
use laya::decision::DecisionModel;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: measure <checkpoint dir> <requests fixture> [iterations]");
        return ExitCode::from(2);
    }
    let iterations: usize = args.get(3).map_or(10, |s| s.parse().expect("iterations"));

    let load_start = Instant::now();
    let model = DecisionModel::load(std::path::Path::new(&args[1]), candle_core::Device::Cpu)
        .expect("checkpoint loads");
    eprintln!("load: {:?}", load_start.elapsed());

    let body = std::fs::read_to_string(&args[2]).expect("fixture file");
    let cases: serde_json::Value = serde_json::from_str(&body).expect("fixture json");
    let requests: Vec<(String, SystemOneRequest)> = cases
        .as_array()
        .expect("case array")
        .iter()
        .map(|case| {
            (
                case["name"].as_str().unwrap().to_string(),
                serde_json::from_value(case["request"].clone()).unwrap(),
            )
        })
        .collect();

    for (name, request) in &requests {
        let mut times = Vec::with_capacity(iterations);
        for _ in 0..iterations {
            let start = Instant::now();
            let out = model.system_one(request).expect("system_one");
            times.push(start.elapsed());
            std::hint::black_box(&out);
        }
        times.sort();
        eprintln!(
            "{name}: min {:?} median {:?} max {:?} ({} iterations)",
            times[0],
            times[times.len() / 2],
            times[times.len() - 1],
            iterations
        );
    }
    ExitCode::SUCCESS
}
