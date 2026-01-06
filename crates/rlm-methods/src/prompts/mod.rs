//! Prompt templates for different methods.

/// Base method prompt template.
pub const BASE_PROMPT: &str = r#"You are a helpful assistant. Answer the following question based on the provided context.

Context:
{context}

Question: {query}

Please provide a direct and concise answer."#;

/// Summary agent prompt for summarizing a chunk.
pub const SUMMARY_CHUNK_PROMPT: &str = r#"Summarize the following text, preserving key facts and information that might be relevant for answering questions:

{text}

Summary:"#;

/// Summary agent prompt for combining summaries.
pub const SUMMARY_COMBINE_PROMPT: &str = r#"Combine the following summaries into a single coherent summary, preserving all key facts:

{summaries}

Combined Summary:"#;

/// Summary agent prompt for final answer.
pub const SUMMARY_ANSWER_PROMPT: &str = r#"Based on the following summarized context, answer the question.

Context Summary:
{summary}

Question: {query}

Answer:"#;

/// CodeAct search action prompt.
pub const CODEACT_PROMPT: &str = r#"You are a helpful assistant that can search and execute code to answer questions.

Available actions:
- search(query): Search the context for relevant information
- execute(code): Execute Python code
- final(answer): Provide the final answer

Context (use search to find relevant parts):
{context}

Question: {query}

Think step by step. Use actions to gather information, then provide your final answer.
Format: ACTION: action_name(argument)

Begin:"#;

/// RLM system prompt.
pub const RLM_SYSTEM_PROMPT: &str = r#"You are an AI assistant with access to a Python REPL environment.
You can execute Python code to help answer questions.
You also have access to llm_query(prompt) to ask sub-questions.

Available tools:
- Python code execution (just write Python code)
- llm_query(prompt): Ask a sub-question to another LLM instance

When you have the final answer, output: FINAL_ANSWER: <your answer>

Context:
{context}

Question: {query}"#;

/// RLM prompt without sub-calls (ablation).
pub const RLM_NO_SUBCALLS_PROMPT: &str = r#"You are an AI assistant with access to a Python REPL environment.
You can execute Python code to help answer questions.

When you have the final answer, output: FINAL_ANSWER: <your answer>

Context:
{context}

Question: {query}"#;

/// Extract answer from LLM response.
pub fn extract_final_answer(response: &str) -> Option<String> {
    // Look for FINAL_ANSWER: pattern
    if let Some(idx) = response.find("FINAL_ANSWER:") {
        let answer_start = idx + "FINAL_ANSWER:".len();
        let answer = response[answer_start..].trim();
        // Take until end of line or end of response
        let answer = answer.lines().next().unwrap_or(answer);
        return Some(answer.trim().to_string());
    }

    // Look for Answer: pattern
    if let Some(idx) = response.find("Answer:") {
        let answer_start = idx + "Answer:".len();
        let answer = response[answer_start..].trim();
        let answer = answer.lines().next().unwrap_or(answer);
        return Some(answer.trim().to_string());
    }

    None
}

/// Format a prompt with placeholders.
pub fn format_prompt(template: &str, context: &str, query: &str) -> String {
    template
        .replace("{context}", context)
        .replace("{query}", query)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_final_answer() {
        let response = "Let me think...\nFINAL_ANSWER: 42\nSome more text";
        assert_eq!(extract_final_answer(response), Some("42".to_string()));

        let response = "The answer is Answer: hello world";
        assert_eq!(
            extract_final_answer(response),
            Some("hello world".to_string())
        );

        let response = "No answer here";
        assert_eq!(extract_final_answer(response), None);
    }

    #[test]
    fn test_format_prompt() {
        let prompt = format_prompt(BASE_PROMPT, "Some context", "What is 2+2?");
        assert!(prompt.contains("Some context"));
        assert!(prompt.contains("What is 2+2?"));
    }
}
