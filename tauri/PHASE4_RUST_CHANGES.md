# Phase 4: Rust Backend Changes for Convex Integration

## Overview

Phase 4 (frontend) is complete. This document outlines the **required Rust backend changes** to complete the Convex migration.

## What's Done (Frontend)

✅ Created Convex React hooks:
- `useConvexThreads` - Thread CRUD operations
- `useConvexMessages` - Message streaming and finalization
- `useConvexToolCalls` - Tool call tracking
- `useConvexPlan` - Plan state management
- `useConvexThreadState` - Mode and commands

✅ Added feature flag system:
- `VITE_USE_CONVEX` environment variable
- Granular flags for each feature
- Backwards compatible with Tinyvex

✅ Created `ConvexAcpAdapter`:
- Listens for Tauri events from Rust backend
- Writes ACP events to Convex mutations
- Handles messages, tool calls, plans, state, event log

## What's Needed (Rust Backend)

The Rust backend in `tauri/src-tauri/src/oa_acp/session_manager.rs` currently writes directly to Tinyvex. It needs to be updated to emit Tauri events instead.

### File: `tauri/src-tauri/src/oa_acp/session_manager.rs`

#### Current Flow
```rust
// Current implementation
SessionUpdate received from ACP agent
    ↓
tinyvex_writer.mirror_acp_update_to_tinyvex()
    ↓
SQLite write via Tinyvex
    ↓
WebSocket broadcast to clients
```

#### Target Flow
```rust
// New implementation
SessionUpdate received from ACP agent
    ↓
Emit Tauri event (acp:message, acp:tool_call, etc.)
    ↓
Frontend ConvexAcpAdapter receives event
    ↓
Convex mutation writes to cloud
    ↓
Reactive Convex subscriptions update all clients
```

### Events to Emit

The `ConvexAcpAdapter` is already listening for these events:

**1. Message Events**
```rust
// Emit when receiving AgentMessageChunk or AgentThoughtChunk
app.emit("acp:message", {
    threadId: String,
    itemId: String,
    role: "user" | "assistant" | "system",
    content: String,
    kind: "message" | "reason",  // "reason" for thoughts
    partial: bool,  // true while streaming, false when complete
    seq: u64,  // sequence number
})
```

**2. Message Finalization**
```rust
// Emit when message streaming is complete
app.emit("acp:message:finalize", {
    itemId: String,
})
```

**3. Tool Call Events**
```rust
// Emit when receiving ToolCall update
app.emit("acp:tool_call", {
    threadId: String,
    toolCallId: String,
    title: Option<String>,
    kind: Option<String>,  // "bash", "read", "write", etc.
    status: Option<String>,  // "pending", "running", "completed", "failed"
    contentJson: Option<String>,
    locationsJson: Option<String>,
})
```

**4. Plan Events**
```rust
// Emit when receiving Plan update
app.emit("acp:plan", {
    threadId: String,
    entriesJson: String,  // JSON array of plan entries
})
```

**5. State Events**
```rust
// Emit when receiving CurrentModeUpdate or commands change
app.emit("acp:state", {
    threadId: String,
    currentModeId: Option<String>,
    availableCommandsJson: Option<String>,
})
```

**6. Event Log (Optional)**
```rust
// Emit for audit/replay purposes
app.emit("acp:event", {
    sessionId: Option<String>,
    clientThreadDocId: Option<String>,
    threadId: Option<String>,
    updateKind: Option<String>,
    payload: String,  // JSON serialized full event
})
```

### Implementation Steps

**1. Add Feature Flag Check in Rust**
```rust
// Check if Convex is enabled (read from env or config)
let use_convex = std::env::var("VITE_USE_CONVEX")
    .unwrap_or_else(|_| "false".to_string()) == "true";

if use_convex {
    // Emit Tauri event
    emit_acp_event(&app, &update)?;
} else {
    // Use existing Tinyvex writer
    tinyvex_writer.mirror_acp_update_to_tinyvex(&update)?;
}
```

