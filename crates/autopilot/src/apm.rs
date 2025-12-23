//! APM (Actions Per Minute) calculation module
//!
//! Measures agent velocity and effectiveness using the formula:
//! APM = (messages + tool_calls) / duration_minutes
//!
//! This metric enables comparison between:
//! - Interactive Claude Code usage (~4.5 APM)
//! - Autonomous Autopilot runs (~19 APM)

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// APM statistics for various time windows
#[derive(Debug, Clone, Serialize, Deserialize)]
#[derive(Default)]
pub struct APMStats {
    /// APM for current session only
    pub session: Option<f64>,
    /// APM for last 1 hour
    pub last_1h: Option<f64>,
    /// APM for last 6 hours
    pub last_6h: Option<f64>,
    /// APM for last 24 hours
    pub last_1d: Option<f64>,
    /// APM for last 7 days
    pub last_1w: Option<f64>,
    /// APM for last 30 days
    pub last_1m: Option<f64>,
    /// APM for all recorded data
    pub lifetime: Option<f64>,
}

/// APM baseline for a project or category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APMBaseline {
    /// Baseline identifier (e.g., "openagents", "small-project", "autopilot-default")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Source type this baseline applies to
    pub source: APMSource,
    /// Expected median APM
    pub median_apm: f64,
    /// Minimum acceptable APM (below this triggers alert)
    pub min_apm: f64,
    /// Maximum expected APM (above this is excellent)
    pub max_apm: f64,
    /// When this baseline was established
    pub created_at: DateTime<Utc>,
    /// When this baseline was last updated
    pub updated_at: DateTime<Utc>,
    /// Sample size used to establish baseline
    pub sample_size: u32,
}

impl APMBaseline {
    /// Create a new baseline
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        source: APMSource,
        median_apm: f64,
    ) -> Self {
        let now = Utc::now();
        // Min is 20% below median, max is 50% above median by default
        let min_apm = median_apm * 0.8;
        let max_apm = median_apm * 1.5;

        Self {
            id: id.into(),
            name: name.into(),
            source,
            median_apm,
            min_apm,
            max_apm,
            created_at: now,
            updated_at: now,
            sample_size: 0,
        }
    }

    /// Create baseline with custom min/max thresholds
    pub fn with_thresholds(
        id: impl Into<String>,
        name: impl Into<String>,
        source: APMSource,
        median_apm: f64,
        min_apm: f64,
        max_apm: f64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            source,
            median_apm,
            min_apm,
            max_apm,
            created_at: now,
            updated_at: now,
            sample_size: 0,
        }
    }

    /// Check if APM is below baseline threshold
    pub fn is_below_baseline(&self, apm: f64) -> bool {
        apm < self.min_apm
    }

    /// Check if APM is above excellent threshold
    pub fn is_above_baseline(&self, apm: f64) -> bool {
        apm > self.max_apm
    }

    /// Get percentage deviation from median
    pub fn deviation_pct(&self, apm: f64) -> f64 {
        ((apm - self.median_apm) / self.median_apm) * 100.0
    }

    /// Get status relative to baseline
    pub fn status(&self, apm: f64) -> BaselineStatus {
        if self.is_below_baseline(apm) {
            BaselineStatus::BelowBaseline
        } else if self.is_above_baseline(apm) {
            BaselineStatus::AboveBaseline
        } else {
            BaselineStatus::Normal
        }
    }
}

/// Status of current APM relative to baseline
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BaselineStatus {
    /// APM is below minimum threshold (performance regression)
    BelowBaseline,
    /// APM is within expected range
    Normal,
    /// APM is above maximum threshold (excellent performance)
    AboveBaseline,
}

impl BaselineStatus {
    pub fn emoji(&self) -> &str {
        match self {
            Self::BelowBaseline => "⚠️",
            Self::Normal => "✓",
            Self::AboveBaseline => "⭐",
        }
    }

    pub fn color(&self) -> &str {
        match self {
            Self::BelowBaseline => "red",
            Self::Normal => "green",
            Self::AboveBaseline => "gold",
        }
    }
}


