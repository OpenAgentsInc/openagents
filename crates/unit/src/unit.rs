//! Unit: Base trait for MIMO (Multi-Input Multi-Output) finite state machines
//!
//! A Unit is the fundamental computation node in the dataflow graph.
//! It has named input and output pins and a lifecycle (paused/playing).

use crate::any_pin::AnyPin;
use serde_json::Value;

/// Unit lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lifecycle {
    /// Unit is paused, not processing data
    #[default]
    Paused,
    /// Unit is playing, actively processing data
    Playing,
}

/// Events that can occur on a Unit
#[derive(Debug, Clone, PartialEq)]
pub enum UnitEvent {
    /// Unit started playing
    Play,
    /// Unit was paused
    Pause,
    /// Unit was reset
    Reset,
    /// Unit encountered an error
    Error(String),
    /// Input pin was added
    InputAdded { name: String },
    /// Input pin was removed
    InputRemoved { name: String },
    /// Output pin was added
    OutputAdded { name: String },
    /// Output pin was removed
    OutputRemoved { name: String },
}

/// Base trait for all Unit types
///
/// A Unit is a computation node with named input/output pins.
/// Data flows through pins, triggering computation.
pub trait Unit: Send + Sync {
    /// Get the unit's unique identifier
    fn id(&self) -> &str;

    /// Get current lifecycle state
    fn lifecycle(&self) -> Lifecycle;

    /// Start the unit (begin processing data)
    fn play(&mut self);

    /// Pause the unit (stop processing data)
    fn pause(&mut self);

    /// Reset the unit to initial state
    fn reset(&mut self);

    /// Check if the unit is paused
    fn is_paused(&self) -> bool {
        self.lifecycle() == Lifecycle::Paused
    }

    /// Check if the unit is playing
    fn is_playing(&self) -> bool {
        self.lifecycle() == Lifecycle::Playing
    }

    // Pin access

    /// Get an input pin by name
    fn input(&self, name: &str) -> Option<&dyn AnyPin>;

    /// Get a mutable input pin by name
    fn input_mut(&mut self, name: &str) -> Option<&mut (dyn AnyPin + 'static)>;

    /// Get an output pin by name
    fn output(&self, name: &str) -> Option<&dyn AnyPin>;

    /// Get a mutable output pin by name
    fn output_mut(&mut self, name: &str) -> Option<&mut (dyn AnyPin + 'static)>;

    /// Get all input pin names
    fn input_names(&self) -> Vec<&str>;

    /// Get all output pin names
    fn output_names(&self) -> Vec<&str>;

    /// Get count of input pins
    fn input_count(&self) -> usize {
        self.input_names().len()
    }

    /// Get count of output pins
    fn output_count(&self) -> usize {
        self.output_names().len()
    }

    /// Check if unit has an input with given name
    fn has_input(&self, name: &str) -> bool {
        self.input(name).is_some()
    }

    /// Check if unit has an output with given name
    fn has_output(&self, name: &str) -> bool {
        self.output(name).is_some()
    }

    // Error handling

    /// Get current error (if any)
    fn error(&self) -> Option<&str>;

    /// Set an error on the unit
    fn set_error(&mut self, error: String);

    /// Clear any error
    fn clear_error(&mut self);

    /// Check if unit has an error
    fn has_error(&self) -> bool {
        self.error().is_some()
    }

    // Serialization

    /// Create a snapshot of the unit's state
    fn snapshot(&self) -> Value;

    /// Restore the unit's state from a snapshot
    fn restore(&mut self, state: &Value);

    // Convenience methods for data flow

    /// Push data to an input pin
    fn push_input(&mut self, name: &str, data: Box<dyn std::any::Any + Send>) -> Result<(), String>;

    /// Take data from an output pin
    fn take_output(&mut self, name: &str) -> Option<Box<dyn std::any::Any + Send>>;

    /// Get a description of what this unit does
    fn description(&self) -> &str {
        ""
    }
}

/// Input/Output type discriminator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IO {
    Input,
    Output,
}

impl std::fmt::Display for IO {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IO::Input => write!(f, "input"),
            IO::Output => write!(f, "output"),
        }
    }
}

/// Extension trait for convenient pin access
#[allow(dead_code)]
pub trait UnitExt: Unit {
    /// Get a pin by type and name
    fn pin(&self, io: IO, name: &str) -> Option<&dyn AnyPin> {
        match io {
            IO::Input => self.input(name),
            IO::Output => self.output(name),
        }
    }

    /// Get pin names by type
    fn pin_names(&self, io: IO) -> Vec<&str> {
        match io {
            IO::Input => self.input_names(),
            IO::Output => self.output_names(),
        }
    }

    /// Check if all inputs have data
    fn all_inputs_ready(&self) -> bool {
        self.input_names()
            .iter()
            .all(|name| self.input(name).map(|p| p.is_active()).unwrap_or(false))
    }

    /// Check if any input has data
    fn any_input_ready(&self) -> bool {
        self.input_names()
            .iter()
            .any(|name| self.input(name).map(|p| p.is_active()).unwrap_or(false))
    }
}

// Blanket implementation
impl<T: Unit + ?Sized> UnitExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // Test implementation would go here once we have a concrete Unit
}
