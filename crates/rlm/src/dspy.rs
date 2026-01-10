//! DSPy signatures for RLM execution.
//!
//! Replaces hardcoded prompts in `prompts.rs` with optimizable signatures.
//! Each signature corresponds to a prompt tier:
//! - `RlmQuerySignature` — Basic RLM (replaces BASIC_SYSTEM_PROMPT)
//! - `RlmContextQuerySignature` — Full RLM (replaces CONTEXT_SYSTEM_PROMPT)
//! - `RlmGuidedQuerySignature` — Guided tier (replaces GUIDED_SYSTEM_PROMPT)
//! - `RlmCodeGenerationSignature` — Code generation for REPL

use dsrs::Signature;

/// Basic RLM query signature.
///
/// For simple queries without context. Generates Python REPL code
/// to solve the problem, then provides FINAL answer.
#[Signature]
pub struct RlmQuerySignature {
    /// Execute a simple RLM query with code execution.
    /// You MUST use code to solve problems. You cannot answer directly.
    /// 1. Execute code using ```repl blocks
    /// 2. Wait for code output
    /// 3. Provide FINAL answer only after seeing output
    ///
    /// Format for code:
    /// ```repl
    /// # your code here
    /// print(result)
    /// ```
    ///
    /// Format for final answer:
    /// FINAL <your answer>

    /// The user's query to solve
    #[input]
    pub query: String,

    /// Chain of thought reasoning about how to solve the problem
    #[output]
    pub reasoning: String,

    /// Python REPL code to execute (without the ```repl wrapper)
    #[output]
    pub code: String,

    /// Whether more iterations are needed (false when FINAL is ready)
    #[output]
    pub needs_continuation: bool,
}

/// Context-aware RLM query signature.
///
/// Full RLM capabilities with llm_query() for recursive sub-queries.
/// Based on RLM paper (arXiv:2512.24601) Appendix D.
#[Signature]
pub struct RlmContextQuerySignature {
    /// RLM query with context for recursive analysis.
    /// You have access to:
    /// 1. A 'context' variable containing important information
    /// 2. An 'llm_query(prompt, context)' function for sub-queries
    /// 3. 'print()' statements to view output
    ///
    /// Strategy:
    /// 1. First examine the context with print(context[:2000])
    /// 2. Figure out a chunking strategy based on content
    /// 3. Use llm_query() per chunk to analyze
    /// 4. Aggregate results into final answer
    ///
    /// When done, use:
    /// - FINAL(answer) for direct answer
    /// - FINAL_VAR(variable_name) to return a variable
    ///
    /// Generate Python REPL code that uses llm_query() for sub-queries.

    /// The user's query to answer
    #[input]
    pub query: String,

    /// Length of the context in characters
    #[input]
    pub context_length: u64,

    /// Source description of the context (e.g., "document.pdf", "src/*.rs")
    #[input]
    pub context_source: String,

    /// Chain of thought reasoning
    #[output]
    pub reasoning: String,

    /// Python REPL code to execute (may include llm_query() calls)
    #[output]
    pub code: String,

    /// Whether more iterations are needed
    #[output]
    pub needs_continuation: bool,
}

/// Guided RLM query signature.
///
/// For models like Apple FM that need simpler prompts.
/// No llm_query() - uses direct string operations only.
#[Signature]
pub struct RlmGuidedQuerySignature {
    /// Guided RLM query for simpler models.
    /// IMPORTANT: You have NOT seen the context yet. First examine it.
    ///
    /// RULES:
    /// 1. Do NOT import any modules. Use only built-in Python.
    /// 2. First examine context with print(context[:2000])
    /// 3. Use string methods: split(), find(), count(), etc.
    /// 4. When done: print("FINAL:", your_answer)
    ///
    /// Example:
    /// ```repl
    /// print(f"Length: {len(context)} chars")
    /// print(context[:2000])
    /// ```

    /// The user's query to answer
    #[input]
    pub query: String,

    /// Length of the context in characters
    #[input]
    pub context_length: u64,

    /// Preview of the context (first ~2000 chars)
    #[input]
    pub context_preview: String,

    /// Python REPL code to execute (no imports)
    #[output]
    pub code: String,

    /// Explanation of what the code does
    #[output]
    pub explanation: String,

    /// Whether more iterations are needed
    #[output]
    pub needs_continuation: bool,
}

/// Code generation signature for REPL execution.
///
/// Generates Python code for context analysis tasks.
#[Signature]
pub struct RlmCodeGenerationSignature {
    /// Generate Python REPL code for context analysis.
    /// Use only built-in Python. No imports.
    ///
    /// Available:
    /// - 'context' variable with text content
    /// - String methods: split(), find(), count(), replace(), strip()
    /// - List comprehensions and basic loops
    ///
    /// Output code that prints results for visibility.

    /// The task to accomplish with code
    #[input]
    pub task: String,

    /// Preview of the context to analyze
    #[input]
    pub context_preview: String,

    /// Python code to execute
    #[output]
    pub code: String,

    /// Explanation of what the code does
    #[output]
    pub explanation: String,
}

/// Continuation prompt signature.
///
/// Used after code execution to continue or finalize.
#[Signature]
pub struct RlmContinuationSignature {
    /// Continue solving after seeing execution output.
    /// If you have enough information, provide FINAL answer.
    /// Otherwise, generate more code to continue analysis.

    /// The original query being solved
    #[input]
    pub original_query: String,

    /// Output from the previous code execution
    #[input]
    pub execution_output: String,

    /// Summary of work done so far
    #[input]
    pub progress_summary: String,

    /// Next action: "continue" with more code or "final" with answer
    #[output]
    pub action: String,

    /// Python code if continuing, empty if finalizing
    #[output]
    pub code: String,

    /// Final answer if action is "final"
    #[output]
    pub final_answer: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use dsrs::MetaSignature;

    #[test]
    fn test_rlm_query_signature_fields() {
        let inputs = RlmQuerySignature::input_fields();
        let outputs = RlmQuerySignature::output_fields();

        assert_eq!(inputs.len(), 1);
        assert!(inputs.iter().any(|f| f.name == "query"));

        assert_eq!(outputs.len(), 3);
        assert!(outputs.iter().any(|f| f.name == "reasoning"));
        assert!(outputs.iter().any(|f| f.name == "code"));
        assert!(outputs.iter().any(|f| f.name == "needs_continuation"));
    }

    #[test]
    fn test_rlm_context_query_signature_fields() {
        let inputs = RlmContextQuerySignature::input_fields();
        let outputs = RlmContextQuerySignature::output_fields();

        assert_eq!(inputs.len(), 3);
        assert!(inputs.iter().any(|f| f.name == "query"));
        assert!(inputs.iter().any(|f| f.name == "context_length"));
        assert!(inputs.iter().any(|f| f.name == "context_source"));

        assert_eq!(outputs.len(), 3);
    }

    #[test]
    fn test_rlm_guided_query_signature_fields() {
        let inputs = RlmGuidedQuerySignature::input_fields();

        assert_eq!(inputs.len(), 3);
        assert!(inputs.iter().any(|f| f.name == "context_preview"));
    }

    #[test]
    fn test_signatures_have_instructions() {
        // All signatures should have non-empty instructions from docstrings
        assert!(!RlmQuerySignature::instruction().is_empty());
        assert!(!RlmContextQuerySignature::instruction().is_empty());
        assert!(!RlmGuidedQuerySignature::instruction().is_empty());
        assert!(!RlmCodeGenerationSignature::instruction().is_empty());
        assert!(!RlmContinuationSignature::instruction().is_empty());
    }
}
