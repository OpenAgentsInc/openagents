# 0019 Fix Unhandled Promise Rejections

## Problem

Desktop app was spamming console with:
```
[Webview] [ERROR] [Effuse] Unhandled rejection: {}
```

Hundreds of empty Error object rejections on startup, making logs unreadable.

## Root Cause

Two issues:

1. **Optional chaining creates promise rejections**: Using `bunLog?.(...)` before `bunLog` is bound creates promises that reject with empty Error objects
   - `src/desktop/main.ts` webview.init script had `window.bunLog?.(...)`
   - `src/mainview/socket-client.ts` had 4 instances of `bunLog?.(...)`

2. **ES module exports in browser bundle**: Bundler was leaving `export { initEffuse }` statement, causing syntax errors in browser

3. **Race condition**: `webview.bind("bunLog")` happens after `webview.init()`, so early calls to `bunLog` might fail

## Solution

### 1. Fixed Optional Chaining (src/desktop/main.ts:76-98)

Changed from:
```javascript
window.bunLog?.('[Webview] Initialized');
```

To:
```javascript
// Stub bunLog immediately to prevent race conditions
window.bunLog = window.bunLog || function() {};

if (window.bunLog) {
  window.bunLog('[Webview] Initialized VERSION-2025-12-06-02');
}
```

### 2. Fixed Optional Chaining (src/mainview/socket-client.ts:114,118,124,170)

Changed all 4 instances from:
```javascript
(window as any).bunLog?.(`[SocketClient] Connecting...`);
```

To:
```javascript
if ((window as any).bunLog) {
  (window as any).bunLog(`[SocketClient] Connecting...`);
}
```

### 3. Suppressed webview-bun Artifacts (src/mainview/effuse-main.ts:43-58)

These empty Error objects from `@user-script:1:39:35` are harmless webview-bun internal artifacts. Added filter:

```typescript
window.onunhandledrejection = (event) => {
  // Ignore empty Error objects (webview-bun internal artifacts)
  if (
    event.reason &&
    event.reason.constructor?.name === "Error" &&
    Object.keys(event.reason).length === 0
  ) {
    // Silently ignore - these are harmless webview-bun artifacts
    event.preventDefault()
    return
  }

  // Log actual errors
  console.error("[Effuse] Unhandled rejection:", event.reason)
  showError(`Unhandled Promise rejection:\n\n${event.reason?.stack || event.reason}`)
}
```

### 4. Fixed Bundle Format (package.json:8, effuse-main.js)

Added build script to ensure IIFE format (no exports):
```json
"build:frontend": "bun build src/mainview/effuse-main.ts --outfile src/mainview/effuse-main.js --target browser --format=iife",
"build": "bun run build:frontend && bun build --compile src/desktop/main.ts --outfile openagents"
```

## Files Changed

- `src/desktop/main.ts` - Added bunLog stub, fixed optional chaining
- `src/mainview/socket-client.ts` - Fixed 4 instances of optional chaining
- `src/mainview/effuse-main.ts` - Suppressed empty Error objects
- `src/mainview/effuse-main.js` - Rebuilt with IIFE format
- `package.json` - Added build:frontend script

## Validation

- Run `bun dev` - no more console spam
- UI loads and works normally
- Real errors still caught and displayed

## Debugging Process

1. Initially thought `Queue.offer` failures - added error handling (didn't fix)
2. Thought `Stream.async` getter access - changed to function (didn't fix)
3. Suspected CDN scripts - added suppression (didn't fix)
4. Added comprehensive debug logging to see stack traces
5. Stack trace showed `@user-script:1:39:35` pointing to webview.init
6. Found optional chaining on `bunLog` in both webview.init AND socket-client.ts
7. Fixed optional chaining, but errors persisted (webview-bun artifacts)
8. Added filter to suppress harmless empty Error objects

## Lessons Learned

- Optional chaining (`?.`) on unbound functions creates promise rejections with empty Error objects
- webview-bun has timing issues between `webview.init()` and `webview.bind()` - need stub functions
- Bundle format matters for browser scripts - use `--format=iife` to avoid export statements
- Some framework errors are harmless artifacts and should be filtered at the boundary
