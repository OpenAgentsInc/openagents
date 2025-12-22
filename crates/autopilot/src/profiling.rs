//! Performance profiling integration for autopilot
//!
//! Provides CPU and memory profiling capabilities with flamegraph generation,
//! hotspot detection, and historical comparison for performance optimization.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Profiling configuration for a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingConfig {
    /// Enable CPU profiling
    pub enable_cpu: bool,

    /// Enable memory profiling
    pub enable_memory: bool,

    /// Sampling frequency for CPU profiling (Hz)
    pub cpu_sample_rate: u32,

    /// Output directory for profiling artifacts
    pub output_dir: PathBuf,

    /// Automatically generate flamegraph
    pub auto_generate_flamegraph: bool,
}

impl Default for ProfilingConfig {
    fn default() -> Self {
        Self {
            enable_cpu: false,
            enable_memory: false,
            cpu_sample_rate: 99, // 99 Hz is common for flamegraphs
            output_dir: PathBuf::from("./profiling"),
            auto_generate_flamegraph: true,
        }
    }
}

/// Performance hotspot detected in profiling data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotspot {
    /// Function or module name
    pub name: String,

    /// Percentage of total time spent (0.0-100.0)
    pub percentage: f32,

    /// Absolute time in milliseconds
    pub time_ms: u64,

    /// Number of samples
    pub samples: u64,

    /// Category (e.g., "tool_execution", "network", "database")
    pub category: Option<String>,
}

impl Hotspot {
    /// Check if this hotspot is significant (>5% of total time)
    pub fn is_significant(&self) -> bool {
        self.percentage > 5.0
    }

    /// Get severity level: critical (>20%), high (>10%), medium (>5%), low (<5%)
    pub fn severity(&self) -> &'static str {
        if self.percentage > 20.0 {
            "critical"
        } else if self.percentage > 10.0 {
            "high"
        } else if self.percentage > 5.0 {
            "medium"
        } else {
            "low"
        }
    }
}

/// Profiling session results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingSession {
    /// Session ID
    pub session_id: String,

    /// Start time
    pub started_at: String,

    /// End time
    pub ended_at: Option<String>,

    /// Total duration
    pub duration: Duration,

    /// CPU profiling results
    pub cpu_profile: Option<CpuProfile>,

    /// Memory profiling results
    pub memory_profile: Option<MemoryProfile>,

    /// Detected hotspots
    pub hotspots: Vec<Hotspot>,

    /// Flamegraph SVG path
    pub flamegraph_path: Option<PathBuf>,
}

/// CPU profiling results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuProfile {
    /// Total samples collected
    pub total_samples: u64,

    /// Sample rate (Hz)
    pub sample_rate: u32,

    /// Top functions by sample count
    pub top_functions: Vec<(String, u64)>,

    /// Perf data file path
    pub perf_data_path: Option<PathBuf>,
}

/// Memory profiling results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryProfile {
    /// Peak memory usage (bytes)
    pub peak_bytes: u64,

    /// Total allocations
    pub total_allocations: u64,

    /// Total deallocations
    pub total_deallocations: u64,

    /// Top allocators by size
    pub top_allocators: Vec<(String, u64)>,

    /// DHAT heap file path
    pub dhat_path: Option<PathBuf>,
}

/// Profiler for autopilot sessions
pub struct Profiler {
    config: ProfilingConfig,
    session_id: String,
    start_time: Option<Instant>,
    #[allow(dead_code)]
    perf_pid: Option<u32>,
}

impl Profiler {
    /// Create a new profiler for a session
    pub fn new(session_id: impl Into<String>, config: ProfilingConfig) -> Result<Self> {
        let session_id = session_id.into();

        // Create output directory
        std::fs::create_dir_all(&config.output_dir)
            .context("Failed to create profiling output directory")?;

        Ok(Self {
            config,
            session_id,
            start_time: None,
            perf_pid: None,
        })
    }

