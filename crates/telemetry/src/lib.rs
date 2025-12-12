//! Telemetry crate for structured logging and distributed tracing.
//!
//! Provides:
//! - Structured logging with JSON (bunyan-style) or pretty terminal output
//! - Correlation IDs for request tracking
//! - Secret string wrapper for redacting sensitive data
//! - Optional OpenTelemetry OTLP export (feature-gated)
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use telemetry::init_default;
//!
//! fn main() {
//!     init_default("my_app");
//!     tracing::info!("Application started");
//! }
//! ```
//!
//! # With Correlation IDs
//!
//! ```rust,no_run
//! use telemetry::{init_default, CorrelationId, correlation_span};
//!
//! fn main() {
//!     init_default("my_app");
//!
//!     let id = CorrelationId::generate();
//!     let _span = correlation_span!("request", &id).entered();
//!     tracing::info!("Processing request");
//! }
//! ```
//!
//! # Configuration
//!
//! Environment variables:
//! - `RUST_LOG` - Log filter directive (default: "info")
//! - `LOG_FORMAT` - Output format: "json" or "pretty" (default: "pretty")
//! - `OTEL_EXPORTER_OTLP_ENDPOINT` - Enables OTLP export if set
//! - `OTEL_SERVICE_NAME` - Service name for traces
//! - `TEST_LOG` - If set, enables logs in test mode

pub mod config;
pub mod correlation;
pub mod error;
pub mod layers;
pub mod secrets;

pub use config::{LogFormat, OtelConfig, TelemetryConfig};
pub use correlation::CorrelationId;
pub use error::{Result, TelemetryError};
pub use secrets::SecretString;

// Re-export tracing macros for convenience
pub use tracing::{debug, error, info, trace, warn};

use tracing_log::LogTracer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{fmt, EnvFilter, Registry};

/// Initialize telemetry with default settings from environment variables.
///
/// This is the simplest way to set up telemetry. It reads configuration
/// from environment variables and initializes the global subscriber.
///
/// # Arguments
///
/// * `name` - Application name (used in JSON output and OTEL)
///
/// # Example
///
/// ```rust,no_run
/// telemetry::init_default("my_app");
/// tracing::info!("Application started");
/// ```
pub fn init_default(name: &str) {
    let config = TelemetryConfig::from_env();
    init_with_config(name, config);
}

/// Initialize telemetry with a custom default filter.
///
/// Uses environment variables for other configuration (LOG_FORMAT, etc.)
/// but allows specifying a custom default filter.
///
/// # Arguments
///
/// * `name` - Application name
/// * `default_filter` - Default filter directive (e.g., "info", "debug")
///
/// # Example
///
/// ```rust,no_run
/// telemetry::init_with_filter("my_app", "debug");
/// ```
pub fn init_with_filter(name: &str, default_filter: &str) {
    let config = TelemetryConfig::from_env().with_filter(default_filter);
    init_with_config(name, config);
}

/// Initialize telemetry with full configuration control.
///
/// # Arguments
///
/// * `name` - Application name
/// * `config` - Telemetry configuration
///
/// # Example
///
/// ```rust,no_run
/// use telemetry::{TelemetryConfig, LogFormat};
///
/// let config = TelemetryConfig::new()
///     .with_filter("debug")
///     .with_format(LogFormat::Json);
///
/// telemetry::init_with_config("my_app", config);
/// ```
pub fn init_with_config(name: &str, config: TelemetryConfig) {
    // Redirect log crate to tracing
    let _ = LogTracer::init();

    // Create env filter
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.filter));

    // Build and set subscriber based on format
    match config.format {
        LogFormat::Json => {
            let (storage_layer, formatting_layer) = layers::json::JsonLayer::layers(name);
            let subscriber = Registry::default()
                .with(env_filter)
                .with(storage_layer)
                .with(formatting_layer);
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set subscriber");
        }
        LogFormat::Pretty => {
            let formatting_layer = fmt::layer()
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false);
            let subscriber = Registry::default()
                .with(env_filter)
                .with(formatting_layer);
            tracing::subscriber::set_global_default(subscriber)
                .expect("Failed to set subscriber");
        }
    }
}

/// Initialize telemetry for tests.
///
/// By default, logs are suppressed in tests unless `TEST_LOG` is set.
/// This prevents test output from being cluttered with log messages.
///
/// # Example
///
/// ```rust
/// #[test]
/// fn my_test() {
///     telemetry::init_test();
///     // Logs are suppressed unless TEST_LOG=1
///     tracing::info!("This won't show unless TEST_LOG=1");
/// }
/// ```
pub fn init_test() {
    let config = TelemetryConfig::from_env().with_test_mode(true);

    if config.should_suppress_logs() {
        // Don't initialize anything - logs go nowhere
        return;
    }

    // Only initialize once
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        // Redirect log crate to tracing
        let _ = LogTracer::init();

        let env_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&config.filter));

        let formatting_layer = fmt::layer()
            .with_target(true)
            .with_test_writer();

        let subscriber = Registry::default()
            .with(env_filter)
            .with(formatting_layer);

        let _ = tracing::subscriber::set_global_default(subscriber);
    });
}

