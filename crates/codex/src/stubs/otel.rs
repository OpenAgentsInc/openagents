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
        /// Create a new no-op OtelManager with no arguments
        pub fn new() -> Self {
            Self {
                _inner: Arc::new(()),
            }
        }

        /// Create a new no-op OtelManager (accepts any arguments for compatibility)
        #[allow(clippy::too_many_arguments)]
        pub fn with_config<T1, T2, T3, T4, T5, T6, T7, T8, T9>(
            _conversation_id: T1,
            _model: T2,
            _model_family: T3,
            _account_id: T4,
            _account_email: T5,
            _auth_mode: T6,
            _otel_settings: T7,
            _session_source: T8,
            _codex_home: T9,
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
        pub fn conversation_starts<T1, T2, T3, T4, T5, T6, T7, T8, T9>(
            &self,
            _model_provider: T1,
            _reasoning_effort: T2,
            _reasoning_summary: T3,
            _context_window: T4,
            _auto_compact_limit: T5,
            _cwd: T6,
            _approval_policy: T7,
            _sandbox_policy: T8,
            _features: T9,
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
        pub fn record_responses<T: std::fmt::Debug>(
            &self,
            _responses: &T,
            _event: impl std::fmt::Debug,
        ) {
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
        pub fn tool_result<T: AsRef<str>>(
            &self,
            _tool_name: &str,
            _call_id: &str,
            _log_payload: Option<T>,
            _duration: std::time::Duration,
            _success: bool,
            _message: &str,
        ) {
            // No-op
        }

        /// No-op: Record SSE event completed (accepts up to 6 token counts for compatibility)
        #[allow(clippy::too_many_arguments)]
        pub fn sse_event_completed(
            &self,
            _input_tokens: i64,
            _output_tokens: i64,
            _cached_input_tokens: Option<i64>,
            _reasoning_output_tokens: Option<i64>,
            _total_tokens: i64,
            _reasoning_tokens: i64,
        ) {
            // No-op
        }

        /// No-op: Record SSE event completed failed
        pub fn see_event_completed_failed(&self, _error: impl std::fmt::Debug) {
            // No-op
        }

        /// No-op: Record API request (flexible signature)
        pub fn record_api_request<T1, T2, T3>(
            &self,
            _attempt: u64,
            _status_or_method: T1,
            _url_or_error: T2,
            _duration_or_status: T3,
        ) {
            // No-op
        }

        /// No-op: Tool decision
        pub fn tool_decision<T, U>(
            &self,
            _tool_name: &str,
            _call_id: &str,
            _decision: T,
            _user_or_cfg: U,
        ) {
            // No-op
        }

        /// No-op: Log tool result - returns the result of the closure
        pub async fn log_tool_result<T, F, Fut>(
            &self,
            _tool_name: &str,
            _call_id: &str,
            _log_payload: Option<&str>,
            f: F,
        ) -> T
        where
            F: FnOnce() -> Fut,
            Fut: std::future::Future<Output = T>,
        {
            f().await
        }

        /// No-op: Log tool failed
        pub fn log_tool_failed(&self, _tool: &str, _error: &str) {
            // No-op
        }

        /// No-op: Log SSE event
        pub fn log_sse_event<T: std::fmt::Debug>(
            &self,
            _result: &T,
            _duration: std::time::Duration,
        ) {
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

pub use config::{OtelExporter, OtelHttpProtocol, OtelSettings, OtelTlsConfig};
pub use otel_manager::OtelManager;
pub use otel_provider::OtelProvider;
