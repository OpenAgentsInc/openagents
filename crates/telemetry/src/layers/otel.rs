//! OpenTelemetry layer for distributed tracing.

use crate::config::OtelConfig;
use crate::error::{Result, TelemetryError};

use opentelemetry::trace::TracerProvider;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::Tracer;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::Registry;

/// OpenTelemetry tracing layer.
pub struct OtelLayer;

impl OtelLayer {
    /// Initialize OpenTelemetry and return the tracing layer.
    ///
    /// # Arguments
    ///
    /// * `config` - OpenTelemetry configuration
    /// * `service_name` - Fallback service name if not in config
    ///
    /// # Errors
    ///
    /// Returns `TelemetryError::OpenTelemetry` if initialization fails.
    pub fn build(
        config: &OtelConfig,
        service_name: &str,
    ) -> Result<OpenTelemetryLayer<Registry, Tracer>> {
        let endpoint = config
            .endpoint
            .as_ref()
            .ok_or_else(|| TelemetryError::OpenTelemetry("OTLP endpoint not configured".into()))?;

        let service_name = config
            .service_name
            .clone()
            .unwrap_or_else(|| service_name.to_string());

        // Create OTLP exporter
        let exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(endpoint)
            .build()
            .map_err(|e| TelemetryError::OpenTelemetry(e.to_string()))?;

        // Create tracer provider
        let provider = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_resource(opentelemetry_sdk::Resource::new(vec![
                opentelemetry::KeyValue::new("service.name", service_name),
            ]))
            .build();

        let tracer = provider.tracer("telemetry");

        // Set global provider
        opentelemetry::global::set_tracer_provider(provider);

        Ok(tracing_opentelemetry::layer().with_tracer(tracer))
    }

    /// Shutdown OpenTelemetry, flushing any pending spans.
    pub fn shutdown() {
        opentelemetry::global::shutdown_tracer_provider();
    }
}