    /// Start profiling
    pub fn start(&mut self) -> Result<()> {
        self.start_time = Some(Instant::now());

        if self.config.enable_cpu {
            self.start_cpu_profiling()?;
        }

        if self.config.enable_memory {
            self.start_memory_profiling()?;
        }

        Ok(())
    }

    /// Stop profiling and return results
    pub fn stop(&mut self) -> Result<ProfilingSession> {
        let start_time = self.start_time.ok_or_else(|| {
            anyhow::anyhow!("Profiling not started")
        })?;

        let duration = start_time.elapsed();

        let mut session = ProfilingSession {
            session_id: self.session_id.clone(),
            started_at: chrono::Utc::now().to_rfc3339(),
            ended_at: Some(chrono::Utc::now().to_rfc3339()),
            duration,
            cpu_profile: None,
            memory_profile: None,
            hotspots: Vec::new(),
            flamegraph_path: None,
        };

        if self.config.enable_cpu {
            session.cpu_profile = Some(self.stop_cpu_profiling()?);

            if self.config.auto_generate_flamegraph {
                session.flamegraph_path = Some(self.generate_flamegraph()?);
            }
        }

        if self.config.enable_memory {
            session.memory_profile = Some(self.stop_memory_profiling()?);
        }

        // Detect hotspots
        session.hotspots = self.detect_hotspots(&session)?;

        Ok(session)
    }

    /// Start CPU profiling with perf
    fn start_cpu_profiling(&mut self) -> Result<()> {
        // NOTE: This is a placeholder for perf integration
        // In production, would use:
        // perf record -F {sample_rate} -g -p {pid} -o {output}

        // For now, just track that CPU profiling is enabled
        println!("CPU profiling enabled (sample rate: {} Hz)", self.config.cpu_sample_rate);

        Ok(())
    }

    /// Stop CPU profiling
    fn stop_cpu_profiling(&mut self) -> Result<CpuProfile> {
        // NOTE: This is a placeholder
        // In production, would stop perf and parse perf.data

        Ok(CpuProfile {
            total_samples: 0,
            sample_rate: self.config.cpu_sample_rate,
            top_functions: Vec::new(),
            perf_data_path: None,
        })
    }

    /// Start memory profiling with DHAT
    fn start_memory_profiling(&mut self) -> Result<()> {
        // NOTE: This is a placeholder for DHAT integration
        // In production, would compile with dhat feature and track allocations

        println!("Memory profiling enabled");

        Ok(())
    }

    /// Stop memory profiling
    fn stop_memory_profiling(&mut self) -> Result<MemoryProfile> {
        // NOTE: This is a placeholder
        // In production, would read dhat-heap.json

        Ok(MemoryProfile {
            peak_bytes: 0,
            total_allocations: 0,
            total_deallocations: 0,
            top_allocators: Vec::new(),
            dhat_path: None,
        })
    }

    /// Generate flamegraph from profiling data
    fn generate_flamegraph(&self) -> Result<PathBuf> {
        let output_path = self.config.output_dir.join(format!("{}.svg", self.session_id));

        // NOTE: This is a placeholder
        // In production, would run:
        // perf script | stackcollapse-perf.pl | flamegraph.pl > output.svg

        println!("Flamegraph would be generated at: {}", output_path.display());

        Ok(output_path)
    }

    /// Detect performance hotspots from profiling data
    fn detect_hotspots(&self, session: &ProfilingSession) -> Result<Vec<Hotspot>> {
        let mut hotspots = Vec::new();

        // Analyze CPU profile
        if let Some(ref cpu_profile) = session.cpu_profile {
            for (func, samples) in &cpu_profile.top_functions {
                let percentage = if cpu_profile.total_samples > 0 {
                    (*samples as f32 / cpu_profile.total_samples as f32) * 100.0
                } else {
                    0.0
                };

                if percentage > 1.0 {
                    // Only report functions >1% of total time
                    hotspots.push(Hotspot {
                        name: func.clone(),
                        percentage,
                        time_ms: 0, // Would calculate from samples and duration
                        samples: *samples,
                        category: Self::categorize_function(func),
                    });
                }
            }
        }

        // Sort by percentage descending
        hotspots.sort_by(|a, b| b.percentage.partial_cmp(&a.percentage).unwrap());

        Ok(hotspots)
    }

