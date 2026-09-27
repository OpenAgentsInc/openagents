//! Telemetry configuration loaded from data/config/telemetry.toml with env overrides.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct TelemetryCfg {
    pub log_level: Option<String>,
    pub json_logs: Option<bool>,
    pub metrics_addr: Option<String>,  // e.g., 127.0.0.1:9000
    pub otlp_endpoint: Option<String>, // e.g., http://localhost:4317
    pub trace_sample: Option<f64>,     // 0.0..1.0
    pub enable_client: Option<bool>,
}

impl Default for TelemetryCfg {
    fn default() -> Self {
        Self {
            log_level: Some("info".to_string()),
            json_logs: Some(true),
            metrics_addr: None,
            otlp_endpoint: None,
            trace_sample: Some(0.0),
            enable_client: Some(true),
        }
    }
}

fn data_root() -> PathBuf {
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ws = here.join("../../data");
    if ws.is_dir() { ws } else { here.join("data") }
}

pub fn load_default() -> Result<TelemetryCfg> {
    let path = data_root().join("config/telemetry.toml");
    let mut cfg = if path.is_file() {
        let txt =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str::<TelemetryCfg>(&txt).context("parse telemetry TOML")?
    } else {
        TelemetryCfg::default()
    };
    // Env overrides
    if let Ok(lvl) = std::env::var("LOG_LEVEL") {
        cfg.log_level = Some(lvl);
    }
    if let Ok(addr) = std::env::var("METRICS_ADDR") {
        cfg.metrics_addr = Some(addr);
    }
    if let Ok(ep) = std::env::var("OTLP_ENDPOINT") {
        cfg.otlp_endpoint = Some(ep);
    }
    if let Some(json) = std::env::var("JSON_LOGS").ok().and_then(|v| v.parse().ok()) {
        cfg.json_logs = Some(json);
    }
    if let Some(s) = std::env::var("TRACE_SAMPLE")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        cfg.trace_sample = Some(s);
    }
    Ok(cfg)
}
