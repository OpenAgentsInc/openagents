//! Control Units: Flow control and data routing
//!
//! Provides units for controlling data flow:
//! - Identity: Pass-through unit
//! - If: Conditional routing
//! - Switch: Multi-way routing
//! - Gate: Enable/disable data flow

use crate::Lifecycle;
use crate::primitive::PrimitiveState;
use crate::primitive_unit_boilerplate;
use crate::unit::Unit;
use std::any::Any;

/// Identity unit - passes input directly to output
#[derive(Debug, Default)]
pub struct Identity {
    primitive: PrimitiveState,
}

impl Identity {
    pub fn new() -> Self {
        let mut unit = Self {
            primitive: PrimitiveState::new("Identity"),
        };
        unit.primitive.add_input::<f64>("x");
        unit.primitive.add_output::<f64>("result");
        unit
    }
}

impl Unit for Identity {
    fn id(&self) -> &str {
        self.primitive.id()
    }

    fn lifecycle(&self) -> Lifecycle {
        self.primitive.lifecycle()
    }

    fn play(&mut self) {
        self.primitive.play();
    }

    fn pause(&mut self) {
        self.primitive.pause();
    }

    primitive_unit_boilerplate!();

    fn push_input(&mut self, name: &str, data: Box<dyn Any + Send>) -> Result<(), String> {
        if let Err(e) = self.primitive.push_input(name, data) {
            return Err(e);
        }

        if let Some(x) = self
            .primitive
            .input::<f64>("x")
            .and_then(|p| p.peak().copied())
        {
            if let Some(out) = self.primitive.output_mut::<f64>("result") {
                let _ = out.push(x);
            }
        }
        Ok(())
    }

    fn take_output(&mut self, name: &str) -> Option<Box<dyn Any + Send>> {
        self.primitive.take_output(name)
    }

    fn description(&self) -> &str {
        "Identity: passes input to output"
    }
}

/// If unit - conditional routing based on boolean condition
/// Outputs to `then` if condition is true, `else` if false
#[derive(Debug, Default)]
pub struct If {
    primitive: PrimitiveState,
}

impl If {
    pub fn new() -> Self {
        let mut unit = Self {
            primitive: PrimitiveState::new("If"),
        };
        unit.primitive.add_input::<bool>("condition");
        unit.primitive.add_input::<f64>("value");
        unit.primitive.add_output::<f64>("then");
        unit.primitive.add_output::<f64>("else");
        unit
    }
}

impl Unit for If {
    fn id(&self) -> &str {
        self.primitive.id()
    }

    fn lifecycle(&self) -> Lifecycle {
        self.primitive.lifecycle()
    }

    fn play(&mut self) {
        self.primitive.play();
    }

    fn pause(&mut self) {
        self.primitive.pause();
    }

    primitive_unit_boilerplate!();

    fn push_input(&mut self, name: &str, data: Box<dyn Any + Send>) -> Result<(), String> {
        if let Err(e) = self.primitive.push_input(name, data) {
            return Err(e);
        }

        if let (Some(cond), Some(value)) = (
            self.primitive
                .input::<bool>("condition")
                .and_then(|p| p.peak().copied()),
            self.primitive
                .input::<f64>("value")
                .and_then(|p| p.peak().copied()),
        ) {
            if cond {
                if let Some(out) = self.primitive.output_mut::<f64>("then") {
                    let _ = out.push(value);
                }
            } else {
                if let Some(out) = self.primitive.output_mut::<f64>("else") {
                    let _ = out.push(value);
                }
            }
        }
        Ok(())
    }

    fn take_output(&mut self, name: &str) -> Option<Box<dyn Any + Send>> {
        self.primitive.take_output(name)
    }

    fn description(&self) -> &str {
        "If: routes value to 'then' or 'else' based on condition"
    }
}

/// Gate unit - passes value only when enabled
#[derive(Debug, Default)]
pub struct Gate {
    primitive: PrimitiveState,
}

impl Gate {
    pub fn new() -> Self {
        let mut unit = Self {
            primitive: PrimitiveState::new("Gate"),
        };
        unit.primitive.add_input::<bool>("enable");
        unit.primitive.add_input::<f64>("value");
        unit.primitive.add_output::<f64>("result");
        unit
    }
}

impl Unit for Gate {
    fn id(&self) -> &str {
        self.primitive.id()
    }

    fn lifecycle(&self) -> Lifecycle {
        self.primitive.lifecycle()
    }

    fn play(&mut self) {
        self.primitive.play();
    }

    fn pause(&mut self) {
        self.primitive.pause();
    }

    primitive_unit_boilerplate!();

    fn push_input(&mut self, name: &str, data: Box<dyn Any + Send>) -> Result<(), String> {
        if let Err(e) = self.primitive.push_input(name, data) {
            return Err(e);
        }

        if let (Some(enabled), Some(value)) = (
            self.primitive
                .input::<bool>("enable")
                .and_then(|p| p.peak().copied()),
            self.primitive
                .input::<f64>("value")
                .and_then(|p| p.peak().copied()),
        ) {
            if enabled {
                if let Some(out) = self.primitive.output_mut::<f64>("result") {
                    let _ = out.push(value);
                }
            }
        }
        Ok(())
    }