**2. Create Event Emission Helper**
```rust
fn emit_acp_event(
    app: &AppHandle,
    update: &SessionUpdate,
) -> Result<()> {
    match &update.kind {
        SessionUpdateKind::AgentMessageChunk { item_id, content, .. } => {
            app.emit("acp:message", json!({
                "threadId": update.thread_id,
                "itemId": item_id,
                "role": "assistant",
                "content": content,
                "kind": "message",
                "partial": true,
                "seq": update.seq,
            }))?;
        }
        SessionUpdateKind::AgentThoughtChunk { item_id, content, .. } => {
            app.emit("acp:message", json!({
                "threadId": update.thread_id,
                "itemId": item_id,
                "role": "assistant",
                "content": content,
                "kind": "reason",
                "partial": true,
                "seq": update.seq,
            }))?;
        }
        SessionUpdateKind::MessageComplete { item_id } => {
            app.emit("acp:message:finalize", json!({
                "itemId": item_id,
            }))?;
        }
        SessionUpdateKind::ToolCall { tool_call_id, title, kind, status, content, locations } => {
            app.emit("acp:tool_call", json!({
                "threadId": update.thread_id,
                "toolCallId": tool_call_id,
                "title": title,
                "kind": kind,
                "status": status,
                "contentJson": content.map(|c| serde_json::to_string(&c).ok()).flatten(),
                "locationsJson": locations.map(|l| serde_json::to_string(&l).ok()).flatten(),
            }))?;
        }
        SessionUpdateKind::Plan { entries } => {
            app.emit("acp:plan", json!({
                "threadId": update.thread_id,
                "entriesJson": serde_json::to_string(&entries)?,
            }))?;
        }
        SessionUpdateKind::CurrentModeUpdate { mode_id } => {
            app.emit("acp:state", json!({
                "threadId": update.thread_id,
                "currentModeId": mode_id,
                "availableCommandsJson": null,
            }))?;
        }
        // ... handle other update kinds
    }
    Ok(())
}
```

**3. Update Session Manager Initialization**
```rust
// In session_manager.rs initialization
pub fn new(
    app: AppHandle,
    tinyvex_writer: Option<TinyvexWriter>,  // Make optional
    use_convex: bool,  // Add flag
) -> Self {
    Self {
        app,
        tinyvex_writer,
        use_convex,
        // ... other fields
    }
}
```

**4. Modify ACP Update Handler**
```rust
// In handle_session_update() or equivalent
async fn handle_session_update(&self, update: SessionUpdate) -> Result<()> {
    if self.use_convex {
        // Emit event to frontend
        self.emit_acp_event(&update)?;
    } else {
        // Write to Tinyvex (legacy)
        if let Some(writer) = &self.tinyvex_writer {
            writer.mirror_acp_update_to_tinyvex(&update)?;
        }
    }
    Ok(())
}
```

### Testing Checklist

After implementing Rust changes:

1. **Start Tauri app with Convex enabled**
   ```bash
   # In .env.local
   VITE_USE_CONVEX=true
   VITE_CONVEX_URL=https://your-deployment.convex.cloud

   bun run dev
   ```

2. **Test message streaming**
   - Send a prompt to Claude Code or Codex
   - Verify messages appear in Convex database
   - Check browser console for `[ConvexAcpAdapter]` logs

3. **Test tool calls**
   - Run a command that uses tools (e.g., bash, read, write)
   - Verify tool calls are saved to Convex
   - Check status updates work

4. **Test plans and state**
   - Use plan mode
   - Verify plan entries are saved
   - Check mode transitions work

5. **Test multi-device sync**
   - Open app on two devices
   - Send message on one device
   - Verify it appears instantly on the other

6. **Test offline/online**
   - Disconnect from network
   - Send messages (should queue)
   - Reconnect
   - Verify messages sync

### Backwards Compatibility

The implementation maintains backwards compatibility:

- If `VITE_USE_CONVEX=false` or unset → Use Tinyvex (current behavior)
- If `VITE_USE_CONVEX=true` → Use Convex (new behavior)
- No breaking changes for existing users

### Performance Considerations

**Tinyvex (Current)**:
- Write: ~1ms (local SQLite)
- Read: ~1ms (local query)
- Latency: Effectively 0ms (local)

**Convex (New)**:
- Write: ~50-100ms (cloud round-trip)
- Read: ~10-20ms (cached query)
- Latency: Network dependent

**Mitigation**:
- Use optimistic updates in UI
- Batch rapid updates where possible
- Convex handles deduplication automatically

### Error Handling

Add proper error handling for Tauri event emission:

```rust
match self.emit_acp_event(&update) {
    Ok(_) => {
        log::debug!("Emitted ACP event for thread {}", update.thread_id);
    }
    Err(e) => {
        log::error!("Failed to emit ACP event: {}", e);
        // Optionally fall back to Tinyvex
        if let Some(writer) = &self.tinyvex_writer {
            writer.mirror_acp_update_to_tinyvex(&update)?;
        }
    }
}
```

### Next Steps After Rust Changes

Once Rust backend emits events:

1. Test end-to-end flow with real ACP agents
2. Verify all update types are handled correctly
3. Test error cases and reconnection
4. Performance testing with large message histories
5. Phase 5: Add authentication
6. Phase 6: Remove Tinyvex code entirely

## Files to Modify

**Primary**:
- `tauri/src-tauri/src/oa_acp/session_manager.rs` - Main changes

**Secondary** (if needed):
- `tauri/src-tauri/src/main.rs` - Add Convex flag to initialization
- `tauri/src-tauri/src/lib.rs` - Export event emission helpers
- `tauri/src-tauri/Cargo.toml` - Add serde_json if not already present

## Questions?

See the main migration issue: #1488
See `CONVEX_MIGRATION.md` for frontend migration details
