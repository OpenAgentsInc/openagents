//! AnyPin: Type-erased pin operations
//!
//! This trait allows storing pins of different types in a single container
//! (e.g., HashMap<String, Box<dyn AnyPin>>), enabling dynamic graph construction.

use crate::cloneable_any::CloneableAny;
use std::any::Any;
use std::sync::Arc;

/// Type-erased pin operations
///
/// Allows working with pins without knowing their concrete type at compile time.
/// Used by Graph and Merge to store heterogeneous pin collections.
pub trait AnyPin: Send + Sync {
    /// Push type-erased data into the pin
    ///
    /// Returns Err if the data type doesn't match the pin's type.
    fn push_any(&mut self, data: Box<dyn Any + Send>) -> Result<(), PinTypeError>;

    /// Push cloneable data into the pin
    ///
    /// This allows pushing data that can later be cloned for fan-out.
    fn push_cloneable(&mut self, data: Arc<dyn CloneableAny>) -> Result<(), PinTypeError>;

    /// Take type-erased data from the pin
    fn take_any(&mut self) -> Option<Box<dyn Any + Send>>;

    /// Pull type-erased data (clone if constant, take otherwise)
    fn pull_any(&mut self) -> Option<Box<dyn Any + Send>>;

    /// Clone the current data without consuming it
    ///
    /// Returns None if pin is empty or invalid.
    fn clone_data(&self) -> Option<Arc<dyn CloneableAny>>;

    /// Peek at the data without consuming (returns None if empty)
    fn peak_any(&self) -> Option<&dyn Any>;

    /// Check if pin is empty
    fn is_empty(&self) -> bool;

    /// Check if pin has valid data
    fn is_active(&self) -> bool;

    /// Check if pin is idle
    fn is_idle(&self) -> bool;

    /// Invalidate the pin's data
    fn invalidate(&mut self);

    /// Check if pin is constant
    fn is_constant(&self) -> bool;

    /// Check if pin is ignored
    fn is_ignored(&self) -> bool;

    /// Check if pin is a reference pin
    fn is_ref(&self) -> bool;

    /// Get the TypeId of the pin's data type
    fn type_id(&self) -> std::any::TypeId;

    /// Get type name for debugging
    fn type_name(&self) -> &'static str;

    /// Get a pointer to self for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get a mutable pointer to self for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Error when pushing data of wrong type to a pin
#[derive(Debug, Clone)]
pub struct PinTypeError {
    pub expected: &'static str,
    pub got: &'static str,
}

impl std::fmt::Display for PinTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pin type mismatch: expected {}, got {}",
            self.expected, self.got
        )
    }
}

impl std::error::Error for PinTypeError {}

// Implement AnyPin for Pin<T>
use crate::pin::Pin;

impl<T: Clone + Send + Sync + std::fmt::Debug + 'static> AnyPin for Pin<T> {
    fn push_any(&mut self, data: Box<dyn Any + Send>) -> Result<(), PinTypeError> {
        match data.downcast::<T>() {
            Ok(typed_data) => {
                self.push(*typed_data);
                Ok(())
            }
            Err(_) => Err(PinTypeError {
                expected: std::any::type_name::<T>(),
                got: "unknown",
            }),
        }
    }

    fn push_cloneable(&mut self, data: Arc<dyn CloneableAny>) -> Result<(), PinTypeError> {
        match data.as_any().downcast_ref::<T>() {
            Some(typed_data) => {
                self.push(typed_data.clone());
                Ok(())
            }
            None => Err(PinTypeError {
                expected: std::any::type_name::<T>(),
                got: data.type_name_of(),
            }),
        }
    }

    fn take_any(&mut self) -> Option<Box<dyn Any + Send>> {
        self.take().map(|v| Box::new(v) as Box<dyn Any + Send>)
    }

    fn pull_any(&mut self) -> Option<Box<dyn Any + Send>> {
        self.pull().map(|v| Box::new(v) as Box<dyn Any + Send>)
    }

    fn clone_data(&self) -> Option<Arc<dyn CloneableAny>> {
        self.peak().map(|v| Arc::new(v.clone()) as Arc<dyn CloneableAny>)
    }

    fn peak_any(&self) -> Option<&dyn Any> {
        self.peak().map(|v| v as &dyn Any)
    }

    fn is_empty(&self) -> bool {
        Pin::is_empty(self)
    }

    fn is_active(&self) -> bool {
        Pin::is_active(self)
    }

    fn is_idle(&self) -> bool {
        Pin::is_idle(self)
    }

    fn invalidate(&mut self) {
        Pin::invalidate(self);
    }

    fn is_constant(&self) -> bool {
        Pin::is_constant(self)
    }

    fn is_ignored(&self) -> bool {
        Pin::is_ignored(self)
    }

    fn is_ref(&self) -> bool {
        Pin::is_ref(self)
    }

    fn type_id(&self) -> std::any::TypeId {
        std::any::TypeId::of::<T>()
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Extension trait for downcasting AnyPin to typed Pin
pub trait AnyPinExt {
    /// Downcast to a typed Pin reference
    fn downcast_ref<T: Clone + Send + Sync + 'static>(&self) -> Option<&Pin<T>>;

    /// Downcast to a mutable typed Pin reference
    fn downcast_mut<T: Clone + Send + Sync + 'static>(&mut self) -> Option<&mut Pin<T>>;
}

impl AnyPinExt for dyn AnyPin {
    fn downcast_ref<T: Clone + Send + Sync + 'static>(&self) -> Option<&Pin<T>> {
        self.as_any().downcast_ref::<Pin<T>>()
    }

    fn downcast_mut<T: Clone + Send + Sync + 'static>(&mut self) -> Option<&mut Pin<T>> {
        self.as_any_mut().downcast_mut::<Pin<T>>()
    }
}

impl AnyPinExt for Box<dyn AnyPin> {
    fn downcast_ref<T: Clone + Send + Sync + 'static>(&self) -> Option<&Pin<T>> {
        (**self).as_any().downcast_ref::<Pin<T>>()
    }

    fn downcast_mut<T: Clone + Send + Sync + 'static>(&mut self) -> Option<&mut Pin<T>> {
        (**self).as_any_mut().downcast_mut::<Pin<T>>()
    }
}

// Note: Downcasting functions use Any's built-in downcast which is safe
// and proper lifetime handling. Use push_any/take_any for type-erased operations.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pin::PinOpt;

    #[test]
    fn test_any_pin_push_take() {
        let mut pin: Pin<i32> = Pin::default();
        let any_pin: &mut dyn AnyPin = &mut pin;

        // Push type-erased data
        any_pin.push_any(Box::new(42i32)).unwrap();
        assert!(!any_pin.is_empty());

        // Take type-erased data
        let data = any_pin.take_any().unwrap();
        let value = data.downcast::<i32>().unwrap();
        assert_eq!(*value, 42);
        assert!(any_pin.is_empty());
    }

    #[test]
    fn test_any_pin_type_mismatch() {
        let mut pin: Pin<i32> = Pin::default();
        let any_pin: &mut dyn AnyPin = &mut pin;

        // Try to push wrong type
        let result = any_pin.push_any(Box::new("wrong type"));
        assert!(result.is_err());
    }

    #[test]
    fn test_any_pin_peak() {
        let mut pin: Pin<String> = Pin::default();
        pin.push("hello".to_string());
        let any_pin: &dyn AnyPin = &pin;

        let data = any_pin.peak_any().unwrap();
        let value = data.downcast_ref::<String>().unwrap();
        assert_eq!(value, "hello");
    }
}
