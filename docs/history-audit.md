# History System Audit

## Overview
- The mobile app persists every line shown in the session feed to an internal log store so users can revisit prior turns.
- Storage lives in `expo/lib/log-store.ts` and is backed by AsyncStorage. An in-memory `Map` tracks the live session log; helpers expose `putLog`, `getAllLogs`, `subscribe`, `loadLogs`, `saveLogs`, and `clearLogs`.
- Writers only exist in the session screen (`expo/app/(tabs)/session.tsx:63`); every parsed Codex line generates a `LogDetail` entry and immediately calls `saveLogs()` to flush the full snapshot back to AsyncStorage.

## Persistence & IDs
- Each entry is keyed by an incrementing `id` (`SessionScreen` maintains `idRef`). The store sorts by `id` to produce chronological history (`log-store.ts:43`).
- `loadLogs()` rehydrates from AsyncStorage on boot and sanitizes out `exec_command_end` fragments before repopulating the in-memory map (`log-store.ts:46`).
- `clearLogs()` wipes both the map and AsyncStorage and notifies subscribers (`log-store.ts:62`), which the **New Chat** button invokes through the WebSocket provider (`expo/app/_layout.tsx:101` + `expo/providers/ws.tsx:111`).

## Consumers
- **Session feed** keeps its own React state mirror for rendering, but still writes through the store for persistence (`session.tsx:63-153`). After `loadLogs()` runs on mount (`session.tsx:210`), the feed includes prior entries.
- **History tab** uses `React.useSyncExternalStore` with `subscribe`/`getAllLogs` to stay in sync with the store (`expo/app/(tabs)/history.tsx:9`). It reverses the array to show newest-first and renders each entry as a tappable card.
- **Message detail** looks up a single log entry via `getLog(id)` (`expo/app/message/[id].tsx:6`). It does not subscribe, so deep-links depend on the store already being hydrated.
- **Drawer sidebar** (hamburger menu) shows the last ten user-authored prompts by filtering `getAllLogs()` (`expo/app/_layout.tsx:25-57`).

## Identified Issues
- Drawer history never updates after the initial render. `DrawerContent` calls `loadLogs()` once inside `useEffect` solely to force a re-render through a dummy state setter, but it does **not** subscribe to further updates (`expo/app/_layout.tsx:24-27`). Because it reads `getAllLogs()` only during render and nothing re-triggers render when new logs arrive, freshly queued messages or sessions never appear.
- Drawer history also filters synchronously while `loadLogs()` is still pending, so the initial open can show “No history yet.” until the effect resolves.
- There is no notion of session boundaries in the data model. Clearing logs is the only way to start a “new chat,” so all history views mix entries from every turn into one flat list.
- Message detail depends on the log map already being populated via some earlier `loadLogs()` call; direct navigation before hydration yields “Message not found.”

## Follow-Up Questions
1. Should the drawer show the most recent *threads* instead of raw user prompts (requires grouping by turn/thread)?
2. Should history store separate user/system/agent metadata so downstream UIs can filter without regex heuristics (`/^\s*>/`)?

## Recent Fixes
- Drawer now subscribes to the log store so new prompts appear instantly (`expo/app/_layout.tsx`).
- History tab and message detail views wait for hydration before rendering empty states, preventing the “No history yet” flicker on first load.
- Log persistence is throttled via a 150ms debounce and survives concurrent loads with a shared hydration flag (`expo/lib/log-store.ts`).

---

## Historical Codex Chats: What Prior Apps Did vs. Now

This section documents how earlier apps loaded Codex history and what changes are needed to support true historical chats here.

### Prior Implementations (external references)

- Tricoder (RN/Expo app):
  - Server-driven history. The mobile app called a local daemon to list and view sessions.
  - Code (for reference on your machine): `~/code/tricoder/lib/store.ts` implements `loadHistory()` to GET `http://<host>/history?limit=…&since=…`, caches in AsyncStorage, and opens `app/session/[id].tsx` which fetches `GET /session?id=<id>`.
  - Tapping a history row navigates to a dedicated session view and can resume that session.

