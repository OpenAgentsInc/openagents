# Phase 4 Option B: Direct Rust → Convex Integration

## Overview

After reviewing the official Convex Rust SDK, **Option B is significantly simpler than Option A** (Tauri events).

The Convex Rust SDK provides:
- ✅ Official `convex` crate for Rust
- ✅ `ConvexClient` with mutation/query support
- ✅ Async/await with Tokio
- ✅ Real-time subscriptions
- ✅ Type-safe `Value` types

## Comparison: Option A vs Option B

### Option A (Current Implementation)
```
Rust ACP Agent → Emit Tauri Event → Frontend Listener → Convex Mutation
```
- ❌ Extra hop through frontend
- ❌ More complex event passing
- ❌ Frontend must be running to write data

### Option B (Using Rust SDK)
```
Rust ACP Agent → Convex Rust Client → Convex Mutation
```
- ✅ Direct write from Rust
- ✅ Simpler flow
- ✅ Works even if frontend disconnects

**Recommendation**: Option B is clearly better with the official SDK.

## Implementation Plan

### 1. Add Convex Rust Dependency

**File**: `tauri/src-tauri/Cargo.toml`

```toml
[dependencies]
convex = "0.10"
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 2. Initialize Convex Client in Rust

**File**: `tauri/src-tauri/src/convex_client.rs` (new)

```rust
use convex::{ConvexClient, Value};
use std::collections::BTreeMap;
use std::env;

pub struct ConvexClientWrapper {
    client: ConvexClient,
    user_token: Option<String>, // For auth
}

impl ConvexClientWrapper {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let deployment_url = env::var("VITE_CONVEX_URL")
            .expect("VITE_CONVEX_URL must be set");

        let client = ConvexClient::new(&deployment_url).await?;

        Ok(Self {
            client,
            user_token: None,
        })
    }

    pub fn set_auth_token(&mut self, token: String) {
        self.user_token = Some(token);
    }

    pub async fn upsert_streaming_message(
        &mut self,
        thread_id: &str,
        item_id: &str,
        role: &str,
        content: &str,
        kind: Option<&str>,
        partial: bool,
        seq: Option<i64>,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let mut args = BTreeMap::new();
        args.insert("threadId".into(), thread_id.into());
        args.insert("itemId".into(), item_id.into());
        args.insert("role".into(), role.into());
        args.insert("content".into(), content.into());

        if let Some(k) = kind {
            args.insert("kind".into(), k.into());
        }

        args.insert("partial".into(), partial.into());

        if let Some(s) = seq {
            args.insert("seq".into(), s.into());
        }

        let result = self.client
            .mutation("chat:upsertStreamingMessage", args)
            .await?;

        Ok(result)
    }

    pub async fn upsert_tool_call(
        &mut self,
        thread_id: &str,
        tool_call_id: &str,
        title: Option<&str>,
        kind: Option<&str>,
        status: Option<&str>,
        content_json: Option<&str>,
        locations_json: Option<&str>,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let mut args = BTreeMap::new();
        args.insert("threadId".into(), thread_id.into());
        args.insert("toolCallId".into(), tool_call_id.into());

        if let Some(t) = title {
            args.insert("title".into(), t.into());
        }
        if let Some(k) = kind {
            args.insert("kind".into(), k.into());
        }
        if let Some(s) = status {
            args.insert("status".into(), s.into());
        }
        if let Some(c) = content_json {
            args.insert("contentJson".into(), c.into());
        }
        if let Some(l) = locations_json {
            args.insert("locationsJson".into(), l.into());
        }

        let result = self.client
            .mutation("toolCalls:upsertToolCall", args)
            .await?;

        Ok(result)
    }

    pub async fn upsert_plan(
        &mut self,
        thread_id: &str,
        entries_json: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let mut args = BTreeMap::new();
        args.insert("threadId".into(), thread_id.into());
        args.insert("entriesJson".into(), entries_json.into());

        let result = self.client
            .mutation("planEntries:upsertPlan", args)
            .await?;

        Ok(result)
    }

    pub async fn upsert_thread_state(
        &mut self,
        thread_id: &str,
        current_mode_id: Option<&str>,
        available_commands_json: Option<&str>,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let mut args = BTreeMap::new();
        args.insert("threadId".into(), thread_id.into());

        if let Some(m) = current_mode_id {
            args.insert("currentModeId".into(), m.into());
        }
        if let Some(c) = available_commands_json {
            args.insert("availableCommandsJson".into(), c.into());
        }

        let result = self.client
            .mutation("threadState:upsertThreadState", args)
            .await?;

        Ok(result)
    }

    pub async fn finalize_message(
        &mut self,
        item_id: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let mut args = BTreeMap::new();
        args.insert("itemId".into(), item_id.into());

        let result = self.client
            .mutation("chat:finalizeMessage", args)
            .await?;

        Ok(result)
    }
}
```

### 3. Update Session Manager to Use Convex Client

**File**: `tauri/src-tauri/src/oa_acp/session_manager.rs`

```rust
use crate::convex_client::ConvexClientWrapper;

