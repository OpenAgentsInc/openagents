//! Comparison Units: Comparison operations
//!
//! Provides units for comparing values:
//! - LessThan, GreaterThan, LessThanOrEqual, GreaterThanOrEqual
//! - Equal, NotEqual

use crate::primitive::PrimitiveState;
use crate::unit::Unit;
use crate::Lifecycle;
use crate::primitive_unit_boilerplate;
use std::any::Any;

// Use the macros for comparison operations
crate::comparison_unit!(LessThan, <, "Less than: a < b");
crate::comparison_unit!(GreaterThan, >, "Greater than: a > b");
crate::comparison_unit!(LessThanOrEqual, <=, "Less than or equal: a <= b");
crate::comparison_unit!(GreaterThanOrEqual, >=, "Greater than or equal: a >= b");

/// Equal unit - checks if two numbers are equal
#[derive(Debug, Default)]
pub struct Equal {
    primitive: PrimitiveState,
}

impl Equal {
    pub fn new() -> Self {
        let mut unit = Self {
            primitive: PrimitiveState::new("Equal"),
        };
        unit.primitive.add_input::<f64>("a");
        unit.primitive.add_input::<f64>("b");
        unit.primitive.add_output::<bool>("result");
        unit
    }
}

impl Unit for Equal {
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

        if let (Some(a), Some(b)) = (
            self.primitive.input::<f64>("a").and_then(|p| p.peak().copied()),
            self.primitive.input::<f64>("b").and_then(|p| p.peak().copied()),
        ) {
            // Use epsilon comparison for floating point
            let result = (a - b).abs() < f64::EPSILON;
            if let Some(out) = self.primitive.output_mut::<bool>("result") {
                let _ = out.push(result);
            }
        }
        Ok(())
    }

    fn take_output(&mut self, name: &str) -> Option<Box<dyn Any + Send>> {
        self.primitive.take_output(name)
    }

    fn description(&self) -> &str {
        "Equal: a == b"
    }
}

/// NotEqual unit - checks if two numbers are not equal
#[derive(Debug, Default)]
pub struct NotEqual {
    primitive: PrimitiveState,
}

impl NotEqual {
    pub fn new() -> Self {
        let mut unit = Self {
            primitive: PrimitiveState::new("NotEqual"),
        };
        unit.primitive.add_input::<f64>("a");
        unit.primitive.add_input::<f64>("b");
        unit.primitive.add_output::<bool>("result");
        unit
    }
}

impl Unit for NotEqual {
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

        if let (Some(a), Some(b)) = (
            self.primitive.input::<f64>("a").and_then(|p| p.peak().copied()),
            self.primitive.input::<f64>("b").and_then(|p| p.peak().copied()),
        ) {
            let result = (a - b).abs() >= f64::EPSILON;
            if let Some(out) = self.primitive.output_mut::<bool>("result") {
                let _ = out.push(result);
            }
        }
        Ok(())
    }

    fn take_output(&mut self, name: &str) -> Option<Box<dyn Any + Send>> {
        self.primitive.take_output(name)
    }

    fn description(&self) -> &str {
        "Not equal: a != b"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_less_than() {
        let mut lt = LessThan::new();
        lt.play();

        lt.push_input("a", Box::new(3.0f64)).unwrap();
        lt.push_input("b", Box::new(5.0f64)).unwrap();
        let result = lt.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);

        lt.push_input("a", Box::new(5.0f64)).unwrap();
        lt.push_input("b", Box::new(3.0f64)).unwrap();
        let result = lt.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, false);
    }

    #[test]
    fn test_greater_than() {
        let mut gt = GreaterThan::new();
        gt.play();

        gt.push_input("a", Box::new(5.0f64)).unwrap();
        gt.push_input("b", Box::new(3.0f64)).unwrap();
        let result = gt.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);
    }

    #[test]
    fn test_equal() {
        let mut eq = Equal::new();
        eq.play();

        eq.push_input("a", Box::new(5.0f64)).unwrap();
        eq.push_input("b", Box::new(5.0f64)).unwrap();
        let result = eq.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);

        eq.push_input("a", Box::new(5.0f64)).unwrap();
        eq.push_input("b", Box::new(3.0f64)).unwrap();
        let result = eq.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, false);
    }

    #[test]
    fn test_not_equal() {
        let mut ne = NotEqual::new();
        ne.play();

        ne.push_input("a", Box::new(5.0f64)).unwrap();
        ne.push_input("b", Box::new(3.0f64)).unwrap();
        let result = ne.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);

        ne.push_input("a", Box::new(5.0f64)).unwrap();
        ne.push_input("b", Box::new(5.0f64)).unwrap();
        let result = ne.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, false);
    }

    #[test]
    fn test_less_than_or_equal() {
        let mut lte = LessThanOrEqual::new();
        lte.play();

        lte.push_input("a", Box::new(3.0f64)).unwrap();
        lte.push_input("b", Box::new(5.0f64)).unwrap();
        let result = lte.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);

        lte.push_input("a", Box::new(5.0f64)).unwrap();
        lte.push_input("b", Box::new(5.0f64)).unwrap();
        let result = lte.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);

        lte.push_input("a", Box::new(6.0f64)).unwrap();
        lte.push_input("b", Box::new(5.0f64)).unwrap();
        let result = lte.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, false);
    }

    #[test]
    fn test_greater_than_or_equal() {
        let mut gte = GreaterThanOrEqual::new();
        gte.play();

        gte.push_input("a", Box::new(5.0f64)).unwrap();
        gte.push_input("b", Box::new(3.0f64)).unwrap();
        let result = gte.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);

        gte.push_input("a", Box::new(5.0f64)).unwrap();
        gte.push_input("b", Box::new(5.0f64)).unwrap();
        let result = gte.take_output("result").unwrap().downcast::<bool>().unwrap();
        assert_eq!(*result, true);
    }
}
