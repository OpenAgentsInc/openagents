# Session State & Background Behaviour

## Current Flow

- **Live feed** – `SessionScreen` appends every parsed Codex JSONL line to an in-memory log and mirrors the entries to `AsyncStorage` via `log-store.ts`.
- **History tab** – Reads from the shared store using `React.useSyncExternalStore`, so the persisted timeline is consistent across screens.
- **Queued follow‑ups** – While a turn is running, outbound prompts are staged locally and replayed as soon as the agent publishes a `turn.completed` or `turn.failed` event.
- **Bridge replay buffer** – `oa-bridge` now keeps the most recent 2,000 JSONL lines and replays them to any client that (re)connects, so foregrounding the app immediately restores missed output.

## Known Gaps

- The queue and replay buffer live entirely on the device and the bridge—if Codex restarts, we still rely on `resume` support to recover the thread.
- Backgrounded iOS apps can be suspended for multiple minutes; when that happens the WebSocket is dropped and the UI must rely on the bridge’s backlog replay to catch up.
- AsyncStorage writes are best-effort; large bursts of events may still exceed mobile storage quotas until we introduce compaction.

## Next Steps

1. **Progress indicators while replaying backlog** – surface a lightweight “Rehydrating” banner when we apply buffered history so users know they’re seeing delayed output.
2. **Persistent queue storage** – write queued follow-ups to disk so they survive app restarts, not just temporary backgrounding.
3. **Thread resume command** – expose a manual “Refresh session” action that explicitly asks Codex for the latest transcript (`exec resume <thread_id>`) in case the bridge and CLI drift out of sync.
