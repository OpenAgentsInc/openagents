# D1-F: lossless Codex history and subagent inspector in OpenAgents Desktop

- Issue: #8674
- Parents: #8574, #8566
- Priority: P0 Desktop D1 product slice
- Landed seam: #8673 confirmed agent timeline through Desktop Runtime Gateway
  v3 at `bf4037e923`
- Evidence:
  [`../../teardowns/2026-07-10-codex-subagents-rendering-analysis.md`](../../teardowns/2026-07-10-codex-subagents-rendering-analysis.md),
  [`../../teardowns/2026-07-10-openagents-subagents-design.md`](../../teardowns/2026-07-10-openagents-subagents-design.md)

## Outcome

OpenAgents Desktop renders the complete authorized history of a Codex
conversation—including nested subagents, reasoning summaries, tool calls and
outputs, inter-agent messages, lifecycle edges, errors, usage, and final
responses—without flattening child threads into one-line status text or hiding
completed historical agents.

“Lossless” means every supported source item is represented once at its exact
thread sequence, explicitly redacted by an existing security rule, or reported
as an unsupported/gap item. It does not claim inaccessible chain-of-thought or
content Codex itself never persisted. Unknown, corrupt, missing, or newer event
types are visible completeness failures, never silently dropped.

This extends the current local Codex-history lane. Today
`apps/openagents-desktop/src/codex-history.ts` intentionally:

- lists only top-level sessions changed in the last 24 hours;
- filters every child session;
- projects only user/assistant message rows;
- drops reasoning, tools, plans, lifecycle, collaboration, usage, and errors;
- reads a 96 KiB tail and shows five messages initially / forty after hydrate.

Those optimizations may remain for first paint and indexing, but none may
remain an invisible ceiling on inspectable history.

## Opinionated Desktop product shape

Use one coherent three-pane conversation workspace:

1. **Left — conversations.** Keep the current top-level conversation list.
   Add search/paging and an honest descendant count/activity indicator; do not
   clutter it with every child as an unrelated top-level chat.
2. **Center — selected transcript.** Render the selected parent or child thread
   in exact source order. User/assistant text, reasoning summaries, plans,
   questions, approvals, tool calls, errors, usage, collaboration edges, and
   final outcomes are distinct typed timeline items. Spawn/send/wait/resume/
   interrupt/close never disappear while in progress.
3. **Right — Agents / Item inspector.** A persistent, collapsible, resizable
   rail shows the full parent→child→grandchild tree with path or honest path-
   unavailable state, nickname/role, lifecycle, effective model/reasoning,
   last activity, and child counts. Selecting an agent focuses its transcript
   in the center while the tree remains visible. Selecting a tool/timeline item
   switches the rail to structured item details; Back returns to the exact
   prior tree selection.

The right rail is topology and inspection, not a second cramped transcript.
An **All activity** center mode may interleave the whole tree by source
timestamp, but it must preserve per-thread sequence and label concurrency; it
must not invent a total causal order across simultaneous agents.

At narrow Desktop widths the rail becomes an explicit drawer/focus mode. It
may collapse, but a visible agents/activity affordance and counts remain; no
fixed six-item preview or silent truncation is allowed. Use light dividers and
recessed nested rows rather than a card around every event. Selected/active
states use background/text contrast without font-weight shifts. Status is
never color-only, and tree/item navigation is keyboard and screen-reader
operable.

## Typed historical projection

Add one versioned, schema-decoded, owner-private history projection behind the
host-owned Desktop Runtime Gateway. Prefer Codex app-server's canonical reduced
`ThreadItem` history for archived sessions when real fixture proof shows it can
open them losslessly. Otherwise implement a structured JSONL adapter pinned to
the audited Codex rollout/app-server item versions; regex-only inference is not
the product contract.

The projection includes:

- top-level thread and every descendant thread, stable provider thread id,
  parent id, depth, V2 `agent_path` when present, nickname/role, and source
  version;
- per-thread ordered item refs/sequences/timestamps and explicit cross-thread
  collaboration edges;
- lifecycle `pending_init | running | waiting | interrupted | completed |
  errored | shutdown | not_found`, including in-progress tool state;
- effective child model, reasoning effort/summary capability, personality/role
  metadata when persisted, with absent distinguished from unknown;
- typed user/assistant/system messages, reasoning summaries, plans/todos,
  questions, permissions/approvals, tool calls, tool results, file/artifact
  refs, usage, errors, and terminal outcome;
