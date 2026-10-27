//! Benchmark cases shared by the native runners (`render-lab-native` offscreen
//! and `render-lab-window` in a window), so both measure the same scenes.

use crate::Config;
use serde_json::{Value, json};

/// The heavy scene's settings, merged under each heavy case. The same values
/// are in `apps/aethyr/tools/measure-render-lab.mjs` for the browser run.
pub fn heavy_base() -> Value {
    json!({"instances": 500_000, "variants": 32, "crowd": 100_000, "camera": "overview"})
}

pub fn heavy() -> Vec<(&'static str, Value)> {
    [
        ("heavy-cpu-taa", json!({"path": "cpu"})),
        ("heavy-gpu-taa", json!({"path": "gpu"})),
        ("heavy-visbuffer-taa", json!({"path": "visbuffer"})),
        ("heavy-gpu-no-occlusion", json!({"path": "gpu", "occlusion": false})),
        ("heavy-visbuffer-no-occlusion", json!({"path": "visbuffer", "occlusion": false})),
        ("heavy-gpu-no-lod", json!({"path": "gpu", "lod": false})),
        ("heavy-visbuffer-no-lod", json!({"path": "visbuffer", "lod": false})),
        ("heavy-gpu-crowd-tiers-off", json!({"path": "gpu", "crowd-tiers": false})),
    ]
    .into_iter()
    .map(|(name, case)| {
        let mut merged = heavy_base();
        for (k, v) in case.as_object().unwrap() {
            merged[k] = v.clone();
        }
        (name, merged)
    })
    .collect()
}

/// The comparisons the web lab makes (decision 0007), at default scene size.
pub fn suite(name: &str) -> Vec<(&'static str, Value)> {
    let compare = vec![
        ("cpu-forward-taa", json!({"path": "cpu", "aa": "taa"})),
        ("gpu-forward-taa", json!({"path": "gpu", "aa": "taa"})),
        ("visbuffer-taa", json!({"path": "visbuffer", "aa": "taa"})),
        ("cpu-forward-msaa4", json!({"path": "cpu", "aa": "msaa4"})),
        ("gpu-forward-msaa4", json!({"path": "gpu", "aa": "msaa4"})),
        ("gpu-forward-no-aa", json!({"path": "gpu", "aa": "none"})),
        ("visbuffer-no-aa", json!({"path": "visbuffer", "aa": "none"})),
        ("gpu-forward-taa-no-occlusion", json!({"path": "gpu", "aa": "taa", "occlusion": false})),
        ("cpu-overview-taa", json!({"path": "cpu", "camera": "overview"})),
        ("visbuffer-overview-taa", json!({"path": "visbuffer", "camera": "overview"})),
        ("gpu-overview-crowd-tiers-off", json!({"path": "gpu", "camera": "overview", "crowd-tiers": false})),
        ("gpu-overview-crowd-tiers-on", json!({"path": "gpu", "camera": "overview", "crowd-tiers": true})),
    ];
    match name {
        "compare" => compare,
        "quick" => compare.into_iter().filter(|(n, _)| matches!(*n, "cpu-forward-taa" | "gpu-forward-taa" | "visbuffer-taa")).collect(),
        "heavy" => heavy(),
        "all" => compare.into_iter().chain(heavy()).collect(),
        other => panic!("unknown suite {other}"),
    }
}

/// The lab's default configuration with `overrides` merged over it, at a
/// fixed scene time unless the case sets one.
pub fn config_from(overrides: &Value) -> Config {
    let mut base = serde_json::to_value(Config::default()).unwrap();
    if let (Some(base), Some(over)) = (base.as_object_mut(), overrides.as_object()) {
        for (k, v) in over {
            base.insert(k.clone(), v.clone());
        }
    }
    // A fixed scene time keeps frames comparable across runs and machines.
    if base["time"].is_null() {
        base["time"] = json!(12.0);
    }
    serde_json::from_value(base).expect("config")
}

/// The middle value (upper median), or 0 for no samples.
pub fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// The sample at rank `round((n - 1) * p)` for `p` in 0..=1, or 0 for no samples.
pub fn percentile(mut v: Vec<f64>, p: f64) -> f64 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f64::total_cmp);
    v[((v.len() - 1) as f64 * p).round() as usize]
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Path, camera::CameraMode};

    #[test]
    fn every_suite_case_parses() {
        for name in ["compare", "quick", "heavy", "all"] {
            for (case, overrides) in suite(name) {
                let config = config_from(&overrides);
                assert_eq!(config.time, Some(12.0), "{case}");
            }
        }
    }

    #[test]
    fn heavy_cases_keep_the_heavy_scene() {
        let cases = heavy();
        assert_eq!(cases.len(), 8);
        for (case, overrides) in cases {
            let config = config_from(&overrides).normalized();
            assert_eq!(config.instances, 500_000, "{case}");
            assert_eq!(config.crowd, 100_000, "{case}");
            assert_eq!(config.camera, CameraMode::Overview, "{case}");
        }
        assert_eq!(config_from(&suite("heavy")[0].1).path, Path::Cpu);
    }

    #[test]
    fn percentiles_pick_nearest_ranks() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(median(v.clone()), 51.0);
        assert_eq!(percentile(v.clone(), 0.95), 95.0);
        assert_eq!(percentile(Vec::new(), 0.5), 0.0);
    }
}