- v7 (Rust + iOS renderer):
  - Daemon `codexd` exposes `/history`, `/session`, and `/message` (resume) using Axum.
  - `/history` scans `~/.codex/sessions/**/rollout-*.jsonl`, sorts by mtime, and returns `{ id, path, mtime, title, snippet }`.
  - `/session` parses one JSONL file into `{ title, items: [{ ts, kind, role?, text }] }` for the UI to render.
  - `/message` resumes an existing session id and forwards events to connected clients.

### Current Project (this repo)

- No server-side history. The app only persists the current live feed to `AsyncStorage` via `expo/lib/log-store.ts`.
- “History” and the Drawer show entries from that local store; there are no session boundaries and no access to Codex’s on-disk rollouts.
- The Rust bridge (`crates/codex-bridge`) only serves a WebSocket at `/ws`; it has no `/history` or `/session` routes.

### Gap Analysis

- Missing discovery of Codex rollouts on disk (e.g., `~/.codex/sessions`).
- No HTTP endpoints to list sessions, fetch a historical transcript, or resume by id.
- UI uses a flat log of one run; there is no session catalog.

---

## Path Forward: Enable Real Historical Chats

There are two viable approaches; both converge on the same client API.

### Option A — Extend the existing bridge

Add endpoints to `crates/codex-bridge` (already Axum-based):

- `GET /history?limit=&since=`
  - Scan a base dir (default `$HOME/.codex/sessions`; override with `CODEXD_HISTORY_DIR`).
  - Return newest-first array of `{ id, path, mtime, title, snippet }`.
  - Title can be derived from the first bold segment or first N words; snippet is the last assistant message.

- `GET /session?id=&path=`
  - Resolve to a JSONL file (exact filename or absolute `path`).
  - Parse and normalize to UI-friendly items: `{ ts, kind: 'message'|'reason'|'cmd'|'pending', role?, text }` (drop noisy `exec_command_output_delta`).

- `POST /message` (or `GET /message` query) — optional
  - Resume an existing session id (reusing the bridge’s existing `exec resume` flow) and stream new output over `/ws`.

Server impl notes:

- Port the scanning/handlers from v7 `crates/codexd/src/main.rs` into a new `history.rs` module under the bridge.
- Keep scan bounds conservative (depth, extension, item cap) and support an auth token if exposing beyond localhost.

### Option B — Ship a separate `codexd` next to the bridge

- Keep `codex-bridge` focused on streaming; run a second process for history endpoints.
- Pros: code reuse is simplest. Cons: two ports to manage in the app.

Recommendation: Option A for a single-port developer experience.

---

## Client Changes (Expo)

- Introduce a sessions store distinct from the live feed:
  - Add `expo/lib/sessions-store.ts` that fetches `/history` and caches results.
  - Update the History tab to display sessions from `/history` (newest first). Tapping a row opens a new `app/session/[id].tsx` which loads `/session?id=…`.
  - Keep `expo/lib/log-store.ts` for the current run only; do not mix with historical transcripts.
  - Optional: a “Continue chat” button on the session view that sets the next send’s preface `{ "resume": "<id>" }` so the bridge resumes that session.

---

## Minimal Implementation Plan

1. Bridge: add `/history` and `/session` routes to `crates/codex-bridge` (port from v7 `codexd`).
2. App: create `sessions-store.ts`; wire the History tab and a new `session/[id].tsx` to those endpoints.
3. Resume: plumb a “Continue chat” path (preface or explicit action) that the bridge already understands.
4. Test with large rollouts and ensure we ignore `exec_command_output_delta` lines.

Follow-ups:
- Add a small in-bridge index cache (last scan time + memoized results).
- Optional `/search` endpoint for titles/snippets.

---

## Summary

Today’s app only shows a locally persisted, single-session feed. Tricoder/v7 loaded true Codex history by scanning `~/.codex/sessions` and serving it over HTTP. We can replicate that by adding `/history` and `/session` to the Axum bridge and updating the History tab to query them. This yields immediate historical chat browsing with a clean path to “resume from history”.