- completeness metadata: decoded/source counts by thread, redaction count,
  unsupported/corrupt/missing-child counts, loaded page/range, total known
  bytes/items, source version, and exact gap reasons.

V1 histories without canonical paths remain an accurate parent/id tree labeled
`path unavailable`; clients do not fabricate V2 paths. Source/provider
redactions remain visibly redacted. Credentials and disallowed private fields
never cross preload or enter renderer logs/public evidence.

Keep loading lazy and bounded per request: metadata index first, then paged/
virtualized thread items and on-demand structured tool detail. Large output is
chunked with continuation and exact truncation metadata. Move CPU-heavy decode/
index work to the existing worker or planned utility process. The renderer
never receives session file paths, Node/filesystem authority, raw JSONL, raw
runtime events, credentials, or a generic IPC transport.

Local provider-native Codex history and confirmed Khala Sync agent history may
have different custody/retention, but they render through the same Effect
Native item/tree grammar. Raw local history is not uploaded or Sync-replicated
by default.

## Acceptance

1. A real top-level historical Codex session with at least two simultaneous
   children and one nested grandchild opens from the existing conversation
   list without a 24-hour or message-tail visibility ceiling.
2. The right rail shows all running, completed, interrupted, and errored
   descendants with exact parent/depth/status/config facts. Selecting every
   node opens its own complete paged transcript in the center.
3. Parent and child histories render message, reasoning-summary, plan,
   spawn/send/wait/resume/interrupt/close, tool start/result/error, approval,
   usage, and terminal outcome items once and in source thread order.
4. Tool rows expose name, status, start/end/duration when available, bounded
   structured input/output, affected files/artifacts, error/redaction state,
   and source item ref. Details survive close/reopen.
5. A completeness oracle proves for each supported fixture:
   `source items = rendered items + explicit redactions + explicit gaps`, with
   zero gaps for the pinned current Codex V1 and V2 fixture corpus.
6. Corrupt line, missing child file, unknown event/item type, unsupported
   source version, clock skew, duplicate item, and truncated output fixtures
   fail visibly without duplicating, reordering, inventing, or hiding work.
7. A large-history fixture (at least 100 MiB, 100 child threads, and 100,000
   items) preserves the existing fast metadata-first first paint; paging and
   virtualization keep navigation responsive and disclose every unloaded
   range/count.
8. Restart/reopen restores conversation, focused agent, scroll anchor, expanded
   tree nodes, selected item, filters, and completeness state without
   reclassifying history.
9. Keyboard-only and screen-reader tests cover tree traversal, expand/collapse,
   agent focus, All activity, tool detail, drawer mode, statuses, errors, and
   return to parent.
10. Effect Native owns the shared tree/timeline/inspector semantics and action
    ids. Electron remains a least-authority host; no app-local React/shadcn
    state universe or raw renderer authority is introduced.

## Ordered implementation slices

1. **D1-F1 — source contract and golden corpus:** choose canonical adapter,
   freeze types/completeness equation, and add pinned V1/V2/nested/tool/gap
   fixtures.
2. **D1-F2 — worker/gateway pagination:** index every top-level/child session,
   add graph + item-page/detail queries, schema bounds, continuation, and
   adversarial boundary tests. Consume #8673's closed query/routeRef law; claim
   any further Runtime Gateway hot files explicitly.
3. **D1-F3 — Effect Native primitives:** land reusable AgentTree,
   AgentTimelineItem, ToolCall/Result, CompletenessState, and Inspector
   semantics/renderers with accessibility and responsive behavior.
4. **D1-F4 — Desktop three-pane composition:** extend the current conversation
   UI with the right rail, agent focus, All activity, item detail, search/
   filters, restoration, and virtualized history.
5. **D1-F5 — real-history receipt:** exercise current real Codex history,
   nested subagents and tools in packaged Electron; publish only public-safe
   counts/hashes/screenshots, never private conversation/tool bodies.

## Non-goals

- inventing a second agent runtime, topology, or claim registry;
- exposing hidden chain-of-thought or bypassing Codex/source redactions;
- uploading local Codex rollouts to Khala Sync by default;
- mirroring the terminal TUI's bounded six-item status feed;
- literal cloning of the closed Codex desktop app;
- duplicating or weakening #8673's landed bounded gateway seam and routeRef law.

## Close

Close only after the real-history receipt and completeness oracle prove the
selected history can be audited end-to-end with no silent omissions. A polished
tree over a five-message/twenty-four-hour projection is not completion.
