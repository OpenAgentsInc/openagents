# CUT-11 canonical live-agent graph receipt

- Date: 2026-07-11
- Issue: [#8691](https://github.com/OpenAgentsInc/openagents/issues/8691)
- Status: shared schema/replay, provider normalization, Khala Sync entity,
  named server writer, live Codex/Claude root transaction binding, and confirmed
  Runtime Gateway v8 delivery/reconnect complete; real Claude child topology is
  active, while a typed Codex child source and named-account traces remain
  pending
- Contract: `openagents.live_agent_graph.v1`

## Registered graph facts

The canonical provider-neutral graph now types:

- stable graph/session/agent/thread/transcript/run and edge refs;
- explicit root, known-parent, or loss-accounted unknown parentage;
- known-or-unknown provider, runtime, worktree, and current-tool facts;
- queued/running/waiting/terminal/unknown status, attention, terminal reason,
  timestamps, attachment generation, per-agent activity cursor, and version;
- stable parent and tool edges; and
- one bounded graph snapshot plus exact-cursor delta.

Provider omission is never encoded as an absent or fabricated fact. Unknowns
name why the fact is unavailable. Public refs cannot contain paths or arbitrary
text, and the schema contains no credential/provider payload field.

## Deterministic laws

The reducer:

- treats an exact last-delta replay as idempotent;
- requires one exact next graph cursor and attachment generation;
- rejects conflicting stale replay and cursor gaps;
- prevents stable node/edge identity changes and node/edge version regression;
- prevents per-agent activity-cursor and timestamp regression;
- prevents terminal agents from reopening or changing terminal class;
- requires parent facts and parent edges to agree exactly;
- rejects missing parents, multiple parents, orphan parent/tool edges, and
  cycles; and
- sorts nodes/edges by stable ref so source/provider delivery order cannot
  change the canonical snapshot.

## Provider normalization

Two typed observation contracts now terminate provider vocabulary at the
shared boundary:

- Codex app-server `notStarted`, `inProgress`, `waitingForInput`, terminal,
  and tool-call states map into the closed canonical state set.
- Claude Agent SDK `queued`, `running`, `waiting_for_permission`, terminal, and
  tool-use states map into the same set.

Both adapters emit graph-valid roots, children, parent edges, and tool edges.
They use provider-specific identity/runtime refs without leaking raw payloads.
If either source omits parent, worktree, attention, or current-tool facts, the
adapter emits the named unknown reason instead of copying the other provider or
fabricating parity. Terminal observations without an end time and non-terminal
observations with one fail closed.

## Khala Sync post-image

`@openagentsinc/khala-sync` now registers `live_agent_graph` as one full
post-image per canonical `scope.thread.<threadRef>`, keyed by stable `graphRef`.
The top-level `sessionRef` and canonical `threadRef` are distinct, and provider-
native node thread refs never choose the authorization scope. Creation,
advance, JSON encoding, and JSON decoding all re-run the shared graph laws.
Advancement accepts only the shared exact next-cursor reducer, so bootstrap or
reconnect needs no provider history and cannot silently accept a stale/gapped
provider patch.

The post-image test reaches the schema maximum of 2,000 nodes, round-trips it
through the exact stored JSON bytes, and rejects node 2,001.

## Server changelog writer

`@openagentsinc/khala-sync-server` now exposes one named system writer that:

- decodes and validates the entire graph before storage;
- refuses secret-, credential-, email-, or host-path-shaped structural
  material with a bounded diagnostic;
- derives scope only from canonical `threadRef`;
- appends `live_agent_graph` / `graphRef` through the standard transactional
  writer, preserving dense per-scope versions; and
- fails soft until live provider observation and session business authority
  share one transaction.

A real throwaway-Postgres receipt appends two full post-images at versions 1
and 2, with the registered system-writer ref, and decodes the stored bytes back
through the graph contract.

## Live runtime transaction binding

The existing `runtime.startTurn`, runtime control, and `runtime.recordEvent`
server transaction now calls the same graph adapters and appends the graph with
the business mutation writer. There is no second poller or provider-history
store:

- `codex_app_server` maps through the Codex observation adapter;
- `claude_pylon` maps through the Claude Agent SDK observation adapter;
- queued roots start with explicit unknown provider identity;
- only a real runtime event `source.providerRef` upgrades that identity;
- later provider-omitted events preserve an already observed identity;
- the provider event count remains the per-agent activity cursor while every
  graph change advances its own cursor/version; and
- retrying after terminal creates a new attachment generation/root attempt
  rather than reopening the terminal node.

Real-Postgres tests prove a Codex root reaches running with the observed named
provider ref and a Claude root reaches completed/terminal through the same
writer. These are deterministic provider-shaped fixtures, not named-account
live traces.

## Typed Claude child topology

The shared runtime-event contract now includes body-free
`agent.child.started`, `agent.child.progress`, and `agent.child.finished`
records. They carry stable child/run/parent/task refs, terminal reason, and
optional bounded usage—not task prompts, descriptions, result summaries,
output paths, or provider payloads.

The Pylon Claude producer maps the Agent SDK's real `system/task_started`,
`task_progress`, and `task_notification` messages only when the task is a
subagent. Stable task correlation survives all three records; background shell
tasks and uncorrelated notifications are ignored. The server transaction then:

- adds one stable Claude child and parent edge;
- advances only the child activity cursor/version for later child records;
- retains the child through root tool/text changes and root terminal state;
- refuses terminal reopening through the existing graph laws; and
- removes the prior attempt's children when retry advances attachment
  generation.

The installed `@openai/codex-sdk` 0.139.0 public `ThreadEvent` / `ThreadItem`
union exposes no child or subagent record. Codex live child production is
therefore still explicit provider-unsupported; CUT-11 does not infer parentage
from tool names, renderer cards, or historical transcript text.

## Confirmed client and Runtime Gateway delivery

The Khala Sync client now reads `live_agent_graph` post-images only from the
exact canonical thread scope and only while that scope is live. It revalidates
every graph, ignores malformed or cross-thread rows, and emits the newest
bounded set: at most eight graphs and at most 2,000 nodes / 4,000 edges in
aggregate.

Runtime Gateway protocol v8 carries those post-images and matching `graphRefs`
inside the existing `conversation.subscribe` update. This deliberately reuses
the thread subscription's durable Sync cursor, generation fence, serialized
backpressure, and exact unsubscribe. It adds no graph poller, provider-history
store, socket, or raw provider event surface.

Deterministic reconnect coverage proves both cases:

- an exact cursor resume emits the current confirmed graph set; and
- a proven cursor gap emits one newest `authoritative_refetch` snapshot.

An interrupted/non-live scope emits no cached graph authority. This is a
deterministic protocol receipt, not the still-required redacted named-account
Codex/Claude trace.

## Verification

- `@openagentsinc/agent-runtime-schema`: 35 pass, 0 fail, 261 expectations;
  typecheck passes.
- CUT-11 focused graph corpus: 11 pass, 111 expectations.
- `@openagentsinc/khala-sync`: 178 pass, 0 fail, 2,601 expectations;
  typecheck passes.
- CUT-11 focused Sync post-image corpus: 3 pass, 11 expectations.
- CUT-11 focused Sync server writer: 4 pass, 18 expectations; typecheck passes.
- Runtime transaction binding: 12 pass, 87 expectations against real
  throwaway Postgres; server typecheck passes.
- Claude child producer focus: 57 pass, 199 expectations; Pylon typecheck
  passes. It covers both direct task normalization and the dispatch call site.
- Full Pylon verification: 2,374 pass, 3 opt-in live skips, 0 fail, 12,143
  expectations; the supervisor-store bypass check and typecheck pass.
- Server writer + runtime/child binding focus: 16 pass, 105 expectations
  against real throwaway Postgres.
- Confirmed client graph/read/reconnect focus: 13 pass, 49 expectations;
  client typecheck passes.
- Full `@openagentsinc/khala-sync-client`: 173 pass, 3 opt-in live-smoke
  skips, 0 fail, 12,729 expectations; import-coverage check passes.
- Runtime Gateway graph boundary and existing no-poll consumer focus: 40 pass,
  151 expectations; Desktop typecheck passes.
- Full Desktop verification: 343 pass, 0 fail, 1,725 expectations; typecheck,
  build, protocol-v8 Electron smoke, and teardown with zero active host
  subscriptions pass.
- Shared-contract mobile compatibility: 66 pass, 0 fail, 287 expectations;
  mobile typecheck passes. This tranche does not yet present graph UI on mobile.
- Full `@openagentsinc/khala-sync-server`: 510 pass, 1 fail, 4,543
  expectations. The existing `runtime-intents.test.ts` event-count case returns
  three in-band rejections instead of its expected applies and fails identically
  when rerun alone; that reader path and its test are unchanged by this tranche.
- A deterministic property corpus applies 50 independently shuffled batches
  of 64 child nodes and parent edges; every result is byte-identical.
- Negative cases cover malformed/private-shaped refs, missing explicit facts,
  exact-replay conflict, gap, stale generation, cursor/timestamp regression,
  terminal mismatch/reopen, missing/mismatched parent, orphan tool, and cycle.

## Residual

This receipt does not claim Codex live child/subagent topology or named-account
live traces. The remaining CUT-11 work must bind a real typed Codex app-server
child source and attach redacted named-account Codex and Claude reconnect
traces. Graph presentation remains CUT-12.
