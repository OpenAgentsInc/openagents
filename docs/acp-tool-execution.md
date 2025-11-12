# ACP Tool Execution Architecture

This document explains how tool execution works for ACP agents (Codex and Claude Code) in OpenAgents.

## Overview

ACP agents need to execute tools (like reading files, running bash commands) to accomplish tasks. There are two approaches:

1. **Client-side tool execution** - The agent sends JSON-RPC requests to the OpenAgents client, which executes tools and returns results
2. **MCP (Model Context Protocol) tool execution** - The agent registers MCP servers that handle tools internally

OpenAgents currently uses the **MCP approach** for Claude Code.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ OpenAgents (Rust)                                           │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ ACPClient (client.rs)                                   │ │
│ │ - Spawns agent process (tsx/codex binary)              │ │
│ │ - Sends JSON-RPC over stdin/stdout                     │ │
│ │ - Advertises client capabilities (fs, terminal)        │ │
│ │ - Receives SessionUpdate notifications                 │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ ACP Protocol (JSON-RPC)
                            │
┌─────────────────────────────────────────────────────────────┐
│ Claude Code Agent (TypeScript)                              │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ ClaudeAcpAgent (acp-agent.ts)                          │ │
│ │ - Receives prompts via ACP                             │ │
│ │ - Creates MCP server with tools                        │ │
│ │ - Configures Claude Agent SDK with MCP tools           │ │
│ └─────────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ MCP Server (mcp-server.ts)                             │ │
│ │ - Registers tools: Read, Write, Edit, Bash, etc.      │ │
│ │ - Tool handlers call back to ACPClient methods         │ │
│ └─────────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ Claude Agent SDK                                        │ │
│ │ - Streams responses from Claude API                    │ │
│ │ - Invokes MCP tools when Claude requests them          │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
                            │
                            │ Tool execution callbacks
                            │
┌─────────────────────────────────────────────────────────────┐
│ OpenAgents (Rust)                                           │
│ ┌─────────────────────────────────────────────────────────┐ │
│ │ ACPClient methods                                       │ │
│ │ - readTextFile() - reads files from filesystem         │ │
│ │ - writeTextFile() - writes files                       │ │
│ │ - terminalRun() - executes bash commands               │ │
│ └─────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Client Capabilities

During the ACP `initialize` handshake, OpenAgents advertises its capabilities:

```rust
// src-tauri/src/oa_acp/client.rs:122-125
client_capabilities: acp::ClientCapabilities {
    fs: acp::FileSystemCapability {
        read_text_file: true,   // Can read files
        write_text_file: true,  // Can write files
        meta: None
    },
    terminal: true,  // Can run bash commands
    meta: None,
}
```

These capabilities tell the agent what the client can do.

## Tool Registration (Claude Code)

When a new session is created, Claude Code registers MCP tools based on client capabilities:

```typescript
// packages/claude-code-acp/acp-agent.ts:228-241
const allowedTools = [];
const disallowedTools = [];

if (this.clientCapabilities?.fs?.readTextFile) {
  allowedTools.push(toolNames.read);          // mcp__acp__Read
  disallowedTools.push("Read");                // Disable built-in Read
}

if (this.clientCapabilities?.fs?.writeTextFile) {
  allowedTools.push(toolNames.write, toolNames.edit);  // mcp__acp__Write, mcp__acp__Edit
  disallowedTools.push("Write", "Edit");               // Disable built-ins
}

if (this.clientCapabilities?.terminal) {
  allowedTools.push(toolNames.bash, toolNames.bashOutput, toolNames.killShell);
  disallowedTools.push("Bash", "BashOutput", "KillShell");
}
```

This creates 6 MCP tools:
- `mcp__acp__Read` - Read files
- `mcp__acp__Write` - Write files
- `mcp__acp__Edit` - Edit files with diffs
- `mcp__acp__Bash` - Execute bash commands
- `mcp__acp__BashOutput` - Get output from background bash
- `mcp__acp__KillShell` - Kill background bash processes

## MCP Tool Implementation

Each MCP tool is registered with a handler that calls back to the ACP client:

```typescript
// packages/claude-code-acp/mcp-server.ts:46-149
server.registerTool(
  unqualifiedToolNames.read,  // "Read"
  {
    title: "Read",
    description: "Reads files from the local filesystem...",
    inputSchema: {
      file_path: z.string(),
      offset: z.number().optional(),
      limit: z.number().optional(),
    },
  },
  async (input) => {
    // Call back to ACP client
    const content = await agent.readTextFile({
      sessionId,
      path: input.file_path,
      line: input.offset,
      limit: input.limit,
    });

    return {
      content: [{ type: "text", text: content.content }]
    };
  }
);
```

The `agent.readTextFile()` method sends a JSON-RPC request to the OpenAgents client:

