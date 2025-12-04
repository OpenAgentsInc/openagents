# Plan: Migrate Electrobun to webview-bun with Effect Socket

## Overview

Replace Electrobun desktop framework with webview-bun, removing the RPC layer entirely and replacing with a unified Effect Socket protocol for bidirectional client-server communication.

## Current Architecture

```
Agent Process → WebSocket (4242) → Bun Server → Electrobun RPC → Webview
                                            ↑
                         TB controls use RPC calls (loadTBSuite, startTBRun, etc.)
```

## Target Architecture

```
Agent Process → Effect Socket → Bun Server ← Effect Socket ← Webview (webview-bun)
                                    ↓
                      Unified protocol for events + requests
```

## Key Changes

1. **Remove electrobun** - Replace with webview-bun native window
2. **Remove RPC layer** - Merge into unified WebSocket protocol
3. **Effect Socket both sides** - @effect/platform-browser on frontend, @effect/platform-bun on backend
4. **Unified protocol** - Extend HudMessage types to include request/response patterns

## Epic: webview-bun Migration

**Type**: epic
**Priority**: P1 (High)
**Labels**: desktop, refactor, effect

### Subtasks

#### Task 1: Add webview-bun dependency and basic window setup
- Add `webview-bun` to package.json
- Create `src/desktop/main.ts` entry point
- Basic window creation pointing to localhost server
- **Files**: `package.json`, `src/desktop/main.ts` (new)

#### Task 2: Create unified Effect Socket protocol schema
- Extend `src/hud/protocol.ts` with request/response message types
- Add schema types for: loadTBSuite, startTBRun, stopTBRun, loadRecentTBRuns, loadTBRunDetails
- Define bidirectional message discriminator
- **Files**: `src/hud/protocol.ts`, `src/desktop/protocol.ts` (new)

#### Task 3: Implement Effect Socket server with unified protocol
- Replace Bun.serve WebSocket with Effect Socket
- Handle both HUD events and request/response messages
- Maintain backwards compatibility with existing HudClient
- **Files**: `src/hud/server.ts`, `src/desktop/server.ts` (new)

#### Task 4: Implement Effect Socket frontend client
- Create @effect/platform-browser Socket client
- Build Effect-based message handling for unified protocol
- Replace Electrobun RPC calls with socket requests
- **Files**: `src/mainview/socket-client.ts` (new), `src/mainview/index.ts`

#### Task 5: Port TB controls from RPC to Effect Socket
- Move loadTBSuite handler to socket protocol
- Move startTBRun/stopTBRun handlers
- Move loadRecentTBRuns/loadTBRunDetails handlers
- **Files**: `src/bun/index.ts`, `src/desktop/handlers.ts` (new)

#### Task 6: Update mainview to use webview-bun
- Remove Electroview/Electrobun imports
- Wire up Effect Socket client
- Update UI event handlers to use socket protocol
- **Files**: `src/mainview/index.ts`, `src/mainview/index.html`

#### Task 7: Remove Electrobun dependencies and config
- Remove electrobun from package.json
- Delete `electrobun.config.ts`
- Remove `types/electrobun.d.ts`
- Update build scripts
- **Files**: `package.json`, `electrobun.config.ts`, `types/electrobun.d.ts`

#### Task 8: Update build/start scripts for webview-bun
- Create new build script for webview-bun binary
- Update `bun start` to use new entry point
- Add `bun build --compile` configuration
- **Files**: `package.json`

## Critical Files Reference

### To Modify
- `src/bun/index.ts` (403 lines) - Current main process, port handlers
- `src/mainview/index.ts` (2037 lines) - UI logic, remove Electroview
- `src/hud/protocol.ts` (680 lines) - Extend with request/response types
- `src/hud/server.ts` (238 lines) - Replace with Effect Socket
- `package.json` - Dependencies

### To Delete
- `electrobun.config.ts`
- `types/electrobun.d.ts`

### To Create
- `src/desktop/main.ts` - webview-bun entry point
- `src/desktop/server.ts` - Effect Socket server
- `src/desktop/protocol.ts` - Unified protocol types
- `src/desktop/handlers.ts` - Request handlers
- `src/mainview/socket-client.ts` - Frontend Effect Socket client

## Dependencies

- `webview-bun` - Native window creation
- `@effect/platform-browser` - Frontend Effect Socket (already have @effect/platform)

## Risk Considerations

1. **@effect/platform-browser in webview** - Need to verify bundling works correctly
2. **Request/response over WebSocket** - Need correlation IDs for matching responses
3. **Error handling** - Effect errors need serialization across socket boundary
4. **Hot reload** - May need custom dev server setup without Electrobun's built-in support

## Task Creation Commands

```bash
# Create epic
bun run tasks:create --title "Epic: Migrate Electrobun to webview-bun with Effect Socket" --type epic --priority 1 --labels "desktop,refactor,effect" --json

# Then create subtasks with parent-child deps to the epic
```
