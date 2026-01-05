//! System prompts for the RLM engine.
//!
//! Based on the RLM paper (arXiv:2512.24601) Appendix D system prompt design.
//!
//! # Tiered Prompt System
//!
//! Different models have different capabilities. This module provides three
//! prompt tiers to match model capability:
//!
//! - **Full**: For GPT-5 class models that can orchestrate llm_query() calls
//! - **Guided**: For Apple FM and similar - step-by-step with explicit examples
//! - **Minimal**: For very small models - one instruction at a time

use crate::context::Context;

/// Prompt tier based on model capability.
///
/// Different models require different prompt complexity levels:
/// - GPT-5 class models can handle the full RLM prompt with llm_query()
/// - Apple FM needs simpler, guided prompts without meta-reasoning
/// - Very small models need minimal, single-instruction prompts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromptTier {
    /// Full RLM capabilities: llm_query(), FINAL_VAR, batch queries.
    /// For GPT-5 class models with strong instruction following.
    #[default]
    Full,

    /// Guided prompts with explicit examples, no llm_query().
    /// For Apple FM and similar models that need step-by-step guidance.
    Guided,

    /// Minimal prompts - one instruction at a time.
    /// For very small models with limited context windows.
    Minimal,
}

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

/// Guided system prompt for Apple FM and similar models.
///
/// Key differences from the full prompt:
/// - No llm_query() - Apple FM can't do meta-reasoning about recursive calls
/// - No FINAL_VAR - caused syntax errors, use print("FINAL:", ...) instead
/// - Explicit code examples for common patterns
/// - Explicit "Do NOT import any modules" instruction
/// - Iteration 0 safeguard: reminds model it hasn't seen context yet
pub const GUIDED_SYSTEM_PROMPT: &str = r#"You have a Python REPL with a `context` variable containing {context_length} characters from {context_source}.

IMPORTANT: You have NOT seen the context yet. Your first step MUST be to examine it.

RULES:
1. Do NOT import any modules. Use only built-in Python (strings, lists, dicts, loops).
2. First, examine the context with print(context[:2000]) to see what you're working with.
3. Use string methods: split(), find(), count(), replace(), strip(), etc.
4. When done, print your final answer as: print("FINAL:", your_answer)

Example for finding information in a document:

Step 1 - Examine the context:
```repl
print(f"Length: {len(context)} chars")
print("First 2000 chars:")
print(context[:2000])
```

Step 2 - After seeing the content, process it:
```repl
lines = context.split('\n')
# Find lines containing keywords
matches = [line for line in lines if 'keyword' in line.lower()]
print(f"Found {len(matches)} matches")
for m in matches[:10]:
    print(m)
```

Step 3 - When you have the answer:
```repl
answer = "Based on the analysis, the answer is..."
print("FINAL:", answer)
```

REMEMBER:
- You MUST execute code first - never answer directly
- ALWAYS examine context before processing
- Use only built-in Python - no imports
- Output your final answer with print("FINAL:", answer)"#;

/// Minimal system prompt for very small models.
///
/// Single instruction at a time, very constrained.
pub const MINIMAL_SYSTEM_PROMPT: &str = r#"You have a variable `context` with text to analyze.

STEP 1: Run this code FIRST:
```repl
print(f"Length: {len(context)}")
print(context[:1000])
```

After seeing output, I will give you the next step."#;

/// Generate the full system prompt based on context (uses Full tier by default).
pub fn system_prompt_with_context(context: &Context) -> String {
    system_prompt_for_tier(PromptTier::Full, context)
}

/// Generate system prompt for a specific tier and context.
///
/// This is the main entry point for prompt generation. It selects the
/// appropriate prompt template based on the tier and fills in context details.
pub fn system_prompt_for_tier(tier: PromptTier, context: &Context) -> String {
    match tier {
        PromptTier::Full => CONTEXT_SYSTEM_PROMPT
            .replace("{context_length}", &context.length.to_string())
            .replace("{context_source}", &context.source),
        PromptTier::Guided => GUIDED_SYSTEM_PROMPT
            .replace("{context_length}", &context.length.to_string())
            .replace("{context_source}", &context.source),
        PromptTier::Minimal => MINIMAL_SYSTEM_PROMPT.to_string(),
    }
}

