# Adding New Tools and CLI Demos

This guide explains how to add new tools to the OpenAgents system and create CLI demos for them, using the GitHub issue tool as an example.

## Overview

The process involves:

1. Creating a service module for the tool
2. Adding the service to the module system
3. Extending the CLI to demonstrate the tool
4. Testing the implementation

## Step-by-Step Guide

### 1. Create the Service Module

First, create a new service file in `src/server/services/`. For example, `github_issue.rs`:

```rust
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct GitHubService {
    client: Client,
    token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GitHubIssue {
    pub number: i32,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub html_url: String,
}

impl GitHubService {
    pub fn new(token: Option<String>) -> Self {
        Self {
            client: Client::new(),
            token,
        }
    }

    pub async fn get_issue(&self, owner: &str, repo: &str, issue_number: i32) -> Result<GitHubIssue> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/issues/{}",
            owner, repo, issue_number
        );

        let mut request = self.client.get(&url);

        if let Some(token) = &self.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        request = request.header("User-Agent", "OpenAgents");

        let response = request.send().await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "GitHub API request failed: {}",
                response.status()
            ));
        }

        let issue = response.json::<GitHubIssue>().await?;
        Ok(issue)
    }
}
```

### 2. Add Service to Module System

Update `src/server/services/mod.rs` to expose the new service:

```rust
pub mod github_issue;
pub use github_issue::GitHubService as GitHubIssueService;
```

### 3. Extend the CLI

Add a new command to `src/bin/deepseek-cli.rs`:

1. Add the command to the Commands enum:

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands ...

    /// GitHub issue tool example
    Issue {
        /// The issue number to fetch
        issue_number: i32,
        /// The repository owner
        #[arg(long, default_value = "OpenAgentsInc")]
        owner: String,
        /// The repository name
        #[arg(long, default_value = "openagents")]
        repo: String,
        /// Enable debug output
        #[arg(long)]
        debug: bool,
    },
}
```

2. Create the tool definition:

```rust
let get_issue_tool = DeepSeekService::create_tool(
    "get_github_issue".to_string(),
    Some("Get a GitHub issue by number".to_string()),
    json!({
        "type": "object",
        "properties": {
            "owner": {
                "type": "string",
                "description": "The owner of the repository"
            },
            "repo": {
                "type": "string",
                "description": "The name of the repository"
            },
            "issue_number": {
                "type": "integer",
                "description": "The issue number"
            }
        },
        "required": ["owner", "repo", "issue_number"]
    }),
);
```

3. Implement the command handler:

```rust
Commands::Issue {
    issue_number,
    owner,
    repo,
    debug,
} => {
    // Create GitHub issue tool
    let get_issue_tool = DeepSeekService::create_tool(/* ... */);

    // Make initial request with tool
    print_colored("Fetching GitHub issue...\n", Color::Blue)?;

    // Initial user message
    let user_message = ChatMessage {
        role: "user".to_string(),
        content: format!("What's in GitHub issue #{} in {}/{}?", issue_number, owner, repo),
        tool_call_id: None,
        tool_calls: None,
    };

    // Get initial response with tool call
    let (content, _, tool_calls) = service
        .chat_with_tools(
            format!("What's in GitHub issue #{} in {}/{}?", issue_number, owner, repo),
            vec![get_issue_tool.clone()],
            None,
            false,
        )
        .await?;

    // Handle tool calls and responses
    if let Some(tool_calls) = tool_calls {
        // ... handle tool calls ...
    }
}
```

### 4. Testing the Implementation

1. Build and run:

```bash
cargo run --bin deepseek-cli issue 584 --debug
```

2. Required environment variables:

```bash
export DEEPSEEK_API_KEY="your-api-key"
export GITHUB_TOKEN="your-github-token"  # Optional
```

## Tool Implementation Pattern

The general pattern for implementing tools is:

1. **Service Layer**

   - Create a service struct with required dependencies
   - Implement core functionality (API calls, data processing, etc.)
   - Define data structures for input/output

2. **Module Integration**

   - Add module to services/mod.rs
   - Export necessary types and functions

3. **CLI Demo**

   - Add new command to Commands enum
   - Create tool definition with JSON schema
   - Implement command handler with:
     - Initial message
     - Tool call handling
     - Response processing
     - Debug output options

4. **Testing**
   - Manual testing via CLI
   - Add unit tests if needed
   - Document required environment variables

## Best Practices

1. **Error Handling**

   - Use anyhow::Result for error propagation
   - Provide clear error messages
   - Handle API errors gracefully

2. **Configuration**

   - Use environment variables for sensitive data
   - Provide sensible defaults where appropriate
   - Document all configuration options

3. **Documentation**

   - Document the service API
   - Include usage examples
   - List all dependencies and requirements

4. **Debug Output**
   - Add --debug flag support
   - Log important operations
   - Show tool definitions and calls

## Example Usage

```bash
# Basic usage
cargo run --bin deepseek-cli issue 584

# With debug output
cargo run --bin deepseek-cli issue 584 --debug

# Custom repository
cargo run --bin deepseek-cli issue 123 --owner username --repo reponame
```

## Next Steps

After implementing a tool:

1. Add unit tests
2. Add integration tests
3. Update documentation
4. Consider adding the tool to the web interface
5. Add any necessary configuration options
6. Consider error cases and edge conditions
