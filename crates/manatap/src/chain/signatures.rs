//! Signature definitions for the markdown summarization chain.

use anyhow::Result;
use dsrs::core::signature::MetaSignature;
use dsrs::data::example::Example;
use serde_json::{Value, json};
use std::collections::HashMap;

/// Signature for analyzing a user's task and extracting intent.
#[derive(Debug, Clone)]
pub struct TaskAnalysisSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for TaskAnalysisSignature {
    fn default() -> Self {
        let instruction = r#"Analyze the user's request to understand what they want to accomplish.

Extract:
1. task_type: The type of task (summarize, search, analyze, modify, list)
2. file_pattern: Glob pattern for target files (e.g., "*.md", "src/**/*.rs")
3. scope: Where to look - "root" (only root directory), "recursive" (all subdirectories), or a specific path
4. output_action: What to do with results (summarize, list, aggregate, count)
5. confidence: How confident you are in this interpretation (0.0-1.0)

Be precise about file patterns. "root level" means only files directly in the root, not subdirectories.
"markdown files" means "*.md" pattern."#
            .to_string();

        // Demo example
        let mut demo_data = HashMap::new();
        demo_data.insert(
            "prompt".to_string(),
            json!("Summarize the markdown files in the root level of this repository."),
        );
        demo_data.insert("task_type".to_string(), json!("summarize"));
        demo_data.insert("file_pattern".to_string(), json!("*.md"));
        demo_data.insert("scope".to_string(), json!("root"));
        demo_data.insert("output_action".to_string(), json!("summarize"));
        demo_data.insert("confidence".to_string(), json!(0.95));

        let demo = Example {
            data: demo_data,
            input_keys: vec!["prompt".to_string()],
            output_keys: vec![
                "task_type".to_string(),
                "file_pattern".to_string(),
                "scope".to_string(),
                "output_action".to_string(),
                "confidence".to_string(),
            ],
            node_id: None,
        };

        Self {
            instruction,
            demos: vec![demo],
        }
    }
}

