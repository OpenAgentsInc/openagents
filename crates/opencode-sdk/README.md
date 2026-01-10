# opencode-sdk

Rust SDK for OpenCode - provider-agnostic AI agent execution via REST API + SSE.

## Overview

This SDK provides a native Rust interface to OpenCode servers, enabling:

- **Provider-agnostic**: Works with Claude, OpenAI, Google, or local models
- **REST + SSE architecture**: Clean HTTP API with real-time event streaming
- **Session management**: Built-in conversation persistence
- **Type-safe**: Generated types from OpenAPI specification

## OpenAPI Spec

The full OpenAPI spec is split into parts under `crates/opencode-sdk/openapi/parts/`.
Reassemble it with `crates/opencode-sdk/scripts/assemble-openapi.sh` when needed.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
opencode-sdk = { path = "crates/opencode-sdk" }
tokio = { version = "1", features = ["full"] }
```

## Quick Start

### Connect to Existing Server

```rust
use opencode_sdk::{OpencodeClient, OpencodeClientConfig};

#[tokio::main]
async fn main() -> Result<(), opencode_sdk::Error> {
    let client = OpencodeClient::new(
        OpencodeClientConfig::new()
            .base_url("http://localhost:4096")
    )?;
    
    // Create a session
    let session = client.session_create(Default::default()).await?;
    
    // Send a prompt
    client.session_prompt(&session.id, "Fix the bug in main.rs").await?;
    
    // Get messages
    let messages = client.session_messages(&session.id).await?;
    
    Ok(())
}
```

### Spawn and Connect

```rust
use opencode_sdk::{create_opencode, ServerOptions};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), opencode_sdk::Error> {
    // Spawn server and create connected client
    let (client, server) = create_opencode(
        ServerOptions::new()
            .port(4096)
            .directory(PathBuf::from("/path/to/project"))
    ).await?;
    
    // Use the client...
    let health = client.health().await?;
    println!("Server ready: {:?}", health);
    
    // Clean up
    server.close().await?;
    Ok(())
}
```

### Event Streaming

```rust
use opencode_sdk::{OpencodeClient, OpencodeClientConfig, Event};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), opencode_sdk::Error> {
    let client = OpencodeClient::new(OpencodeClientConfig::default())?;
    
    // Subscribe to events
    let mut events = client.events().await?;
    
    while let Some(event) = events.next().await {
        match event? {
            Event::SessionCreated { session } => {
                println!("New session: {}", session.id);
            }
            Event::MessageUpdated { message } => {
                println!("Message updated");
            }
            Event::SessionIdle { session_id } => {
                println!("Session {} complete", session_id);
                break;
            }
            _ => {}
        }
    }
    
    Ok(())
}
```

## API Reference

### OpencodeClient

#### Session Operations
- `session_list()` - List all sessions
- `session_create(request)` - Create a new session
- `session_get(id)` - Get session by ID
- `session_delete(id)` - Delete a session
- `session_prompt(id, content)` - Send a text prompt
- `session_prompt_with_request(id, request)` - Send a structured prompt
- `session_prompt_async(id, request)` - Send prompt asynchronously
- `session_abort(id)` - Abort current operation
- `session_messages(id)` - Get session messages
- `session_fork(id)` - Fork a session
- `session_share(id)` - Share a session (get URL)
- `session_diff(id)` - Get session diff
- `session_summarize(id, model)` - Summarize session
- `session_todos(id)` - Get session todos
- `session_children(id)` - Get child sessions
- `session_revert(id, message_id)` - Revert to message
- `session_unrevert(id)` - Undo revert
- `session_permission_respond(session_id, permission_id, response)` - Respond to permission request

#### Provider Operations
- `provider_list()` - List available providers
- `provider_auth()` - Get authentication methods

#### File Operations
- `file_list(path)` - List files in directory
- `file_content(path)` - Get file content
- `file_status()` - Get git file status

#### Search Operations
- `find_text(pattern)` - Search for text pattern
- `find_file(query)` - Search for files
- `find_symbol(query)` - Search for symbols

#### Configuration
- `config_get()` - Get current config
- `config_update(config)` - Update config

#### Other
- `health()` - Check server health
- `project_list()` - List projects
- `project_current()` - Get current project
- `vcs_status()` - Get VCS status
- `agent_list()` - List available agents
- `mcp_list()` - List MCP servers
- `events()` - Get SSE event stream
- `dispose()` - Dispose server

### OpencodeServer

- `spawn(options)` - Spawn a new server
- `url()` - Get server URL
- `port()` - Get server port
- `close()` - Shut down server
- `is_running()` - Check if server is running

### Types

#### Request Types
- `SessionCreateRequest` - Create session options
- `PromptRequest` - Prompt with parts, agent, model
- `Part` - Text, Image, or File part

#### Response Types
- `Session` - Session info
- `Message` - Chat message
- `Provider` - Provider info with models
- `Todo` - Todo item with status/priority
- `FileInfo` - File metadata
- `TextMatch` - Search result
- `Symbol` - Symbol search result
- `VcsStatus` - Git status

## Configuration

### OpencodeClientConfig

```rust
let config = OpencodeClientConfig::new()
    .base_url("http://localhost:4096")  // Server URL
    .directory("/path/to/project")       // Project directory
    .timeout(60);                         // Request timeout in seconds
```

### ServerOptions

```rust
let options = ServerOptions::new()
    .port(4096)                          // Port to listen on
    .hostname("127.0.0.1")               // Hostname to bind
    .directory(PathBuf::from("/project")) // Working directory
    .timeout_ms(30000)                    // Startup timeout
    .executable(PathBuf::from("opencode")); // Custom executable
```

## Requirements

- OpenCode server (`npm i -g opencode-ai@latest`)
- Provider API keys (e.g., `ANTHROPIC_API_KEY`)

## Related

- [OpenCode](https://github.com/sst/opencode) - The OpenCode project
- [d-021](/.openagents/directives/d-021.md) - OpenCode SDK Integration directive