/// Generate system prompt for a tier without context.
pub fn system_prompt_for_tier_no_context(tier: PromptTier) -> String {
    match tier {
        PromptTier::Full | PromptTier::Guided => BASIC_SYSTEM_PROMPT.to_string(),
        PromptTier::Minimal => MINIMAL_SYSTEM_PROMPT.to_string(),
    }
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

/// Build an error prompt with task reminder for weaker models.
///
/// When Apple FM and similar models encounter errors, they often forget
/// the original task. This prompt includes a reminder of what they're
/// supposed to be doing.
pub fn error_prompt_with_reminder(
    error_message: &str,
    original_query: &str,
    context_info: Option<&str>,
) -> String {
    let context_reminder = if let Some(info) = context_info {
        format!("\nYou have a `context` variable with {}.", info)
    } else {
        String::new()
    };

    format!(
        "The code execution failed with error:\n\n```\n{}\n```\n\n\
         REMINDER: Your task is to answer: \"{}\"{}\n\n\
         Do NOT import external modules. Use only built-in Python.\n\
         Fix the error and continue working on the task.",
        error_message, original_query, context_reminder
    )
}

/// Build a continuation prompt with task reminder for weaker models.
///
/// For models that lose track of the task after seeing output.
pub fn continuation_prompt_with_reminder(
    execution_output: &str,
    original_query: &str,
) -> String {
    format!(
        "The code executed with the following output:\n\n```\n{}\n```\n\n\
         REMINDER: Your task is to answer: \"{}\"\n\n\
         Continue working on the task or provide your final answer with print(\"FINAL:\", answer)",
        execution_output, original_query
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
    use crate::context::ContextType;

    fn test_context() -> Context {
        Context {
            content: "Test content".to_string(),
            length: 12,
            source: "test.txt".to_string(),
            context_type: ContextType::File,
            file_count: None,
            files: vec![],
        }
    }

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

    #[test]
    fn test_prompt_tier_default() {
        assert_eq!(PromptTier::default(), PromptTier::Full);
    }

    #[test]
    fn test_system_prompt_for_tier_full() {
        let ctx = test_context();
        let prompt = system_prompt_for_tier(PromptTier::Full, &ctx);
        assert!(prompt.contains("llm_query"));
        assert!(prompt.contains("FINAL_VAR"));
        assert!(prompt.contains("12 characters"));
    }

    #[test]
    fn test_system_prompt_for_tier_guided() {
        let ctx = test_context();
        let prompt = system_prompt_for_tier(PromptTier::Guided, &ctx);
        // Guided prompt should NOT contain llm_query
        assert!(!prompt.contains("llm_query"));
        // Should contain key guidance
        assert!(prompt.contains("Do NOT import"));
        assert!(prompt.contains("You have NOT seen the context yet"));
        assert!(prompt.contains("12 characters"));
    }

    #[test]
    fn test_system_prompt_for_tier_minimal() {
        let ctx = test_context();
        let prompt = system_prompt_for_tier(PromptTier::Minimal, &ctx);
        // Minimal should be short and simple
        assert!(prompt.contains("STEP 1"));
        assert!(prompt.len() < 500);
    }

    #[test]
    fn test_error_prompt_with_reminder() {
        let prompt = error_prompt_with_reminder(
            "NameError: x is not defined",
            "Find all function names",
            Some("50000 characters"),
        );
        assert!(prompt.contains("NameError"));
        assert!(prompt.contains("Find all function names"));
        assert!(prompt.contains("50000 characters"));
        assert!(prompt.contains("Do NOT import"));
    }

    #[test]
    fn test_continuation_prompt_with_reminder() {
        let prompt = continuation_prompt_with_reminder("Output: 42", "Calculate the sum");
        assert!(prompt.contains("Output: 42"));
        assert!(prompt.contains("Calculate the sum"));
        assert!(prompt.contains("print(\"FINAL:\""));
    }
}
