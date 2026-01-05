//! Sub-query execution for llm_query() calls.
//!
//! Implements the core RLM innovation: the model can programmatically
//! call sub-LMs over fragments of the context.
//!
//! This module:
//! 1. Parses Python code for llm_query() calls
//! 2. Extracts the arguments (prompt and text)
//! 3. Executes the sub-queries via FM Bridge
//! 4. Replaces the calls with their results

use regex::Regex;

use crate::error::{Result, RlmError};

/// A parsed llm_query call from Python code.
#[derive(Debug, Clone)]
pub struct LlmQueryCall {
    /// The full matched text (e.g., `llm_query("prompt", context[0:100])`)
    pub full_match: String,
    /// Start position in the original code.
    pub start: usize,
    /// End position in the original code.
    pub end: usize,
    /// The prompt argument.
    pub prompt: String,
    /// The text argument (may be a variable reference like `context[0:100]`).
    pub text_expr: String,
}

/// A batch of llm_query calls (for llm_query_batch).
#[derive(Debug, Clone)]
pub struct LlmQueryBatchCall {
    /// The full matched text.
    pub full_match: String,
    /// Start position in the original code.
    pub start: usize,
    /// End position in the original code.
    pub end: usize,
    /// The list of (prompt, text) pairs.
    pub queries: Vec<(String, String)>,
}

