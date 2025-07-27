//! # APM Analysis Module
//! 
//! This module provides Actions Per Minute (APM) analysis functionality
//! for Claude Code sessions, including historical data processing and
//! real-time metrics calculation.

pub mod models;
pub mod analyzer;
pub mod historical;
pub mod combined;
pub mod utils;

// Re-export commonly used types
pub use models::{APMStats, CombinedAPMStats, HistoricalAPMResponse};
pub use analyzer::APMAnalyzer;
pub use historical::generate_historical_apm_data;
pub use combined::{combine_apm_stats, fetch_convex_apm_stats};