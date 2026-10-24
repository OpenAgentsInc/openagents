//! The detail controller: measured frame time in, one `Detail` value out.
//!
//! Samples are grouped into windows. Each full window yields its 90th
//! percentile, which is compared against the budget:
//!
//! - above `lower_ratio` times the budget, `Detail` drops by `step_down`
//!   (twice that when the window is 1.5 times over);
//! - at or below the raise ratio for `raise_after` windows in a row, it
//!   rises by `step_up`;
//! - in between, nothing changes and the run of good windows restarts.
//!
//! That dead band, the run requirement, and a smaller step up than down are
//! the hysteresis. A raise that overloads the frame within two windows
//! doubles `raise_after` (up to `max_raise_after`), so a workload that sits
//! on the edge between two levels settles on the lower one instead of
//! oscillating. A long quiet stretch halves it again.
//!
//! GPU timestamps measure headroom directly, so a GPU window counts as good
//! below `raise_ratio_gpu` (75% by default). Frame pacing can't show
//! headroom on a display that waits for vertical sync, so a pacing window
//! counts as good when it meets the budget (`raise_ratio_pacing`), and the
//! backoff keeps the resulting probes rare.

use serde::{Deserialize, Serialize};

use crate::{Detail, round6};

/// Tuning for one controller. The defaults are provisional until the
/// ADR 0007 qualification runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ControllerConfig {
    /// Wall-time budget per frame, for pacing samples.
    pub budget_ms: f64,
    /// GPU-time budget per frame, for timestamp samples.
    pub gpu_budget_ms: f64,
    /// The lowest `Detail` the controller sets. A zone's floor goes here.
    pub floor: f64,
    /// The highest `Detail` the controller sets.
    pub ceiling: f64,
    /// `Detail` before any measurement.
    pub start: f64,
    /// Samples per decision.
    pub window: u32,
    /// Samples ignored after start or reset (shader compilation, uploads).
    pub warmup: u32,
    /// Intervals longer than this are pauses (hidden tab, debugger), not load.
    pub gap_ms: f64,
    pub lower_ratio: f64,
    pub raise_ratio_gpu: f64,
    pub raise_ratio_pacing: f64,
    pub step_down: f64,
    pub step_up: f64,
    pub raise_after: u32,
    pub max_raise_after: u32,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            budget_ms: crate::Budget::FULL_MS,
            gpu_budget_ms: crate::Budget::FULL_MS * 0.8,
            floor: 0.0,
            ceiling: 1.0,
            start: 1.0,
            window: 30,
            warmup: 60,
            gap_ms: 250.0,
            lower_ratio: 1.1,
            raise_ratio_gpu: 0.75,
            raise_ratio_pacing: 1.05,
            step_down: 0.1,
            step_up: 0.05,
            raise_after: 4,
            max_raise_after: 64,
        }
    }
}

impl ControllerConfig {
    /// Budgets from a [`crate::Budget`], everything else default.
    pub fn for_budget(budget: &crate::Budget) -> Self {
        Self {
            budget_ms: budget.frame_ms,
            gpu_budget_ms: budget.gpu_ms,
            ..Self::default()
        }
    }

    fn sanitized(mut self) -> Self {
        let positive = |v: f64, d: f64| if v.is_finite() && v > 0.0 { v } else { d };
        let base = Self::default();
        self.budget_ms = positive(self.budget_ms, base.budget_ms);
        self.gpu_budget_ms = positive(self.gpu_budget_ms, base.gpu_budget_ms);
        self.gap_ms = positive(self.gap_ms, base.gap_ms);
        self.lower_ratio = positive(self.lower_ratio, base.lower_ratio);
        self.raise_ratio_gpu = positive(self.raise_ratio_gpu, base.raise_ratio_gpu);
        self.raise_ratio_pacing = positive(self.raise_ratio_pacing, base.raise_ratio_pacing);
        self.step_down = positive(self.step_down, base.step_down);
        self.step_up = positive(self.step_up, base.step_up);
        let unit = |v: f64, d: f64| if v.is_finite() { v.clamp(0.0, 1.0) } else { d };
        self.floor = unit(self.floor, 0.0);
        self.ceiling = unit(self.ceiling, 1.0).max(self.floor);
        self.start = unit(self.start, 1.0).clamp(self.floor, self.ceiling);
        self.window = self.window.max(1);
        self.raise_after = self.raise_after.max(1);
        self.max_raise_after = self.max_raise_after.max(self.raise_after);
        self
    }
}

/// One frame's measurement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// Time since the previous frame started.
    pub interval_ms: f64,
    /// GPU time for a recent frame, when the host has timestamps.
    #[serde(default)]
    pub gpu_ms: Option<f64>,
}

/// Which measurement the last window used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Gpu,
    Pacing,
}

/// Whether the frame holds its budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pressure {
    #[default]
    Holding,
    Lowering,
    /// Over budget with `Detail` already at the floor: the device can't hold
    /// this zone's budget, and the shell should say so.
    AtFloor,
}

/// What a report carries about the controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ControllerState {
    pub detail: Detail,
    pub floor: f64,
    pub ceiling: f64,
    pub source: Option<Source>,
    /// The last window's 90th percentile.
    pub window_ms: Option<f64>,
    pub budget_ms: f64,
    pub pressure: Pressure,
    pub changes: u32,
    pub windows: u64,
    pub raise_after: u32,
    /// `Detail` is held (for a capture or a benchmark) and does not move.
    pub held: bool,
}