/// Parse llm_query() calls from Python code.
///
/// Matches patterns like:
/// - `llm_query("prompt", "text")`
/// - `llm_query("prompt", context[0:100])`
/// - `llm_query("prompt", some_variable)`
pub fn parse_llm_query_calls(code: &str) -> Vec<LlmQueryCall> {
    let mut calls = Vec::new();

    // Match llm_query("...", ...)
    // This is a simplified parser - it handles common cases but not all Python edge cases
    let re = Regex::new(r#"llm_query\s*\(\s*("[^"]*"|'[^']*')\s*,\s*([^)]+)\)"#).unwrap();

    for cap in re.captures_iter(code) {
        let full_match = cap.get(0).unwrap();
        let prompt_match = cap.get(1).unwrap();
        let text_match = cap.get(2).unwrap();

        // Remove quotes from prompt
        let prompt = prompt_match.as_str();
        let prompt = &prompt[1..prompt.len() - 1]; // Remove surrounding quotes

        calls.push(LlmQueryCall {
            full_match: full_match.as_str().to_string(),
            start: full_match.start(),
            end: full_match.end(),
            prompt: prompt.to_string(),
            text_expr: text_match.as_str().trim().to_string(),
        });
    }

    calls
}

/// Check if a text expression is a simple string literal.
pub fn is_string_literal(expr: &str) -> bool {
    let trimmed = expr.trim();
    (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
}

/// Extract string literal value (remove quotes).
pub fn extract_string_literal(expr: &str) -> Option<String> {
    let trimmed = expr.trim();
    if is_string_literal(trimmed) {
        Some(trimmed[1..trimmed.len() - 1].to_string())
    } else {
        None
    }
}

/// Evaluate a context slice expression and return the text.
///
/// Handles expressions like:
/// - `context[0:100]`
/// - `context[start:end]`
/// - `context[:500]`
///
/// Returns None if the expression can't be evaluated statically.
pub fn evaluate_context_slice(expr: &str, context: &str) -> Option<String> {
    let trimmed = expr.trim();

    // Check for context[start:end] pattern
    let re = Regex::new(r"^context\[(\d*):(\d*)\]$").unwrap();
    if let Some(cap) = re.captures(trimmed) {
        let start_str = cap.get(1).map(|m| m.as_str()).unwrap_or("");
        let end_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");

        let start: usize = if start_str.is_empty() {
            0
        } else {
            start_str.parse().ok()?
        };

        let end: usize = if end_str.is_empty() {
            context.len()
        } else {
            end_str.parse().ok()?
        };

        let start = start.min(context.len());
        let end = end.min(context.len());

        if start <= end {
            return Some(context[start..end].to_string());
        }
    }

    None
}

/// Result of processing llm_query calls in code.
#[derive(Debug, Clone)]
pub struct ProcessedCode {
    /// The modified code with llm_query calls replaced.
    pub code: String,
    /// The sub-queries that need to be executed.
    pub pending_queries: Vec<PendingQuery>,
    /// Whether any dynamic expressions couldn't be resolved.
    pub has_unresolved: bool,
}

/// A query that needs to be executed.
#[derive(Debug, Clone)]
pub struct PendingQuery {
    /// Unique identifier for this query (used to replace in code).
    pub id: String,
    /// The prompt to send to the LLM.
    pub prompt: String,
    /// The text fragment to query over.
    pub text: String,
}

/// Process code to extract and prepare llm_query calls.
///
/// For each llm_query call:
/// 1. Try to resolve the text expression (context slice or literal)
/// 2. If resolvable, create a PendingQuery and replace with a placeholder
/// 3. If not resolvable, leave as-is (will be handled at runtime)
pub fn process_code_for_queries(code: &str, context: Option<&str>) -> ProcessedCode {
    let calls = parse_llm_query_calls(code);

    if calls.is_empty() {
        return ProcessedCode {
            code: code.to_string(),
            pending_queries: vec![],
            has_unresolved: false,
        };
    }

    let mut result_code = code.to_string();
    let mut pending_queries = Vec::new();
    let mut has_unresolved = false;

    // Process in reverse order to maintain correct positions
    let mut sorted_calls = calls;
    sorted_calls.sort_by(|a, b| b.start.cmp(&a.start));

    for (idx, call) in sorted_calls.iter().enumerate() {
        let query_id = format!("__llm_query_result_{}", idx);

        // Try to resolve the text expression
        let text = if let Some(literal) = extract_string_literal(&call.text_expr) {
            Some(literal)
        } else if let Some(ctx) = context {
            evaluate_context_slice(&call.text_expr, ctx)
        } else {
            None
        };

        if let Some(text) = text {
            pending_queries.push(PendingQuery {
                id: query_id.clone(),
                prompt: call.prompt.clone(),
                text,
            });

            // Replace the llm_query call with a variable reference
            result_code.replace_range(call.start..call.end, &query_id);
        } else {
            has_unresolved = true;
        }
    }

    // Reverse pending_queries since we processed in reverse order
    pending_queries.reverse();

    ProcessedCode {
        code: result_code,
        pending_queries,
        has_unresolved,
    }
}

/// Execute a sub-query via FM Bridge and return the result.
pub async fn execute_sub_query(
    client: &fm_bridge::FMClient,
    prompt: &str,
    text: &str,
) -> Result<String> {
    // Build the sub-query prompt
    let sub_prompt = format!(
        "Process the following text according to the instruction.\n\n\
         Instruction: {}\n\n\
         Text:\n{}\n\n\
         Response:",
        prompt, text
    );

    let response = client.complete(&sub_prompt, None).await?;
    let result = response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(result)
}

/// Generate code to inject query results as variables.
pub fn generate_result_injection(results: &[(String, String)]) -> String {
    let mut code = String::new();
    code.push_str("# Injected llm_query results\n");

    for (id, result) in results {
        // Escape the result for Python string literal
        let escaped = result
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t");

        code.push_str(&format!("{} = \"{}\"\n", id, escaped));
    }

    code.push('\n');
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_llm_query() {
        let code = r#"result = llm_query("Summarize this", "some text here")"#;
        let calls = parse_llm_query_calls(code);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].prompt, "Summarize this");
        assert_eq!(calls[0].text_expr, "\"some text here\"");
    }

    #[test]
    fn test_parse_context_slice() {
        let code = r#"result = llm_query("Extract names", context[0:1000])"#;
        let calls = parse_llm_query_calls(code);

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].prompt, "Extract names");
        assert_eq!(calls[0].text_expr, "context[0:1000]");
    }

    #[test]
    fn test_parse_multiple_calls() {
        let code = r#"
a = llm_query("Task A", context[0:500])
b = llm_query("Task B", context[500:1000])
"#;
        let calls = parse_llm_query_calls(code);
        assert_eq!(calls.len(), 2);
    }

    #[test]
    fn test_evaluate_context_slice() {
        let context = "Hello, world! This is a test.";

        assert_eq!(
            evaluate_context_slice("context[0:5]", context),
            Some("Hello".to_string())
        );
        assert_eq!(
            evaluate_context_slice("context[:5]", context),
            Some("Hello".to_string())
        );
        assert_eq!(
            evaluate_context_slice("context[7:]", context),
            Some("world! This is a test.".to_string())
        );
    }

    #[test]
    fn test_is_string_literal() {
        assert!(is_string_literal("\"hello\""));
        assert!(is_string_literal("'hello'"));
        assert!(!is_string_literal("context[0:100]"));
        assert!(!is_string_literal("some_variable"));
    }

    #[test]
    fn test_process_code_for_queries() {
        let code = r#"result = llm_query("Summarize", context[0:100])"#;
        let context = "The quick brown fox jumps over the lazy dog. This is additional text to make it longer than 100 characters for testing purposes.";

        let processed = process_code_for_queries(code, Some(context));

        assert_eq!(processed.pending_queries.len(), 1);
        assert_eq!(processed.pending_queries[0].prompt, "Summarize");
        assert!(processed.pending_queries[0].text.starts_with("The quick"));
        assert!(!processed.has_unresolved);
    }
}
