# Delegate Tool (`delegate.run`)

## Purpose

Foundation Models delegation tool for routing coding tasks to specialized agents (Codex or Claude Code). Enables on-device Apple Intelligence to act as an orchestrator, deciding when to delegate specific tasks to more powerful coding agents.

## Name

`delegate.run`

## Overview

The `delegate.run` tool creates a hierarchical agent architecture:
- **Foundation Models (FM)** runs on-device and handles conversation
- **Specialized agents** (Codex/Claude Code) handle actual code work
- FM decides when delegation is appropriate and routes tasks accordingly

## Implementation

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift:144-219`

### Tool Definition

```swift
struct FMTool_DelegateRun: Tool {
    let name = "delegate.run"
    let description = "Route a coding task to specialized agents (Codex or Claude Code) for execution. Use this for any file operations, code analysis, refactoring, or workspace exploration."

    @Generable
    struct Arguments {
        @Guide(description: "The specific coding task to perform")
        var user_prompt: String

        @Guide(description: "Which agent to use: 'codex' or 'claude_code' (default: codex)")
        var provider: String?

        @Guide(description: "Optional brief description of the delegation")
        var description: String?
    }

    typealias Output = String
}
```

## Arguments

- **`user_prompt`** (string, required)
  - The exact instruction to pass to the delegated agent
  - Examples: "list files in my repo", "refactor the auth module"

- **`provider`** (string, optional, default: "codex")
  - Which agent to delegate to
  - Allowed values: `"codex"`, `"claude_code"`, `"claude-code"`

- **`description`** (string, optional)
  - Human-readable one-liner shown in UI
  - Currently not used in display (UI infers from tool call)

## Result

Returns a confirmation string: `"✓ Delegated to {provider}. Task: {user_prompt}"`

The actual work happens asynchronously:
1. Tool emits an ACP `tool_call` update to the UI
2. Session mode switches to the delegated provider
3. Provider's responses stream back in the same conversation

## Delegation Flow

1. **User sends request**: "tell codex to list files in my repo"

2. **FM invokes tool**:
   ```swift
   delegate.run(
       user_prompt: "list files in my repo",
       provider: "codex"
   )
   ```

3. **Tool execution**:
   - Emits `toolCall` update with name "codex.run"
   - Switches session mode from `default_mode` to `codex`
   - Sends user prompt to Codex provider in same session
   - Returns acknowledgment to FM

4. **Codex responds**: Results stream back as `agentMessageChunk` updates

5. **Mode restoration**: After Codex completes, mode switches back to `default_mode`

6. **UI rendering**:
   - FM's acknowledgment: Normal assistant bubble (tagged with `_meta.source = "fm_orchestrator"`)
   - Codex's response: Distinct card with "Codex" header and accent border

7. **Next user message**: Routes to FM (the orchestrator), not to Codex

## Message Tagging

### Foundation Models Messages

All FM responses include `_meta` to identify them as orchestrator messages:

```swift
let chunk = ACP.Client.ContentChunk(
    content: .text(.init(text: response.content)),
    _meta: [
        "source": AnyEncodable("fm_orchestrator"),
        "provider": AnyEncodable("foundation_models")
    ]
)
```

**Location**: `OpenAgentsLocalProvider.swift:132-140`

### Delegated Agent Messages

Messages from Codex/Claude Code have no special `_meta` (or could add `_meta.source = "delegated_agent"`).

The absence of `fm_orchestrator` tag triggers the UI to render them in delegated agent cards.

## UI Integration

### Rendering Logic

**Location**: `ios/OpenAgents/Views/macOS/ChatAreaView.swift:112-135`

```swift
case .agentMessageChunk(let chunk):
    let text = extractText(from: chunk)
    let isFromOrchestrator = checkIsFromOrchestrator(chunk)

    if isFromOrchestrator {
        // Foundation Models orchestrator response - render normally
        bubble(text: text, isUser: false)
    } else {
        // Delegated agent response - render in distinct card
        DelegatedAgentCard(text: text, provider: inferProvider(from: note))
    }