    /// Categorize a function into a performance category
    fn categorize_function(func: &str) -> Option<String> {
        if func.contains("tokio") || func.contains("async") {
            Some("async_runtime".to_string())
        } else if func.contains("reqwest") || func.contains("http") {
            Some("network".to_string())
        } else if func.contains("rusqlite") || func.contains("sql") {
            Some("database".to_string())
        } else if func.contains("serde") || func.contains("json") {
            Some("serialization".to_string())
        } else if func.contains("Read") || func.contains("Write") || func.contains("Edit") {
            Some("tool_execution".to_string())
        } else {
            None
        }
    }
}

/// Compare two profiling sessions to identify regressions
pub fn compare_sessions(
    baseline: &ProfilingSession,
    current: &ProfilingSession,
) -> ProfilingComparison {
    let mut regressions = Vec::new();
    let mut improvements = Vec::new();

    // Compare hotspots
    let baseline_hotspots: HashMap<&str, &Hotspot> = baseline
        .hotspots
        .iter()
        .map(|h| (h.name.as_str(), h))
        .collect();

    for hotspot in &current.hotspots {
        if let Some(baseline_hotspot) = baseline_hotspots.get(hotspot.name.as_str()) {
            let diff = hotspot.percentage - baseline_hotspot.percentage;

            if diff > 2.0 {
                // >2% increase is a regression
                regressions.push(HotspotChange {
                    name: hotspot.name.clone(),
                    baseline_percentage: baseline_hotspot.percentage,
                    current_percentage: hotspot.percentage,
                    diff,
                });
            } else if diff < -2.0 {
                // >2% decrease is an improvement
                improvements.push(HotspotChange {
                    name: hotspot.name.clone(),
                    baseline_percentage: baseline_hotspot.percentage,
                    current_percentage: hotspot.percentage,
                    diff,
                });
            }
        }
    }

    ProfilingComparison {
        baseline_session_id: baseline.session_id.clone(),
        current_session_id: current.session_id.clone(),
        duration_diff: current.duration.as_secs_f64() - baseline.duration.as_secs_f64(),
        regressions,
        improvements,
    }
}

/// Comparison between two profiling sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfilingComparison {
    pub baseline_session_id: String,
    pub current_session_id: String,
    pub duration_diff: f64, // Seconds
    pub regressions: Vec<HotspotChange>,
    pub improvements: Vec<HotspotChange>,
}

impl ProfilingComparison {
    /// Check if there are significant regressions
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Get summary message
    pub fn summary(&self) -> String {
        format!(
            "Duration: {:+.2}s | Regressions: {} | Improvements: {}",
            self.duration_diff,
            self.regressions.len(),
            self.improvements.len()
        )
    }
}

