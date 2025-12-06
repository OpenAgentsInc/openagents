# Plan: CLI Event Emission to Connected UIs

## Problem Summary

CLI scripts (`tbench-local.ts`, `tbench-iterate.ts`) already emit events via `TBEmitter` → `HudClient`, but events never reach the UI because of a **port mismatch**:

| Component | Connects To | Actual Server |
|-----------|------------|---------------|
| HudClient (CLI) | `ws://localhost:4242` | **NOTHING** (deprecated) |
| DesktopServer | - | `ws://localhost:8080/ws` |
| SocketClient (UI) | `ws://localhost:8080/ws` | DesktopServer |

**Root cause**: `HudClient` defaults to port 4242, but no server listens there. The desktop server is on 8080.

## Solution: Unify on Port 8080 + Full E2E Tests

### Phase 1: Fix Port Everywhere (~30 lines)

**File: `src/hud/client.ts`**
- Change default URL to use desktop server URL (`ws://localhost:8080/ws`)
- Remove references to deprecated port 4242

**File: `src/hud/protocol.ts`**
- Remove deprecated `HUD_WS_PORT = 4242` constant (or update to 8080)
- Update `HUD_WS_URL` to use port 8080

### Phase 2: Add CLI URL Override Flag (~20 lines per file)

**Files: `src/cli/tbench-local.ts`, `src/cli/tbench-iterate.ts`**
- Add `--hud-url` CLI flag to override WebSocket URL
- Pass URL to `createTBEmitter()`

### Phase 3: Add Injection API to DesktopServer (~40 lines)

**File: `src/desktop/server.ts`**
- Add `POST /api/inject-hud` endpoint for test message injection
- Add `GET /api/health` endpoint for diagnostics

### Phase 4: E2E Test Infrastructure (~200 lines)

**New file: `e2e/fixtures/tb-messages.ts`**
- Factory functions for TB message types (TBRunStart, TBTaskStart, etc.)
- `createTBRunSequence()` helper for complete workflow

**New file: `e2e/fixtures/cli-runner.ts`**
- Helper to spawn `tbench-local.ts` subprocess
- Pass `--hud-url` pointing to test server
- Capture stdout/stderr for assertions
- Wait for process exit with timeout

**New file: `e2e/tests/tb/tb-message-flow.spec.ts`**
- Tests that TB events flow from injection to UI
- Tests message ordering and completeness

**New file: `e2e/tests/tb/tb-cli-integration.spec.ts`**
- Spawn actual CLI with `--hud-url` pointing to test server
- Verify events flow from CLI → server → UI
- Full end-to-end validation

---

## Implementation Order

1. **Fix HudClient default URL** - Immediate unblock
2. **Add `--hud-url` flags to CLI** - Flexibility for tests
3. **Add injection API to DesktopServer** - E2E test support
4. **Create TB message factories** - Test fixtures
5. **Write TB event flow tests** - Verification

## Files to Modify

| File | Changes |
|------|---------|
| `src/hud/client.ts` | Change default URL, add env var support |
| `src/hud/protocol.ts` | Add deprecation comment |
| `src/cli/tbench-local.ts` | Add `--hud-url` flag |
| `src/cli/tbench-iterate.ts` | Add `--hud-url` flag |
| `src/desktop/server.ts` | Add `/api/inject-hud`, `/api/health` routes |

## Files to Create

| File | Purpose |
|------|---------|
| `e2e/fixtures/tb-messages.ts` | TB message factory functions |
| `e2e/fixtures/cli-runner.ts` | CLI subprocess spawner |
| `e2e/tests/tb/tb-message-flow.spec.ts` | TB event flow tests (injection) |
| `e2e/tests/tb/tb-cli-integration.spec.ts` | Full CLI→UI tests |

## Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `HUD_WS_URL` | Override WebSocket URL for HUD events | `ws://localhost:8080/ws` |

## Cleanup: Remove Port 4242 References

Search for and update any references to port 4242:
- `src/hud/protocol.ts` - `HUD_WS_PORT`, `HUD_WS_URL`
- `src/hud/client.ts` - Default URL
- Any other files referencing 4242

## Test Verification

```bash
# E2E tests (message injection + CLI spawn)
bun run e2e e2e/tests/tb/

# Headed mode for visual verification
HEADED=true bun run e2e e2e/tests/tb/

# Manual integration test
# Terminal 1: bun run desktop:dev
# Terminal 2: open http://localhost:8080
# Terminal 3: bun run tbench-local --suite ./tasks/fm-mini-suite.json -o ./results
# → Events should appear in browser UI
```

## Estimated Scope

- ~300 lines total across 5 modified + 4 new files
- Low risk - mostly additive changes
- Breaking: removes port 4242 (no backward compatibility per user request)
