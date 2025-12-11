//! Pretty formatting layer for terminal output.

use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

/// Create a pretty formatting layer for human-readable terminal output.
pub struct PrettyLayer;

impl PrettyLayer {
    /// Create the pretty layer attached to a registry.
    ///
    /// Output looks like:
    /// ```text
    /// 2024-01-15T10:30:00.000Z  INFO my_app Processing request correlation_id=abc-123
    /// ```
    pub fn build() -> impl tracing::Subscriber + Send + Sync {
        let formatting_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_file(false)
            .with_line_number(false);

        Registry::default().with(formatting_layer)
    }

    /// Create a pretty layer that can be composed with other layers.
    pub fn layer() -> fmt::Layer<Registry> {
        fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_file(false)
            .with_line_number(false)
    }
}
