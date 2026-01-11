use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use adjutant::dspy::{
    AutoOptimizerConfig, PerformanceSummary, PerformanceTracker, SessionIndex, SessionStore,
};

#[derive(Clone, Debug)]
pub(crate) enum DspyConfigSource {
    Default,
    File,
    Error(String),
}

impl DspyConfigSource {
    pub(crate) fn label(&self) -> &str {
        match self {
            DspyConfigSource::Default => "Default",
            DspyConfigSource::File => "File",
            DspyConfigSource::Error(_) => "Error",
        }
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            DspyConfigSource::Error(message) => Some(message),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DspyAutoOptimizerSnapshot {
    pub(crate) config: AutoOptimizerConfig,
    pub(crate) config_path: Option<PathBuf>,
    pub(crate) source: DspyConfigSource,
}

impl DspyAutoOptimizerSnapshot {
    fn load() -> Self {
        let config_path = auto_optimizer_config_path();
        let mut source = DspyConfigSource::Default;
        let config = if let Some(path) = &config_path {
            match fs::read_to_string(path) {
                Ok(contents) => match serde_json::from_str::<AutoOptimizerConfig>(&contents) {
                    Ok(config) => {
                        source = DspyConfigSource::File;
                        config
                    }
                    Err(err) => {
                        source = DspyConfigSource::Error(format!(
                            "Failed to parse auto-optimizer config: {}",
                            err
                        ));
                        AutoOptimizerConfig::default()
                    }
                },
                Err(err) if err.kind() == ErrorKind::NotFound => AutoOptimizerConfig::default(),
                Err(err) => {
                    source = DspyConfigSource::Error(format!(
                        "Failed to read auto-optimizer config: {}",
                        err
                    ));
                    AutoOptimizerConfig::default()
                }
            }
        } else {
            source = DspyConfigSource::Error("No home directory available".to_string());
            AutoOptimizerConfig::default()
        };

        Self {
            config,
            config_path,
            source,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DspySessionSummary {
    pub(crate) total_sessions: usize,
    pub(crate) success_count: usize,
    pub(crate) failed_count: usize,
    pub(crate) interrupted_count: usize,
    pub(crate) success_rate: f32,
    pub(crate) last_optimization_ts: Option<u64>,
    pub(crate) updated_ts: u64,
}

impl DspySessionSummary {
    pub(crate) fn from_index(index: &SessionIndex) -> Self {
        let updated_ts = timestamp_to_u64(index.updated_at.timestamp());
        let last_optimization_ts =
            index.last_optimization.map(|stamp| timestamp_to_u64(stamp.timestamp()));
        Self {
            total_sessions: index.total_sessions,
            success_count: index.success_count,
            failed_count: index.failed_count,
            interrupted_count: index.interrupted_count,
            success_rate: index.success_rate(),
            last_optimization_ts,
            updated_ts,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DspyPerformanceSummary {
    pub(crate) total_decisions: usize,
    pub(crate) total_correct: usize,
    pub(crate) overall_accuracy: f32,
    pub(crate) complexity_accuracy: f32,
    pub(crate) delegation_accuracy: f32,
    pub(crate) rlm_accuracy: f32,
    pub(crate) optimization_count: usize,
    pub(crate) last_optimization_ts: Option<u64>,
    pub(crate) updated_ts: u64,
}

impl DspyPerformanceSummary {
    pub(crate) fn from_summary(summary: &PerformanceSummary, updated_ts: u64) -> Self {
        let last_optimization_ts = summary
            .last_optimization
            .map(|stamp| timestamp_to_u64(stamp.timestamp()));
        Self {
            total_decisions: summary.total_decisions,
            total_correct: summary.total_correct,
            overall_accuracy: summary.overall_accuracy,
            complexity_accuracy: summary.complexity_accuracy,
            delegation_accuracy: summary.delegation_accuracy,
            rlm_accuracy: summary.rlm_accuracy,
            optimization_count: summary.optimization_count,
            last_optimization_ts,
            updated_ts,
        }
    }

    fn from_tracker(tracker: &PerformanceTracker) -> Self {
        let summary = tracker.summary();
        let updated_ts = timestamp_to_u64(tracker.metrics().updated_at.timestamp());
        Self::from_summary(&summary, updated_ts)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DspySnapshot {
    pub(crate) sessions: Option<DspySessionSummary>,
    pub(crate) sessions_error: Option<String>,
    pub(crate) performance: Option<DspyPerformanceSummary>,
    pub(crate) performance_error: Option<String>,
    pub(crate) auto_optimizer: DspyAutoOptimizerSnapshot,
}

impl DspySnapshot {
    pub(crate) fn build() -> Self {
        let (sessions, sessions_error) = load_session_summary();
        let (performance, performance_error) = load_performance_summary();
        let auto_optimizer = DspyAutoOptimizerSnapshot::load();
        Self {
            sessions,
            sessions_error,
            performance,
            performance_error,
            auto_optimizer,
        }
    }
}

pub(crate) struct DspyState {
    pub(crate) snapshot: DspySnapshot,
}

impl DspyState {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: DspySnapshot::build(),
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.snapshot = DspySnapshot::build();
    }

    pub(crate) fn update_auto_optimizer<F>(&mut self, update: F) -> Result<(), String>
    where
        F: FnOnce(&mut AutoOptimizerConfig),
    {
        let mut config = self.snapshot.auto_optimizer.config.clone();
        update(&mut config);
        config.save().map_err(|err| err.to_string())?;
        self.refresh();
        Ok(())
    }
}

fn load_session_summary() -> (Option<DspySessionSummary>, Option<String>) {
    match SessionStore::open() {
        Ok(store) => (Some(DspySessionSummary::from_index(store.index())), None),
        Err(err) => (None, Some(err.to_string())),
    }
}

fn load_performance_summary() -> (Option<DspyPerformanceSummary>, Option<String>) {
    match PerformanceTracker::open() {
        Ok(tracker) => (Some(DspyPerformanceSummary::from_tracker(&tracker)), None),
        Err(err) => (None, Some(err.to_string())),
    }
}

fn auto_optimizer_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| {
        home.join(".openagents")
            .join("adjutant")
            .join("config")
            .join("auto_optimizer.json")
    })
}

fn timestamp_to_u64(timestamp: i64) -> u64 {
    if timestamp <= 0 {
        0
    } else {
        timestamp as u64
    }
}
