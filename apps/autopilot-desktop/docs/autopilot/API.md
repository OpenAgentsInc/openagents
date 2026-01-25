# API Reference

## Tauri Commands

All commands use camelCase parameter names (Tauri automatically converts from Rust snake_case).

### `connect_unified_agent`

Connects an agent to a workspace.

**Parameters**:
```typescript
{
  agentIdStr: "codex" | "claude_code" | "cursor",
  workspacePath: string,
  workspaceId: string
}
```

**Returns**:
```typescript
{
  success: boolean,
  sessionId: string,  // Temporary workspace ID, actual comes from SessionStarted event
  agentId: string,
  workspaceId: string
}
```

**Example**:
```typescript
const result = await invoke("connect_unified_agent", {
  agentIdStr: "codex",
  workspacePath: "/path/to/workspace",
  workspaceId: "workspace-123"
});
```

**Note**: The returned `sessionId` is temporary. The actual ACP session ID will be emitted via the `SessionStarted` event.

---

### `start_unified_session`

Starts a new ACP session.

**Parameters**:
```typescript
{
  sessionId: string,  // Temporary session ID from connect_unified_agent
  workspacePath: string
}
```

**Returns**:
```typescript
{
  success: boolean,
  sessionId: string  // Actual ACP session ID (from session/new response)
}
```

**Example**:
```typescript
await invoke("start_unified_session", {
  sessionId: result.sessionId,
  workspacePath: "/path/to/workspace"
});
```

**Note**: The actual ACP session ID will also be emitted via the `SessionStarted` unified event.

---

### `send_unified_message`

Sends a message to an agent session.

**Parameters**:
```typescript
{
  sessionId: string,  // Actual ACP session ID (from SessionStarted event)
  text: string
}
```

**Returns**:
```typescript
{
  success: boolean,
  sessionId: string
}
```

**Example**:
```typescript
await invoke("send_unified_message", {
  sessionId: unifiedSessionId,  // From SessionStarted event
  text: "Hello, Codex!"
});
```

---

### `disconnect_unified_agent`

Disconnects an agent session.

**Parameters**:
```typescript
{
  sessionId: string
}
```

**Returns**:
```typescript
{
  success: boolean,
  sessionId: string
}
```

**Example**:
```typescript
await invoke("disconnect_unified_agent", {
  sessionId: unifiedSessionId
});
```

---

### `get_unified_conversation_items`

Gets conversation items for a session.

**Parameters**:
```typescript
{
  sessionId: string
}
```

**Returns**:
```typescript
{
  success: boolean,
  sessionId: string,
  items: UnifiedConversationItem[]
}
```

**Example**:
```typescript
const result = await invoke("get_unified_conversation_items", {
  sessionId: unifiedSessionId
});
```

---

## Tauri Events

### `unified-event`

Emitted when a unified event occurs.

**Event Payload**:
```typescript
UnifiedEvent
```

**UnifiedEvent Types**:

#### `MessageChunk`
```typescript
{
  type: "MessageChunk",
  session_id: string,
  content: string,
  is_complete: boolean
}
```

#### `ThoughtChunk`
```typescript
{
  type: "ThoughtChunk",
  session_id: string,
  content: string,
  is_complete: boolean
}
```

#### `ToolCall`
```typescript
{
  type: "ToolCall",
  session_id: string,
  tool_id: string,
  tool_name: string,
  arguments: Record<string, unknown>
}
```

#### `ToolCallUpdate`
```typescript
{
  type: "ToolCallUpdate",
  session_id: string,
  tool_id: string,
  output: string,
  is_complete: boolean
}
```

#### `SessionStarted`
```typescript
{
  type: "SessionStarted",
  session_id: string,  // Actual ACP session ID
  agent_id: "codex" | "claude_code" | "cursor"
}
```

#### `SessionCompleted`
```typescript
{
  type: "SessionCompleted",
  session_id: string,
  stop_reason: string
}
```

#### `TokenUsage`
```typescript
{
  type: "TokenUsage",
  session_id: string,
  input_tokens: number,
  output_tokens: number,
  total_tokens: number
}
```

#### `RateLimitUpdate`
```typescript
{
  type: "RateLimitUpdate",
  agent_id: "codex" | "claude_code" | "cursor",
  used_percent: number,
  resets_at?: number
}
```