#[derive(Debug, Clone)]
pub struct Controller {
    config: ControllerConfig,
    detail: f64,
    held: Option<f64>,
    samples: Vec<f64>,
    source: Option<Source>,
    warm: u32,
    good: u32,
    raise_after: u32,
    last_raise: Option<u64>,
    quiet: u64,
    windows: u64,
    changes: u32,
    window_ms: Option<f64>,
    last_source: Option<Source>,
    pressure: Pressure,
}

impl Controller {
    pub fn new(config: ControllerConfig) -> Self {
        let config = config.sanitized();
        Self {
            detail: config.start,
            held: None,
            samples: Vec::with_capacity(config.window as usize),
            source: None,
            warm: 0,
            good: 0,
            raise_after: config.raise_after,
            last_raise: None,
            quiet: 0,
            windows: 0,
            changes: 0,
            window_ms: None,
            last_source: None,
            pressure: Pressure::Holding,
            config,
        }
    }

    pub fn config(&self) -> &ControllerConfig {
        &self.config
    }

    /// The `Detail` subsystems should use now.
    pub fn detail(&self) -> Detail {
        Detail::new(self.held.unwrap_or(self.detail)).unwrap_or(Detail::FULL)
    }

    /// Pin `Detail` for a capture or a benchmark. Measurements are dropped
    /// while held, so the pinned frames don't teach the controller anything.
    pub fn hold(&mut self, detail: Detail) {
        self.held = Some(detail.get());
        self.samples.clear();
        self.source = None;
    }

    /// Resume control from where it was before the hold, with a new warmup.
    pub fn release(&mut self) {
        self.held = None;
        self.reset();
    }

    /// Start a new warmup without forgetting `Detail` or the backoff: after
    /// a resize, a zone change, or a hidden tab.
    pub fn reset(&mut self) {
        self.samples.clear();
        self.source = None;
        self.warm = 0;
        self.good = 0;
    }

    /// Feed one frame. Returns the new `Detail` when it changed.
    pub fn observe(&mut self, sample: Sample) -> Option<Detail> {
        if self.held.is_some() {
            return None;
        }
        let interval = sample.interval_ms;
        if !interval.is_finite() || interval < 0.0 {
            return None;
        }
        if interval > self.config.gap_ms {
            self.reset();
            return None;
        }
        if self.warm < self.config.warmup {
            self.warm += 1;
            return None;
        }
        let (source, ms) = match sample.gpu_ms {
            Some(gpu) if gpu.is_finite() && gpu >= 0.0 => (Source::Gpu, gpu),
            _ => (Source::Pacing, interval),
        };
        if self.source != Some(source) {
            self.samples.clear();
            self.source = Some(source);
        }
        self.samples.push(ms);
        if self.samples.len() < self.config.window as usize {
            return None;
        }
        self.decide(source)
    }

    fn decide(&mut self, source: Source) -> Option<Detail> {
        let stat = p90(&mut self.samples);
        self.samples.clear();
        self.windows += 1;
        self.quiet += 1;
        self.window_ms = Some(stat);
        self.last_source = Some(source);
        let c = &self.config;
        let (budget, raise_ratio) = match source {
            Source::Gpu => (c.gpu_budget_ms, c.raise_ratio_gpu),
            Source::Pacing => (c.budget_ms, c.raise_ratio_pacing),
        };
        let ratio = stat / budget;
        let before = self.detail;
        if ratio > c.lower_ratio {
            self.good = 0;
            self.quiet = 0;
            if self.last_raise.is_some_and(|at| self.windows - at <= 2) {
                self.raise_after = (self.raise_after * 2).min(c.max_raise_after);
            }
            self.last_raise = None;
            let step = if ratio > 1.5 {
                c.step_down * 2.0
            } else {
                c.step_down
            };
            self.detail = round6((self.detail - step).max(c.floor));
            self.pressure = if before <= c.floor {
                Pressure::AtFloor
            } else {
                Pressure::Lowering
            };
        } else {
            self.pressure = Pressure::Holding;
            if ratio <= raise_ratio {
                self.good += 1;
                if self.good >= self.raise_after && self.detail < c.ceiling {
                    self.detail = round6((self.detail + c.step_up).min(c.ceiling));
                    self.good = 0;
                    self.last_raise = Some(self.windows);
                }
            } else {
                self.good = 0;
            }
            if self.quiet >= 2 * u64::from(c.max_raise_after) {
                self.raise_after = (self.raise_after / 2).max(c.raise_after);
                self.quiet = 0;
            }
        }
        if self.detail != before {
            self.changes += 1;
            Some(self.detail())
        } else {
            None
        }
    }

    pub fn state(&self) -> ControllerState {
        ControllerState {
            detail: self.detail(),
            floor: self.config.floor,
            ceiling: self.config.ceiling,
            source: self.last_source,
            window_ms: self.window_ms.map(round6),
            budget_ms: match self.last_source {
                Some(Source::Gpu) => self.config.gpu_budget_ms,
                _ => self.config.budget_ms,
            },
            pressure: self.pressure,
            changes: self.changes,
            windows: self.windows,
            raise_after: self.raise_after,
            held: self.held.is_some(),
        }
    }
}

/// The 90th percentile, nearest rank.
fn p90(samples: &mut [f64]) -> f64 {
    samples.sort_by(f64::total_cmp);
    let n = samples.len();
    samples[(9 * n).div_ceil(10) - 1]
}
