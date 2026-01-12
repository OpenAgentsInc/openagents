//! Code Edit Signature.
//!
//! Generates code changes as unified diff patches.

use crate::core::signature::MetaSignature;
use crate::data::example::Example;
use anyhow::Result;
use serde_json::{Value, json};

/// Signature for generating code edits as unified diffs.
///
/// # Inputs
/// - `file_path`: Path to the file being edited
/// - `current_content`: Current file contents
/// - `edit_instruction`: What to change (from plan step)
/// - `code_context`: Related code for consistency
///
/// # Outputs
/// - `unified_diff`: Patch in unified diff format
/// - `edit_summary`: Human-readable description of changes
/// - `affected_lines`: Line numbers affected
/// - `confidence`: Confidence in the edit (0.0-1.0)
#[derive(Debug, Clone)]
pub struct CodeEditSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for CodeEditSignature {
    fn default() -> Self {
        Self {
            instruction: r#"You are an expert code editor. Given a file and an edit instruction,
generate precise code changes in unified diff format.

Rules:
1. Output ONLY valid unified diff format (--- a/file, +++ b/file, @@ lines, +/- changes)
2. Preserve existing code style (indentation, naming conventions, etc.)
3. Make minimal changes - only modify what's necessary
4. Ensure the resulting code will compile/run correctly
5. Include enough context lines (3) for the diff to apply cleanly

The unified diff format:
```
--- a/path/to/file
+++ b/path/to/file
@@ -start,count +start,count @@
 context line
-removed line
+added line
 context line
```

Be precise and surgical with your edits."#
                .to_string(),
            demos: vec![],
        }
    }
}

impl CodeEditSignature {
    /// Create a new code edit signature.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set custom instruction.
    pub fn with_instruction(mut self, instruction: impl Into<String>) -> Self {
        self.instruction = instruction.into();
        self
    }

    /// Add a demonstration example.
    pub fn with_demo(mut self, demo: Example) -> Self {
        self.demos.push(demo);
        self
    }
}

impl MetaSignature for CodeEditSignature {
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
        json!({
            "file_path": {
                "type": "String",
                "desc": "Path to the file being edited",
                "__dsrs_field_type": "input"
            },
            "current_content": {
                "type": "String",
                "desc": "Current contents of the file",
                "__dsrs_field_type": "input"
            },
            "edit_instruction": {
                "type": "String",
                "desc": "What to change (from implementation plan step)",
                "__dsrs_field_type": "input"
            },
            "code_context": {
                "type": "String",
                "desc": "Related code snippets for style consistency",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "unified_diff": {
                "type": "String",
                "desc": "Code changes in unified diff format",
                "__dsrs_field_type": "output"
            },
            "edit_summary": {
                "type": "String",
                "desc": "Human-readable summary of the changes made",
                "__dsrs_field_type": "output"
            },
            "affected_lines": {
                "type": "String",
                "desc": "Line numbers affected by this edit",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in the correctness of this edit (0.0-1.0)",
                "__dsrs_field_type": "output"
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_code_edit_signature() {
        let sig = CodeEditSignature::new();

        assert!(!sig.instruction().is_empty());

        let inputs = sig.input_fields();
        assert!(inputs.get("file_path").is_some());
        assert!(inputs.get("current_content").is_some());
        assert!(inputs.get("edit_instruction").is_some());
        assert!(inputs.get("code_context").is_some());

        let outputs = sig.output_fields();
        assert!(outputs.get("unified_diff").is_some());
        assert!(outputs.get("edit_summary").is_some());
        assert!(outputs.get("affected_lines").is_some());
        assert!(outputs.get("confidence").is_some());
    }
}
