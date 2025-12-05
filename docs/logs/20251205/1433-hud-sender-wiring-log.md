# 1433 HUD Sender Wiring

## Objective
Wire `setATIFHudSender()` to desktop server's WebSocket broadcast function to enable real-time ATIF step emission to UI clients.

## Changes Made

### Desktop Server Worker (`src/desktop/server-worker.ts`)
- Added import: `import { setATIFHudSender } from "../atif/hud-emitter.js";`
- Wired ATIF emitter after server creation:
  ```typescript
  setATIFHudSender((message) => {
    server.sendHudMessage(message);
  });
  ```
- Added log statement: "ATIF HUD emitter initialized"

## Architecture

Complete data flow now functional:
```
TB Runner (subagent)
  ↓
SDK → ATIF Adapter (emitATIFStep)
  ↓
setATIFHudSender callback
  ↓
DesktopServer.sendHudMessage()
  ↓
handleHudMessage() (stores + broadcasts)
  ↓
WebSocket → All UI Clients
```

## Status
✅ **Task 7 Complete** - Backend pipeline fully wired from SDK to frontend WebSocket.

Remaining frontend work:
- Task 8: Add ATIF session state to frontend (index.ts)
- Task 9: Implement HUD message handler for atif_step
- Task 10: Create rendering engine (renderATIFTimeline, renderStep)
- Task 11: Add CSS styling for ATIF timeline
- Task 12: Test full pipeline with real TB run

## Files Modified
- `src/desktop/server-worker.ts` (+2 lines import, +4 lines initialization)