#[cfg(feature = "otel")]
pub use layers::otel::OtelLayer;

#[cfg(feature = "otel")]
/// Shutdown OpenTelemetry, flushing pending spans.
pub fn shutdown_otel() {
    layers::otel::OtelLayer::shutdown();
}

// =============================================================================
// Client telemetry event system
// =============================================================================

use futures::channel::mpsc;
use std::sync::OnceLock;
use parking_lot::Mutex;

pub use telemetry_events::FlexibleEvent;

// Re-export serde_json for use in event! macro
pub use serde_json;

/// Global event sender for client telemetry
static EVENT_SENDER: OnceLock<Mutex<Option<mpsc::UnboundedSender<FlexibleEvent>>>> = OnceLock::new();

/// Initialize the telemetry event system with a channel sender.
///
/// Events sent via `event!` macro will be forwarded to this channel.
/// The receiver should be processed by the Telemetry struct in the client crate.
pub fn init(tx: mpsc::UnboundedSender<FlexibleEvent>) {
    let sender = EVENT_SENDER.get_or_init(|| Mutex::new(None));
    *sender.lock() = Some(tx);
}

/// Send an event through the telemetry channel.
///
/// Returns true if the event was sent, false if no sender is configured.
#[doc(hidden)]
pub fn send_event(event: FlexibleEvent) -> bool {
    if let Some(sender) = EVENT_SENDER.get() {
        if let Some(tx) = sender.lock().as_ref() {
            return tx.unbounded_send(event).is_ok();
        }
    }
    // Fall back to tracing if no sender configured
    tracing::info!(
        target: "telemetry::event",
        event_type = %event.event_type,
        properties = ?event.event_properties,
        "telemetry event"
    );
    false
}

/// Log a telemetry event with the given type and properties.
///
/// Events are sent through the channel configured via `init()`, or logged
/// via tracing if no channel is configured.
///
/// # Example
///
/// ```ignore
/// telemetry::event!("App Closed");
/// telemetry::event!("Editor Edited", duration = 1000, environment = "local");
/// let ext = "rs";
/// telemetry::event!("File Opened", ext); // shorthand for ext = ext
/// ```
#[macro_export]
macro_rules! event {
    // Base case: just event type
    ($event_type:expr $(,)?) => {{
        let event = $crate::FlexibleEvent {
            event_type: $event_type.to_string(),
            event_properties: std::collections::HashMap::new(),
        };
        $crate::send_event(event);
    }};
    // With properties - delegate to helper
    ($event_type:expr, $($rest:tt)+) => {{
        let mut properties = std::collections::HashMap::new();
        $crate::event_props!(@insert properties; $($rest)+);
        let event = $crate::FlexibleEvent {
            event_type: $event_type.to_string(),
            event_properties: properties,
        };
        $crate::send_event(event);
    }};
}

/// Helper macro for event property insertion (supports both `key = value` and `key` shorthand)
#[macro_export]
#[doc(hidden)]
macro_rules! event_props {
    // key = value form followed by more
    (@insert $props:ident; $key:ident = $value:expr, $($rest:tt)+) => {
        $props.insert(
            stringify!($key).to_string(),
            $crate::serde_json::to_value(&$value).unwrap_or($crate::serde_json::Value::Null),
        );
        $crate::event_props!(@insert $props; $($rest)+);
    };
    // key = value form - terminal
    (@insert $props:ident; $key:ident = $value:expr $(,)?) => {
        $props.insert(
            stringify!($key).to_string(),
            $crate::serde_json::to_value(&$value).unwrap_or($crate::serde_json::Value::Null),
        );
    };
    // shorthand form followed by more
    (@insert $props:ident; $key:ident, $($rest:tt)+) => {
        $props.insert(
            stringify!($key).to_string(),
            $crate::serde_json::to_value(&$key).unwrap_or($crate::serde_json::Value::Null),
        );
        $crate::event_props!(@insert $props; $($rest)+);
    };
    // shorthand form - terminal (with optional trailing comma)
    (@insert $props:ident; $key:ident $(,)?) => {
        $props.insert(
            stringify!($key).to_string(),
            $crate::serde_json::to_value(&$key).unwrap_or($crate::serde_json::Value::Null),
        );
    };
}
