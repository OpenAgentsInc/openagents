//! Pin: Core data container for input/output ports
//!
//! A Pin holds a single value and tracks its state through a lifecycle:
//! - Empty (idle) -> Data pushed -> Valid (active) -> Data taken -> Empty
//!
//! Pins can be configured with options:
//! - `constant`: Data is never consumed by take(), only cloned by pull()
//! - `ignored`: Data passes through immediately (auto-taken after push)
//! - `ref_pin`: Holds a reference to another Unit (not just data)

use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Pin state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PinState {
    /// No data, pin is idle
    #[default]
    Empty,
    /// Has valid data, pin is active
    Valid,
    /// Had data that was invalidated (not taken, but marked invalid)
    Invalid,
}

/// Configuration options for a Pin
#[derive(Debug, Clone, Default)]
pub struct PinOpt {
    /// If true, data is never consumed - take() returns None, pull() clones
    pub constant: bool,
    /// If true, data passes through immediately (auto-taken after push)
    pub ignored: bool,
    /// If true, this pin holds a reference to a Unit, not just data
    pub ref_pin: bool,
}

/// Events that can occur on a Pin
#[derive(Debug, Clone, PartialEq)]
pub enum PinEvent<T: Clone> {
    /// New data was pushed
    Data(T),
    /// Data was taken/consumed
    Drop(T),
    /// Data was invalidated
    Invalid,
    /// Pin transitioned from idle to active
    Start,
    /// Pin transitioned from active to idle
    End,
}

/// Snapshot of Pin state for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PinSnapshot<T> {
    /// The current value (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<T>,
    /// Whether the pin is in invalid state
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub invalid: bool,
    /// Whether the pin is constant
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub constant: bool,
    /// Whether the pin is ignored
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ignored: bool,
    /// Whether the pin is idle (no active data)
    #[serde(default = "default_true", skip_serializing_if = "std::clone::Clone::clone")]
    pub idle: bool,
}

fn default_true() -> bool {
    true
}

/// Pin: Core data container
///
/// Holds a single value with push/take/pull/peak semantics.
/// Tracks state transitions and can notify callbacks on events.
pub struct Pin<T: Clone + Send + 'static> {
    /// The current value
    value: Option<T>,
    /// Current state
    state: PinState,
    /// Whether pin is idle (no active computation)
    idle: bool,
    /// Configuration
    opt: PinOpt,

    // Event callbacks
    on_data: Vec<Box<dyn Fn(&T) + Send + Sync>>,
    on_drop: Vec<Box<dyn Fn(&T) + Send + Sync>>,
    on_invalid: Vec<Box<dyn Fn() + Send + Sync>>,
    on_start: Vec<Box<dyn Fn() + Send + Sync>>,
    on_end: Vec<Box<dyn Fn() + Send + Sync>>,
}

