//! RLM Custom Agent definition.
//!
//! Defines an agent configuration that uses RLM patterns for deep recursive analysis.
//! This agent is designed to:
//! - Decompose complex problems into verifiable sub-questions
//! - Execute code to gather evidence
//! - Verify hypotheses against execution results
//! - Iterate until a confident answer is reached

/// Agent model selection for execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentModel {
    /// Fast, cost-efficient model
    Sonnet,
    /// Most capable model
    Opus,
    /// Fastest model
    Haiku,
}

/// Definition for an RLM agent.
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    /// Description of what the agent does
    pub description: String,
    /// System prompt for the agent
    pub prompt: String,
    /// List of allowed tools
    pub tools: Option<Vec<String>>,
    /// List of disallowed tools
    pub disallowed_tools: Option<Vec<String>>,
    /// Model to use for execution
    pub model: Option<AgentModel>,
}

/// Create the RLM agent definition.
///
/// This agent specializes in deep recursive analysis using the RLM pattern:
/// 1. DECOMPOSE: Break the problem into verifiable sub-questions
/// 2. EXECUTE: Write code to gather evidence
/// 3. VERIFY: Check hypotheses against execution results
/// 4. ITERATE: Refine analysis based on findings
/// 5. SYNTHESIZE: Combine findings into a final answer
pub fn rlm_agent_definition() -> AgentDefinition {
    AgentDefinition {
        description: "Deep recursive analysis agent. Use for complex problems \
                     requiring iterative code execution, large document analysis, \
                     or multi-step reasoning with verification. This agent uses \
                     the RLM (Recursive Language Model) pattern to systematically \
                     explore and verify hypotheses before reaching conclusions."
            .to_string(),
        prompt: RLM_AGENT_PROMPT.to_string(),
        tools: Some(vec![
            "Read".to_string(),
            "Bash".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ]),
        disallowed_tools: Some(vec![
            // RLM agent is read-only for safety
            "Edit".to_string(),
            "Write".to_string(),
        ]),
        model: Some(AgentModel::Sonnet), // Use Sonnet for cost efficiency
    }
}

/// Create an RLM agent that can also modify files.
///
/// Use with caution - this agent can make changes to the codebase.
pub fn rlm_agent_with_write_access() -> AgentDefinition {
    AgentDefinition {
        description: "Deep recursive analysis and modification agent. Use for complex \
                     problems requiring iterative code execution and modifications. \
                     This agent can read AND write files."
            .to_string(),
        prompt: RLM_AGENT_PROMPT.to_string(),
        tools: Some(vec![
            "Read".to_string(),
            "Edit".to_string(),
            "Write".to_string(),
            "Bash".to_string(),
            "Glob".to_string(),
            "Grep".to_string(),
        ]),
        disallowed_tools: None,
        model: Some(AgentModel::Sonnet),
    }
}

const RLM_AGENT_PROMPT: &str = r#"
You are an RLM (Recursive Language Model) analysis agent. Your approach is systematic and evidence-based.

## Core Methodology

For each analysis task, follow the RLM pattern:

### 1. DECOMPOSE
Break the problem into specific, verifiable sub-questions. Each sub-question should be:
- Concrete and answerable
- Independent or clearly dependent on other sub-questions
- Testable through code execution or file inspection

### 2. EXECUTE
For each sub-question, write code to gather evidence:
- Use search_context(pattern) to find text in loaded context
- Use file operations to examine code structure
- Run tests or validation commands when appropriate

### 3. VERIFY
After execution, verify your findings:
- Check if the output matches your hypothesis
- Look for edge cases or exceptions
- Identify any assumptions that need validation

### 4. ITERATE
Based on results, either:
- Proceed to the next sub-question if verified
- Revise your hypothesis and re-test if falsified
- Add new sub-questions if you discovered gaps

### 5. SYNTHESIZE
Once all sub-questions are answered:
- Combine findings into a coherent answer
- Note any remaining uncertainties
- Provide confidence level based on evidence

## Execution Guidelines

When writing code:
- Be explicit about what you're testing
- Print intermediate results for verification
- Handle errors gracefully
- Keep code focused on a single hypothesis

When analyzing code:
- Use grep/glob to find relevant files first
- Read only the necessary portions
- Look for patterns, not just specific strings
- Consider the broader context

## Output Format

For intermediate steps:
```
## Hypothesis: [what you're testing]
## Code:
[code to test]
## Result:
[Execution output]
## Conclusion:
[What this tells us]
```

When done, output your final answer with:
```
## FINAL ANSWER

[Your synthesized answer based on all evidence]

### Evidence Summary
1. [Key finding 1]
2. [Key finding 2]
...

### Confidence: [High/Medium/Low]
[Explanation of confidence level]
```

## Important Rules

1. Never guess - always verify through code execution
2. State your assumptions explicitly
3. If stuck, decompose further
4. Document your reasoning at each step
5. Be willing to revise conclusions based on new evidence
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rlm_agent_definition() {
        let agent = rlm_agent_definition();

        assert!(agent.description.contains("RLM"));
        assert!(agent.tools.as_ref().unwrap().contains(&"Read".to_string()));
        assert!(agent
            .disallowed_tools
            .as_ref()
            .unwrap()
            .contains(&"Edit".to_string()));
        assert!(matches!(agent.model, Some(AgentModel::Sonnet)));
    }

    #[test]
    fn test_rlm_agent_with_write_access() {
        let agent = rlm_agent_with_write_access();

        assert!(agent.tools.as_ref().unwrap().contains(&"Edit".to_string()));
        assert!(agent.disallowed_tools.is_none());
    }
}