```

### Delegated Agent Card

**Location**: `ChatAreaView.swift:323-364`

Visual design:
- **Header**: Agent name ("Codex" or "Claude Code") with arrow icon
- **Content**: Agent's response with markdown rendering
- **Background**: `OATheme.Colors.bgTertiary.opacity(0.5)`
- **Border**: `OATheme.Colors.accent.opacity(0.2)` (1px)
- **Padding**: 12px

### Provider Inference

When rendering a delegated agent card, the UI infers which agent sent the message by searching backwards through the transcript for:

1. Most recent `toolCall` with name "codex.run" or "claude_code.run"
2. Most recent `currentModeUpdate` with mode `codex` or `claude_code`

**Location**: `ChatAreaView.swift:298-320`

## ACP Protocol

### Tool Call Update

```json
{
  "sessionUpdate": "tool_call",
  "tool_call": {
    "call_id": "uuid",
    "name": "codex.run",
    "arguments": {
      "user_prompt": "list files in my repo",
      "provider": "codex"
    }
  }
}
```

### FM Orchestrator Response

```json
{
  "sessionUpdate": "agent_message_chunk",
  "content": {
    "type": "text",
    "text": "✓ Delegated to codex. Task: list files in my repo",
    "_meta": {
      "source": "fm_orchestrator",
      "provider": "foundation_models"
    }
  }
}
```

### Delegated Agent Response

```json
{
  "sessionUpdate": "agent_message_chunk",
  "content": {
    "type": "text",
    "text": "Found 42 files in /Users/.../repo:\n- src/main.swift\n- src/utils.swift\n..."
  }
}
```
(No `_meta` or different `_meta` indicating it's from the delegated agent)

## FM Instructions

Foundation Models is instructed when to use delegation via its system instructions:

```
You are OpenAgents, a helpful assistant that can delegate coding tasks to specialized agents.

You can respond conversationally to questions about yourself and your capabilities.

When the user wants to work with code/files (list, read, write, search, analyze, refactor, etc.),
use the delegate.run tool to route the task to Codex or Claude Code.

For general conversation, introductions, or capability questions, respond conversationally.
```

**Location**: `OpenAgentsLocalProvider.swift:96-105`

## Usage Examples

### Delegation to Codex
```
User: tell codex to list files in my repo
[FM bubble] ✓ Delegated to codex. Task: list files in my repo
[Codex card]
  → Codex
  Found 42 files in /Users/.../repo:
  - src/main.swift
  - src/utils.swift
  - tests/MainTests.swift
  ...
```

### Delegation to Claude Code
```
User: ask claude code to refactor the auth module
[FM bubble] ✓ Delegated to claude_code. Task: refactor the auth module
[Claude Code card]
  → Claude Code
  I'll refactor the auth module for better separation of concerns.
  Let me start by reading the current implementation...
```

### Conversational (no delegation)
```
User: what can you do?
[FM bubble] I'm OpenAgents, an assistant that helps with coding tasks.
            I can delegate work to specialized agents like Codex and
            Claude Code. Just tell me what you'd like to work on.
```

## Safety & Guardrails

- **Workspace constraints**: All file operations are limited to the configured workspace
- **Session isolation**: Each delegation happens in the same session for conversation continuity
- **Mode switching**: Session mode changes to the delegated provider during execution, then automatically restores to `default_mode` (orchestrator) when delegation completes
- **Cancellation**: Users can cancel at any time via UI; cancellation propagates to the provider

## Benefits

1. **On-device orchestration**: Foundation Models runs entirely on-device (Apple Intelligence)
2. **Clear visual hierarchy**: User immediately sees which agent is responding via card design
3. **Conversation continuity**: All responses in same session, proper context flow
4. **Flexible routing**: FM can choose appropriate agent based on task type
5. **Privacy**: Orchestration happens on-device; only delegated tasks go to external agents

## Limitations

- **Single session**: All delegation happens in the current session (no parallel sessions yet)
- **No confirmation**: Delegation happens immediately without user approval (future: add confirmation UI)
- **Simple arguments**: Current implementation only supports user_prompt, provider, and description
- **No workspace scoping**: Delegated agents use global workspace setting (future: per-delegation workspace)

## Future Enhancements

- [ ] Multi-agent workflows (chain multiple delegations)
- [ ] Delegation confirmation UI (approve before delegating)
- [ ] Per-delegation workspace and file scoping
- [ ] Delegation history and analytics
- [ ] Error handling and retry logic for failed delegations
- [ ] Streaming progress updates from delegated agents
- [ ] Parallel delegations to multiple agents

## Related Files

- **Tool Implementation**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/OpenAgentsLocalProvider.swift`
- **UI Rendering**: `ios/OpenAgents/Views/macOS/ChatAreaView.swift`
- **ACP Protocol**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/client.swift`
- **Session Management**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Local.swift`
- **Tool Call Rendering**: `ios/OpenAgents/ACP/Renderers/ToolCallView.swift`

## See Also

- [Foundation Models Tool Calling](./apple/tool-calling.md)
- [ACP ADR-0002](../adr/0002-agent-client-protocol.md)
- [Agent Registry Architecture](../../ios/OpenAgentsCore/Sources/OpenAgentsCore/Agents/README.md)