impl<T: Clone + Send + 'static> Pin<T> {
    /// Create a new empty Pin with options
    pub fn new(opt: PinOpt) -> Self {
        Self {
            value: None,
            state: PinState::Empty,
            idle: true,
            opt,
            on_data: Vec::new(),
            on_drop: Vec::new(),
            on_invalid: Vec::new(),
            on_start: Vec::new(),
            on_end: Vec::new(),
        }
    }

    /// Create a Pin with initial data
    pub fn with_data(data: T, opt: PinOpt) -> Self {
        let mut pin = Self::new(opt);
        pin.value = Some(data);
        pin.state = PinState::Valid;
        pin.idle = false;
        pin
    }

    /// Push data into the pin
    ///
    /// This invalidates any existing data, then stores the new data.
    /// Emits: Invalid (if had data), Start (if was idle), Data
    ///
    /// If `ignored` is true and not `constant`, data is auto-taken.
    pub fn push(&mut self, data: T) -> Vec<PinEvent<T>> {
        let mut events = Vec::new();

        // Invalidate existing data
        if self.value.is_some() {
            self.invalidate_internal(&mut events);
        }

        // Mark as valid
        self.state = PinState::Valid;

        // Start if was idle
        if self.idle {
            self.idle = false;
            for cb in &self.on_start {
                cb();
            }
            events.push(PinEvent::Start);
        }

        // Store the data
        self.value = Some(data.clone());

        // Emit data event
        for cb in &self.on_data {
            cb(&data);
        }
        events.push(PinEvent::Data(data));

        // Auto-take if ignored and not constant
        if self.opt.ignored && !self.opt.constant {
            if let Some(taken) = self.take_internal(&mut events) {
                // Data was auto-taken
                let _ = taken;
            }
        }

        events
    }

    /// Take (consume) the data from the pin
    ///
    /// Returns the data and marks the pin as empty.
    /// If `constant` is true, returns None (use pull() instead).
    /// Emits: Drop, End (if becomes idle)
    pub fn take(&mut self) -> Option<T> {
        let mut events = Vec::new();
        self.take_internal(&mut events)
    }

    fn take_internal(&mut self, events: &mut Vec<PinEvent<T>>) -> Option<T> {
        if self.opt.constant {
            // Constant pins don't allow take
            return None;
        }

        if let Some(data) = self.value.take() {
            self.state = PinState::Empty;

            // Emit drop event
            for cb in &self.on_drop {
                cb(&data);
            }
            events.push(PinEvent::Drop(data.clone()));

            // End if now idle
            self.end_internal(events);

            Some(data)
        } else {
            None
        }
    }

    /// Pull data from the pin
    ///
    /// If `constant`, clones the data without consuming.
    /// Otherwise, calls take().
    pub fn pull(&mut self) -> Option<T> {
        if self.opt.constant {
            // Clone without consuming
            self.value.clone()
        } else {
            self.take()
        }
    }

    /// Peek at the current data without consuming
    pub fn peak(&self) -> Option<&T> {
        self.value.as_ref()
    }

    /// Peek at the current data (mutable)
    pub fn peak_mut(&mut self) -> Option<&mut T> {
        self.value.as_mut()
    }

    /// Invalidate the current data
    ///
    /// Marks data as invalid without consuming it.
    /// Emits: Invalid
    pub fn invalidate(&mut self) -> Vec<PinEvent<T>> {
        let mut events = Vec::new();
        self.invalidate_internal(&mut events);
        events
    }

    fn invalidate_internal(&mut self, events: &mut Vec<PinEvent<T>>) {
        if self.value.is_some() && self.state != PinState::Invalid {
            self.state = PinState::Invalid;
            self.idle = true;

            for cb in &self.on_invalid {
                cb();
            }
            events.push(PinEvent::Invalid);
        }
    }

    fn end_internal(&mut self, events: &mut Vec<PinEvent<T>>) {
        if self.value.is_none() && !self.idle {
            self.idle = true;

            for cb in &self.on_end {
                cb();
            }
            events.push(PinEvent::End);
        }
    }

    // State queries

    /// Check if pin has no data
    pub fn is_empty(&self) -> bool {
        self.value.is_none()
    }

    /// Check if pin has valid data
    pub fn is_active(&self) -> bool {
        self.value.is_some() && self.state == PinState::Valid
    }

    /// Check if pin is idle (not participating in computation)
    pub fn is_idle(&self) -> bool {
        self.idle
    }

    /// Get current state
    pub fn state(&self) -> PinState {
        self.state
    }

    // Option getters/setters

    /// Check if pin is constant
    pub fn is_constant(&self) -> bool {
        self.opt.constant
    }

    /// Set constant flag
    pub fn set_constant(&mut self, constant: bool) {
        self.opt.constant = constant;
    }

    /// Check if pin is ignored
    pub fn is_ignored(&self) -> bool {
        self.opt.ignored
    }

    /// Set ignored flag
    ///
    /// If setting to true and pin has non-constant data, it's auto-taken.
    pub fn set_ignored(&mut self, ignored: bool) {
        self.opt.ignored = ignored;

        if ignored && !self.opt.constant && self.value.is_some() {
            self.take();
        }
    }

    /// Check if pin is a reference pin
    pub fn is_ref(&self) -> bool {
        self.opt.ref_pin
    }

    /// Set ref flag
    pub fn set_ref(&mut self, ref_pin: bool) {
        self.opt.ref_pin = ref_pin;
    }

    /// Get pin options
    pub fn opt(&self) -> &PinOpt {
        &self.opt
    }

    // Callback registration

    /// Register callback for data events
    pub fn on_data<F>(&mut self, f: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        self.on_data.push(Box::new(f));
    }

    /// Register callback for drop events
    pub fn on_drop<F>(&mut self, f: F)
    where
        F: Fn(&T) + Send + Sync + 'static,
    {
        self.on_drop.push(Box::new(f));
    }

    /// Register callback for invalid events
    pub fn on_invalid<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_invalid.push(Box::new(f));
    }

    /// Register callback for start events
    pub fn on_start<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_start.push(Box::new(f));
    }

    /// Register callback for end events
    pub fn on_end<F>(&mut self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.on_end.push(Box::new(f));
    }

    // Serialization

    /// Create a snapshot of the pin state
    pub fn snapshot(&self) -> PinSnapshot<T>
    where
        T: Clone + Serialize,
    {
        PinSnapshot {
            value: self.value.clone(),
            invalid: self.state == PinState::Invalid,
            constant: self.opt.constant,
            ignored: self.opt.ignored,
            idle: self.idle,
        }
    }

    /// Restore pin state from a snapshot
    pub fn restore(&mut self, snapshot: PinSnapshot<T>)
    where
        T: Clone + for<'de> Deserialize<'de>,
    {
        self.value = snapshot.value;
        self.state = if snapshot.invalid {
            PinState::Invalid
        } else if self.value.is_some() {
            PinState::Valid
        } else {
            PinState::Empty
        };
        self.opt.constant = snapshot.constant;
        self.opt.ignored = snapshot.ignored;
        self.idle = snapshot.idle;
    }
}

