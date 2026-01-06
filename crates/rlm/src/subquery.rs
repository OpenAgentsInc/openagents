//! Sub-query execution for llm_query() calls.
//!
//! Implements the core RLM innovation: the model can programmatically
//! call sub-LMs over fragments of the context.
//!
//! This module:
//! 1. Parses Python code for llm_query() calls
//! 2. Extracts the arguments (prompt and text)
//! 3. Executes the sub-queries via LLM client
//! 4. Replaces the calls with their results

use regex::Regex;

use crate::client::LlmClient;
use crate::error::Result;

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

/// Find the comma that separates function arguments.
///
/// Skips commas inside quotes to find the argument separator.
fn find_arg_separator(args: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for (i, ch) in args.char_indices() {
        if ch == '"' || ch == '\'' {
            if !in_quotes {
                in_quotes = true;
                quote_char = ch;
            } else if ch == quote_char {
                in_quotes = false;
            }
        } else if ch == ',' && !in_quotes {
            return Some(i);
        }
    }

    None
}

/// Extract string content from f"...", r"...", or "..." format.
///
/// This is a simplified extractor - it doesn't handle all Python edge cases
/// but works for the common patterns GPT-5 generates.
fn extract_string_content(s: &str) -> String {
    let trimmed = s.trim();

    // Handle f"..." or f'...'
    if trimmed.starts_with('f') || trimmed.starts_with('F') {
        let without_prefix = &trimmed[1..];
        return strip_quotes(without_prefix);
    }

    // Handle r"..." or r'...'
    if trimmed.starts_with('r') || trimmed.starts_with('R') {
        let without_prefix = &trimmed[1..];
        return strip_quotes(without_prefix);
    }

    // Regular "..." or '...'
    strip_quotes(trimmed)
}

fn strip_quotes(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        trimmed[1..trimmed.len() - 1].to_string()
    } else {
        trimmed.to_string()
    }
}

/// Parse llm_query() calls from Python code.
///
/// Matches patterns like:
/// - `llm_query("prompt", "text")`
/// - `llm_query(f"prompt with {var}", context[0:100])`
/// - `llm_query("prompt", some_variable)`
pub fn parse_llm_query_calls(code: &str) -> Vec<LlmQueryCall> {
    let mut calls = Vec::new();

    // Parse llm_query calls manually to handle f-strings with complex content
    let mut search_pos = 0;
    while let Some(call_start) = code[search_pos..].find("llm_query") {
        let abs_call_start = search_pos + call_start;
        let after_name = &code[abs_call_start + 9..]; // Skip "llm_query"

        // Find opening paren
        if let Some(paren_idx) = after_name.find('(') {
            let args_start = abs_call_start + 9 + paren_idx + 1;

            // Find the matching closing paren by counting parens
            let mut depth = 1;
            let mut pos = args_start;
            let mut close_paren = None;

            for (i, ch) in code[args_start..].char_indices() {
                if ch == '(' {
                    depth += 1;
                } else if ch == ')' {
                    depth -= 1;
                    if depth == 0 {
                        close_paren = Some(args_start + i);
                        break;
                    }
                }
            }

            if let Some(close_pos) = close_paren {
                // Extract the full call
                let full_call = &code[abs_call_start..close_pos + 1];
                let args = &code[args_start..close_pos];

                // Find the comma that separates arguments (not commas inside quotes)
                if let Some(comma_pos) = find_arg_separator(args) {
                    let prompt_str = args[..comma_pos].trim();
                    let text_expr = args[comma_pos + 1..].trim();

                    // Extract prompt content (strip f" or " prefix/suffix)
                    let prompt = extract_string_content(prompt_str);

                    calls.push(LlmQueryCall {
                        full_match: full_call.to_string(),
                        start: abs_call_start,
                        end: close_pos + 1,
                        prompt,
                        text_expr: text_expr.to_string(),
                    });
                }

                search_pos = close_pos + 1;
            } else {
                search_pos = args_start;
            }
        } else {
            break;
        }
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
/// - `context` - the full context
/// - `context[0:100]` - a slice
/// - `context[start:end]` - a slice with numeric bounds
/// - `context[:500]` - from start to 500
///
/// Returns None if the expression can't be evaluated statically.
pub fn evaluate_context_slice(expr: &str, context: &str) -> Option<String> {
    let trimmed = expr.trim();

    // Handle plain "context" - return full context
    if trimmed == "context" {
        return Some(context.to_string());
    }

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

/// Execute a sub-query via LLM client and return the result.
pub async fn execute_sub_query<C: LlmClient>(
    client: &C,
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
    let result = response.content().to_string();

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
