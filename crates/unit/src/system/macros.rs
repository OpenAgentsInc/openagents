//! Macros: Code generation helpers for system units
//!
//! This module provides macros to reduce boilerplate when defining
//! common unit patterns.

/// Define a simple binary arithmetic unit (two inputs, one output)
///
/// Creates a unit that takes inputs `a` and `b`, applies an operator,
/// and produces output `result`.
#[macro_export]
macro_rules! binary_op_unit {
    ($name:ident, $op:tt, $desc:literal) => {
        #[derive(Debug)]
        pub struct $name {
            primitive: $crate::PrimitiveState,
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            pub fn new() -> Self {
                let mut primitive = $crate::PrimitiveState::new(stringify!($name));
                primitive.add_input::<f64>("a");
                primitive.add_input::<f64>("b");
                primitive.add_output::<f64>("result");
                Self { primitive }
            }
        }

        impl $crate::Unit for $name {
            fn id(&self) -> &str {
                self.primitive.id()
            }

            fn lifecycle(&self) -> $crate::Lifecycle {
                self.primitive.lifecycle()
            }

            fn play(&mut self) {
                self.primitive.play();
            }

            fn pause(&mut self) {
                self.primitive.pause();
            }

            fn reset(&mut self) {
                self.primitive.reset();
            }

            fn input(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None // PrimitiveState doesn't expose AnyPin directly
            }

            fn input_mut(&mut self, _name: &str) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn output(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None
            }

            fn output_mut(&mut self, _name: &str) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn input_names(&self) -> Vec<&str> {
                self.primitive.input_names()
            }

            fn output_names(&self) -> Vec<&str> {
                self.primitive.output_names()
            }

            fn error(&self) -> Option<&str> {
                self.primitive.error()
            }

            fn set_error(&mut self, error: String) {
                self.primitive.set_error(error);
            }

            fn clear_error(&mut self) {
                self.primitive.clear_error();
            }

            fn snapshot(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            fn restore(&mut self, _state: &serde_json::Value) {
                // No state to restore
            }

            fn push_input(&mut self, name: &str, data: Box<dyn std::any::Any + Send>) -> Result<(), String> {
                if let Err(e) = self.primitive.push_input(name, data) {
                    return Err(e);
                }

                // Check if we have both inputs
                if let (Some(a), Some(b)) = (
                    self.primitive.input::<f64>("a").and_then(|p| p.peak().copied()),
                    self.primitive.input::<f64>("b").and_then(|p| p.peak().copied()),
                ) {
                    let result = a $op b;
                    if let Some(out) = self.primitive.output_mut::<f64>("result") {
                        let _ = out.push(result);
                    }
                }
                Ok(())
            }

            fn take_output(&mut self, name: &str) -> Option<Box<dyn std::any::Any + Send>> {
                self.primitive.take_output(name)
            }

            fn description(&self) -> &str {
                $desc
            }
        }
    };
}

/// Define a unary math unit (one input, one output)
#[macro_export]
macro_rules! unary_math_unit {
    ($name:ident, $func:expr, $desc:literal) => {
        #[derive(Debug)]
        pub struct $name {
            primitive: $crate::PrimitiveState,
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            pub fn new() -> Self {
                let mut primitive = $crate::PrimitiveState::new(stringify!($name));
                primitive.add_input::<f64>("x");
                primitive.add_output::<f64>("result");
                Self { primitive }
            }
        }

        impl $crate::Unit for $name {
            fn id(&self) -> &str {
                self.primitive.id()
            }

            fn lifecycle(&self) -> $crate::Lifecycle {
                self.primitive.lifecycle()
            }

            fn play(&mut self) {
                self.primitive.play();
            }

            fn pause(&mut self) {
                self.primitive.pause();
            }

            fn reset(&mut self) {
                self.primitive.reset();
            }

            fn input(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None
            }

            fn input_mut(
                &mut self,
                _name: &str,
            ) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn output(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None
            }

            fn output_mut(
                &mut self,
                _name: &str,
            ) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn input_names(&self) -> Vec<&str> {
                self.primitive.input_names()
            }

            fn output_names(&self) -> Vec<&str> {
                self.primitive.output_names()
            }

            fn error(&self) -> Option<&str> {
                self.primitive.error()
            }

            fn set_error(&mut self, error: String) {
                self.primitive.set_error(error);
            }

            fn clear_error(&mut self) {
                self.primitive.clear_error();
            }

            fn snapshot(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            fn restore(&mut self, _state: &serde_json::Value) {}

            fn push_input(
                &mut self,
                name: &str,
                data: Box<dyn std::any::Any + Send>,
            ) -> Result<(), String> {
                if let Err(e) = self.primitive.push_input(name, data) {
                    return Err(e);
                }

                if let Some(x) = self
                    .primitive
                    .input::<f64>("x")
                    .and_then(|p| p.peak().copied())
                {
                    let func: fn(f64) -> f64 = $func;
                    let result = func(x);
                    if let Some(out) = self.primitive.output_mut::<f64>("result") {
                        let _ = out.push(result);
                    }
                }
                Ok(())
            }

            fn take_output(&mut self, name: &str) -> Option<Box<dyn std::any::Any + Send>> {
                self.primitive.take_output(name)
            }

            fn description(&self) -> &str {
                $desc
            }
        }
    };
}

/// Define a simple logic gate unit (two boolean inputs, one output)
#[macro_export]
macro_rules! logic_gate_unit {
    ($name:ident, $op:tt, $desc:literal) => {
        #[derive(Debug)]
        pub struct $name {
            primitive: $crate::PrimitiveState,
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            pub fn new() -> Self {
                let mut primitive = $crate::PrimitiveState::new(stringify!($name));
                primitive.add_input::<bool>("a");
                primitive.add_input::<bool>("b");
                primitive.add_output::<bool>("result");
                Self { primitive }
            }
        }

        impl $crate::Unit for $name {
            fn id(&self) -> &str {
                self.primitive.id()
            }

            fn lifecycle(&self) -> $crate::Lifecycle {
                self.primitive.lifecycle()
            }

            fn play(&mut self) {
                self.primitive.play();
            }

            fn pause(&mut self) {
                self.primitive.pause();
            }

            fn reset(&mut self) {
                self.primitive.reset();
            }

            fn input(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None
            }

            fn input_mut(&mut self, _name: &str) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn output(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None
            }

            fn output_mut(&mut self, _name: &str) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn input_names(&self) -> Vec<&str> {
                self.primitive.input_names()
            }

            fn output_names(&self) -> Vec<&str> {
                self.primitive.output_names()
            }

            fn error(&self) -> Option<&str> {
                self.primitive.error()
            }

            fn set_error(&mut self, error: String) {
                self.primitive.set_error(error);
            }

            fn clear_error(&mut self) {
                self.primitive.clear_error();
            }

            fn snapshot(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            fn restore(&mut self, _state: &serde_json::Value) {}

            fn push_input(&mut self, name: &str, data: Box<dyn std::any::Any + Send>) -> Result<(), String> {
                if let Err(e) = self.primitive.push_input(name, data) {
                    return Err(e);
                }

                if let (Some(a), Some(b)) = (
                    self.primitive.input::<bool>("a").and_then(|p| p.peak().copied()),
                    self.primitive.input::<bool>("b").and_then(|p| p.peak().copied()),
                ) {
                    let result = a $op b;
                    if let Some(out) = self.primitive.output_mut::<bool>("result") {
                        let _ = out.push(result);
                    }
                }
                Ok(())
            }

            fn take_output(&mut self, name: &str) -> Option<Box<dyn std::any::Any + Send>> {
                self.primitive.take_output(name)
            }

            fn description(&self) -> &str {
                $desc
            }
        }
    };
}

/// Helper macro to implement common Unit trait methods for units using PrimitiveState
///
/// This is used by custom units that need special logic in push_input but want
/// the standard implementations for other methods.
#[macro_export]
macro_rules! primitive_unit_boilerplate {
    () => {
        fn reset(&mut self) {
            self.primitive.reset();
        }

        fn input(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
            None // PrimitiveState doesn't expose AnyPin directly
        }

        fn input_mut(
            &mut self,
            _name: &str,
        ) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
            None
        }

        fn output(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
            None
        }

        fn output_mut(
            &mut self,
            _name: &str,
        ) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
            None
        }

        fn input_names(&self) -> Vec<&str> {
            self.primitive.input_names()
        }

        fn output_names(&self) -> Vec<&str> {
            self.primitive.output_names()
        }

        fn error(&self) -> Option<&str> {
            self.primitive.error()
        }

        fn set_error(&mut self, error: String) {
            self.primitive.set_error(error);
        }

        fn clear_error(&mut self) {
            self.primitive.clear_error();
        }

        fn snapshot(&self) -> serde_json::Value {
            serde_json::json!({})
        }

        fn restore(&mut self, _state: &serde_json::Value) {
            // No state to restore
        }
    };
}

/// Define a comparison unit (two inputs, boolean output)
#[macro_export]
macro_rules! comparison_unit {
    ($name:ident, $op:tt, $desc:literal) => {
        #[derive(Debug)]
        pub struct $name {
            primitive: $crate::PrimitiveState,
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            pub fn new() -> Self {
                let mut primitive = $crate::PrimitiveState::new(stringify!($name));
                primitive.add_input::<f64>("a");
                primitive.add_input::<f64>("b");
                primitive.add_output::<bool>("result");
                Self { primitive }
            }
        }

        impl $crate::Unit for $name {
            fn id(&self) -> &str {
                self.primitive.id()
            }

            fn lifecycle(&self) -> $crate::Lifecycle {
                self.primitive.lifecycle()
            }

            fn play(&mut self) {
                self.primitive.play();
            }

            fn pause(&mut self) {
                self.primitive.pause();
            }

            fn reset(&mut self) {
                self.primitive.reset();
            }

            fn input(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None
            }

            fn input_mut(&mut self, _name: &str) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn output(&self, _name: &str) -> Option<&dyn $crate::any_pin::AnyPin> {
                None
            }

            fn output_mut(&mut self, _name: &str) -> Option<&mut (dyn $crate::any_pin::AnyPin + 'static)> {
                None
            }

            fn input_names(&self) -> Vec<&str> {
                self.primitive.input_names()
            }

            fn output_names(&self) -> Vec<&str> {
                self.primitive.output_names()
            }

            fn error(&self) -> Option<&str> {
                self.primitive.error()
            }

            fn set_error(&mut self, error: String) {
                self.primitive.set_error(error);
            }

            fn clear_error(&mut self) {
                self.primitive.clear_error();
            }

            fn snapshot(&self) -> serde_json::Value {
                serde_json::json!({})
            }

            fn restore(&mut self, _state: &serde_json::Value) {}

            fn push_input(&mut self, name: &str, data: Box<dyn std::any::Any + Send>) -> Result<(), String> {
                if let Err(e) = self.primitive.push_input(name, data) {
                    return Err(e);
                }

                if let (Some(a), Some(b)) = (
                    self.primitive.input::<f64>("a").and_then(|p| p.peak().copied()),
                    self.primitive.input::<f64>("b").and_then(|p| p.peak().copied()),
                ) {
                    let result = a $op b;
                    if let Some(out) = self.primitive.output_mut::<bool>("result") {
                        let _ = out.push(result);
                    }
                }
                Ok(())
            }

            fn take_output(&mut self, name: &str) -> Option<Box<dyn std::any::Any + Send>> {
                self.primitive.take_output(name)
            }

            fn description(&self) -> &str {
                $desc
            }
        }
    };
}