    fn take_output(&mut self, name: &str) -> Option<Box<dyn Any + Send>> {
        self.primitive.take_output(name)
    }

    fn description(&self) -> &str {
        "Gate: passes value only when enabled"
    }
}

/// Select unit - chooses between two inputs based on condition
#[derive(Debug, Default)]
pub struct Select {
    primitive: PrimitiveState,
}

impl Select {
    pub fn new() -> Self {
        let mut unit = Self {
            primitive: PrimitiveState::new("Select"),
        };
        unit.primitive.add_input::<bool>("condition");
        unit.primitive.add_input::<f64>("a");
        unit.primitive.add_input::<f64>("b");
        unit.primitive.add_output::<f64>("result");
        unit
    }
}

impl Unit for Select {
    fn id(&self) -> &str {
        self.primitive.id()
    }

    fn lifecycle(&self) -> Lifecycle {
        self.primitive.lifecycle()
    }

    fn play(&mut self) {
        self.primitive.play();
    }

    fn pause(&mut self) {
        self.primitive.pause();
    }

    primitive_unit_boilerplate!();

    fn push_input(&mut self, name: &str, data: Box<dyn Any + Send>) -> Result<(), String> {
        if let Err(e) = self.primitive.push_input(name, data) {
            return Err(e);
        }

        if let (Some(cond), Some(a), Some(b)) = (
            self.primitive
                .input::<bool>("condition")
                .and_then(|p| p.peak().copied()),
            self.primitive
                .input::<f64>("a")
                .and_then(|p| p.peak().copied()),
            self.primitive
                .input::<f64>("b")
                .and_then(|p| p.peak().copied()),
        ) {
            let result = if cond { a } else { b };
            if let Some(out) = self.primitive.output_mut::<f64>("result") {
                let _ = out.push(result);
            }
        }
        Ok(())
    }

    fn take_output(&mut self, name: &str) -> Option<Box<dyn Any + Send>> {
        self.primitive.take_output(name)
    }

    fn description(&self) -> &str {
        "Select: outputs a if condition is true, b otherwise"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let mut id = Identity::new();
        id.play();

        id.push_input("x", Box::new(42.0f64)).unwrap();
        let result = id.take_output("result").unwrap().downcast::<f64>().unwrap();
        assert_eq!(*result, 42.0);
    }

    #[test]
    fn test_if_true() {
        let mut if_unit = If::new();
        if_unit.play();

        if_unit.push_input("condition", Box::new(true)).unwrap();
        if_unit.push_input("value", Box::new(10.0f64)).unwrap();

        let then_result = if_unit.take_output("then");
        let else_result = if_unit.take_output("else");

        assert!(then_result.is_some());
        assert!(else_result.is_none());
        assert_eq!(*then_result.unwrap().downcast::<f64>().unwrap(), 10.0);
    }

    #[test]
    fn test_if_false() {
        let mut if_unit = If::new();
        if_unit.play();

        if_unit.push_input("condition", Box::new(false)).unwrap();
        if_unit.push_input("value", Box::new(10.0f64)).unwrap();

        let then_result = if_unit.take_output("then");
        let else_result = if_unit.take_output("else");

        assert!(then_result.is_none());
        assert!(else_result.is_some());
        assert_eq!(*else_result.unwrap().downcast::<f64>().unwrap(), 10.0);
    }

    #[test]
    fn test_gate_enabled() {
        let mut gate = Gate::new();
        gate.play();

        gate.push_input("enable", Box::new(true)).unwrap();
        gate.push_input("value", Box::new(5.0f64)).unwrap();

        let result = gate.take_output("result");
        assert!(result.is_some());
        assert_eq!(*result.unwrap().downcast::<f64>().unwrap(), 5.0);
    }

    #[test]
    fn test_gate_disabled() {
        let mut gate = Gate::new();
        gate.play();

        gate.push_input("enable", Box::new(false)).unwrap();
        gate.push_input("value", Box::new(5.0f64)).unwrap();

        let result = gate.take_output("result");
        assert!(result.is_none());
    }

    #[test]
    fn test_select_true() {
        let mut sel = Select::new();
        sel.play();

        sel.push_input("condition", Box::new(true)).unwrap();
        sel.push_input("a", Box::new(10.0f64)).unwrap();
        sel.push_input("b", Box::new(20.0f64)).unwrap();

        let result = sel
            .take_output("result")
            .unwrap()
            .downcast::<f64>()
            .unwrap();
        assert_eq!(*result, 10.0);
    }

    #[test]
    fn test_select_false() {
        let mut sel = Select::new();
        sel.play();

        sel.push_input("condition", Box::new(false)).unwrap();
        sel.push_input("a", Box::new(10.0f64)).unwrap();
        sel.push_input("b", Box::new(20.0f64)).unwrap();

        let result = sel
            .take_output("result")
            .unwrap()
            .downcast::<f64>()
            .unwrap();
        assert_eq!(*result, 20.0);
    }
}