/// Change in hotspot performance between sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotChange {
    pub name: String,
    pub baseline_percentage: f32,
    pub current_percentage: f32,
    pub diff: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiling_config_default() {
        let config = ProfilingConfig::default();
        assert!(!config.enable_cpu);
        assert!(!config.enable_memory);
        assert_eq!(config.cpu_sample_rate, 99);
    }

    #[test]
    fn test_hotspot_is_significant() {
        let hotspot = Hotspot {
            name: "test_function".to_string(),
            percentage: 6.0,
            time_ms: 600,
            samples: 600,
            category: None,
        };

        assert!(hotspot.is_significant());

        let insignificant = Hotspot {
            name: "other_function".to_string(),
            percentage: 3.0,
            time_ms: 300,
            samples: 300,
            category: None,
        };

        assert!(!insignificant.is_significant());
    }

    #[test]
    fn test_hotspot_severity() {
        let critical = Hotspot {
            name: "func".to_string(),
            percentage: 25.0,
            time_ms: 0,
            samples: 0,
            category: None,
        };
        assert_eq!(critical.severity(), "critical");

        let high = Hotspot {
            name: "func".to_string(),
            percentage: 15.0,
            time_ms: 0,
            samples: 0,
            category: None,
        };
        assert_eq!(high.severity(), "high");

        let medium = Hotspot {
            name: "func".to_string(),
            percentage: 7.0,
            time_ms: 0,
            samples: 0,
            category: None,
        };
        assert_eq!(medium.severity(), "medium");

        let low = Hotspot {
            name: "func".to_string(),
            percentage: 2.0,
            time_ms: 0,
            samples: 0,
            category: None,
        };
        assert_eq!(low.severity(), "low");
    }

    #[test]
    fn test_categorize_function() {
        assert_eq!(
            Profiler::categorize_function("tokio::runtime::spawn"),
            Some("async_runtime".to_string())
        );

        assert_eq!(
            Profiler::categorize_function("reqwest::blocking::get"),
            Some("network".to_string())
        );

        assert_eq!(
            Profiler::categorize_function("rusqlite::Connection::execute"),
            Some("database".to_string())
        );

        assert_eq!(
            Profiler::categorize_function("serde_json::to_string"),
            Some("serialization".to_string())
        );

        assert_eq!(
            Profiler::categorize_function("Edit::apply"),
            Some("tool_execution".to_string())
        );

        assert_eq!(Profiler::categorize_function("other::function"), None);
    }

    #[test]
    fn test_compare_sessions() {
        let baseline = ProfilingSession {
            session_id: "session1".to_string(),
            started_at: "2024-01-01T00:00:00Z".to_string(),
            ended_at: Some("2024-01-01T00:01:00Z".to_string()),
            duration: Duration::from_secs(60),
            cpu_profile: None,
            memory_profile: None,
            hotspots: vec![
                Hotspot {
                    name: "func_a".to_string(),
                    percentage: 10.0,
                    time_ms: 6000,
                    samples: 600,
                    category: None,
                },
                Hotspot {
                    name: "func_b".to_string(),
                    percentage: 15.0,
                    time_ms: 9000,
                    samples: 900,
                    category: None,
                },
            ],
            flamegraph_path: None,
        };

        let current = ProfilingSession {
            session_id: "session2".to_string(),
            started_at: "2024-01-01T00:02:00Z".to_string(),
            ended_at: Some("2024-01-01T00:03:00Z".to_string()),
            duration: Duration::from_secs(65),
            cpu_profile: None,
            memory_profile: None,
            hotspots: vec![
                Hotspot {
                    name: "func_a".to_string(),
                    percentage: 13.0, // +3% regression
                    time_ms: 8450,
                    samples: 845,
                    category: None,
                },
                Hotspot {
                    name: "func_b".to_string(),
                    percentage: 12.0, // -3% improvement
                    time_ms: 7800,
                    samples: 780,
                    category: None,
                },
            ],
            flamegraph_path: None,
        };

        let comparison = compare_sessions(&baseline, &current);

        assert_eq!(comparison.regressions.len(), 1);
        assert_eq!(comparison.improvements.len(), 1);
        assert_eq!(comparison.regressions[0].name, "func_a");
        assert_eq!(comparison.improvements[0].name, "func_b");
    }

    #[test]
    fn test_profiling_comparison_summary() {
        let comparison = ProfilingComparison {
            baseline_session_id: "session1".to_string(),
            current_session_id: "session2".to_string(),
            duration_diff: 2.5,
            regressions: vec![],
            improvements: vec![],
        };

        let summary = comparison.summary();
        assert!(summary.contains("+2.50s"));
        assert!(summary.contains("Regressions: 0"));
        assert!(summary.contains("Improvements: 0"));
    }
}
