//! Stub OpenTelemetry implementation
//!
//! This is a no-op implementation of the OtelManager.
//! For production use with telemetry, implement proper OpenTelemetry integration.

pub mod otel_manager {
    use std::sync::Arc;

    /// No-op OpenTelemetry manager
    #[derive(Clone)]
    pub struct OtelManager {
        _inner: Arc<()>,
    }

    impl Default for OtelManager {
        fn default() -> Self {
            Self::new()
        }
    }

    impl OtelManager {
        /// Create a new no-op OtelManager
        pub fn new() -> Self {
            Self {
                _inner: Arc::new(()),
            }
        }

        /// No-op: Record an event
        pub fn record_event(&self, _name: &str, _attributes: &[(&str, &str)]) {
            // No-op
        }

        /// No-op: Start a span
        pub fn start_span(&self, _name: &str) -> OtelSpan {
            OtelSpan
        }

        /// No-op: Get the current trace ID
        pub fn trace_id(&self) -> Option<String> {
            None
        }
    }

    /// Tool decision source for telemetry
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ToolDecisionSource {
        User,
        Config,
        Model,
    }

    /// No-op span
    pub struct OtelSpan;

    impl OtelSpan {
        /// No-op: End the span
        pub fn end(self) {
            // No-op
        }

        /// No-op: Add an attribute
        pub fn set_attribute(&mut self, _key: &str, _value: &str) {
            // No-op
        }
    }
}

pub mod config {
    use serde::{Deserialize, Serialize};

    /// Stub OTEL exporter type
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub enum OtelExporter {
        #[default]
        None,
        Otlp,
    }

    /// Stub OTEL HTTP protocol
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub enum OtelHttpProtocol {
        #[default]
        Http,
        Grpc,
    }

    /// Stub OTEL settings
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OtelSettings {
        pub exporter: OtelExporter,
        pub endpoint: Option<String>,
        pub protocol: OtelHttpProtocol,
        pub tls: Option<OtelTlsConfig>,
    }

    /// Stub OTEL TLS config
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OtelTlsConfig {
        pub cert_path: Option<String>,
        pub key_path: Option<String>,
        pub ca_path: Option<String>,
    }
}

pub mod otel_provider {
    use super::config::OtelSettings;
    use super::otel_manager::OtelManager;

    /// Stub OTEL provider
    pub struct OtelProvider;

    impl OtelProvider {
        /// Create a new no-op provider
        pub fn new(_settings: &OtelSettings) -> anyhow::Result<Self> {
            Ok(Self)
        }

        /// Get the manager (returns a new no-op manager)
        pub fn manager(&self) -> OtelManager {
            OtelManager::new()
        }
    }
}

pub use otel_manager::OtelManager;
pub use config::{OtelExporter, OtelHttpProtocol, OtelSettings, OtelTlsConfig};
pub use otel_provider::OtelProvider;