```typescript
// packages/claude-code-acp/acp-agent.ts:486-492
async readTextFile(params: ReadTextFileRequest): Promise<ReadTextFileResponse> {
  const response = await this.client.readTextFile(params);
  if (!params.limit && !params.line) {
    this.fileContentCache[params.path] = response.content;
  }
  return response;
}
```

## Tool Execution Flow

1. **User sends prompt** → OpenAgents calls `session_manager.prompt()`
2. **Prompt sent to agent** → ACPClient sends JSON-RPC `session/prompt` request
3. **Agent invokes Claude API** → Claude Agent SDK streams responses
4. **Claude requests tool** → Claude API returns tool_use block
5. **MCP tool invoked** → Claude Agent SDK calls registered MCP tool handler
6. **Tool handler calls ACP client** → `agent.readTextFile()` sends JSON-RPC request
7. **OpenAgents executes tool** → ACPClient method executes and returns result
8. **Result returned to Claude** → Tool result sent back to Claude API
9. **Claude continues** → Streams more text chunks
10. **Updates broadcast** → SessionUpdate notifications sent to frontend via WebSocket

## Message Interleaving

Tool calls must be properly interleaved with text messages. The tinyvex writer handles this:

```rust
// crates/tinyvex/src/writer.rs:225-254
SU::ToolCall(tc) => {
    // Finalize any active assistant or reasoning streams before creating tool call
    // This ensures messages are segmented chronologically around tool calls
    let mut notifs = Vec::new();
    if let Some(mut fin) = self.try_finalize_stream_kind(thread_id, "assistant").await {
        notifs.append(&mut fin);
    }
    if let Some(mut fin) = self.try_finalize_stream_kind(thread_id, "reason").await {
        notifs.append(&mut fin);
    }

    // Create tool call entry
    // ...
}
```

When a tool call arrives:
1. **Finalize active text streams** - Creates a message segment with all accumulated text
2. **Create tool call entry** - Stores tool call with timestamp
3. **New text starts new segment** - Next AgentMessageChunk creates new message

This ensures the timeline looks like:
```
Message 1: "I'll read the file"
Tool Call 1: Read src/main.rs
Message 2: "The file contains..."
Tool Call 2: Edit src/main.rs
Message 3: "I've updated the code"
```

Instead of:
```
Message 1: "I'll read the file The file contains... I've updated the code"
Tool Call 1: Read src/main.rs
Tool Call 2: Edit src/main.rs
```

## Why MCP Instead of Client-Side?

Client-side tool execution would require implementing tool handlers in the Rust ACPClient:

```rust
// Would need to implement (NOT DONE):
match method {
    "fs/readTextFile" => handle_read_text_file(params),
    "fs/writeTextFile" => handle_write_text_file(params),
    "terminal/run" => handle_terminal_run(params),
    // etc...
}
```

MCP is simpler because:
1. **All logic in TypeScript** - Tool handlers are in the same language as the agent
2. **No JSON-RPC request handling** - Agent handles tools internally, just sends notifications
3. **Easier to add new tools** - Just register in MCP server
4. **Callbacks to existing methods** - MCP tools call the same ACP methods Codex uses

## Debugging

### Check tool registration

Look for this in logs when a session starts:
```
allowedTools: ["mcp__acp__Read", "mcp__acp__Write", "mcp__acp__Edit",
               "mcp__acp__Bash", "mcp__acp__BashOutput", "mcp__acp__KillShell"]
```

If tools are missing, check:
1. **Client capabilities** - Are they set to `true` in `client.rs:122-125`?
2. **Tool registration** - Are tools added to `allowedTools` in `acp-agent.ts:228-241`?

### Check tool execution

When a tool is called, you should see:
```
ToolCall(ToolCall { id: ToolCallId("toolu_..."), title: "Read file", kind: Edit, status: Pending, ... })
```

Then later:
```
ToolCallUpdate(ToolCallUpdate { id: ToolCallId("toolu_..."), fields: { status: Some(Completed), ... } })
```

If tool calls stay Pending forever:
1. **MCP tool handler failed** - Check agent stderr for errors
2. **ACP client method not responding** - Check if `readTextFile`/etc methods exist
3. **JSON-RPC parse error** - Check message format

## Future: Codex Integration

Codex (Rust agent) currently doesn't use this MCP approach. It has tools built-in and doesn't need callbacks. To add MCP support to Codex:

1. Create MCP server in Codex
2. Register tools that call local Rust functions
3. No ACP client callbacks needed (everything is local)

Or, implement client-side tool execution:
1. Add JSON-RPC request handler to ACPClient
2. Implement tool methods (read_file, write_file, run_terminal)
3. Agent sends requests instead of using MCP

## Summary

- **Client capabilities** tell agents what the client can do
- **MCP tools** are registered by Claude Code based on capabilities
- **Tool handlers** call back to ACP client methods via JSON-RPC
- **Message interleaving** is handled by finalizing streams when tool calls arrive
- **All tools work** as long as capabilities are `true` and tools are added to `allowedTools`
