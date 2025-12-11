//! Configuration for telemetry initialization.

use std::env;

/// Log output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// JSON format (bunyan-style) - machine readable, good for log aggregation.
    Json,
    /// Pretty format - human readable, good for local development.
    #[default]
    Pretty,
}

impl LogFormat {
    /// Parse log format from a string.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" | "bunyan" => Self::Json,
            "pretty" | "human" | "text" => Self::Pretty,
            _ => Self::Pretty,
        }
    }

    /// Parse log format from environment variable `LOG_FORMAT`.
    pub fn from_env() -> Self {
        env::var("LOG_FORMAT")
            .map(|s| Self::from_str(&s))
            .unwrap_or_default()
    }
}

/// OpenTelemetry configuration.
#[derive(Debug, Clone, Default)]
pub struct OtelConfig {
    /// OTLP endpoint URL (e.g., "http://localhost:4317").
    pub endpoint: Option<String>,
    /// Service name for traces.
    pub service_name: Option<String>,
}

impl OtelConfig {
    /// Create OtelConfig from environment variables.
    pub fn from_env() -> Self {
        Self {
            endpoint: env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            service_name: env::var("OTEL_SERVICE_NAME").ok(),
        }
    }

    /// Check if OpenTelemetry export is enabled.
    pub fn is_enabled(&self) -> bool {
        self.endpoint.is_some()
    }
}

/// Telemetry configuration.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Log filter directive (e.g., "info", "debug", "my_crate=trace").
    pub filter: String,
    /// Output format.
    pub format: LogFormat,
    /// OpenTelemetry configuration.
    pub otel: OtelConfig,
    /// Test mode - if true and TEST_LOG is not set, logs are suppressed.
    pub test_mode: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            filter: "info".to_string(),
            format: LogFormat::default(),
            otel: OtelConfig::default(),
            test_mode: false,
        }
    }
}

impl TelemetryConfig {
    /// Create a new TelemetryConfig with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create TelemetryConfig from environment variables.
    ///
    /// Reads:
    /// - `RUST_LOG` - log filter directive (default: "info")
    /// - `LOG_FORMAT` - output format: "json" or "pretty" (default: "pretty")
    /// - `OTEL_EXPORTER_OTLP_ENDPOINT` - enables OTLP export if set
    /// - `OTEL_SERVICE_NAME` - service name for traces
    /// - `TEST_LOG` - if set, enables logs in test mode
    pub fn from_env() -> Self {
        Self {
            filter: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            format: LogFormat::from_env(),
            otel: OtelConfig::from_env(),
            test_mode: false,
        }
    }

    /// Set the log filter directive.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = filter.into();
        self
    }

    /// Set the output format.
    pub fn with_format(mut self, format: LogFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the OpenTelemetry endpoint.
    pub fn with_otel_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.otel.endpoint = Some(endpoint.into());
        self
    }

    /// Set the service name for OpenTelemetry.
    pub fn with_service_name(mut self, name: impl Into<String>) -> Self {
        self.otel.service_name = Some(name.into());
        self
    }

    /// Enable test mode.
    pub fn with_test_mode(mut self, test_mode: bool) -> Self {
        self.test_mode = test_mode;
        self
    }

    /// Check if logs should be suppressed (test mode without TEST_LOG).
    pub fn should_suppress_logs(&self) -> bool {
        self.test_mode && env::var("TEST_LOG").is_err()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_from_str() {
        assert_eq!(LogFormat::from_str("json"), LogFormat::Json);
        assert_eq!(LogFormat::from_str("JSON"), LogFormat::Json);
        assert_eq!(LogFormat::from_str("bunyan"), LogFormat::Json);
        assert_eq!(LogFormat::from_str("pretty"), LogFormat::Pretty);
        assert_eq!(LogFormat::from_str("PRETTY"), LogFormat::Pretty);
        assert_eq!(LogFormat::from_str("human"), LogFormat::Pretty);
        assert_eq!(LogFormat::from_str("text"), LogFormat::Pretty);
        assert_eq!(LogFormat::from_str("unknown"), LogFormat::Pretty);
    }

    #[test]
    fn test_config_builder() {
        let config = TelemetryConfig::new()
            .with_filter("debug")
            .with_format(LogFormat::Json)
            .with_otel_endpoint("http://localhost:4317")
            .with_service_name("test-service")
            .with_test_mode(true);

        assert_eq!(config.filter, "debug");
        assert_eq!(config.format, LogFormat::Json);
        assert_eq!(
            config.otel.endpoint,
            Some("http://localhost:4317".to_string())
        );
        assert_eq!(config.otel.service_name, Some("test-service".to_string()));
        assert!(config.test_mode);
    }

    #[test]
    fn test_otel_is_enabled() {
        let config = OtelConfig::default();
        assert!(!config.is_enabled());

        let config = OtelConfig {
            endpoint: Some("http://localhost:4317".to_string()),
            service_name: None,
        };
        assert!(config.is_enabled());
    }
}
