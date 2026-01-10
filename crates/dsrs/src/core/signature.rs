use crate::Example;
use anyhow::Result;
use serde_json::Value;

pub trait MetaSignature: Send + Sync {
    fn signature_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
    fn demos(&self) -> Vec<Example>;
    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()>;
    fn instruction(&self) -> String;
    fn input_fields(&self) -> Value;
    fn output_fields(&self) -> Value;

    fn update_instruction(&mut self, instruction: String) -> Result<()>;
    fn append(&mut self, name: &str, value: Value) -> Result<()>;
}

/// A dummy signature for testing purposes.
#[derive(Debug, Clone, Default)]
pub struct DummySignature {
    instruction: String,
    demos: Vec<Example>,
}

impl DummySignature {
    /// Create a new dummy signature.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with an instruction.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }
}

impl MetaSignature for DummySignature {
    fn demos(&self) -> Vec<Example> {
        self.demos.clone()
    }

    fn set_demos(&mut self, demos: Vec<Example>) -> Result<()> {
        self.demos = demos;
        Ok(())
    }

    fn instruction(&self) -> String {
        self.instruction.clone()
    }

    fn input_fields(&self) -> Value {
        serde_json::json!({
            "input": {"type": "String", "desc": "Input field"}
        })
    }

    fn output_fields(&self) -> Value {
        serde_json::json!({
            "output": {"type": "String", "desc": "Output field"}
        })
    }

    fn update_instruction(&mut self, instruction: String) -> Result<()> {
        self.instruction = instruction;
        Ok(())
    }

    fn append(&mut self, _name: &str, _value: Value) -> Result<()> {
        Ok(())
    }
}
