//! Water admission and sustained-overrun policy. Gameplay and underwater
//! extinction do not depend on any optional effect in this policy.

use super::Tier;

/// Water's incremental cost per displayed frame, at 1920 × 1080.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaterBudget {
    pub gpu_ms: f64,
    pub gpu_bytes: u64,
    pub main_ms: f64,
    /// Amortized CPU time on the native synthesis worker.
    pub worker_ms: f64,
}

impl WaterBudget {
    #[must_use]
    pub const fn of(tier: Tier) -> Self {
        let mib = 1024 * 1024;
        match tier {
            Tier::Low => Self {
                gpu_ms: 1.5,
                gpu_bytes: 8 * mib,
                main_ms: 0.3,
                worker_ms: 0.5,
            },
            Tier::Medium => Self {
                gpu_ms: 2.5,
                gpu_bytes: 32 * mib,
                main_ms: 0.5,
                worker_ms: 1.0,
            },
            Tier::High => Self {
                gpu_ms: 4.0,
                gpu_bytes: 96 * mib,
                main_ms: 0.8,
                worker_ms: 2.0,
            },
        }
    }
}

/// Optional optics and visual update frequency. The surface, body tint,
/// underwater extinction, and authoritative water continue at every level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum WaterEffects {
    #[default]
    Full,
    NoSsr,
    NoMirror,
    NoCopies,
    HalfRate,
    QuarterRate,
    EighthRate,
}

impl WaterEffects {
    #[must_use]
    pub fn copies(self) -> bool {
        self < Self::NoCopies
    }
    #[must_use]
    pub fn mirror(self) -> bool {
        self < Self::NoMirror
    }
    #[must_use]
    pub fn ssr(self) -> bool {
        self == Self::Full
    }
    #[must_use]
    pub fn refresh_every(self) -> u32 {
        match self {
            Self::HalfRate => 2,
            Self::QuarterRate => 4,
            Self::EighthRate => 8,
            _ => 1,
        }
    }
    fn less(self) -> Self {
        match self {
            Self::Full => Self::NoSsr,
            Self::NoSsr => Self::NoMirror,
            Self::NoMirror => Self::NoCopies,
            Self::NoCopies => Self::HalfRate,
            Self::HalfRate => Self::QuarterRate,
            _ => Self::EighthRate,
        }
    }
}

/// A completed water measurement. Missing GPU timestamps remain unknown;
/// queue fences must not be passed here as if they were native timestamps.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WaterLoad {
    pub gpu_ms: Option<f64>,
    pub gpu_bytes: u64,
    pub main_ms: f64,
    pub worker_ms: f64,
}

/// A monotonic reduction for the current scene. Eight consecutive overruns
/// cause one reduction after 120 warm-up frames. A 60-frame cooldown keeps
/// delayed samples from reducing several effects for the same overload.
/// Creating a new renderer resets the policy; recovery does not flap optics.
#[derive(Clone, Debug)]
pub struct WaterPolicy {
    budget: WaterBudget,
    effects: WaterEffects,
    frames: u32,
    cpu_over: u32,
    gpu_over: u32,
    cooldown: u32,
    main_ema: f64,
    worker_ema: f64,
}

