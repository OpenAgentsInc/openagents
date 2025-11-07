# Tinyvex History — Recent Sessions Drawer + Load Messages

Labels: area:tinyvex, type:feature, priority:P0
Assignees: TBD

## Summary

Show a list of the 10 most recent ACP sessions in the left drawer and, when clicked, load that session’s persisted messages into the main timeline.

## Server

Add JSON‑RPC methods on the existing DesktopWebSocketServer (backed by Tinyvex DB):

- `tinyvex/history.recentSessions`
  - result: `[ { session_id: string, last_ts: number, message_count: number } ]` sorted by `last_ts` desc, limited 10.
  - SQL: `SELECT session_id, MAX(ts) AS last_ts, COUNT(*) AS cnt FROM acp_events GROUP BY session_id ORDER BY last_ts DESC LIMIT 10`.

- `tinyvex/history.sessionTimeline`
  - params: `{ session_id: string, limit?: number, direction?: "backward" | "forward", from_seq?: number }`
  - result: `ACP.Client.SessionNotificationWire[]` (full ACP updates in send order). MVP: return all for the session ordered by `ts ASC`.
  - SQL: `SELECT update_json FROM acp_events WHERE session_id = ? ORDER BY ts ASC [LIMIT ?]`.

## Client (BridgeManager + UI)

- BridgeManager
  - Add calls for `recentSessions()` and `loadSessionTimeline(sessionId:)`.
  - Maintain `@Published var recentSessions: [RecentSession]` and expose a loader for timeline that clears + appends to `updates`.

- HistorySidebar
  - When Tinyvex DB is attached, prefer `bridge.recentSessions` over local Codex/Claude scanning.
  - Render up to 10 sessions by `last_ts` and show the relative time + message_count.
  - On tap, call `bridge.loadSessionTimeline(sessionId:)` and switch the active session id for UI.

- Timeline View
  - Reuse existing rendering for `bridge.updates` (already ACP‑compliant). Show loaded messages immediately.

## Logging

- Server: log `[Bridge][tinyvex.history] recentSessions count=...` and `[Bridge][tinyvex.history] sessionTimeline session=... rows=...`.
- Client: log `[Bridge][history] loaded recentSessions ...` and `[Bridge][history] loaded sessionTimeline session=... count=...`.

## Acceptance Criteria

- Drawer shows up to 10 sessions from Tinyvex (sorted by activity) on macOS and iOS.
- Tapping a session loads persisted messages into the main timeline within 500ms for small sessions.
- No crashes if Tinyvex DB is empty (drawer shows “No chats found”).
- Basic error surfaces as a small toast/log.

## Notes

- Keep methods lightweight (no projections needed for MVP). We will add projections later if needed.
- For very large sessions, we can paginate by `from_seq` in a follow‑up.

