# Convex Migration Guide

## Phase 3 Status: Foundation Complete ✅

The Convex client integration foundation is complete. This document outlines what's done and what remains for full migration from Tinyvex to Convex.

## Completed in Phase 3

### 1. Dependencies & Setup
- ✅ Added `convex` and `@convex-dev/auth` packages
- ✅ Created `src/lib/convexClient.ts` singleton
- ✅ Wrapped app with `ConvexProvider` in `App.tsx`
- ✅ Created `.env.example` for configuration

### 2. Example Integration
- ✅ Created `ConvexProjectsList.tsx` demonstrating:
  - `useQuery` for reactive data fetching
  - `useMutation` for data modifications
  - Proper TypeScript integration with generated types
  - Real-time updates via Convex subscriptions

### 3. Backend API
- ✅ Full CRUD operations for projects
- ✅ Thread/message streaming support
- ✅ Tool calls, plans, state, and events

## Remaining Work for Full Migration

### Phase 4: Runtime Integration (Major Refactor)

The `useAcpRuntime.tsx` file needs significant refactoring to use Convex instead of Tinyvex WebSocket.

#### Current Architecture
```typescript
useSharedTinyvexWebSocket()
  └─> WebSocket connection to desktop Tauri backend
      └─> Queries SQLite via WebSocket control messages
          └─> Receives real-time updates via WS notifications
```

#### Target Architecture
```typescript
useQuery(api.chat.getMessages, { threadId })
  └─> Convex reactive query
      └─> Automatically re-renders on data changes
```

#### Files Requiring Changes

**1. `src/runtime/useAcpRuntime.tsx`** (~1000 lines)
- Replace `useSharedTinyvexWebSocket()` with Convex hooks
- Replace WebSocket queries with:
  - `useQuery(api.chat.getMessages, { threadId })`
  - `useQuery(api.chat.getThreads)`
  - `useQuery(api.toolCalls.listToolCalls, { threadId })`
  - `useQuery(api.planEntries.getPlan, { threadId })`
  - `useQuery(api.threadState.getThreadState, { threadId })`
- Replace manual state management with Convex reactivity
- Remove WebSocket subscription/unsubscription logic
- Remove replay buffer and manual deduplication (Convex handles this)

**2. `src/lib/tinyvexWebSocketSingleton.ts`**
- Delete file (no longer needed)
- Convex client handles connection management

**3. `src/lib/tauri-acp.ts`**
- Keep ACP session management (desktop still runs agents)
- Modify to write ACP events to Convex instead of Tinyvex
- Options:
  - **Option A**: Emit Tauri events, frontend writes to Convex
  - **Option B**: Call Convex HTTP API from Rust directly

**4. Component Updates**
- `src/components/assistant-ui/thread-list.tsx`: Use `useQuery(api.chat.getThreads)`
- `src/components/assistant-ui/thread.tsx`: Use `useQuery(api.chat.getMessages)`
- `src/components/nav-projects-assistant.tsx`: Use `useQuery(api.projects.listProjects)`

### Phase 5: Authentication

Add Convex Auth integration for user management.

**Files to Create:**
- `src/components/auth/SignIn.tsx`
- `src/components/auth/SignUp.tsx`
- `src/lib/useConvexAuth.ts`

**Files to Modify:**
- `src/App.tsx`: Wrap with `ConvexAuthProvider`
- All components: Check auth state before rendering

**Backend:**
- Already configured in `convex/auth.ts`
- Password provider ready to use

### Phase 6: Cleanup

**Delete Tinyvex Code:**
- `crates/tinyvex/` - Entire Rust crate
- `tauri/src-tauri/src/tinyvex_*.rs` - WebSocket server files
- Remove from `tauri/src-tauri/Cargo.toml`
- Remove mDNS discovery code

**Update Rust Backend:**
- `tauri/src-tauri/src/oa_acp/session_manager.rs`
  - Replace Tinyvex Writer with Convex HTTP API calls
  - Or emit events to frontend for Convex writes

## Usage Example

### Before (Tinyvex WebSocket)
```typescript
const ws = useSharedTinyvexWebSocket();

useEffect(() => {
  ws.send({
    control: "tvx.query",
    name: "threads.list",
    args: { limit: 10 }
  });

  const unsub = ws.subscribe((msg) => {
    if (msg.type === "tinyvex.query_result") {
      setThreads(msg.rows);
    }
  });

  return unsub;
}, []);
```

### After (Convex)
```typescript
const threads = useQuery(api.chat.getThreads);
// That's it! Auto-updates when data changes.
```

### Mutations

```typescript
const createThread = useMutation(api.chat.createThreadExtended);
const addMessage = useMutation(api.chat.upsertStreamingMessage);

await createThread({
  title: "New Chat",
  source: "claude-code",
  workingDirectory: "/path/to/project"
});

await addMessage({
  threadId,
  itemId: "msg-123",
  role: "user",
  content: "Hello!",
});
```

## Benefits After Full Migration

1. **Simpler Code**: Remove ~2000 lines of WebSocket/SQLite code
2. **Real-time by Default**: No manual subscription management
3. **Type Safety**: Generated TypeScript types for all queries/mutations
4. **Multi-device Sync**: Works automatically across all devices
5. **Offline Support**: Convex handles queuing and sync
6. **No Desktop Dependency**: Mobile/web apps work standalone

## Migration Strategy

1. **Incremental Approach** (Recommended)
   - Migrate one component at a time
   - Keep Tinyvex running in parallel during transition
   - Test thoroughly before removing old code

2. **Feature Flagging**
   - Add `USE_CONVEX` flag to switch between backends
   - Allow testing both paths

3. **Data Migration**
   - No automatic migration (user decision from Phase 1)
   - Users start with clean Convex database
   - Historical Tinyvex data remains in SQLite (read-only)

## Testing Checklist

- [ ] Authentication flow (sign up, sign in, sign out)
- [ ] Create/read/update/delete threads
- [ ] Send messages and receive streaming responses
- [ ] Tool calls display correctly
- [ ] Plans and state updates work
- [ ] Multi-device sync (desktop + mobile)
- [ ] Offline mode and reconnection
- [ ] Performance with large message histories

## Environment Setup

1. Copy `.env.example` to `.env.local`:
   ```bash
   cp .env.example .env.local
   ```

2. Get your Convex URL from https://dashboard.convex.dev

3. Add to `.env.local`:
   ```
   VITE_CONVEX_URL=https://your-deployment.convex.cloud
   ```

4. The Convex backend is already deployed and ready to use!

## Next Steps

1. Decide on migration approach (incremental vs. big bang)
2. Start with thread list component (simpler than full runtime)
3. Migrate message rendering next
4. Then migrate ACP integration
5. Add authentication
6. Remove Tinyvex code

## Questions?

See the main migration issue: #1488
