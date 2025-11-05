## Summary
- Purpose: describe how the iOS app performs “initial hydration” of a chat thread and practical strategies to optimize perceived load time and memory/CPU usage.
- Status: implemented; first frame shows immediately, heavy timeline compute now runs off the main thread.

## Current Flow
- App launch
  - `OpenAgentsApp` registers fonts and starts `BridgeManager` in a `.task {}`.
- Bridge connect
  - iOS client connects to the desktop bridge via WebSocket (Bonjour or configured host).
  - On connect, the client sends `thread/load_latest` with params to request a larger window: `limit_lines: 16000`, `max_bytes: 1500000` (server may cap tighter).
- Server response
  - Desktop bridge tails the thread JSONL, prunes heavy payloads, then caps to ~900KB before sending the JSON‑RPC result.
- Initial hydrate in app
  - `BridgeManager.latestLines` is updated with the full JSONL line array (mobile snapshot of the thread).
  - `AcpThreadView` observes `latestLines` and offloads the heavy parse to a background queue:
    - `AcpThreadView_computeTimeline` translates lines → timeline items (user/assistant messages, reasoning summaries, raw JSON events for tools/other).
    - “Initial metadata” lines are siphoned into an info bucket (hidden from the main feed) using heuristics:
      - `type == "turn_context"`.
      - `event_msg.payload.type ∈ {token_count, session_meta}`.
      - or `payload` includes recognizably meta keys: `cwd`, `git`, `cli_version`.
      - or a nested `instructions` field anywhere in `payload`.
  - Results are published back to the main thread and the view renders.

## Limits and Caps
- Server tail window: up to 16k lines pre‑prune (configurable), then aggressively pruned and byte‑capped to keep transport < ~1MB.
- App timeline cap: 400 items (configurable via `AcpThreadView.maxMessages`).
- Raw JSON preview inline: 5 lines; tap opens a sheet with full pretty‑printed JSON.

## Rendering Notes
- Messages: user and assistant only (system/preface hidden). User text slightly lighter; assistant darker.
- Reasoning: coalesced into “Thought for Xm Ys” glass capsule. Tap opens a sheet with the raw thought chunks.
- Raw events: tool calls/results and other non‑message events are shown as raw JSON blocks (truncated) unless filtered as metadata.
- Markdown: custom bullet/number parser for lists; inline emphasis via `AttributedString`.

## Known Costs / Hotspots
- Parsing and building the timeline for large snapshots is CPU‑heavy:
  - Deep JSON scans (timestamps, metadata detection).
  - Markdown inline parsing on many items.
  - Pretty‑printing JSON for previews/sheets.
- UI work is now off‑main for compute, but publishing large arrays still incurs diffing/layout.

## Near‑Term Optimizations (Low Risk)
- Progressive hydrate
  - Load last N messages first (e.g., 60–100), render immediately, then page older messages on demand (“Show older”).
  - Keep current wide server window for power users but default the app to a smaller initial slice.
- Chunked timeline insert
  - Insert items in batches (e.g., 100 at a time) with a brief `DispatchQueue.main.async` yield between batches to keep UI responsive.
- Cache and reuse
  - Cache the last computed timeline to disk (e.g., JSON with minimal shape) keyed by thread id + last file mtime; on cold start, render cached timeline instantly while new data streams in.
- Inline JSON preview caching
  - Memoize `prettyJSON(line)` results for visible items to avoid re‑pretty‑printing during scrolling.
- Parser shortcuts
  - Stop timestamp/meta deep‑scans early once found; whitelist likely keys first.
- Markdown guardrails
  - Skip `AttributedString(markdown:)` for lines without markdown characters; render plain `Text` to reduce parser cost.

## Medium‑Term Optimizations
- Server‑side precompute (bridge)
  - Produce a lightweight “timeline” wire format (messages, reasoning windows, summarized tool results) alongside JSONL. The app would use this directly for initial render.
  - Keep full JSONL available for detail sheets / “View raw”.
- Compression
  - Negotiate per‑message compression (e.g., permessage‑deflate) for the JSON‑RPC payload.
- Streaming hydrate
  - Stream the timeline in logical sections (metadata → last 100 items → previous 500, etc.) and append as they arrive.

## Instrumentation & Monitoring
- OSLog signposts around:
  - WS connect → latest thread response → timeline parse start/finish → first content drawn.
- Simple counters/timers printed in DEBUG for:
  - Parsed line count, items created, ms spent in translation, markdown parse/pretty‑print counts.
- Use Instruments (Time Profiler + SwiftUI) to check for long main‑thread blocks and layout thrash.

## Operational Tweaks
- Adjustable caps via a Feature/Settings flag to trade fidelity vs. startup time.
- Fallback path if payload exceeds mobile limits (show an infobar allowing the user to fetch fewer lines or only recent messages).

## Summary / Action Items
- Move to progressive hydrate by default (render last N quickly, page older on demand).
- Add batch insertion to keep main thread free during large initial renders.
- Cache last timeline and reuse on cold starts.
- Add DEBUG signposts to confirm there are no remaining main‑thread stalls.

