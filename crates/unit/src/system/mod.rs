//! System Units: Built-in unit types for common operations
//!
//! This module provides the standard library of units including:
//! - **Arithmetic**: Add, Subtract, Multiply, Divide, Modulo, Negate, Increment, Decrement
//! - **Logic**: And, Or
//! - **Comparison**: LessThan, GreaterThan, Equal, NotEqual, etc.
//! - **Control**: Identity, If, Gate, Select
//!
//! # Example
//!
//! ```ignore
//! use unit::system::{Add, register_system_units};
//! use unit::spec::UnitRegistry;
//!
//! // Create a registry with all system units
//! let mut registry = UnitRegistry::new();
//! register_system_units(&mut registry);
//!
//! // Create an Add unit directly
//! let mut add = Add::new();
//! add.play();
//! add.push_input("a", Box::new(2.0f64)).unwrap();
//! add.push_input("b", Box::new(3.0f64)).unwrap();
//! let result = add.take_output("result"); // Some(5.0)
//! ```

#[macro_use]
pub mod macros;

pub mod arithmetic;
pub mod comparison;
pub mod control;
pub mod logic;

// Re-export all units
pub use arithmetic::{Add, Decrement, Divide, Increment, Modulo, Multiply, Negate, Subtract};
pub use comparison::{Equal, GreaterThan, GreaterThanOrEqual, LessThan, LessThanOrEqual, NotEqual};
pub use control::{Gate, Identity, If, Select};
pub use logic::{And, Or};

use crate::spec::UnitRegistry;

/// Register all system units with a UnitRegistry
///
/// This registers all built-in unit types so they can be instantiated
/// from GraphSpec files.
pub fn register_system_units(registry: &mut UnitRegistry) {
    // Arithmetic units
    registry.register("system/Add", Box::new(|| Box::new(Add::new())));
    registry.register("system/Subtract", Box::new(|| Box::new(Subtract::new())));
    registry.register("system/Multiply", Box::new(|| Box::new(Multiply::new())));
    registry.register("system/Divide", Box::new(|| Box::new(Divide::new())));
    registry.register("system/Modulo", Box::new(|| Box::new(Modulo::new())));
    registry.register("system/Negate", Box::new(|| Box::new(Negate::new())));
    registry.register("system/Increment", Box::new(|| Box::new(Increment::new())));
    registry.register("system/Decrement", Box::new(|| Box::new(Decrement::new())));

    // Logic units
    registry.register("system/And", Box::new(|| Box::new(And::new())));
    registry.register("system/Or", Box::new(|| Box::new(Or::new())));

    // Comparison units
    registry.register("system/LessThan", Box::new(|| Box::new(LessThan::new())));
    registry.register("system/GreaterThan", Box::new(|| Box::new(GreaterThan::new())));
    registry.register("system/LessThanOrEqual", Box::new(|| Box::new(LessThanOrEqual::new())));
    registry.register("system/GreaterThanOrEqual", Box::new(|| Box::new(GreaterThanOrEqual::new())));
    registry.register("system/Equal", Box::new(|| Box::new(Equal::new())));
    registry.register("system/NotEqual", Box::new(|| Box::new(NotEqual::new())));

    // Control units
    registry.register("system/Identity", Box::new(|| Box::new(Identity::new())));
    registry.register("system/If", Box::new(|| Box::new(If::new())));
    registry.register("system/Gate", Box::new(|| Box::new(Gate::new())));
    registry.register("system/Select", Box::new(|| Box::new(Select::new())));
}

/// Create a UnitRegistry pre-populated with all system units
pub fn system_registry() -> UnitRegistry {
    let mut registry = UnitRegistry::new();
    register_system_units(&mut registry);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unit::Unit;

    #[test]
    fn test_register_system_units() {
        let registry = system_registry();

        // Check that all units are registered
        assert!(registry.has_type("system/Add"));
        assert!(registry.has_type("system/Subtract"));
        assert!(registry.has_type("system/And"));
        assert!(registry.has_type("system/Or"));
        assert!(registry.has_type("system/LessThan"));
        assert!(registry.has_type("system/Identity"));
        assert!(registry.has_type("system/If"));
    }

    #[test]
    fn test_create_unit_from_registry() {
        let registry = system_registry();

        let mut add = registry.create("system/Add").unwrap();
        add.play();

        add.push_input("a", Box::new(10.0f64)).unwrap();
        add.push_input("b", Box::new(5.0f64)).unwrap();

        let result = add.take_output("result").unwrap().downcast::<f64>().unwrap();
        assert_eq!(*result, 15.0);
    }

    #[test]
    fn test_registry_type_count() {
        let registry = system_registry();
        let types = registry.type_ids();

        // Should have: 8 arithmetic + 2 logic + 6 comparison + 4 control = 20 total
        assert_eq!(types.len(), 20);
    }

    #[test]
    fn test_integration_chain() {
        // Test a chain of units: Add -> Multiply -> Identity
        let registry = system_registry();

        let mut add = registry.create("system/Add").unwrap();
        let mut mul = registry.create("system/Multiply").unwrap();
        let mut id = registry.create("system/Identity").unwrap();

        add.play();
        mul.play();
        id.play();

        // 2 + 3 = 5
        add.push_input("a", Box::new(2.0f64)).unwrap();
        add.push_input("b", Box::new(3.0f64)).unwrap();
        let sum = add.take_output("result").unwrap();

        // 5 * 4 = 20
        mul.push_input("a", sum).unwrap();
        mul.push_input("b", Box::new(4.0f64)).unwrap();
        let product = mul.take_output("result").unwrap();

        // Identity pass-through
        id.push_input("x", product).unwrap();
        let result = id.take_output("result").unwrap().downcast::<f64>().unwrap();

        assert_eq!(*result, 20.0);
    }
}