impl<T: Clone + Send + Debug + 'static> Debug for Pin<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Pin")
            .field("value", &self.value)
            .field("state", &self.state)
            .field("idle", &self.idle)
            .field("constant", &self.opt.constant)
            .field("ignored", &self.opt.ignored)
            .field("ref_pin", &self.opt.ref_pin)
            .finish()
    }
}

impl<T: Clone + Send + 'static> Default for Pin<T> {
    fn default() -> Self {
        Self::new(PinOpt::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pin_push_take() {
        let mut pin: Pin<i32> = Pin::default();
        assert!(pin.is_empty());
        assert!(pin.is_idle());

        let events = pin.push(42);
        assert!(events.contains(&PinEvent::Start));
        assert!(events.contains(&PinEvent::Data(42)));

        assert!(!pin.is_empty());
        assert!(pin.is_active());
        assert_eq!(pin.peak(), Some(&42));

        let value = pin.take();
        assert_eq!(value, Some(42));
        assert!(pin.is_empty());
        assert!(pin.is_idle());
    }

    #[test]
    fn test_pin_constant() {
        let mut pin: Pin<i32> = Pin::new(PinOpt {
            constant: true,
            ..Default::default()
        });

        pin.push(42);
        assert_eq!(pin.peak(), Some(&42));

        // take() returns None for constant pins
        assert_eq!(pin.take(), None);
        assert_eq!(pin.peak(), Some(&42));

        // pull() clones for constant pins
        assert_eq!(pin.pull(), Some(42));
        assert_eq!(pin.peak(), Some(&42));
    }

    #[test]
    fn test_pin_ignored() {
        let mut pin: Pin<i32> = Pin::new(PinOpt {
            ignored: true,
            ..Default::default()
        });

        // Data is auto-taken when pushed to ignored pin
        let events = pin.push(42);
        assert!(events.contains(&PinEvent::Data(42)));
        assert!(events.contains(&PinEvent::Drop(42)));
        assert!(pin.is_empty());
    }

    #[test]
    fn test_pin_invalidate() {
        let mut pin: Pin<i32> = Pin::default();
        pin.push(42);

        let events = pin.invalidate();
        assert!(events.contains(&PinEvent::Invalid));
        assert_eq!(pin.state(), PinState::Invalid);
        assert!(pin.is_idle());
        // Value is still there, just marked invalid
        assert_eq!(pin.peak(), Some(&42));
    }

    #[test]
    fn test_pin_callbacks() {
        use std::sync::{Arc, Mutex};

        let data_received = Arc::new(Mutex::new(None));
        let data_received_clone = data_received.clone();

        let mut pin: Pin<i32> = Pin::default();
        pin.on_data(move |data| {
            *data_received_clone.lock().unwrap() = Some(*data);
        });

        pin.push(42);
        assert_eq!(*data_received.lock().unwrap(), Some(42));
    }

    #[test]
    fn test_pin_pull() {
        let mut pin: Pin<i32> = Pin::default();
        pin.push(42);

        // pull() on non-constant pin takes the data
        assert_eq!(pin.pull(), Some(42));
        assert!(pin.is_empty());
    }

    #[test]
    fn test_pin_snapshot_restore() {
        let mut pin: Pin<i32> = Pin::new(PinOpt {
            constant: true,
            ..Default::default()
        });
        pin.push(42);

        let snapshot = pin.snapshot();
        assert_eq!(snapshot.value, Some(42));
        assert!(snapshot.constant);

        let mut pin2: Pin<i32> = Pin::default();
        pin2.restore(snapshot);
        assert_eq!(pin2.peak(), Some(&42));
        assert!(pin2.is_constant());
    }
}
