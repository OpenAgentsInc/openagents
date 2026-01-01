//! Client-side telemetry collection for WASM
//!
//! Buffers events in memory and flushes via sendBeacon on page unload
//! to avoid continuous HTTP hits during normal operation.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Event types for telemetry
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    PageView {
        path: String,
        referrer: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    Interaction {
        action: String,
        target: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
    },
    Performance {
        metric: String,
        duration_ms: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        details: Option<String>,
    },
    Error {
        error_type: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        stack: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<String>,
    },
}

/// A timestamped telemetry event ready for transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampedEvent {
    pub event: TelemetryEvent,
    pub timestamp_ms: f64,
    pub page_path: String,
}

/// Batch payload sent to the server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryBatch {
    pub session_id: String,
    pub events: Vec<TimestampedEvent>,
    pub user_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
}

/// Telemetry collector that buffers events and flushes on page unload
pub struct TelemetryCollector {
    session_id: String,
    buffer: Vec<TimestampedEvent>,
    user_id: Option<String>,
    max_buffer_size: usize,
    endpoint: String,
    init_time: f64,
    _beforeunload_closure: Option<Closure<dyn FnMut()>>,
    _visibilitychange_closure: Option<Closure<dyn FnMut()>>,
    _error_closure: Option<Closure<dyn FnMut(web_sys::ErrorEvent)>>,
    _unhandledrejection_closure: Option<Closure<dyn FnMut(JsValue)>>,
}

impl TelemetryCollector {
    /// Create a new telemetry collector
    pub fn new() -> Self {
        let session_id = generate_session_id();
        let init_time = now_ms();

        Self {
            session_id,
            buffer: Vec::with_capacity(100),
            user_id: None,
            max_buffer_size: 100,
            endpoint: "/api/telemetry/batch".to_string(),
            init_time,
            _beforeunload_closure: None,
            _visibilitychange_closure: None,
            _error_closure: None,
            _unhandledrejection_closure: None,
        }
    }

    /// Initialize event listeners for auto-capture
    pub fn init(self) -> Rc<RefCell<Self>> {
        let collector = Rc::new(RefCell::new(self));

        Self::setup_page_lifecycle(collector.clone());
        Self::setup_error_handlers(collector.clone());

        // Track initial page view and WASM init performance
        {
            let mut c = collector.borrow_mut();
            c.track_page_view();
            c.track_wasm_init();
        }

        collector
    }

    /// Set the user ID (call after authentication)
    pub fn set_user_id(&mut self, user_id: Option<String>) {
        self.user_id = user_id;
    }

    /// Track a page view event
    pub fn track_page_view(&mut self) {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        let location = window.location();
        let path = location.pathname().unwrap_or_default();
        let referrer = window
            .document()
            .map(|d| d.referrer())
            .filter(|r| !r.is_empty());
        let title = window.document().map(|d| d.title()).filter(|t| !t.is_empty());

        self.track(TelemetryEvent::PageView {
            path,
            referrer,
            title,
        });
    }

    /// Track an interaction event
    pub fn track_interaction(&mut self, action: &str, target: &str, value: Option<&str>) {
        self.track(TelemetryEvent::Interaction {
            action: action.to_string(),
            target: target.to_string(),
            value: value.map(String::from),
        });
    }

    /// Track a performance metric
    pub fn track_performance(&mut self, metric: &str, duration_ms: f64, details: Option<&str>) {
        self.track(TelemetryEvent::Performance {
            metric: metric.to_string(),
            duration_ms,
            details: details.map(String::from),
        });
    }

    /// Track an error
    pub fn track_error(
        &mut self,
        error_type: &str,
        message: &str,
        stack: Option<&str>,
        source: Option<&str>,
    ) {
        self.track(TelemetryEvent::Error {
            error_type: error_type.to_string(),
            message: message.to_string(),
            stack: stack.map(String::from),
            source: source.map(String::from),
        });
    }

    /// Generic event tracking
    pub fn track(&mut self, event: TelemetryEvent) {
        let page_path = web_sys::window()
            .and_then(|w| w.location().pathname().ok())
            .unwrap_or_default();

        self.buffer.push(TimestampedEvent {
            event,
            timestamp_ms: now_ms(),
            page_path,
        });

        // Flush if buffer is full
        if self.buffer.len() >= self.max_buffer_size {
            self.flush();
        }
    }

    /// Track WASM initialization time
    fn track_wasm_init(&mut self) {
        let duration_ms = now_ms() - self.init_time;
        self.track_performance("wasm_init", duration_ms, None);
    }

    /// Flush buffered events via sendBeacon
    pub fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        let batch = TelemetryBatch {
            session_id: self.session_id.clone(),
            events: std::mem::take(&mut self.buffer),
            user_agent: window.navigator().user_agent().unwrap_or_default(),
            user_id: self.user_id.clone(),
        };

        let payload = match serde_json::to_string(&batch) {
            Ok(p) => p,
            Err(_) => return,
        };

