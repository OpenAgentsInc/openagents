# Does codex-acp use codex app-server?

## Answer: NO

**`codex-acp` does NOT use `codex app-server`**. It uses the Codex Rust libraries directly.

## Architecture

Looking at the source code (`/Users/christopherdavid/code/codex-acp/`):

1. **Dependencies** (from `Cargo.toml`):
   - `codex-core` - Core Codex functionality
   - `codex-protocol` - Codex protocol types
   - `codex-common` - Common Codex utilities
   - `codex-mcp-server` - MCP server support
   - **NO dependency on `codex-app-server-protocol` or any app-server code**

2. **Implementation** (from `codex_agent.rs`):
   - Uses `ThreadManager` from `codex-core` directly
   - Calls `thread_manager.start_thread(config)` to create threads
   - Uses `Thread` struct to manage sessions
   - **No spawning of `codex app-server` process**

3. **Connection**:
   - `codex-acp` uses Codex libraries to connect directly to Codex API
   - It's a Rust library integration, not a process wrapper

## Current Architecture in Autopilot

We're running **TWO separate connections to Codex**:

```
┌─────────────────┐
│   autopilot     │
│   (our app)     │
└────────┬─────────┘
         │
         ├──────────────────┐
         │                  │
         ▼                  ▼
┌─────────────────┐  ┌──────────────┐
│ codex app-server│  │  codex-acp   │
│  (CLI process)  │  │  (Rust libs) │
└────────┬─────────┘  └──────┬───────┘
         │                   │
         └─────────┬─────────┘
                   │
                   ▼
            ┌──────────────┐
            │  Codex API   │
            └──────────────┘
```

**Two independent connections:**
1. `codex app-server` - CLI process that speaks JSON-RPC
2. `codex-acp` - Uses Codex Rust libraries directly, speaks ACP

## Implications

**We have TWO separate connections to Codex:**
- Both authenticate separately
- Both create separate sessions/threads
- Both consume API quota independently
- Events from one don't appear in the other

This explains why:
- ACP events are minimal (only `session/update`) - it's a different session
- We need to mirror requests to both (they're separate sessions)
- We're not seeing all events in ACP (different connection, different session)

## How to Verify

1. **Check codex-acp source code** - Look for where it spawns `codex` or `codex app-server`
2. **Monitor processes** - When we run `codex-acp`, does it spawn a `codex app-server` process?
3. **Check logs** - Does `codex-acp` stderr show it connecting to Codex?

## Recommendation

If `codex-acp` uses `codex app-server` internally, we should:
- **Option 1**: Use ONLY `codex-acp` (let it manage app-server)
  - Simpler architecture
  - Single connection
  - Need to add extensions for missing events
  
- **Option 2**: Keep dual protocol (current)
  - Get all events from app-server
  - Get standardized events from ACP
  - More complex but more complete

## Next Steps

1. Check if `codex-acp` spawns `codex app-server` when we run it
2. Look at `codex-acp` source to see how it connects to Codex
3. Decide: single protocol (ACP only) or dual protocol (both)
