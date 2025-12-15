//! Stub OpenTelemetry implementation
//!
//! This is a no-op implementation of the OtelManager.
//! For production use with telemetry, implement proper OpenTelemetry integration.

pub mod otel_manager {
    use std::sync::Arc;

    /// No-op OpenTelemetry manager
    #[derive(Clone, Debug)]
    pub struct OtelManager {
        _inner: Arc<()>,
    }

    impl Default for OtelManager {
        fn default() -> Self {
            Self {
                _inner: Arc::new(()),
            }
        }
    }

    impl OtelManager {
        /// Create a new no-op OtelManager (accepts any arguments for compatibility)
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            _service_name: impl Into<String>,
            _service_version: impl Into<String>,
            _environment: impl Into<String>,
            _model: impl Into<String>,
            _model_family: impl Into<String>,
            _cwd: impl Into<String>,
            _otel_settings: impl std::fmt::Debug,
            _session_id: impl Into<String>,
            _codex_home: impl Into<std::path::PathBuf>,
        ) -> Self {
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

        /// No-op: Record conversation start
        #[allow(clippy::too_many_arguments)]
        pub fn conversation_starts(
            &self,
            _conversation_id: impl Into<String>,
            _model: impl Into<String>,
            _model_family: impl Into<String>,
            _model_provider: impl Into<String>,
            _approval_policy: impl Into<String>,
            _sandbox_policy: impl Into<String>,
            _initial_prompt: impl Into<String>,
            _features: impl std::fmt::Debug,
            _rollout_path: impl std::fmt::Debug,
        ) {
            // No-op
        }

        /// No-op: Record user prompt
        pub fn user_prompt(&self, _prompt: impl std::fmt::Debug) {
            // No-op
        }

        /// No-op: Set model info
        pub fn with_model(&self, _model: &str, _model_slug: &str) -> Self {
            self.clone()
        }

        /// No-op: Record response
        pub fn record_responses<T: std::fmt::Debug>(&self, _responses: &T, _event: impl std::fmt::Debug) {
            // No-op
        }

        /// No-op: Record tool call
        pub fn record_tool_call(&self, _tool: &str, _args: &str, _result: &str) {
            // No-op
        }

        /// No-op: Record error
        pub fn record_error(&self, _error: &str) {
            // No-op
        }

        /// No-op: Record tool result
        pub fn tool_result(&self, _tool: &str, _result: &str) {
            // No-op
        }

        /// No-op: Record SSE event completed
        pub fn sse_event_completed(&self, _input_tokens: i64, _output_tokens: i64, _total_tokens: i64, _reasoning_tokens: i64) {
            // No-op
        }

        /// No-op: Record SSE event completed failed
        pub fn see_event_completed_failed(&self, _error: impl std::fmt::Debug) {
            // No-op
        }

        /// No-op: Record API request
        pub fn record_api_request(&self, _attempt: u64, _method: &str, _url: &str, _status: u16) {
            // No-op
        }

        /// No-op: Tool decision
        pub fn tool_decision(&self, _tool: &str, _decision: ToolDecisionSource) {
            // No-op
        }

        /// No-op: Log tool result
        pub fn log_tool_result(&self, _tool: &str, _result: &str) {
            // No-op
        }

        /// No-op: Log tool failed
        pub fn log_tool_failed(&self, _tool: &str, _error: &str) {
            // No-op
        }

        /// No-op: Log SSE event
        pub fn log_sse_event<T: std::fmt::Debug>(&self, _result: &T, _duration: std::time::Duration) {
            // No-op
        }

        /// No-op: Get current span - returns a span ID compatible with tracing
        pub fn current_span(&self) -> tracing::Span {
            tracing::Span::none()
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
    #[derive(Debug)]
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
        OtlpHttp {
            endpoint: String,
            protocol: OtelHttpProtocol,
            headers: std::collections::HashMap<String, String>,
            tls: Option<OtelTlsConfig>,
        },
        OtlpGrpc {
            endpoint: String,
            headers: std::collections::HashMap<String, String>,
            tls: Option<OtelTlsConfig>,
        },
    }

    /// Stub OTEL HTTP protocol
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub enum OtelHttpProtocol {
        #[default]
        Http,
        Grpc,
        Json,
        Binary,
    }

    /// Stub OTEL settings
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OtelSettings {
        pub exporter: OtelExporter,
        pub endpoint: Option<String>,
        pub protocol: OtelHttpProtocol,
        pub tls: Option<OtelTlsConfig>,
        pub service_name: String,
        pub service_version: String,
        pub codex_home: std::path::PathBuf,
        pub environment: String,
        pub trace_exporter: OtelExporter,
    }

    /// Stub OTEL TLS config
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OtelTlsConfig {
        pub cert_path: Option<String>,
        pub key_path: Option<String>,
        pub ca_path: Option<String>,
        pub ca_certificate: Option<String>,
        pub client_certificate: Option<String>,
        pub client_private_key: Option<String>,
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
