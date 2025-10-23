# Session Titles

This document explains how the app decides what to display as the title while you are in a live session, and how that title is derived from resume state and server events.

## TL;DR

- The header text is managed globally via a small Zustand store and a helper hook `useHeaderTitle(title)`.
- In the live Session screen, the title is set to:
  - `"Session <short-id>"` when an id is known
  - `"New session"` when the feed is empty
  - `"Session"` otherwise (fallback)
- The `<short-id>` is the first segment of a UUID (substring before the first `-`). If the id has no hyphen, the first 6 characters are used as a fallback.
- The id source comes from either:
  1) The “resume” id when you continue a chat from history, or
  2) The first `thread` event emitted by the CLI during the session (`thread_id`).

## Where the title actually comes from

- The header view lives in `expo/components/app-header.tsx`. It reads a `title` from a Zustand store and renders it using the app’s theme.
- The store and hook are defined in `expo/lib/header-store.ts`:
  - `useHeaderStore` holds `{ title, setTitle, height, setHeight }`.
  - `useHeaderTitle(title: string)` updates the title when the hook’s argument changes.

## How the Session screen sets the title

File: `expo/app/session/index.tsx`

1) Initial seed (resume id)

- When resuming a session from history, the “Session Detail” view sets a on‑next‑send resume id via the WebSocket provider (see below) and navigates back to the live session tab.
- The session screen reads `resumeNextId` from `useWs()` on mount and immediately seeds the header title with a short form of that id if present.

2) Live update (thread event)

- While streaming, each incoming line is parsed into a typed event. When a `thread` event arrives, the screen extracts `thread_id` and updates the header to `"Session <short-id>"`.
- This takes precedence over any previous value and ensures the final title matches the actual running thread id.

3) Fallbacks

- If there is no resume id and no thread id yet:
  - When the feed is empty, the title shows `"New session"`.
  - Once any content appears but the thread id is still unknown, it shows `"Session"` until the id is known.

### Short id algorithm

```
// Given id: "67e5-5044-10b1-426f-9247-bb680e5fe0c8"
// short-id → "67e5"
// If the id has no hyphen, fall back to first 6 characters.
```

This exact logic is implemented inline in the Session screen and used for both the resume id and the first `thread_id` seen.

## How History and Resume provide the id

Related files:
- History store and endpoints: `expo/lib/sessions-store.ts`
- Session history detail screen: `expo/app/session/[id].tsx`
- WebSocket provider: `expo/providers/ws.tsx`

Flow:
1) The history list is fetched from the bridge’s HTTP API (`/history`). Items include an `id`, `path`, `mtime`, `title`, and `snippet`.
2) Opening a history row shows the session detail (`/session/[id]`), which loads the session transcript via `/session?id=<id>&path=<optional>`.
3) The “Continue chat” button on the detail screen calls `useWs().setResumeNextId(id)` and navigates to the live Session screen (`/session`).
4) The Session screen sees `resumeNextId` on mount and seeds the title accordingly; the final authoritative id (first `thread_id`) will replace it once the stream starts.

Note: `resumeNextId` is cleared (for payload construction) by the Projects provider when forming the next message’s JSON config line. The header value already set from the resume id is not automatically cleared; it will be updated by the first `thread` event.

## Developer pointers

- Title rendering: `expo/components/app-header.tsx`.
- Title store/hook: `expo/lib/header-store.ts` → `useHeaderTitle`.
- Session title policy: `expo/app/session/index.tsx`.
- Resume id wiring: `expo/app/session/[id].tsx` (sets `resumeNextId`), `expo/providers/ws.tsx` (holds `resumeNextId`).
- History/session APIs: `expo/lib/sessions-store.ts` (`/history`, `/session`).

If you need to change the short‑id format, update the small inline helper in the Session screen where we split on `-` or slice to 6 characters.

## Theme notes

The title color is derived from `Colors.foreground` in `AppHeader`. Do not hardcode any colors in components; add or use tokens in `expo/constants/theme.ts` per the repository policy.
