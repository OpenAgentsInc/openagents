//! JSON formatting layer (bunyan-style).

use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Registry;

/// Create a JSON formatting layer stack.
///
/// Returns a subscriber with:
/// - `JsonStorageLayer` - stores span data for bunyan formatter
/// - `BunyanFormattingLayer` - outputs JSON in bunyan format
pub struct JsonLayer;

impl JsonLayer {
    /// Create the JSON layer stack attached to a registry.
    ///
    /// The bunyan format produces JSON like:
    /// ```json
    /// {
    ///   "v": 0,
    ///   "name": "my_app",
    ///   "msg": "Processing request",
    ///   "level": 30,
    ///   "hostname": "localhost",
    ///   "pid": 12345,
    ///   "time": "2024-01-15T10:30:00.000Z",
    ///   "correlation_id": "abc-123"
    /// }
    /// ```
    pub fn build(name: impl Into<String>) -> impl tracing::Subscriber + Send + Sync {
        let name = name.into();

        // JsonStorageLayer stores span/event data
        // BunyanFormattingLayer formats and outputs JSON
        let formatting_layer = BunyanFormattingLayer::new(name, std::io::stdout);

        Registry::default()
            .with(JsonStorageLayer)
            .with(formatting_layer)
    }

    /// Create JSON layers that can be composed with other layers.
    ///
    /// Returns a tuple of (JsonStorageLayer, BunyanFormattingLayer).
    pub fn layers(
        name: impl Into<String>,
    ) -> (
        JsonStorageLayer,
        BunyanFormattingLayer<fn() -> std::io::Stdout>,
    ) {
        let name = name.into();
        let formatting_layer = BunyanFormattingLayer::new(name, std::io::stdout as fn() -> _);
        (JsonStorageLayer, formatting_layer)
    }
}
