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
