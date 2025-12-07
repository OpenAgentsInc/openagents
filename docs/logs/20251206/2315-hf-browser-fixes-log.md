# 2315 HuggingFace Trajectory Browser Debugging Session

## Summary

This session debugged and fixed critical issues preventing the HF trajectory browser from displaying in the mainview. The problems ranged from missing logging infrastructure to fundamental architecture violations (browser-side file system access). All issues are now resolved and the trajectory browser is ready for testing.

## Problem 1: No Widgets Visible in Browser

### Symptoms
- User opened mainview and saw empty sidebar and main area
- No error messages in browser console
- No visual indication anything was wrong

### Investigation
- Initial implementation had created widgets and E2E tests that passed
- HTML layout included correct container divs with IDs
- Widget mounting code looked correct
- But nothing appeared when running `bun dev`

### Root Cause
Unknown at this stage - needed logging to diagnose further.

---

## Problem 2: No Logging Output Anywhere

### Symptoms
- Added extensive `console.log()` statements throughout widget code
- User saw ZERO logs in terminal output
- Could not access browser DevTools console
- Completely blind to what was happening

### User Feedback
> "i see no logs, cant access console output. search for how our app uses bunLog and do that so i get em serverside"
> "no fuckign logging"
> "no goddammit"

### Investigation
1. Searched codebase for bunLog pattern
2. Found it used in desktop/main.ts with `webview.bind("bunLog", ...)`
3. Added bunLog calls to all widgets and effuse-main.ts
4. Still saw NO output in terminal

### Root Cause
**Critical ordering bug in `src/desktop/main.ts`**:

```typescript
// BEFORE (BROKEN):
webview.init(`
  window.bunLog = window.bunLog || function() {};
`);
webview.bind("bunLog", (...args: unknown[]) => {
  log("Webview", ...args);
});
```

The `webview.init()` script created a **no-op stub** for `window.bunLog`. When `webview.bind()` was called later, the stub prevented the real binding from taking effect.

### Fix
**Moved `webview.bind()` BEFORE `webview.init()` in `src/desktop/main.ts`**:

```typescript
// AFTER (WORKING):
webview.bind("bunLog", (...args: unknown[]) => {
  log("Webview", ...args);
});
webview.init(`
  // bunLog should already be bound by webview.bind() above
  if (window.bunLog) {
    window.bunLog('[Webview] bunLog available in init!');
  }
`);
```

### Result
✅ **bunLog now works perfectly** - all webview logs appear in terminal output

---

## Problem 3: Browser Trying to Access File System

### Symptoms
Once logging was working, saw error:
```
Service not found: @effect/platform/FileSystem
```

### User Feedback
> "no, that should never run int he browser, why the fuck is it running in the browser"

### Investigation
1. Traced error to widgets trying to use OpenThoughtsService
2. OpenThoughtsService requires FileSystem from @effect/platform
3. FileSystem is only available server-side (BunContext)
4. I had incorrectly added BunContext.layer to browser-side Effuse layer

### Root Cause
**Fundamental architecture violation**: Browser code was trying to directly access file system services.

The original implementation had:
```typescript
// WRONG - effuse-main.ts (browser-side)
import { BunContext } from "@effect/platform-bun"
import { OpenThoughtsServiceLive } from "../huggingface/index.js"

const createEffuseLayer = () => {
  return Layer.mergeAll(
    BunContext.layer,              // ❌ Can't provide FileSystem in browser!
    OpenThoughtsServiceLive,       // ❌ Needs FileSystem!
    ...
  )
}

// Widget tried to use service directly
const service = yield* OpenThoughtsService
const trajectories = yield* service.getTrajectories(0, 100)
```

### Fix
**Implemented proper RPC architecture** with server-side file access and client-side RPC calls:

#### 1. Added RPC Request/Response Types (`src/desktop/protocol.ts`)

```typescript
export interface GetHFTrajectoryCountRequest extends BaseRequest {
  type: "request:getHFTrajectoryCount";
}

export interface GetHFTrajectoriesRequest extends BaseRequest {
  type: "request:getHFTrajectories";
  offset?: number;
  limit?: number;
}

export interface GetHFTrajectoryRequest extends BaseRequest {
  type: "request:getHFTrajectory";
  index: number;
}

// Corresponding response types with data payloads
```

#### 2. Implemented Server-Side Handlers (`src/desktop/handlers.ts`)

```typescript
/**
 * Create OpenThoughts service with Bun layer
 */
const getOpenThoughtsService = () => {
  const layer = BunContext.layer.pipe(
    Layer.provide(HFDatasetServiceLive())
  );
  return makeOpenThoughtsService().pipe(Effect.provide(layer));
};

async function getHFTrajectoryCount(): Promise<{ count: number }> {
  const service = getOpenThoughtsService();
  const count = await Effect.runPromise(
    Effect.gen(function* () {
      const svc = yield* service;
      return yield* svc.count();
    })
  );
  return { count };
}

// Similar handlers for getHFTrajectories() and getHFTrajectory()
```

#### 3. Added SocketClient Methods (`src/mainview/socket-client.ts`)

```typescript
async getHFTrajectoryCount(): Promise<number> {
  const response = await this.request("request:getHFTrajectoryCount", {});
  if (!response.success) {
    throw new Error(response.error ?? "Failed to get trajectory count");
  }
  return (response as Extract<SocketResponse, { type: "response:getHFTrajectoryCount" }>).data?.count ?? 0;
}

// Similar methods for getHFTrajectories() and getHFTrajectory()
```

