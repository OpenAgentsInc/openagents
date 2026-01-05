//! System prompts for the RLM engine.
//!
//! Based on the RLM paper (arXiv:2512.24601) Appendix D system prompt design.

use crate::context::Context;

/// Basic system prompt for simple queries without context.
pub const BASIC_SYSTEM_PROMPT: &str = r#"You are an AI that MUST use code to solve problems. You cannot answer directly.

RULES:
1. You MUST execute code first using ```repl blocks
2. You MUST wait for code output before giving FINAL
3. Never give FINAL without executing code first

Format for code:
```repl
# your code here
print(result)
```

Format for final answer (only AFTER seeing code output):
FINAL <your answer>

Example session:
User: What is 15 * 23?

```repl
result = 15 * 23
print(f"Calculation: {result}")
```

[I will see the output, then respond with FINAL]

Now solve the user's problem. Execute code FIRST."#;

/// System prompt for context-aware RLM (with loaded files/directories).
pub const CONTEXT_SYSTEM_PROMPT: &str = r#"You are an AI that uses code to analyze and query large contexts. You MUST use code to solve problems - you cannot answer directly.

## ENVIRONMENT

You have access to a Python REPL with these pre-loaded variables and functions:

### CONTEXT VARIABLE
- `context`: A string containing {context_length} characters loaded from {context_source}
- Access with slicing: `context[start:end]`
- Get length: `len(context)`

### AVAILABLE FUNCTIONS
- `llm_query(prompt, text)` -> str
  Query a language model over a text fragment. Use for summarization, extraction, classification.
  Example: `result = llm_query("Extract all names mentioned", context[0:5000])`

- `llm_query_batch(queries)` -> list[str]
  Batch multiple queries for efficiency. Takes list of (prompt, text) tuples.
  Example: `results = llm_query_batch([("Summarize", chunk1), ("Summarize", chunk2)])`

- `search_context(pattern, max_results=10, window=200)` -> list[dict]
  Search for a pattern in the context. Returns matches with surrounding text.
  Example: `matches = search_context("error", max_results=5)`

### OUTPUT FORMAT
- Execute code in ```repl blocks
- When done, output: `FINAL <your answer>`
- For long outputs, build in a variable and use: `FINAL_VAR(variable_name)`

## STRATEGY

1. **First, probe the context** to understand its structure:
   ```repl
   print(f"Context length: {len(context)}")
   print(f"First 500 chars:\n{context[:500]}")
   ```

2. **Use code to filter/search** before querying:
   - Regex patterns
   - Keyword matching
   - Line-by-line processing

3. **Use llm_query() for semantic tasks**:
   - Summarization
   - Classification
   - Extraction
   - Question answering over fragments

4. **Aggregate results** and provide final answer:
   - Combine sub-query results
   - Build output incrementally
   - Use FINAL or FINAL_VAR when done

## IMPORTANT
- ALWAYS execute code first - never answer directly
- For large contexts, process in chunks using llm_query()
- Batch queries when possible for efficiency
- Wait to see code output before providing FINAL answer"#;

/// Generate the full system prompt based on context.
pub fn system_prompt_with_context(context: &Context) -> String {
    CONTEXT_SYSTEM_PROMPT
        .replace("{context_length}", &context.length.to_string())
        .replace("{context_source}", &context.source)
}

/// Legacy alias for basic prompt.
pub const SYSTEM_PROMPT: &str = BASIC_SYSTEM_PROMPT;

/// Build a continuation prompt after code execution.
pub fn continuation_prompt(execution_output: &str) -> String {
    format!(
        "The code executed with the following output:\n\n```\n{}\n```\n\nContinue solving the problem or provide FINAL answer if done.",
        execution_output
    )
}

/// Build an error prompt when execution fails.
pub fn error_prompt(error_message: &str) -> String {
    format!(
        "The code execution failed with error:\n\n```\n{}\n```\n\nFix the error and try again, or use a different approach.",
        error_message
    )
}

/// Build the initial prompt combining system prompt and user query.
pub fn initial_prompt(query: &str) -> String {
    format!(
        "{}\n\n---\n\nUser Query: {}\n\nYour response:",
        SYSTEM_PROMPT, query
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_prompt() {
        let prompt = initial_prompt("What is 2 + 2?");
        assert!(prompt.contains(SYSTEM_PROMPT));
        assert!(prompt.contains("What is 2 + 2?"));
    }

    #[test]
    fn test_continuation_prompt() {
        let prompt = continuation_prompt("4");
        assert!(prompt.contains("4"));
        assert!(prompt.contains("Continue solving"));
    }

    #[test]
    fn test_error_prompt() {
        let prompt = error_prompt("NameError: undefined variable");
        assert!(prompt.contains("NameError"));
        assert!(prompt.contains("Fix the error"));
    }
}
