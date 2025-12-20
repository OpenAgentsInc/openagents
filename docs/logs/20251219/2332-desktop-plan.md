# Plan: Desktop Recorder Feed + Issue #77 Unblock

## Goal
Get the desktop app showing a live feed of recorder events using the existing storybook components, and unblock issue #77.

---

## Current State

### Desktop App (`crates/desktop`)
- Wry/tao webview shell with Actix server on dynamic port
- **Already has `/autopilot` route** with timeline placeholder structure:
  - `#session-sidebar` - empty placeholder
  - `#session-header` - empty placeholder
  - `#timeline` - ready to receive appended fragments
- WebSocket broadcaster in place - can push HTML fragments to all clients
- OOB swap pattern: fragments with `id` replace elements, without `id` append to `#timeline`

### UI Components (`crates/ui`)
- **Complete recorder component library** in `src/recorder/`:
  - Atoms: status_dot, line_type_label, step_badge, timestamp badges, etc.
  - Molecules: LineMeta, LineHeader, result_display, budget_meter, etc.
  - Organisms: UserLine, AgentLine, ToolLine, McpLine, SubagentLine, LifecycleLine, etc.
  - Sections: SessionHeader, SessionStats, ToolIndex, session_sidebar
- Storybook at port 3030 with full demo

### Recorder (`crates/recorder`)
- Parses `.rlog` files with `@start`, `@end`, step lines
- CLI for replay and validation
- **No streaming/event emission yet**

### Gap Analysis
| Have | Need |
|------|------|
| Desktop WebSocket broadcaster | Event source to feed it |
| UI recorder components | Integration to render events as HTML |
| Recorder .rlog parser | Event stream from autopilot OR rlog replay |
| /autopilot route shell | Wire up sidebar + timeline |

---

## Issue #77: Why It's Blocked

**Problem**: Panic handlers are synchronous, can't call async MCP to block issues.

**Current cleanup** (`main.rs`):
- Panic hook cleans up `.mcp.json` file (sync - works)
- Signal handlers do the same
- But: no `@end` marker written, no `issue_block` called, no failure reason logged

**Solution approach**: Don't try to call MCP from panic handler. Instead:
1. Write crash state to a **lockfile/pidfile** when run starts
2. On clean exit, remove the file
3. On next autopilot start, check for stale lockfiles → cleanup orphaned issues
4. This is all **sync filesystem ops** - works in panic handlers

---

## Proposed Issues

### URGENT: Desktop Feed Integration

**Issue A: Wire up recorder components to desktop /autopilot route**
- Import UI recorder organisms into desktop
- Render SessionHeader, session_sidebar with real/mock data
- Render timeline events using UserLine, AgentLine, ToolLine, etc.
- Use existing WebSocket broadcaster for real-time updates

**Issue B: Add event stream from autopilot to desktop**
- Option 1: Autopilot broadcasts events via HTTP POST to desktop `/events`
- Option 2: Shared channel/IPC between autopilot and desktop
- Option 3: Desktop watches .rlog file and streams new lines
- Simplest for Monday: Option 3 (file watching)

**Issue C: Add rlog replay mode to desktop**
- Given an .rlog path, stream events to timeline
- Useful for reviewing past sessions
- Uses existing recorder parser

### HIGH: Unblock #77

**Issue D: Filesystem-based crash recovery for autopilot**
- On run start: write `~/.autopilot/run.lock` with `{issue_number, session_id, rlog_path}`
- On clean exit: delete the lockfile
- On panic/signal: lockfile remains (cleanup_mcp_json already runs)
- On next `autopilot` start: check for stale lockfile, call `issue_block` with crash context

This unblocks #77 without requiring async calls from panic handlers.

---

## Issues to Create (All URGENT)

### 1. Wire recorder components into desktop /autopilot route
**Priority: urgent**
- Import UI recorder organisms (UserLine, AgentLine, ToolLine, etc.) into desktop
- Render SessionHeader in `#session-header`, session_sidebar in `#session-sidebar`
- Create `render_step()` function that converts rlog step data → Maud HTML
- Test with hardcoded mock data first

**Files:**
- `crates/desktop/src/views/autopilot.rs`
- `crates/desktop/Cargo.toml` (add ui dependency)

### 2. Add rlog replay mode to desktop
**Priority: urgent**
- Add route: `GET /autopilot/replay?path=<rlog_path>`
- Parse .rlog using recorder crate
- Stream events to timeline via WebSocket broadcaster
- Add replay controls (play/pause/speed) in header

**Files:**
- `crates/desktop/src/server.rs` (new route)
- `crates/desktop/src/views/autopilot.rs` (replay controls)
- `crates/desktop/Cargo.toml` (add recorder dependency)

### 3. Add live event streaming from autopilot to desktop
**Priority: urgent**
- Autopilot POSTs HTML fragments to `http://localhost:<port>/events`
- Desktop broadcasts to all connected WebSocket clients
- Need to pass desktop port to autopilot (env var or discovery)
- Alternative: autopilot writes to .rlog, desktop watches file

**Files:**
- `crates/autopilot/src/lib.rs` (add event posting)
- `crates/autopilot/src/main.rs` (desktop port discovery)
- `crates/desktop/src/server.rs` (already has /events route)

### 4. Filesystem-based crash recovery for autopilot
**Priority: urgent** (unblocks #77)
- On run start: write `~/.autopilot/run.lock` with JSON: `{issue_number, session_id, rlog_path, started_at}`
- On clean exit: delete lockfile, write proper `@end` marker
- On panic/signal: lockfile remains (sync cleanup already runs)
- On next `autopilot` start: check for stale lockfile → call `issue_block` with crash context

**Files:**
- `crates/autopilot/src/main.rs` (lockfile on start, stale check)
- `crates/autopilot/src/lib.rs` (lockfile cleanup in finish())

---

## Implementation Order

1. **Issue 1** first - get components showing (foundation)
2. **Issue 2** next - replay mode for demos
3. **Issue 4** then - crash recovery (can be parallel)
4. **Issue 3** last - live streaming (depends on 1)

---

## After Plan Mode

Create these 4 issues with priority=urgent, then start implementation.
