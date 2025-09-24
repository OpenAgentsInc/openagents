# Codex Chat UI: Streaming Events, Session Metadata, and What to Render

This guide summarizes how to build a chat UI on top of Codex. It explains what
streams in during a turn, how to map events to UI, and what metadata exists at
the session/conversation level for status and history.

## Data Flow Overview
- User input is converted into a `Prompt` (list of `ResponseItem`s) and sent to
  the model client.
- Codex emits a `ResponseStream` of `ResponseEvent` values as the model responds
  and tools run. The orchestrator translates these into UI updates and also
  persists them to a rollout JSONL file.

## Streaming Events to Render
Key variants from `ResponseEvent` (see docs/codex/core-client-common.md):
- Created — turn started; clear transient UI state.
- OutputTextDelta(String) — incremental assistant text. Append to the visible
  assistant message.
- ReasoningSummaryDelta(String) — high‑level summary chunks (show only if
  `hide_agent_reasoning` is false; otherwise suppress or show in a collapsible).
- ReasoningContentDelta(String) — raw chain‑of‑thought text; only show if
  `show_raw_agent_reasoning` is true.
- OutputItemDone(ResponseItem) — a complete item (assistant message, function
  call, local shell call, etc.). Finalize the message or show a tool row with
  its inputs/outputs.
- RateLimits(RateLimitSnapshotEvent) — update a small rate‑limit widget.
- Completed { response_id, token_usage } — end of stream. Persist token usage
  (input/output/total) and mark the assistant message complete.

Exec/tool streaming (see docs/codex/core-exec-streaming.md):
- While a tool runs, deltas are emitted as `ExecCommandOutputDeltaEvent` with a
  `call_id`/`sub_id`. UI should multiplex by `call_id`, appending stdout/stderr
  chunks to the tool’s live output. A final function‑call output is emitted as a
  `ResponseItem` when the tool completes.

## Conversation vs. Session
- Session (rollout file): A single continuous run written to
  `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` starting with a
  `SessionMetaLine`. It contains: timestamp, `id` (session_id), `cwd`, cli
  version, optional git info.
- Conversation: The in‑memory list of items (`ConversationHistory`) that grows
  turn‑by‑turn within a session. Compaction may insert summaries/bridges.
- UI guidance:
  - Use `session_id` for linking to persisted logs.
  - Treat conversation items as the chat transcript; each turn adds items:
    user text → streaming assistant deltas → tool calls/results → final
    assistant item.

## Turn Lifecycle (What to expect)
1) Created
2) Zero or more OutputTextDelta/Reasoning* deltas
3) Zero or more tool calls (local shell, function, custom, web‑search), with
   their own streaming deltas if applicable
4) One or more OutputItemDone(ResponseItem) finalizers
5) Completed with optional TokenUsage

## Recommended UI Structure
- Transcript list of items:
  - User message bubble
  - Assistant message bubble that grows with OutputTextDelta
  - Tool call blocks with status (running/complete), streamed stdout/stderr,
    and a final result summary when OutputItemDone arrives.
- Sidebar/status area:
  - Workspace (cwd), approval policy, sandbox mode, AGENTS.md presence
  - Account: auth method, email, plan
  - Model: name/provider, reasoning effort/summary prefs
  - Token usage (from Completed)
  - Session id link to open the rollout file
- Optional: rate limit widget showing the latest `RateLimitSnapshotEvent`.

## Reasoning Visibility
- `Config.hide_agent_reasoning` — when true, suppress reasoning deltas in UI.
- `Config.show_raw_agent_reasoning` — when true, allow raw reasoning content in
  a dedicated collapsible.

## Persisted History & Resume
- Rollouts (docs/codex/core-rollout.md): each line is a `RolloutItem` with a
  timestamp. A UI can read this to render transcript after the fact or to
  resume.
- Conversation compaction (docs/codex/core-conversation-history.md) may inject a
  summary + history bridge; UI should render these as system items.

## Minimal Event‑Handling Contract for a Chat UI
- On submit: append a user item; subscribe to `ResponseStream`.
- For each event:
  - OutputTextDelta: append text to the pending assistant bubble.
  - Reasoning*Delta: show/hide based on config flags.
  - OutputItemDone: if message → finalize; if tool → render tool card with
    inputs/outputs; if custom/web search → render accordingly.
  - Exec deltas: route by `call_id` to the correct tool card, appending output.
  - RateLimits: update header widget.
  - Completed: finalize bubble; store `response_id` and `token_usage` on the
    turn; update sidebar totals.

## References
- Streaming: docs/codex/core-sse.md, docs/codex/chat-completions.md
- Orchestration: docs/codex/core-codex.md
- Protocol types: docs/codex/protocol-overview.md
- Exec streaming: docs/codex/core-exec-streaming.md
- Rollouts & sessions: docs/codex/core-rollout.md
- Conversation history/compaction: docs/codex/core-conversation-history.md

