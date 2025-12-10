//! Event system for the Unit runtime
//!
//! Provides event types and an event bus for communication between
//! units in a graph. Events are used to propagate data changes,
//! lifecycle transitions, and errors.

use crate::cloneable_any::CloneableAny;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Events that can occur during graph execution
#[derive(Debug, Clone)]
pub enum RuntimeEvent {
    /// Data was pushed to a pin
    PinData {
        unit_id: String,
        pin_name: String,
        data: Arc<dyn CloneableAny>,
    },
    /// Data was dropped from a pin (consumed)
    PinDrop {
        unit_id: String,
        pin_name: String,
    },
    /// Pin was invalidated
    PinInvalid {
        unit_id: String,
        pin_name: String,
    },
    /// Unit entered error state
    UnitError {
        unit_id: String,
        error: String,
    },
    /// Unit error was cleared
    UnitErrorCleared {
        unit_id: String,
    },
    /// Unit lifecycle changed
    LifecycleChanged {
        unit_id: String,
        playing: bool,
    },
    /// Connection was established
    Connected {
        source_unit: String,
        source_pin: String,
        target_unit: String,
        target_pin: String,
    },
    /// Connection was removed
    Disconnected {
        source_unit: String,
        source_pin: String,
        target_unit: String,
        target_pin: String,
    },
    /// Unit was added to graph
    UnitAdded {
        unit_id: String,
    },
    /// Unit was removed from graph
    UnitRemoved {
        unit_id: String,
    },
}

impl RuntimeEvent {
    /// Create a PinData event
    pub fn pin_data(
        unit_id: impl Into<String>,
        pin_name: impl Into<String>,
        data: impl CloneableAny,
    ) -> Self {
        Self::PinData {
            unit_id: unit_id.into(),
            pin_name: pin_name.into(),
            data: Arc::new(data),
        }
    }

    /// Create a PinDrop event
    pub fn pin_drop(unit_id: impl Into<String>, pin_name: impl Into<String>) -> Self {
        Self::PinDrop {
            unit_id: unit_id.into(),
            pin_name: pin_name.into(),
        }
    }

    /// Create a PinInvalid event
    pub fn pin_invalid(unit_id: impl Into<String>, pin_name: impl Into<String>) -> Self {
        Self::PinInvalid {
            unit_id: unit_id.into(),
            pin_name: pin_name.into(),
        }
    }

    /// Create a UnitError event
    pub fn unit_error(unit_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::UnitError {
            unit_id: unit_id.into(),
            error: error.into(),
        }
    }

    /// Get the unit ID this event relates to (if any)
    pub fn unit_id(&self) -> Option<&str> {
        match self {
            Self::PinData { unit_id, .. }
            | Self::PinDrop { unit_id, .. }
            | Self::PinInvalid { unit_id, .. }
            | Self::UnitError { unit_id, .. }
            | Self::UnitErrorCleared { unit_id }
            | Self::LifecycleChanged { unit_id, .. }
            | Self::UnitAdded { unit_id }
            | Self::UnitRemoved { unit_id } => Some(unit_id),
            Self::Connected { source_unit, .. } | Self::Disconnected { source_unit, .. } => {
                Some(source_unit)
            }
        }
    }
}

/// Callback type for event handlers
pub type EventHandler = Box<dyn Fn(&RuntimeEvent) + Send + Sync>;

/// Event bus for broadcasting runtime events
///
/// The EventBus allows components to subscribe to events and
/// broadcast events to all subscribers.
pub struct EventBus {
    /// Queue of pending events
    queue: Mutex<VecDeque<RuntimeEvent>>,
    /// Registered event handlers
    handlers: Mutex<Vec<EventHandler>>,
    /// Whether the bus is currently processing events
    processing: Mutex<bool>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            handlers: Mutex::new(Vec::new()),
            processing: Mutex::new(false),
        }
    }

    /// Enqueue an event for processing
    pub fn emit(&self, event: RuntimeEvent) {
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(event);
    }

    /// Register an event handler
    pub fn subscribe(&self, handler: EventHandler) {
        let mut handlers = self.handlers.lock().unwrap();
        handlers.push(handler);
    }

    /// Process all pending events
    ///
    /// Returns the number of events processed.
    pub fn process(&self) -> usize {
        // Prevent re-entrancy
        {
            let mut processing = self.processing.lock().unwrap();
            if *processing {
                return 0;
            }
            *processing = true;
        }

        let mut count = 0;

        loop {
            // Take one event from the queue
            let event = {
                let mut queue = self.queue.lock().unwrap();
                queue.pop_front()
            };

            match event {
                Some(e) => {
                    // Dispatch to all handlers
                    let handlers = self.handlers.lock().unwrap();
                    for handler in handlers.iter() {
                        handler(&e);
                    }
                    count += 1;
                }
                None => break,
            }
        }

        *self.processing.lock().unwrap() = false;
        count
    }

    /// Check if there are pending events
    pub fn has_pending(&self) -> bool {
        !self.queue.lock().unwrap().is_empty()
    }

    /// Get the number of pending events
    pub fn pending_count(&self) -> usize {
        self.queue.lock().unwrap().len()
    }

    /// Clear all pending events
    pub fn clear(&self) {
        self.queue.lock().unwrap().clear();
    }

    /// Drain all pending events without processing
    pub fn drain(&self) -> Vec<RuntimeEvent> {
        self.queue.lock().unwrap().drain(..).collect()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for EventBus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventBus")
            .field("pending", &self.pending_count())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_event_creation() {
        let event = RuntimeEvent::pin_data("unit1", "output", 42i32);
        assert_eq!(event.unit_id(), Some("unit1"));
    }

    #[test]
    fn test_event_bus_emit_process() {
        let bus = EventBus::new();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        bus.subscribe(Box::new(move |_| {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }));

        bus.emit(RuntimeEvent::pin_data("a", "out", 1));
        bus.emit(RuntimeEvent::pin_data("b", "out", 2));

        assert_eq!(bus.pending_count(), 2);
        let processed = bus.process();
        assert_eq!(processed, 2);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn test_event_bus_no_reentrant() {
        let bus = Arc::new(EventBus::new());
        let bus_clone = bus.clone();

        // Handler that tries to process during processing (should be no-op)
        bus.subscribe(Box::new(move |_| {
            bus_clone.process(); // This should return 0
        }));

        bus.emit(RuntimeEvent::pin_drop("a", "x"));
        bus.process();

        // Should complete without infinite loop
        assert!(!bus.has_pending());
    }

    #[test]
    fn test_event_bus_drain() {
        let bus = EventBus::new();

        bus.emit(RuntimeEvent::pin_drop("a", "x"));
        bus.emit(RuntimeEvent::pin_invalid("b", "y"));

        let events = bus.drain();
        assert_eq!(events.len(), 2);
        assert!(!bus.has_pending());
    }

    #[test]
    fn test_event_clone() {
        let event = RuntimeEvent::pin_data("unit", "pin", "hello".to_string());
        let cloned = event.clone();

        match cloned {
            RuntimeEvent::PinData {
                unit_id, pin_name, ..
            } => {
                assert_eq!(unit_id, "unit");
                assert_eq!(pin_name, "pin");
            }
            _ => panic!("Wrong event type"),
        }
    }
}