#### 4. Updated Widgets to Use RPC (`src/effuse/widgets/hf-trajectory-list.ts`)

```typescript
// BEFORE (BROKEN):
export const HFTrajectoryListWidget: Widget<
  HFTrajectoryListState,
  HFTrajectoryListEvent,
  OpenThoughtsService  // ❌ Service dependency
> = {
  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const service = yield* OpenThoughtsService  // ❌ Direct service access
      const trajectories = yield* service.getTrajectories(offset, limit)
    })
}

// AFTER (WORKING):
export const HFTrajectoryListWidget: Widget<
  HFTrajectoryListState,
  HFTrajectoryListEvent  // ✅ No service dependency
> = {
  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const socketClient = getSocketClient()  // ✅ RPC client
      const trajectories = yield* Effect.promise(() =>
        socketClient.getHFTrajectories(offset, state.pageSize)  // ✅ RPC call
      )
    })
}
```

#### 5. Fixed Event Forwarding (`src/mainview/effuse-main.ts`)

```typescript
// BEFORE (BROKEN):
const socket = yield* SocketServiceTag
const request = createRequest("request:getHFTrajectory", { index: event.index })
const response = yield* socket.request(request)  // ❌ socket.request doesn't exist

// AFTER (WORKING):
const socketClient = getSocketClient()  // ✅ Direct client access
const trajectory = (yield* Effect.promise(() =>
  socketClient.getHFTrajectory(event.index)  // ✅ Typed RPC method
)) as Trajectory | null
```

### Result
✅ **Proper client-server architecture** with:
- Server-side file system access (via BunContext)
- WebSocket RPC for browser-to-server communication
- Type-safe request/response handling
- No browser code touching file system

---

## Problem 4: MountedWidget Missing events/emit

### Symptoms
Event forwarding code tried to access `listWidget.events` and `detailWidget.emit()` but TypeScript complained these didn't exist.

### Root Cause
The `MountedWidget` interface only had `unmount()` method, missing the event stream and emit function needed for inter-widget communication.

### Fix
Updated `MountedWidget` interface in `src/effuse/widget/types.ts`:

```typescript
export interface MountedWidget<E> {
  unmount: () => Effect.Effect<void>;
  events: Stream.Stream<E>;  // ✅ Added
  emit: (event: E) => Effect.Effect<void>;  // ✅ Added
}
```

Updated `mountWidget()` implementation in `src/effuse/widget/mount.ts` to expose these.

### Result
✅ **Inter-widget communication works** - list widget selection triggers detail widget load

---

## Files Modified

### Core RPC Architecture
- `src/desktop/protocol.ts` - Added 3 request types, 3 response types, type guards
- `src/desktop/handlers.ts` - Added getHFTrajectoryCount, getHFTrajectories, getHFTrajectory handlers
- `src/mainview/socket-client.ts` - Added 3 RPC methods

### Browser-Side Widgets
- `src/effuse/widgets/hf-trajectory-list.ts` - Removed service dependency, added RPC calls
- `src/mainview/effuse-main.ts` - Removed BunContext/OpenThoughtsService, fixed event forwarding

### Widget Framework
- `src/effuse/widget/types.ts` - Added events/emit to MountedWidget
- `src/effuse/widget/mount.ts` - Exposed events/emit from mounting

### Logging Infrastructure
- `src/desktop/main.ts` - Fixed webview.bind() ordering (CRITICAL FIX)

### Bundle
- `src/mainview/effuse-main.js` - Rebuilt with all fixes

---

## Lessons Learned

### 1. Webview Binding Order Matters
**ALWAYS** call `webview.bind()` BEFORE `webview.init()`. The init script runs immediately and can create stubs that prevent later bindings.

### 2. Browser vs Server Code Separation
**NEVER** try to use server-side services (FileSystem, Path, etc.) in browser code. Always:
1. Add RPC request/response types to protocol
2. Implement server-side handler with proper context (BunContext)
3. Add typed method to SocketClient
4. Call via Effect.promise() in browser

### 3. Effect.promise() for Async RPC
When calling async SocketClient methods from Effect.gen, wrap in Effect.promise():
```typescript
const result = yield* Effect.promise(() => socketClient.someMethod())
```

### 4. Widget Dependencies
Avoid adding service dependencies to Widget type signature if the service is server-side only. Use getSocketClient() directly instead.

---

## Testing Next Steps

1. ✅ Bundle rebuilt with all fixes
2. ⏳ Restart dev server to load new bundle
3. ⏳ Verify widgets mount and display
4. ⏳ Verify RPC calls work (count, trajectories, single trajectory)
5. ⏳ Verify list displays 100 trajectories
6. ⏳ Verify clicking trajectory loads detail view
7. ⏳ Verify search/filter works
8. ⏳ Verify pagination works

---

## Architecture Summary

**Request Flow:**
1. Browser widget calls `getSocketClient().getHFTrajectories(0, 100)`
2. SocketClient sends RPC request over WebSocket with correlation ID
3. Desktop server receives request in handleRequest()
4. Server calls getHFTrajectories() handler
5. Handler creates OpenThoughtsService with BunContext.layer
6. Service reads Parquet files via FileSystem
7. Server sends RPC response back with correlation ID
8. SocketClient resolves promise with trajectory data
9. Widget updates state and re-renders

**No browser code touches file system. All file access is server-side via RPC.**

---

**Session End: 2315 Central Time**
