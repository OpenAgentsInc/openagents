//! The spectral sea's worker cost per tier (`docs/verse/water.md`, phase
//! W4): one tick of every cascade, the foam field, and the half-float
//! texels, as `verse_pbr::water::ocean` synthesizes them on its worker
//! thread, and the gameplay band `physics::water` transforms each tick.
//!
//! Usage: water_fft_cost [OUTPUT.json]
//!
//! Each tier and sea state runs 30 ticks to warm up, then 600 ticks two
//! apart (a 60 Hz display on the 120 Hz clock), timed one by one. Run it in
//! release, on a quiet machine.

use std::time::Instant;

use serde_json::json;
use verse_engine::quality::Tier;
use verse_pbr::water::{SeaState, ocean};

const WARM: u64 = 30;
const TIMED: u64 = 600;

fn stats(mut micros: Vec<f64>) -> serde_json::Value {
    micros.sort_by(f64::total_cmp);
    let at = |q: f64| micros[((micros.len() - 1) as f64 * q).round() as usize];
    json!({
        "mean_us": micros.iter().sum::<f64>() / micros.len() as f64,
        "p50_us": at(0.5),
        "p95_us": at(0.95),
        "max_us": at(1.0),
    })
}

fn main() -> Result<(), String> {
    let output = std::env::args().nth(1);
    let mut rows = Vec::new();
    for tier in [Tier::Low, Tier::Medium, Tier::High] {
        for state in SeaState::all() {
            let spectrum = state.spectrum(0.3, 0x5EA, 30.0);
            let built = Instant::now();
            let mut synthesis = ocean::Synthesis::new(&spectrum, tier)?;
            let build_ms = built.elapsed().as_secs_f64() * 1e3;
            for tick in 0..WARM {
                synthesis.step(tick * 2);
            }
            let micros: Vec<f64> = (WARM..WARM + TIMED)
                .map(|tick| {
                    let started = Instant::now();
                    std::hint::black_box(synthesis.step(tick * 2));
                    started.elapsed().as_secs_f64() * 1e6
                })
                .collect();
            let plan = synthesis.plan();
            let row = json!({
                "tier": tier.name(),
                "sea": state.name,
                "cascades": plan.count,
                "texels": plan.size,
                "upload_bytes": plan.bytes(),
                "build_ms": build_ms,
                "worker": stats(micros),
            });
            eprintln!("{row}");
            rows.push(row);
        }
    }
    // The gameplay band, as `physics::water` transforms it once a tick.
    let mut gameplay = Vec::new();
    for state in SeaState::all() {
        let spectrum = state.spectrum(0.3, 0x5EA, 30.0);
        let synth =
            physics::water::Synth::new(&spectrum, 1, physics::water::spectrum::GAMEPLAY_SIZE)?;
        let micros: Vec<f64> = (0..TIMED)
            .map(|tick| {
                let started = Instant::now();
                std::hint::black_box(synth.field(tick));
                started.elapsed().as_secs_f64() * 1e6
            })
            .collect();
        let row = json!({ "sea": state.name, "field": stats(micros) });
        eprintln!("{row}");
        gameplay.push(row);
    }
    let report = json!({
        "command": "cargo run --release -p verse-pbr --example water_fft_cost -- OUTPUT.json",
        "budget_us": { "low": 500, "medium": 1000, "high": 2000 },
        "tiers": rows,
        "gameplay_band": gameplay,
    });
    if let Some(path) = output {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