impl WaterPolicy {
    #[must_use]
    pub fn new(tier: Tier) -> Self {
        Self {
            budget: WaterBudget::of(tier),
            effects: WaterEffects::Full,
            frames: 0,
            cpu_over: 0,
            gpu_over: 0,
            cooldown: 0,
            main_ema: 0.0,
            worker_ema: 0.0,
        }
    }
    #[must_use]
    pub fn effects(&self) -> WaterEffects {
        self.effects
    }
    #[must_use]
    pub fn budget(&self) -> WaterBudget {
        self.budget
    }
    /// Returns whether the frame's optional effects changed. Residency is
    /// admitted immediately, including during shader and spectrum warm-up.
    pub fn observe(&mut self, load: WaterLoad) -> bool {
        let before = self.effects;
        self.frames = self.frames.saturating_add(1);
        // Smooth cadence bursts, so alternating synthesis and reuse frames
        // still reduce effects when their average exceeds the CPU budget.
        if load.main_ms.is_finite() && load.main_ms >= 0.0 {
            self.main_ema += 0.2 * (load.main_ms - self.main_ema);
        }
        if load.worker_ms.is_finite() && load.worker_ms >= 0.0 {
            self.worker_ema += 0.2 * (load.worker_ms - self.worker_ema);
        }
        if load.gpu_bytes > self.budget.gpu_bytes {
            self.effects = self.effects.max(WaterEffects::NoCopies);
        }
        if self.frames <= 120 || self.cooldown > 0 {
            self.cooldown = self.cooldown.saturating_sub(1);
            self.cpu_over = 0;
            self.gpu_over = 0;
            return before != self.effects;
        }
        let exceeds = |value: f64, limit: f64| value.is_finite() && value >= 0.0 && value > limit;
        let cpu = exceeds(self.main_ema, self.budget.main_ms)
            || exceeds(self.worker_ema, self.budget.worker_ms);
        self.cpu_over = if cpu {
            self.cpu_over.saturating_add(1)
        } else {
            0
        };
        // GPU readbacks arrive after the frames they measure. An absent or
        // invalid result is neutral; only a completed, valid result can
        // advance or reset this streak.
        if let Some(ms) = load.gpu_ms.filter(|ms| ms.is_finite() && *ms >= 0.0) {
            self.gpu_over = if exceeds(ms, self.budget.gpu_ms) {
                self.gpu_over.saturating_add(1)
            } else {
                0
            };
        }
        let cpu_due = self.cpu_over >= 8;
        if cpu_due || self.gpu_over >= 8 {
            self.effects = if cpu_due {
                self.effects.max(WaterEffects::NoCopies).less()
            } else {
                self.effects.less()
            };
            self.cpu_over = 0;
            self.gpu_over = 0;
            self.cooldown = 60;
        }
        before != self.effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn water_policy_admits_memory_immediately_and_preserves_the_surface() {
        for tier in [Tier::Low, Tier::Medium, Tier::High] {
            let mut policy = WaterPolicy::new(tier);
            assert!(policy.observe(WaterLoad {
                gpu_bytes: policy.budget.gpu_bytes + 1,
                ..WaterLoad::default()
            }));
            assert_eq!(policy.effects(), WaterEffects::NoCopies);
            assert!(!policy.effects().copies());
            assert_eq!(policy.effects().refresh_every(), 1);
        }
    }
    #[test]
    fn water_policy_requires_sustained_overrun_and_has_a_cooldown() {
        let mut policy = WaterPolicy::new(Tier::High);
        let bad = WaterLoad {
            gpu_ms: Some(5.0),
            ..WaterLoad::default()
        };
        for _ in 0..127 {
            assert!(!policy.observe(bad));
        }
        assert!(policy.observe(bad));
        assert_eq!(policy.effects(), WaterEffects::NoSsr);
        for _ in 0..67 {
            assert!(!policy.observe(bad));
        }
        assert!(policy.observe(bad));
        assert_eq!(policy.effects(), WaterEffects::NoMirror);
    }
    #[test]
    fn water_policy_counts_completed_gpu_samples_across_readback_gaps() {
        let mut policy = WaterPolicy::new(Tier::High);
        for _ in 0..120 {
            assert!(!policy.observe(WaterLoad::default()));
        }
        let bad = WaterLoad {
            gpu_ms: Some(5.0),
            ..WaterLoad::default()
        };
        for _ in 0..7 {
            assert!(!policy.observe(bad));
            for _ in 0..3 {
                assert!(!policy.observe(WaterLoad::default()));
            }
            assert!(!policy.observe(WaterLoad {
                gpu_ms: Some(f64::NAN),
                ..WaterLoad::default()
            }));
        }
        assert!(policy.observe(bad));
        assert_eq!(policy.effects(), WaterEffects::NoSsr);
    }
    #[test]
    fn water_policy_resets_gpu_streak_only_on_a_valid_within_budget_sample() {
        let mut policy = WaterPolicy::new(Tier::High);
        for _ in 0..120 {
            policy.observe(WaterLoad::default());
        }
        let bad = WaterLoad {
            gpu_ms: Some(5.0),
            ..WaterLoad::default()
        };
        for _ in 0..7 {
            assert!(!policy.observe(bad));
        }
        assert!(!policy.observe(WaterLoad {
            gpu_ms: Some(4.0),
            ..WaterLoad::default()
        }));
        for _ in 0..7 {
            assert!(!policy.observe(bad));
        }
        assert!(policy.observe(bad));
        assert_eq!(policy.effects(), WaterEffects::NoSsr);
    }
    #[test]
    fn water_policy_keeps_unknown_gpu_cost_unknown_and_ignores_invalid_samples() {
        let mut policy = WaterPolicy::new(Tier::Low);
        let invalid = WaterLoad {
            gpu_ms: Some(f64::NAN),
            main_ms: -1.0,
            worker_ms: f64::INFINITY,
            gpu_bytes: 0,
        };
        for _ in 0..1000 {
            assert!(!policy.observe(invalid));
        }
        assert_eq!(policy.effects(), WaterEffects::Full);
        // One expensive frame does not trip the policy. A sustained
        // alternating cadence whose mean is too high does.
        assert!(!policy.observe(WaterLoad {
            main_ms: 1.0,
            ..WaterLoad::default()
        }));
        for _ in 0..30 {
            policy.observe(WaterLoad::default());
        }
        assert_eq!(policy.effects(), WaterEffects::Full);
        for i in 0..30 {
            policy.observe(WaterLoad {
                main_ms: if i % 2 == 0 { 2.0 } else { 0.0 },
                ..WaterLoad::default()
            });
        }
        assert_eq!(policy.effects(), WaterEffects::HalfRate);
        for _ in 0..1000 {
            policy.observe(WaterLoad::default());
        }
        assert_eq!(policy.effects(), WaterEffects::HalfRate);
    }
}