impl TaskAnalysisSignature {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MetaSignature for TaskAnalysisSignature {
    fn signature_name(&self) -> &'static str {
        "TaskAnalysisSignature"
    }

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
            "prompt": {
                "type": "String",
                "desc": "The user's natural language request",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "task_type": {
                "type": "String",
                "desc": "Type of task: summarize, search, analyze, modify, list",
                "__dsrs_field_type": "output"
            },
            "file_pattern": {
                "type": "String",
                "desc": "Glob pattern for target files (e.g., *.md, src/**/*.rs)",
                "__dsrs_field_type": "output"
            },
            "scope": {
                "type": "String",
                "desc": "Scope: root, recursive, or specific path",
                "__dsrs_field_type": "output"
            },
            "output_action": {
                "type": "String",
                "desc": "What to do with results: summarize, list, aggregate, count",
                "__dsrs_field_type": "output"
            },
            "confidence": {
                "type": "f32",
                "desc": "Confidence in interpretation (0.0-1.0)",
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

/// Signature for summarizing a single file's content.
#[derive(Debug, Clone)]
pub struct ContentSummarizerSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for ContentSummarizerSignature {
    fn default() -> Self {
        let instruction = r#"Summarize the provided file content.

For the given content:
1. summary: Write a concise one-paragraph summary of the file's purpose and contents
2. key_points: Extract 3-5 key points as a JSON array of strings
3. topic: Identify the primary topic or theme
4. sections: List any major sections/headings found (as JSON array)

Adjust detail based on content length. Be concise but capture essential information."#
            .to_string();

        // Demo example
        let mut demo_data = HashMap::new();
        demo_data.insert(
            "content".to_string(),
            json!("# Project Name\n\nA tool for doing X and Y.\n\n## Installation\n\nRun `cargo install`..."),
        );
        demo_data.insert("filename".to_string(), json!("README.md"));
        demo_data.insert("content_type".to_string(), json!("markdown"));
        demo_data.insert(
            "summary".to_string(),
            json!("README.md documents a tool for X and Y, providing installation instructions."),
        );
        demo_data.insert(
            "key_points".to_string(),
            json!(["Tool for X and Y", "Installation via cargo"]),
        );
        demo_data.insert("topic".to_string(), json!("Project documentation"));
        demo_data.insert(
            "sections".to_string(),
            json!(["Project Name", "Installation"]),
        );

        let demo = Example {
            data: demo_data,
            input_keys: vec![
                "content".to_string(),
                "filename".to_string(),
                "content_type".to_string(),
            ],
            output_keys: vec![
                "summary".to_string(),
                "key_points".to_string(),
                "topic".to_string(),
                "sections".to_string(),
            ],
            node_id: None,
        };

        Self {
            instruction,
            demos: vec![demo],
        }
    }
}

impl ContentSummarizerSignature {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MetaSignature for ContentSummarizerSignature {
    fn signature_name(&self) -> &'static str {
        "ContentSummarizerSignature"
    }

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
            "content": {
                "type": "String",
                "desc": "The file content to summarize",
                "__dsrs_field_type": "input"
            },
            "filename": {
                "type": "String",
                "desc": "Source filename for context",
                "__dsrs_field_type": "input"
            },
            "content_type": {
                "type": "String",
                "desc": "Content type: markdown, code, text, config",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "summary": {
                "type": "String",
                "desc": "One-paragraph summary of the content",
                "__dsrs_field_type": "output"
            },
            "key_points": {
                "type": "String",
                "desc": "JSON array of 3-5 key points",
                "__dsrs_field_type": "output"
            },
            "topic": {
                "type": "String",
                "desc": "Primary topic or theme",
                "__dsrs_field_type": "output"
            },
            "sections": {
                "type": "String",
                "desc": "JSON array of major sections/headings",
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

/// Signature for aggregating multiple summaries into a final output.
#[derive(Debug, Clone)]
pub struct SummaryAggregatorSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for SummaryAggregatorSignature {
    fn default() -> Self {
        let instruction = r#"Synthesize multiple file summaries into a coherent final response.

Given summaries from multiple files and the original user request:
1. final_summary: Write a comprehensive response that addresses the user's original request
2. themes: Identify common themes across all files (as JSON array)
3. key_takeaways: Extract 5-7 key takeaways spanning all files (as JSON array)
4. suggestions: Suggest logical next steps or related files to explore (as JSON array)

The final summary should directly answer what the user asked for, synthesizing information from all the file summaries provided."#
            .to_string();

        Self {
            instruction,
            demos: vec![],
        }
    }
}

impl SummaryAggregatorSignature {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MetaSignature for SummaryAggregatorSignature {
    fn signature_name(&self) -> &'static str {
        "SummaryAggregatorSignature"
    }

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
            "summaries": {
                "type": "String",
                "desc": "JSON array of {filename, summary, key_points, topic} objects",
                "__dsrs_field_type": "input"
            },
            "original_request": {
                "type": "String",
                "desc": "The user's original request",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "final_summary": {
                "type": "String",
                "desc": "Combined summary addressing the user's request",
                "__dsrs_field_type": "output"
            },
            "themes": {
                "type": "String",
                "desc": "JSON array of common themes across files",
                "__dsrs_field_type": "output"
            },
            "key_takeaways": {
                "type": "String",
                "desc": "JSON array of 5-7 key takeaways",
                "__dsrs_field_type": "output"
            },
            "suggestions": {
                "type": "String",
                "desc": "JSON array of suggested next steps",
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

/// Signature for generating curiosity questions about the codebase.
#[derive(Debug, Clone)]
pub struct CuriosityGeneratorSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for CuriosityGeneratorSignature {
    fn default() -> Self {
        let instruction =
            r#"Based on the summary of this codebase and any previous questions explored,
generate a specific, actionable question that would deepen understanding.

Focus on:
- Architecture patterns not fully explained in the summary
- Connections between components that aren't clear
- Implementation details worth exploring
- Potential improvements or concerns raised by the summary

Output:
1. question: A focused, specific question about the codebase
2. search_patterns: JSON array of regex patterns or keywords to search for
3. reasoning: Brief explanation of why this question is interesting

Avoid repeating questions that have already been explored (see previous_questions).
Make search patterns specific enough to find relevant code."#
                .to_string();

        // Demo example
        let mut demo_data = HashMap::new();
        demo_data.insert(
            "summary".to_string(),
            json!("The codebase implements a DSPy chain visualizer with 5 stages..."),
        );
        demo_data.insert("previous_questions".to_string(), json!("[]"));
        demo_data.insert("iteration".to_string(), json!(0));
        demo_data.insert(
            "question".to_string(),
            json!("How does the callback system propagate events from the LLM to the UI?"),
        );
        demo_data.insert(
            "search_patterns".to_string(),
            json!(["DspyCallback", "ChainEvent", "event_sender"]),
        );
        demo_data.insert(
            "reasoning".to_string(),
            json!("Understanding the event flow is key to understanding how the UI stays synchronized."),
        );

        let demo = Example {
            data: demo_data,
            input_keys: vec![
                "summary".to_string(),
                "previous_questions".to_string(),
                "iteration".to_string(),
            ],
            output_keys: vec![
                "question".to_string(),
                "search_patterns".to_string(),
                "reasoning".to_string(),
            ],
            node_id: None,
        };

        Self {
            instruction,
            demos: vec![demo],
        }
    }
}

impl CuriosityGeneratorSignature {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MetaSignature for CuriosityGeneratorSignature {
    fn signature_name(&self) -> &'static str {
        "CuriosityGeneratorSignature"
    }

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
            "summary": {
                "type": "String",
                "desc": "The aggregated summary of the codebase",
                "__dsrs_field_type": "input"
            },
            "previous_questions": {
                "type": "String",
                "desc": "JSON array of questions already explored",
                "__dsrs_field_type": "input"
            },
            "iteration": {
                "type": "i32",
                "desc": "Current iteration number (0-indexed)",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "question": {
                "type": "String",
                "desc": "A specific question about the codebase",
                "__dsrs_field_type": "output"
            },
            "search_patterns": {
                "type": "String",
                "desc": "JSON array of regex patterns or keywords to search",
                "__dsrs_field_type": "output"
            },
            "reasoning": {
                "type": "String",
                "desc": "Brief explanation of why this question is interesting",
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

/// Signature for answering questions based on code snippets.
#[derive(Debug, Clone)]
pub struct QuestionAnswererSignature {
    instruction: String,
    demos: Vec<Example>,
}

impl Default for QuestionAnswererSignature {
    fn default() -> Self {
        let instruction =
            r#"Given a specific question about the codebase and relevant code snippets,
provide a comprehensive answer.

Include in your response:
1. answer: Direct answer to the question based on the code snippets provided
2. insights: JSON array of additional insights discovered while analyzing the code
3. follow_up_topics: JSON array of related topics that could be explored next

Be specific and reference the code snippets directly. If the snippets don't fully answer
the question, note what additional information would be needed."#
                .to_string();

        Self {
            instruction,
            demos: vec![],
        }
    }
}

impl QuestionAnswererSignature {
    pub fn new() -> Self {
        Self::default()
    }
}

impl MetaSignature for QuestionAnswererSignature {
    fn signature_name(&self) -> &'static str {
        "QuestionAnswererSignature"
    }

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
            "question": {
                "type": "String",
                "desc": "The question to answer",
                "__dsrs_field_type": "input"
            },
            "code_snippets": {
                "type": "String",
                "desc": "Relevant code snippets found by searching",
                "__dsrs_field_type": "input"
            },
            "context": {
                "type": "String",
                "desc": "Additional context from previous exploration",
                "__dsrs_field_type": "input"
            }
        })
    }

    fn output_fields(&self) -> Value {
        json!({
            "answer": {
                "type": "String",
                "desc": "Direct answer to the question",
                "__dsrs_field_type": "output"
            },
            "insights": {
                "type": "String",
                "desc": "JSON array of additional insights",
                "__dsrs_field_type": "output"
            },
            "follow_up_topics": {
                "type": "String",
                "desc": "JSON array of related topics to explore",
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
