# Interrupt & Follow-Up Controls

## Overview

The mobile session screen now mirrors the Codex CLI TUI rules for long running turns:

1. **Interrupt button** – Visible while a turn is running. Tapping it sends a `{"control":"interrupt"}` frame to the bridge.
2. **Bridge signal forwarding** – The WebSocket bridge tracks the Codex child PID and forwards `SIGINT` (or `taskkill` on Windows) so the CLI stops cleanly.
3. **Queueing follow-ups** – While an agent turn is active the “Send” button becomes “Queue.” Messages are captured locally and replayed automatically after the agent finishes.
4. **Composer recovery** – If an interrupt aborts the turn before any queued follow-ups are flushed, the pending prompts (plus any draft text) are written back into the composer so nothing is lost.

These additions keep the mobile UX aligned with the CLI: you can stop runaway executions, stack follow-ups, and continue the conversation without manually copying drafts around.

## Bridge Changes

- Track the child process ID and keep a rolling buffer of the last 2,000 JSONL lines.
- Recognize `{"control":"interrupt"}` frames and forward the appropriate signal.
- Replay the buffered history to new WebSocket subscribers so reconnecting clients immediately catch up.

## App Changes

- Session state now records whether a turn is active, keeps a FIFO queue of follow-ups, and flushes them after completion or connection recovery.
- The composer accepts `prefill` / `onDraftChange` props so recovered text stays in sync across the UI.
- The interrupt action is logged in the feed (`> [interrupt] requested`) for quick auditing.

Together these pieces make it clear when an agent is busy, let you nudge it while waiting, and guarantee your queued instructions still run even if you briefly leave the foreground.
