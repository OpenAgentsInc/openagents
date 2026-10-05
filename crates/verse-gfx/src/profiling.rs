//! Bounded frame measurements that retain startup separately from steady work.
use std::collections::BTreeMap;

const SAMPLE_LIMIT: usize = 8192;
const SERIES_LIMIT: usize = 48;

#[derive(Default)]
pub struct Series {
    values: Vec<f64>,
    omitted: u64,
    invalid: u64,
}
impl Series {
    pub fn add(&mut self, value: f64) {
        if !value.is_finite() || value < 0. {
            self.invalid = self.invalid.saturating_add(1);
        } else if self.values.len() < SAMPLE_LIMIT {
            self.values.push(value);
        } else {
            self.omitted = self.omitted.saturating_add(1);
        }
    }
    pub fn summary(&self) -> serde_json::Value {
        let mut values = self.values.clone();
        values.sort_by(f64::total_cmp);
        let at = |p: f64| {
            (!values.is_empty()).then(|| values[((values.len() - 1) as f64 * p).ceil() as usize])
        };
        serde_json::json!({"samples":values.len(), "omitted":self.omitted,
            "invalid":self.invalid, "p50":at(0.5), "p95":at(0.95),
            "p99":at(0.99), "maximum":values.last()})
    }
}

/// Frame IDs belong to submission, so a delayed GPU result keeps its original phase.
pub struct FrameProfile {
    warmup_frames: u64,
    startup: BTreeMap<&'static str, Series>,
    steady: BTreeMap<&'static str, Series>,
    omitted_series: u64,
}
impl Default for FrameProfile {
    fn default() -> Self {
        Self::new(120)
    }
}
impl FrameProfile {
    pub fn new(warmup_frames: u64) -> Self {
        Self {
            warmup_frames,
            startup: BTreeMap::new(),
            steady: BTreeMap::new(),
            omitted_series: 0,
        }
    }
    pub fn warmup_frames(&self) -> u64 {
        self.warmup_frames
    }
    /// Use frame zero for construction; submitted frames start at one.
    pub fn record(&mut self, frame: u64, metric: &'static str, value: f64) {
        let phase = if frame <= self.warmup_frames {
            &mut self.startup
        } else {
            &mut self.steady
        };
        if phase.len() >= SERIES_LIMIT && !phase.contains_key(metric) {
            self.omitted_series = self.omitted_series.saturating_add(1);
            return;
        }
        phase.entry(metric).or_default().add(value);
    }
    pub fn summary(&self) -> serde_json::Value {
        let summarize = |phase: &BTreeMap<&'static str, Series>| {
            phase
                .iter()
                .map(|(name, series)| (*name, series.summary()))
                .collect::<BTreeMap<_, _>>()
        };
        serde_json::json!({"schema":"verse.frame.measurements.v1", "warmup_frames":self.warmup_frames,
            "startup":summarize(&self.startup), "steady":summarize(&self.steady),
            "omitted_series":self.omitted_series, "limits":{"series_per_phase":SERIES_LIMIT,"samples_per_series":SAMPLE_LIMIT},
            "sampling":"First bounded observations in each phase; omissions are counted."})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_gpu_samples_use_submission_phase_and_keep_startup_outliers() {
        let mut profile = FrameProfile::new(2);
        profile.record(3, "cpu_ms", 1.);
        profile.record(1, "gpu_ms", 500.);
        profile.record(3, "gpu_ms", 2.);
        profile.record(2, "cpu_ms", 200.);
        let result = profile.summary();
        assert_eq!(result["startup"]["gpu_ms"]["maximum"], 500.);
        assert_eq!(result["steady"]["gpu_ms"]["p99"], 2.);
        assert_eq!(result["steady"]["cpu_ms"]["samples"], 1);
    }
    #[test]
    fn omitted_and_invalid_observations_do_not_become_latency_values() {
        let mut series = Series::default();
        series.add(f64::NAN);
        series.add(-1.);
        assert!(series.summary()["p95"].is_null());
        for _ in 0..SAMPLE_LIMIT + 3 {
            series.add(4.);
        }
        let result = series.summary();
        assert_eq!(result["samples"], SAMPLE_LIMIT);
        assert_eq!(result["omitted"], 3);
        assert_eq!(result["invalid"], 2);
        assert_eq!(result["p99"], 4.);
    }
}
