# WebSocket Compatibility Fix Log
Date: 2025-06-22
Start Time: 10:30

## Issue
When running `pnpm site`, the app crashes with:
```
TypeError: app.ws is not a function
```

The relay plugin expects WebSocket support but the Effect-based Psionic implementation
doesn't have it yet.

## Root Cause
1. The relay plugin uses `app.ws()` which is an Elysia method
2. The Elysia compatibility layer in Psionic was missing the `ws` method
3. Also missing `options` and `post` methods used by the relay plugin

## Solution
Enhanced the Elysia compatibility layer in Psionic to include:
- `ws()` method that delegates to `websocket()`
- `options()` method (temporarily uses GET)
- `post()` method

## Files Modified
- `/packages/psionic/src/core/app.ts` - Added missing methods to elysia compatibility object

## Status
- WebSocket functionality is still stubbed (logs message but doesn't actually implement WebSocket)
- This allows the app to start without crashing
- Full WebSocket implementation with Effect is still needed (future work)