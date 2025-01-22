# Function Calling Implementation

OpenAgents implements function calling using DeepSeek's API to enable tools and external integrations. This document outlines the implementation approach.

## Overview

Function calling allows the AI model to invoke external tools and APIs through a structured interface. The implementation follows DeepSeek's function calling specification while adding OpenAgents-specific enhancements.

## Architecture

### 1. Tool Definition

Tools are defined using JSON Schema format:

```rust
pub struct Tool {
    pub type_: String,  // Always "function"
    pub function: Function,
}

pub struct Function {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}
```

Example tool definition:

```json
{
    "type": "function",
    "function": {
        "name": "view_file",
        "description": "View file contents at path",
        "parameters": {
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path of the file to view"
                },
                "owner": {
                    "type": "string", 
                    "description": "The owner of the repository"
                },
                "repo": {
                    "type": "string",
                    "description": "The name of the repository"
                },
                "branch": {
                    "type": "string",
                    "description": "The branch to view the file from"
                }
            },
            "required": ["path", "owner", "repo", "branch"]
        }
    }
}
```

### 2. Message Flow

1. User sends message via WebSocket
2. Server processes message and includes available tools
3. DeepSeek model may choose to call functions
4. Server executes function calls and returns results
5. Model incorporates results into final response

### 3. Implementation

#### ChatHandler Integration

```rust
impl ChatHandler {
    async fn process_message(&self, msg: ChatMessage) -> Result<String> {
        let tools = self.get_available_tools();
        
        let response = self.deepseek_service
            .chat_with_tools(msg.content, tools)
            .await?;
            
        match response.choices[0].message {
            Message::FunctionCall { name, arguments } => {
                let result = self.execute_function(name, arguments).await?;
                // Send result back to model
                self.deepseek_service
                    .chat_with_tools(result, tools)
                    .await?
            }
            Message::Content { text } => text,
        }
    }
}
```

#### DeepSeek Service Integration

```rust
impl DeepSeekService {
    pub async fn chat_with_tools(
        &self,
        prompt: String,
        tools: Vec<Tool>,
    ) -> Result<ChatResponse> {
        let request = ChatRequest {
            model: "deepseek-chat",
            messages: vec![Message::user(prompt)],
            tools: Some(tools),
            tool_choice: Some("auto"),
            // Other parameters...
        };
        
        self.client
            .post(&self.chat_endpoint())
            .json(&request)
            .send()
            .await?
            .json()
            .await?
    }
}
```

### 4. Available Tools

Core tools implemented:

1. File Operations
   - `view_file` - View file contents
   - `view_hierarchy` - View file/folder structure
   - `create_file` - Create new file
   - `rewrite_file` - Update file contents
   - `delete_file` - Delete file

2. GitHub Integration  
   - `create_pull_request` - Create PR
   - `update_pull_request` - Update PR
   - `close_pull_request` - Close PR
   - `list_pull_requests` - List open PRs
   - `view_pull_request` - View PR details

3. Issue Management
   - `fetch_github_issue` - Get issue details
   - `list_open_issues` - List open issues
   - `open_issue` - Create new issue
   - `close_issue` - Close issue
   - `post_github_comment` - Comment on issue

4. Repository Management
   - `create_branch` - Create new branch
   - `fetch_commit_contents` - Get file contents from commits

5. External Services
   - `scrape_webpage` - Get webpage content
   - `search_team_knowledge` - Search knowledge base

### 5. Error Handling

The implementation includes comprehensive error handling:

```rust
#[derive(Debug)]
pub enum ToolError {
    InvalidArguments(String),
    ExecutionFailed(String),
    PermissionDenied(String),
    ResourceNotFound(String),
    RateLimitExceeded,
    NetworkError(String),
}

impl ToolExecutor {
    async fn execute_tool(&self, name: &str, args: Value) -> Result<String, ToolError> {
        // Validate arguments
        self.validate_arguments(name, &args)?;
        
        // Execute tool with timeout and retry logic
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            self.tools.get(name).unwrap().execute(args)
        ).await??;
        
        Ok(result)
    }
}
```

### 6. Security Considerations

1. Input Validation
   - Strict argument validation
   - Path traversal prevention
   - Size limits on inputs

2. Access Control
   - Repository access verification
   - Rate limiting
   - Token scope validation

3. Error Handling
   - Safe error messages
   - No sensitive data in responses
   - Proper logging

## Usage Example

```rust
// Define available tools
let tools = vec![
    Tool::new("view_file", view_file_schema()),
    Tool::new("create_pull_request", create_pr_schema()),
    // Add other tools...
];

// Process user message with tools
let response = chat_service
    .process_message_with_tools("Show me the contents of main.rs", tools)
    .await?;

// Handle tool calls in response
if let Some(tool_calls) = response.tool_calls {
    for tool_call in tool_calls {
        let result = tool_executor
            .execute_tool(&tool_call.name, tool_call.arguments)
            .await?;
        // Send result back to model...
    }
}
```

## Future Enhancements

1. Tool Categories
   - Grouping related tools
   - Conditional tool availability
   - Custom tool sets per chat

2. Advanced Features
   - Tool composition
   - Async tool execution
   - Result caching
   - Tool usage analytics

3. UI Integration
   - Tool usage indicators
   - Progress feedback
   - Error displays
   - Usage history