/// APM snapshot for a specific time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct APMSnapshot {
    pub timestamp: DateTime<Utc>,
    pub source: APMSource,
    pub window: APMWindow,
    pub apm: f64,
    pub actions: u32,
    pub duration_minutes: f64,
    pub messages: u32,
    pub tool_calls: u32,
}

/// Source of APM data
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum APMSource {
    /// Data from autopilot autonomous runs
    Autopilot,
    /// Data from interactive Claude Code sessions
    ClaudeCode,
    /// Combined data from both sources
    Combined,
}

impl APMSource {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Autopilot => "autopilot",
            Self::ClaudeCode => "claude_code",
            Self::Combined => "combined",
        }
    }
}

/// Time window for APM calculation
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum APMWindow {
    /// Current session only
    Session,
    /// Last 1 hour
    Hour1,
    /// Last 6 hours
    Hour6,
    /// Last 24 hours
    Day1,
    /// Last 7 days
    Week1,
    /// Last 30 days
    Month1,
    /// All recorded data
    Lifetime,
}

impl APMWindow {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Session => "session",
            Self::Hour1 => "1h",
            Self::Hour6 => "6h",
            Self::Day1 => "1d",
            Self::Week1 => "1w",
            Self::Month1 => "1m",
            Self::Lifetime => "lifetime",
        }
    }

    /// Get the duration for this window (None for Session and Lifetime)
    pub fn duration(&self) -> Option<Duration> {
        match self {
            Self::Session | Self::Lifetime => None,
            Self::Hour1 => Some(Duration::hours(1)),
            Self::Hour6 => Some(Duration::hours(6)),
            Self::Day1 => Some(Duration::hours(24)),
            Self::Week1 => Some(Duration::days(7)),
            Self::Month1 => Some(Duration::days(30)),
        }
    }
}

/// APM performance tier based on APM value
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum APMTier {
    /// 0-5 APM: Baseline performance
    Baseline,
    /// 5-15 APM: Active work
    Active,
    /// 15-30 APM: Productive performance
    Productive,
    /// 30-50 APM: High performance
    HighPerformance,
    /// 50+ APM: Elite performance
    Elite,
}

impl APMTier {
    /// Get the tier for a given APM value
    pub fn from_apm(apm: f64) -> Self {
        if apm < 5.0 {
            Self::Baseline
        } else if apm < 15.0 {
            Self::Active
        } else if apm < 30.0 {
            Self::Productive
        } else if apm < 50.0 {
            Self::HighPerformance
        } else {
            Self::Elite
        }
    }

    /// Get the color for this tier (for terminal/UI display)
    pub fn color(&self) -> &str {
        match self {
            Self::Baseline => "gray",
            Self::Active => "blue",
            Self::Productive => "green",
            Self::HighPerformance => "amber",
            Self::Elite => "gold",
        }
    }

    /// Get the name of this tier
    pub fn name(&self) -> &str {
        match self {
            Self::Baseline => "Baseline",
            Self::Active => "Active",
            Self::Productive => "Productive",
            Self::HighPerformance => "High Performance",
            Self::Elite => "Elite",
        }
    }
}

/// Calculate APM from raw metrics
///
/// # Arguments
/// * `messages` - Total number of user and assistant messages
/// * `tool_calls` - Total number of tool invocations
/// * `duration_minutes` - Wall-clock duration in minutes
///
/// # Returns
/// APM value or None if duration is zero/negative
pub fn calculate_apm(messages: u32, tool_calls: u32, duration_minutes: f64) -> Option<f64> {
    if duration_minutes <= 0.0 {
        return None;
    }

    let actions = messages + tool_calls;
    Some(actions as f64 / duration_minutes)
}

/// Calculate APM from timestamps
///
/// # Arguments
/// * `messages` - Total number of messages
/// * `tool_calls` - Total number of tool calls
/// * `start_time` - Start timestamp
/// * `end_time` - End timestamp
///
/// # Returns
/// APM value or None if duration is invalid
pub fn calculate_apm_from_timestamps(
    messages: u32,
    tool_calls: u32,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
) -> Option<f64> {
    let duration = end_time.signed_duration_since(start_time);
    let duration_minutes = duration.num_milliseconds() as f64 / 60_000.0;

    calculate_apm(messages, tool_calls, duration_minutes)
}

