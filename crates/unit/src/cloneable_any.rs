//! CloneableAny: Type-erased values that can be cloned
//!
//! This module provides traits for storing and cloning type-erased values,
//! enabling data propagation through Merge connections where the concrete
//! type is not known at compile time.

use std::any::{Any, TypeId};
use std::fmt::Debug;

/// A type-erased value that can be cloned
///
/// This trait combines `Any` with cloning capability, allowing values
/// to be stored in type-erased containers and duplicated for fan-out.
pub trait CloneableAny: Any + Send + Sync + Debug {
    /// Clone the value into a new boxed trait object
    fn clone_box(&self) -> Box<dyn CloneableAny>;

    /// Get as Any reference for downcasting
    fn as_any(&self) -> &dyn Any;

    /// Get as mutable Any reference for downcasting
    fn as_any_mut(&mut self) -> &mut dyn Any;

    /// Get the TypeId of the concrete type
    fn type_id_of(&self) -> TypeId;

    /// Get the type name for debugging
    fn type_name_of(&self) -> &'static str;
}

/// Blanket impl for types that are Clone + Send + Sync + Debug
/// This explicitly excludes Box<dyn CloneableAny> to avoid infinite recursion
impl<T> CloneableAny for T
where
    T: Clone + Send + Sync + Debug + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneableAny> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn type_id_of(&self) -> TypeId {
        TypeId::of::<T>()
    }

    fn type_name_of(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

/// Attempt to downcast a CloneableAny to a concrete type
pub fn downcast<T: 'static>(value: Box<dyn CloneableAny>) -> Result<T, Box<dyn CloneableAny>> {
    if (*value).type_id_of() == TypeId::of::<T>() {
        // Safety: We just verified the type matches
        let raw = Box::into_raw(value) as *mut T;
        Ok(unsafe { *Box::from_raw(raw) })
    } else {
        Err(value)
    }
}

/// Attempt to get a reference to the concrete type
pub fn downcast_ref<T: 'static>(value: &dyn CloneableAny) -> Option<&T> {
    value.as_any().downcast_ref::<T>()
}

/// Attempt to get a mutable reference to the concrete type
pub fn downcast_mut<T: 'static>(value: &mut dyn CloneableAny) -> Option<&mut T> {
    value.as_any_mut().downcast_mut::<T>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clone_box() {
        let value: Box<dyn CloneableAny> = Box::new(42i32);
        let cloned = value.clone_box();

        let original = downcast::<i32>(value).unwrap();
        let cloned_val = downcast::<i32>(cloned).unwrap();

        assert_eq!(original, 42);
        assert_eq!(cloned_val, 42);
    }

    #[test]
    fn test_clone_string() {
        let value: Box<dyn CloneableAny> = Box::new("hello".to_string());
        let cloned = value.clone_box();

        let original = downcast::<String>(value).unwrap();
        let cloned_val = downcast::<String>(cloned).unwrap();

        assert_eq!(original, "hello");
        assert_eq!(cloned_val, "hello");
    }

    #[test]
    fn test_downcast_ref() {
        let value: Box<dyn CloneableAny> = Box::new(3.14f64);
        let ref_val = downcast_ref::<f64>(value.as_ref()).unwrap();
        assert_eq!(*ref_val, 3.14);
    }

    #[test]
    fn test_downcast_wrong_type() {
        let value: Box<dyn CloneableAny> = Box::new(42i32);
        let result = downcast::<String>(value);
        assert!(result.is_err());
    }

    #[test]
    fn test_type_name() {
        let value: Box<dyn CloneableAny> = Box::new(vec![1, 2, 3]);
        assert!(value.type_name_of().contains("Vec"));
    }

    #[test]
    fn test_box_clone_via_clone_box() {
        let value: Box<dyn CloneableAny> = Box::new(100u64);
        let cloned: Box<dyn CloneableAny> = value.clone_box();

        assert_eq!(
            downcast::<u64>(value).unwrap(),
            downcast::<u64>(cloned).unwrap()
        );
    }
}