        // Use sendBeacon for reliable delivery on page unload
        let navigator = window.navigator();
        let blob_parts = js_sys::Array::new();
        blob_parts.push(&JsValue::from_str(&payload));

        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/json");

        if let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&blob_parts, &options) {
            let _ = navigator.send_beacon_with_opt_blob(&self.endpoint, Some(&blob));
        }
    }

    /// Set up page lifecycle event handlers
    fn setup_page_lifecycle(collector: Rc<RefCell<Self>>) {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        // beforeunload - flush on page leave
        {
            let collector_clone = collector.clone();
            let closure = Closure::new(move || {
                if let Ok(mut c) = collector_clone.try_borrow_mut() {
                    c.flush();
                }
            });
            let _ = window
                .add_event_listener_with_callback("beforeunload", closure.as_ref().unchecked_ref());
            collector.borrow_mut()._beforeunload_closure = Some(closure);
        }

        // visibilitychange - flush when page becomes hidden
        {
            let collector_clone = collector.clone();
            let closure = Closure::new(move || {
                if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                    if document.visibility_state() == web_sys::VisibilityState::Hidden {
                        if let Ok(mut c) = collector_clone.try_borrow_mut() {
                            c.flush();
                        }
                    }
                }
            });
            if let Some(document) = window.document() {
                let _ = document.add_event_listener_with_callback(
                    "visibilitychange",
                    closure.as_ref().unchecked_ref(),
                );
            }
            collector.borrow_mut()._visibilitychange_closure = Some(closure);
        }
    }

    /// Set up error handlers for auto-capture
    fn setup_error_handlers(collector: Rc<RefCell<Self>>) {
        let window = match web_sys::window() {
            Some(w) => w,
            None => return,
        };

        // Global error handler for JS errors
        {
            let collector_clone = collector.clone();
            let closure = Closure::new(move |event: web_sys::ErrorEvent| {
                if let Ok(mut c) = collector_clone.try_borrow_mut() {
                    let error_val = event.error();
                    let stack = if error_val.is_undefined() || error_val.is_null() {
                        None
                    } else {
                        js_sys::Reflect::get(&error_val, &"stack".into())
                            .ok()
                            .and_then(|s| s.as_string())
                    };

                    c.track_error(
                        "js_error",
                        &event.message(),
                        stack.as_deref(),
                        Some(&format!("{}:{}:{}", event.filename(), event.lineno(), event.colno())),
                    );
                }
            });
            let _ =
                window.add_event_listener_with_callback("error", closure.as_ref().unchecked_ref());
            collector.borrow_mut()._error_closure = Some(closure);
        }

        // Unhandled promise rejection handler
        {
            let collector_clone = collector.clone();
            let closure = Closure::new(move |event: JsValue| {
                if let Ok(mut c) = collector_clone.try_borrow_mut() {
                    let reason = js_sys::Reflect::get(&event, &"reason".into())
                        .ok()
                        .unwrap_or(JsValue::UNDEFINED);

                    let message = reason
                        .as_string()
                        .or_else(|| {
                            js_sys::Reflect::get(&reason, &"message".into())
                                .ok()
                                .and_then(|m| m.as_string())
                        })
                        .unwrap_or_else(|| "Unhandled promise rejection".to_string());

                    c.track_error(
                        "unhandled_rejection",
                        &message,
                        js_sys::Reflect::get(&reason, &"stack".into())
                            .ok()
                            .and_then(|s| s.as_string())
                            .as_deref(),
                        None,
                    );
                }
            });
            let _ = window.add_event_listener_with_callback(
                "unhandledrejection",
                closure.as_ref().unchecked_ref(),
            );
            collector.borrow_mut()._unhandledrejection_closure = Some(closure);
        }
    }
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique session ID (32 hex chars)
fn generate_session_id() -> String {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return "00000000000000000000000000000000".to_string(),
    };

    let crypto = match window.crypto() {
        Ok(c) => c,
        Err(_) => return "00000000000000000000000000000000".to_string(),
    };

    let mut bytes = [0u8; 16];
    if crypto.get_random_values_with_u8_array(&mut bytes).is_err() {
        return "00000000000000000000000000000000".to_string();
    }

    // Manual hex encoding to avoid adding hex crate dependency
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(32);
    for byte in bytes {
        hex.push(HEX_CHARS[(byte >> 4) as usize] as char);
        hex.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }
    hex
}

/// Get current time in milliseconds since epoch
fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now() + p.time_origin())
        .unwrap_or(0.0)
}

/// Set up panic hook (no-op in WASM since Rc<RefCell> isn't Send+Sync)
/// WASM panics are already captured by console_error_panic_hook in main()
/// and JS errors are captured by the error event handler.
#[allow(unused_variables)]
pub fn set_panic_hook(_collector: Rc<RefCell<TelemetryCollector>>) {
    // Note: std::panic::set_hook requires Send+Sync, which Rc<RefCell<_>> doesn't satisfy.
    // In single-threaded WASM this is overly restrictive, but we can't work around it easily.
    // The console_error_panic_hook set in main() will still log panics to the browser console.
}