/// Session data for APM calculation
#[derive(Debug, Clone)]
pub struct SessionData {
    pub source: APMSource,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub messages: u32,
    pub tool_calls: u32,
}

impl SessionData {
    /// Calculate APM for this session
    pub fn apm(&self) -> Option<f64> {
        calculate_apm_from_timestamps(
            self.messages,
            self.tool_calls,
            self.start_time,
            self.end_time,
        )
    }

    /// Get total actions (messages + tool_calls)
    pub fn actions(&self) -> u32 {
        self.messages + self.tool_calls
    }

    /// Get duration in minutes
    pub fn duration_minutes(&self) -> f64 {
        let duration = self.end_time.signed_duration_since(self.start_time);
        duration.num_milliseconds() as f64 / 60_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_apm() {
        // 50 messages + 150 tool_calls = 200 actions over 10 minutes = 20 APM
        let apm = calculate_apm(50, 150, 10.0).unwrap();
        assert_eq!(apm, 20.0);
    }

    #[test]
    fn test_calculate_apm_zero_duration() {
        assert!(calculate_apm(50, 150, 0.0).is_none());
    }

    #[test]
    fn test_calculate_apm_negative_duration() {
        assert!(calculate_apm(50, 150, -5.0).is_none());
    }

    #[test]
    fn test_apm_tier_baseline() {
        assert_eq!(APMTier::from_apm(0.0), APMTier::Baseline);
        assert_eq!(APMTier::from_apm(4.9), APMTier::Baseline);
    }

    #[test]
    fn test_apm_tier_active() {
        assert_eq!(APMTier::from_apm(5.0), APMTier::Active);
        assert_eq!(APMTier::from_apm(10.0), APMTier::Active);
        assert_eq!(APMTier::from_apm(14.9), APMTier::Active);
    }

    #[test]
    fn test_apm_tier_productive() {
        assert_eq!(APMTier::from_apm(15.0), APMTier::Productive);
        assert_eq!(APMTier::from_apm(20.0), APMTier::Productive);
        assert_eq!(APMTier::from_apm(29.9), APMTier::Productive);
    }

    #[test]
    fn test_apm_tier_high_performance() {
        assert_eq!(APMTier::from_apm(30.0), APMTier::HighPerformance);
        assert_eq!(APMTier::from_apm(40.0), APMTier::HighPerformance);
        assert_eq!(APMTier::from_apm(49.9), APMTier::HighPerformance);
    }

    #[test]
    fn test_apm_tier_elite() {
        assert_eq!(APMTier::from_apm(50.0), APMTier::Elite);
        assert_eq!(APMTier::from_apm(100.0), APMTier::Elite);
    }

    #[test]
    fn test_session_data_apm() {
        let start = Utc::now();
        let end = start + Duration::minutes(10);

        let session = SessionData {
            source: APMSource::Autopilot,
            start_time: start,
            end_time: end,
            messages: 50,
            tool_calls: 150,
        };

        assert_eq!(session.actions(), 200);
        assert!((session.duration_minutes() - 10.0).abs() < 0.001);
        assert_eq!(session.apm().unwrap(), 20.0);
    }

    #[test]
    fn test_apm_source_as_str() {
        assert_eq!(APMSource::Autopilot.as_str(), "autopilot");
        assert_eq!(APMSource::ClaudeCode.as_str(), "claude_code");
        assert_eq!(APMSource::Combined.as_str(), "combined");
    }

    #[test]
    fn test_apm_window_as_str() {
        assert_eq!(APMWindow::Session.as_str(), "session");
        assert_eq!(APMWindow::Hour1.as_str(), "1h");
        assert_eq!(APMWindow::Hour6.as_str(), "6h");
        assert_eq!(APMWindow::Day1.as_str(), "1d");
        assert_eq!(APMWindow::Week1.as_str(), "1w");
        assert_eq!(APMWindow::Month1.as_str(), "1m");
        assert_eq!(APMWindow::Lifetime.as_str(), "lifetime");
    }
}