pub struct SessionManager {
    // ... existing fields
    convex_client: Option<ConvexClientWrapper>,
    use_convex: bool,
}

impl SessionManager {
    pub async fn new(
        app: AppHandle,
        tinyvex_writer: Option<TinyvexWriter>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let use_convex = std::env::var("VITE_USE_CONVEX")
            .unwrap_or_else(|_| "false".to_string()) == "true";

        let convex_client = if use_convex {
            Some(ConvexClientWrapper::new().await?)
        } else {
            None
        };

        Ok(Self {
            app,
            tinyvex_writer,
            convex_client,
            use_convex,
            // ... other fields
        })
    }

    async fn handle_session_update(&mut self, update: SessionUpdate) -> Result<()> {
        if self.use_convex {
            // Write to Convex
            self.write_to_convex(&update).await?;
        } else {
            // Write to Tinyvex (legacy)
            if let Some(writer) = &self.tinyvex_writer {
                writer.mirror_acp_update_to_tinyvex(&update)?;
            }
        }
        Ok(())
    }

    async fn write_to_convex(&mut self, update: &SessionUpdate) -> Result<()> {
        let client = self.convex_client.as_mut()
            .ok_or("Convex client not initialized")?;

        match &update.kind {
            SessionUpdateKind::AgentMessageChunk { item_id, content, .. } => {
                client.upsert_streaming_message(
                    &update.thread_id,
                    item_id,
                    "assistant",
                    content,
                    Some("message"),
                    true, // partial
                    Some(update.seq),
                ).await?;
            }

            SessionUpdateKind::AgentThoughtChunk { item_id, content, .. } => {
                client.upsert_streaming_message(
                    &update.thread_id,
                    item_id,
                    "assistant",
                    content,
                    Some("reason"), // kind = "reason" for thoughts
                    true,
                    Some(update.seq),
                ).await?;
            }

            SessionUpdateKind::MessageComplete { item_id } => {
                client.finalize_message(item_id).await?;
            }

            SessionUpdateKind::ToolCall {
                tool_call_id,
                title,
                kind,
                status,
                content,
                locations
            } => {
                let content_json = content.as_ref()
                    .map(|c| serde_json::to_string(c).ok())
                    .flatten();
                let locations_json = locations.as_ref()
                    .map(|l| serde_json::to_string(l).ok())
                    .flatten();

                client.upsert_tool_call(
                    &update.thread_id,
                    tool_call_id,
                    title.as_deref(),
                    kind.as_deref(),
                    status.as_deref(),
                    content_json.as_deref(),
                    locations_json.as_deref(),
                ).await?;
            }

            SessionUpdateKind::Plan { entries } => {
                let entries_json = serde_json::to_string(&entries)?;
                client.upsert_plan(&update.thread_id, &entries_json).await?;
            }

            SessionUpdateKind::CurrentModeUpdate { mode_id } => {
                client.upsert_thread_state(
                    &update.thread_id,
                    Some(mode_id),
                    None,
                ).await?;
            }

            // ... handle other update kinds
        }

        Ok(())
    }
}
```

### 4. Handle Authentication

The big question: **How to pass user authentication from frontend to Rust?**

**Solution**: Pass auth token via Tauri command

**Frontend** (`src/lib/tauri-acp.ts`):
```typescript
import { invoke } from "@tauri-apps/api/core";

