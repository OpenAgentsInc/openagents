//! Fixed storage for lifetime latency distributions; percentiles are bucket bounds.
const BOUNDS: [f64; 16] = [
    0.0001,
    0.00025,
    0.0005,
    0.001,
    0.002,
    0.004,
    0.008,
    0.016,
    1. / 30.,
    1. / 15.,
    0.125,
    0.250,
    0.500,
    1.,
    2.,
    f64::INFINITY,
];
#[derive(Clone, Debug, Default)]
pub struct Timing {
    pub count: u64,
    pub total_seconds: f64,
    pub maximum_seconds: f64,
    buckets: [u64; 16],
}
impl Timing {
    pub(super) fn record(&mut self, seconds: f64) {
        self.count += 1;
        self.total_seconds += seconds;
        self.maximum_seconds = self.maximum_seconds.max(seconds);
        let index = BOUNDS.partition_point(|bound| *bound < seconds);
        self.buckets[index] += 1;
    }
    /// An upper bound in seconds, over all observations since host startup.
    pub fn percentile(&self, fraction: f64) -> Option<f64> {
        if self.count == 0 || !fraction.is_finite() || !(0. ..=1.).contains(&fraction) {
            return None;
        }
        let rank = (self.count as f64 * fraction).ceil().max(1.) as u64;
        let mut count = 0;
        for (bound, bucket) in BOUNDS.into_iter().zip(self.buckets) {
            count += bucket;
            if count >= rank {
                return Some(bound.min(self.maximum_seconds));
            }
        }
        None
    }
}

/// Startup and steady distributions use the first 120 observations of each stage.
#[derive(Clone, Debug, Default)]
pub struct Phases {
    pub startup: Timing,
    pub steady: Timing,
}
impl Phases {
    pub const WARMUP_OBSERVATIONS: u64 = 120;
    pub(super) fn record(&mut self, seconds: f64) {
        if self.startup.count < Self::WARMUP_OBSERVATIONS {
            self.startup.record(seconds);
        } else {
            self.steady.record(seconds);
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn startup_storage_and_tick_outliers_stay_out_of_steady_percentiles() {
        let mut phases = super::Phases::default();
        for _ in 0..super::Phases::WARMUP_OBSERVATIONS {
            phases.record(0.7);
        }
        for _ in 0..100 {
            phases.record(0.004);
        }
        assert_eq!(phases.startup.maximum_seconds, 0.7);
        assert_eq!(phases.steady.percentile(0.99), Some(0.004));
        assert_eq!(phases.steady.count, 100);
    }
}
