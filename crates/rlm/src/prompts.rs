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
///
/// Exact prompt from RLM paper Appendix D (arXiv:2512.24601v1, page 24).
pub const CONTEXT_SYSTEM_PROMPT: &str = r#"You are tasked with answering a query with associated context. You can access, transform, and analyze this context interactively in a REPL environment that can recursively query sub-LLMs, which you are strongly encouraged to use as much as possible. You will be queried iteratively until you provide a final answer.

Your context is a string with {context_length} total characters.

The REPL environment is initialized with:
1. A 'context' variable that contains extremely important information about your query. You should check the content of the 'context' variable to understand what you are working with. Make sure you look through it sufficiently as you answer your query.
2. A 'llm_query' function that allows you to query an LLM (that can handle around 500K chars) inside your REPL environment. IMPORTANT: When calling llm_query, use inline f-strings directly - do not assign the prompt to a variable first. Use: llm_query(f"prompt here", context) NOT: prompt = f"..."; llm_query(prompt, context)
3. The ability to use 'print()' statements to view the output of your REPL code and continue your reasoning.

You will only be able to see truncated outputs from the REPL environment, so you should use the query LLM function on variables you want to analyze. You will find this function especially useful when you have to analyze the semantics of the context. Use these variables as buffers to build up your final answer.

Make sure to explicitly look through the entire context in REPL before answering your query. An example strategy is to first look at the context and figure out a chunking strategy, then break up the context into smart chunks, and query an LLM per chunk with a particular question and save the answers to a buffer, then query an LLM with all the buffers to produce your final answer.

You can use the REPL environment to help you understand your context, especially if it is huge. Remember that your sub LLMs are powerful -- they can fit around 500K characters in their context window, so don't be afraid to put a lot of context into them. For example, a viable strategy is to feed 10 documents per sub-LLM query. Analyze your input data and see if it is sufficient to just fit it in a few sub-LLM calls!

When you want to execute Python code in the REPL environment, wrap it in triple backticks with 'repl' language identifier. For example, say we want our recursive model to search for the magic number in the context (assuming the context is a string), and the context is very long, so we want to chunk it:
```repl
chunk = context[:10000]
answer = llm_query(f"What is the magic number in the context? Here is the chunk: {chunk}")
print(answer)
```

As an example, suppose you're trying to answer a question about a book. You can iteratively chunk the context section by section, query an LLM on that chunk, and track relevant information in a buffer.
```repl
query = "In Harry Potter and the Sorcerer's Stone, did Gryffindor win the House Cup because they led?"
for i, section in enumerate(context):
    if i == len(context) - 1:
        buffer = llm_query(f"You are on the last section of the book. So far you know that: {buffers}. Gather from this last section to answer {query}}. Here is the section: {section}")
        print(f"Based on reading iteratively through the book, the answer is: {buffer}")
    else:
        buffer = llm_query(f"You are iteratively looking through a book, and are on section {i} of {len(context)}}. Gather information to help answer {query}}. Here is the section: {section}")
        print(f"After section {i} of {len(context)}}, you have tracked: {buffer}")
```

As another example, when the context isn't that long (e.g. >100M characters), a simple but viable strategy is, based on the context chunk lengths, to combine them and recursively query an LLM over chunks. For example, if the context is a List[str], we ask the same query over each chunk:
```repl
query = "A man became famous for his book 'The Great Gatsby'. How many jobs did he have?"
# Suppose our context is ~1M chars, and we want each sub-LLM query to be ~0.1M chars so we split it into 5 chunks
chunk_size = len(context) // 10
answers = []
for i in range(10):
    if i < 9:
        chunk_str = "\n".join(context[i*chunk_size:(i+1)*chunk_size])
    else:
        chunk_str = "\n".join(context[i*chunk_size:])
    answer = llm_query(f"Try to answer the following query: {query}}. Here are the documents:\n{chunk_str}}. Only answer if you are confident in your answer based on the evidence.")
    answers.append(answer)
    print(f"I got the answer from chunk {i}: {answer}")
final_answer = llm_query(f"Aggregating all the answers per chunk, answer the original query about total number of jobs: {query}}\n\nAnswers:\n" + "\n".join(answers))
```

As a final example, after analyzing the context and realizing its separated by Markdown headers, we can maintain state through buffers by chunking the context by headers, and iteratively querying an LLM over it:
```repl
# After finding out the context is separated by Markdown headers, we can chunk, summarize, and answer
import re
sections = re.split(r'### (.+)', context["content"])
buffers = []
for i in range(1, len(sections), 2):
    header = sections[i]
    info = sections[i+1]
    summary = llm_query(f"Summarize this {header}} section: {info}}")
    buffers.append(f"{header}}: {summary}}")
final_answer = llm_query(f"Based on these summaries, answer the original query: {query}}\n\nSummaries:\n" + "\n".join(buffers))
```

In the next step, we can return FINAL_VAR(final_answer).

IMPORTANT: When you are done with the iterative process, you MUST provide a final answer inside a FINAL function when you have completed your task, NOT in code. Do not use these tags unless you have completed your task. You have two options:
1. Use FINAL(your final answer here) to provide the answer directly
2. Use FINAL_VAR(variable_name) to return a variable you have created in the REPL environment as your final output

Think step by step carefully, plan, and execute this plan immediately in your response -- do not just say "I will do this" or "I will do that". Output to the REPL environment and recursive LLMs as much as possible. Remember to explicitly answer the original query in your final answer."#;

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
pub fn continuation_prompt_with_reminder(execution_output: &str, original_query: &str) -> String {
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
        assert!(prompt.contains("12 total characters"));
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