// Get auth token from Convex
const authToken = convexClient.auth.getToken();

// Pass to Rust when creating session
await invoke("create_session", {
  authToken,
  // ... other args
});
```

**Rust** (`src-tauri/src/commands.rs`):
```rust
#[tauri::command]
pub async fn create_session(
    app: AppHandle,
    auth_token: String,
    // ... other args
) -> Result<String, String> {
    let mut session_manager = SESSION_MANAGER.lock().await;

    // Set auth token on Convex client
    if let Some(client) = &mut session_manager.convex_client {
        client.set_auth_token(auth_token);
    }

    // ... rest of session creation
}
```

**Convex Client** (updated):
```rust
impl ConvexClientWrapper {
    pub fn set_auth_token(&mut self, token: String) {
        self.user_token = Some(token);
        // TODO: Configure ConvexClient to send Authorization header
        // This may require using client.with_auth() or similar
        // Need to check if Convex Rust SDK supports custom headers
    }
}
```

**⚠️ Note**: Need to verify how the Convex Rust SDK handles authentication. The docs don't show auth headers explicitly. May need to:
1. Check SDK source code for auth support
2. Use HTTP client with custom headers if needed
3. Or use admin tokens (less secure)

### 5. Update Cargo.toml Module Structure

**File**: `tauri/src-tauri/src/lib.rs`

```rust
mod convex_client;

pub use convex_client::ConvexClientWrapper;
```

### 6. Remove Tauri Event Adapter (Option A code)

Since we're going with Option B, we can remove:
- `src/components/ConvexAcpAdapter.tsx` - No longer needed
- Tauri event emission code

Keep:
- `src/hooks/useConvexThreads.ts` - Still used for frontend queries
- `src/hooks/useConvexMessages.ts` - Still used for frontend queries
- `src/lib/feature-flags.ts` - Still useful

## Benefits of Option B

✅ **Direct writes** - No frontend intermediary
✅ **Simpler flow** - One less hop
✅ **Works offline** - Rust can write even if frontend disconnects
✅ **Type safety** - Rust type system + Convex Value types
✅ **Better performance** - Fewer hops, less serialization
✅ **Official SDK** - Maintained by Convex team

## Authentication Challenge

The main challenge is authentication. Need to research:

1. **How does Convex Rust SDK handle auth?**
   - Custom headers?
   - Token in client config?
   - Admin tokens?

2. **How to pass user token from frontend to Rust?**
   - Via Tauri command (shown above)
   - Store in shared state
   - Refresh token handling

3. **Security considerations**
   - Don't log tokens
   - Secure token passing
   - Token expiration/refresh

## Migration Path

1. ✅ Add `convex` crate to Cargo.toml
2. ✅ Create `ConvexClientWrapper` helper
3. ⚠️ Research authentication in Convex Rust SDK
4. ✅ Update `SessionManager` to use Convex client
5. ✅ Pass auth token from frontend
6. ✅ Test with feature flag
7. ✅ Remove Option A code (Tauri events)

## Testing Checklist

- [ ] Messages stream correctly to Convex
- [ ] Tool calls are tracked
- [ ] Plans update properly
- [ ] Thread state changes work
- [ ] Multiple devices see updates in real-time
- [ ] Auth token is passed securely
- [ ] Token refresh works
- [ ] Offline/online transitions handle gracefully

## Next Steps

1. Research Convex Rust SDK authentication
2. Implement auth token passing
3. Create Rust Convex client wrapper
4. Update session manager
5. Test end-to-end
6. Remove Option A code

## Questions to Answer

- Does Convex Rust SDK support auth tokens?
- How are authorization headers set?
- Is there a `client.with_auth(token)` method?
- Can we use admin tokens as fallback?
- How do we handle token refresh in Rust?

## Conclusion

**Option B with the official Rust SDK is significantly better than Option A with Tauri events.** The implementation is more direct, simpler, and leverages official tooling.

The main work is figuring out authentication, which may require diving into the Convex Rust SDK source code or reaching out to Convex support.