**Example**:
```typescript
import { listen } from "@tauri-apps/api/event";

const unlisten = await listen<UnifiedEvent>("unified-event", (event) => {
  const unifiedEvent = event.payload;
  
  switch (unifiedEvent.type) {
    case "MessageChunk":
      console.log("Message:", unifiedEvent.content);
      break;
    case "SessionStarted":
      console.log("Session started:", unifiedEvent.session_id);
      setUnifiedSessionId(unifiedEvent.session_id);
      break;
    // ... handle other event types
  }
});
```

---

## TypeScript Types

### `UnifiedEvent`

```typescript
export type UnifiedEvent =
  | {
      type: "MessageChunk";
      session_id: string;
      content: string;
      is_complete: boolean;
    }
  | {
      type: "ThoughtChunk";
      session_id: string;
      content: string;
      is_complete: boolean;
    }
  | {
      type: "ToolCall";
      session_id: string;
      tool_id: string;
      tool_name: string;
      arguments: Record<string, unknown>;
    }
  | {
      type: "ToolCallUpdate";
      session_id: string;
      tool_id: string;
      output: string;
      is_complete: boolean;
    }
  | {
      type: "SessionStarted";
      session_id: string;
      agent_id: "codex" | "claude_code" | "cursor";
    }
  | {
      type: "SessionCompleted";
      session_id: string;
      stop_reason: string;
    }
  | {
      type: "TokenUsage";
      session_id: string;
      input_tokens: number;
      output_tokens: number;
      total_tokens: number;
    }
  | {
      type: "RateLimitUpdate";
      agent_id: "codex" | "claude_code" | "cursor";
      used_percent: number;
      resets_at?: number;
    };
```

### `ConversationItem`

```typescript
export type ConversationItem =
  | {
      id: string;
      kind: "message";
      role: "user" | "assistant";
      text: string;
    }
  | {
      id: string;
      kind: "reasoning";
      summary: string;
      content: string;
    }
  | {
      id: string;
      kind: "tool";
      toolType: string;
      title: string;
      detail: string;
      status?: string;
      output?: string;
      durationMs?: number | null;
      changes?: { path: string; kind?: string; diff?: string }[];
    }
  | {
      id: string;
      kind: "diff";
      title: string;
      diff: string;
      status?: string;
    }
  | {
      id: string;
      kind: "review";
      state: "started" | "completed";
      text: string;
    };
```

---

## Usage Example

Complete example of connecting, starting a session, sending a message, and listening for events:

```typescript
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UnifiedEvent } from "./types";

// 1. Connect agent
const connectResult = await invoke("connect_unified_agent", {
  agentIdStr: "codex",
  workspacePath: "/path/to/workspace",
  workspaceId: "workspace-123"
});

// 2. Start session
await invoke("start_unified_session", {
  sessionId: connectResult.sessionId,
  workspacePath: "/path/to/workspace"
});

// 3. Listen for events
let actualSessionId: string | null = null;

const unlisten = await listen<UnifiedEvent>("unified-event", (event) => {
  const unifiedEvent = event.payload;
  
  if (unifiedEvent.type === "SessionStarted") {
    actualSessionId = unifiedEvent.session_id;
    console.log("Session started:", actualSessionId);
  } else if (unifiedEvent.type === "MessageChunk") {
    console.log("Message chunk:", unifiedEvent.content);
  } else if (unifiedEvent.type === "SessionCompleted") {
    console.log("Session completed");
  }
});

// 4. Wait for actual session ID
while (!actualSessionId) {
  await new Promise(resolve => setTimeout(resolve, 100));
}

// 5. Send message
await invoke("send_unified_message", {
  sessionId: actualSessionId,
  text: "Hello, Codex!"
});

// 6. Cleanup
unlisten();
```

---

## Error Handling

All commands may throw errors. Handle them appropriately:

```typescript
try {
  const result = await invoke("connect_unified_agent", {
    agentIdStr: "codex",
    workspacePath: "/path/to/workspace",
    workspaceId: "workspace-123"
  });
} catch (error) {
  console.error("Failed to connect agent:", error);
  // Show error to user
}
```

Common errors:
- `codex-acp` not found: Attempts auto-download, may fail
- Connection timeout: Agent process failed to start
- Invalid session ID: Session expired or invalid
- Network errors: API connection issues
