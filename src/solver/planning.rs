use crate::server::services::gateway::Gateway;
use anyhow::{anyhow, Result};
use futures_util::Stream;
use std::pin::Pin;

pub struct PlanningContext {
    service: crate::server::services::ollama::OllamaService,
}

impl PlanningContext {
    pub fn new(ollama_url: &str) -> Result<Self> {
        Ok(Self {
            service: crate::server::services::ollama::OllamaService::with_config(
                ollama_url,
                "codellama:latest",
            ),
        })
    }

    pub async fn generate_plan(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let prompt = format!(
            r#"You are an expert software developer. Your task is to analyze this GitHub issue and generate an implementation plan.

Issue #{}: {}

Description:
{}

Repository Map:
{}

First, think through the changes needed. Then output your solution as a JSON object in a markdown code block like this:

```json
{{
    "changes": [
        {{
            "path": "path/to/file",
            "search": "exact content to find",
            "replace": "new content",
            "reason": "why this change is needed"
        }}
    ],
    "reasoning": "Overall explanation of changes"
}}
```

The JSON object must have:
1. "changes": Array of change blocks with:
   - "path": File path
   - "search": Exact content to find
   - "replace": New content to replace it with
   - "reason": Why this change is needed
2. "reasoning": Overall explanation of changes

Rules:
- Use EXACT content matches for search
- Include enough context for unique matches
- Keep changes minimal and focused
- Preserve code style and formatting
- Empty search means new file content
"#,
            issue_number, title, description, repo_map
        );

        self.service.chat_stream(prompt, true).await
    }

    // Keep the old method for backwards compatibility during transition
    pub async fn generate_plan_sync(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"You are an expert software developer. Your task is to analyze this GitHub issue and generate an implementation plan.

Issue #{}: {}

Description:
{}

Repository Map:
{}

First, think through the changes needed. Then output your solution as a JSON object in a markdown code block like this:

```json
{{
    "changes": [
        {{
            "path": "path/to/file",
            "search": "exact content to find",
            "replace": "new content",
            "reason": "why this change is needed"
        }}
    ],
    "reasoning": "Overall explanation of changes"
}}
```

The JSON object must have:
1. "changes": Array of change blocks with:
   - "path": File path
   - "search": Exact content to find
   - "replace": New content to replace it with
   - "reason": Why this change is needed
2. "reasoning": Overall explanation of changes

Rules:
- Use EXACT content matches for search
- Include enough context for unique matches
- Keep changes minimal and focused
- Preserve code style and formatting
- Empty search means new file content
"#,
            issue_number, title, description, repo_map
        );

        let (response, _) = self.service.chat(prompt, true).await?;
        Ok(response)
    }
}