//! System prompts for the RLM engine.
//!
//! Adapted from rig-rlm's PREAMBLE with modifications for our use case.

/// System prompt that instructs the LLM how to use the REPL environment.
pub const SYSTEM_PROMPT: &str = r#"You are an AI that MUST use code to solve problems. You cannot answer directly.

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
