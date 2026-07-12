# CUT-11 canonical live-agent graph receipt

- Date: 2026-07-11
- Issue: [#8691](https://github.com/OpenAgentsInc/openagents/issues/8691)
- Status: shared schema/replay, provider normalization, Khala Sync entity,
  named server writer, live Codex/Claude root transaction binding, and confirmed
  Runtime Gateway v8 delivery/reconnect complete; real Claude child topology is
  active and redacted named-provider source traces exist for both providers,
  while Codex child transport convergence and a named end-to-end reconnect
  trace remain pending
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

## Redacted named-provider source traces

Two bounded owner-local runs used isolated named-account custody and retained
only stable hashes, typed lifecycle values, and exact token totals. No prompt,
path, account name, provider payload, child response, or credential was
retained.

The Claude Agent SDK run selected
`account.pylon.claude_agent.ba1fd0827726ff7f618c7725` and produced:

- session ref `session.claude.779e1c5bded92ef237595a14`;
- one stable task ref `task.claude.6b175360c64c54c968c1cd7f`;
- `task_started` followed by `task_notification(completed)`; and
- exact usage of 7 input + 516 output = 523 tokens.

The live SDK used `task_type: local_agent` plus a present `subagent_type`, not
the older `task_type: subagent` spelling. The producer already uses the
presence of `subagent_type` as the provider-stable child discriminator. A
different home that the local readiness projection called ready was refused by
the provider's organization policy before inference; its error was retained
only as `blocker.cut11.named_claude.9c2eba1b8800f2cefd52ac1b`. This is a
readiness-freshness finding, not a fabricated successful trace.

The Codex run selected the separately ready default-home account
`account.pylon.codex.6be7b6501be36164f9c6ecda` and completed through the real
app-server transport with:

- root thread ref `thread.codex.9970eecd5d1b682060e69071`;
- turn ref `turn.codex.03905fe35f310d794a60dc45`;
- child thread ref `thread.codex.00f18bdc71240dd9b2e30be7` from the typed
  `subAgentActivity(started)` record; and
- a typed `collabAgentToolCall(wait)` in-progress/completed pair before the
  root completed.

That run establishes that the current app-server has a real typed child source.
It also locates the remaining transport defect precisely: the installed SDK's
bundled Codex binary fails before emitting a frame, while the current PATH
binary succeeds but its `codex exec --experimental-json` encoder drops
`subAgentActivity` and emits only `collab_tool_call(wait)` without a receiver
ID. The app-server source must therefore enter Pylon's one conversation service
or the exec encoder must preserve the child record. History/tool-name parsing
is not an acceptable substitute. This follows the Pylon streamlining audit's
one-conversation-service direction rather than introducing another provider
sidecar.

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

## 2026-07-12 update: app-server source convergence and desktop live wiring

Two of the three remaining residuals landed on `main` (session
`cut11-main-wiring-20260712`):

Desktop main-process live emission is wired. `live-agent-graph-host.ts` owns
per-thread `createLocalAgentGraphAssembler` instances and is fed by exactly
the one-callback wiring the assembler landing named: `beginTurn` before each
fable-local / codex-local `runTurn` plus one `applyEvent` line inside each
lane's existing emit callback, so the canonical graph is assembled from the
SAME typed envelopes the renderer stream receives. Delivery is additive IPC
on the established seams — broadcast push on change
(`openagents:live-agent-graph:update`) and renderer-argument-free snapshot on
invoke (`openagents:live-agent-graph:snapshot`) — with shared-law
revalidation (`decodeLiveAgentGraphEntity`) on the renderer side of the
preload. Retention is bounded to eight thread graphs; a graph with a running
turn is never evicted. Tests drive the REAL emit path: an actual
`makeFableLocalRuntime` turn with a REAL `makeCodexChildRuntime` delegate
child on fixture spawns, and an actual `makeCodexLocalRuntime` turn on
fixture exec stdout.

Pylon converged on the typed Codex app-server child source through the one
conversation service. `codex-app-server-source.ts` is a minimal JSONL
JSON-RPC client for `codex app-server` (initialize/initialized,
`thread/start` or `thread/resume`, `turn/start` with the same owner-local
never/danger-full-access posture as the SDK path) that adapts v2
notifications into the SAME `CodexRawEvent` stream shape the exec encoder
produces — while preserving the typed `subAgentActivity` and
receiver-bearing `collabAgentToolCall` items the exec encoder drops.
`codexRawEventToRuntimeEvents` now accepts both item-type spellings and
normalizes those child records into the shared body-free `agent.child.*`
contract (stable ids keyed by the provider child thread id, nested
sender-parenting, terminal collab-state finish reasons). The dispatch
default is app-server-first with EXACTLY one fallback to the exec SDK on a
typed pre-frame failure (spawn/handshake/typed pre-turn error — the
probe-located bundled-binary failure mode); post-frame failures never
re-execute, `turn/start` timeouts stay fatal, and unexpected server->client
requests are refused fail-closed. The exec encoder's receiver-less
`collab_tool_call` record still yields nothing: no receiver id, no honest
child identity, and tools/history remain forbidden parentage sources.

Verification: focused app-server source corpus (7 pass, 22 expectations,
including one end-to-end fold of the adapted stream through the REAL
translation producing `agent.child.started`/`agent.child.finished`);
enforcement translation corpus extended for both spellings, the
subAgentActivity lifecycle, collab receivers, nested parenting, and the
receiver-less negative (64 pass, 223 expectations); Pylon and Desktop
typechecks and full suites green at landing (exact counts in the landing
commits on #8691).

## 2026-07-12 named live-turn probe (partial; reconnect legs deploy-gated)

With ready isolated-registry accounts present (Codex `codex`/`codex-5`,
Claude `claude-pylon-2`/`claude-pylon-3`), a fresh throwaway thread was
created against production and a real `runtime.startTurn`
(`target.lane: claude_pylon`) was pushed with the owner-linked Pylon agent
credential. The turn executed end-to-end as a REAL named-account Claude
run through the durable production Sync path:

- thread scope `scope.thread.e02d7fcd-7bfc-4865-a1ea-73de983bdd43`, turn
  `94e068a2-e462-477c-b198-4348fb0cb5e3`, intent feed seq 84;
- the single-winner turn-claim admission held live under real concurrency:
  a second bounded consumer observed the typed
  `skipped_stale (runtime turn claim was not newly admitted)` refusal while
  the winning owner-local consumer executed the turn — no double dispatch;
- changelog cursors 1-8 carry `chat_thread`/`chat_message`, the intent, the
  claim (`turn.started` seq 1), `text.delta`/`text.completed`,
  `usage.recorded` (3 input + 13 output, 16,823 cache-write, exact
  `totalTokens: 16`), and `turn.finished(stop)`; `runtime_turn` settled
  `completed` with `eventCount: 5`, all read back over the authenticated
  Sync log API. No prompt text, credential, or provider payload was
  retained in this receipt.

The confirmed-graph reconnect legs could NOT complete: the deployed
production API predates the CUT-11 server graph binding, so this live turn
produced no `live_agent_graph` changelog rows to reconnect against (the
binding is proven by real-Postgres server tests on `main`). The remaining
residual is therefore deploy-gated, not code-gated.

## 2026-07-12 closure: deployed named confirmed reconnects

Production revision `openagents-monolith-00085-k4v` serves 100% of traffic;
health, the Cloud Run document, and its exact hashed JavaScript asset returned
200. Fresh private thread scopes then carried both named provider lanes through
the deployed `live_agent_graph` writer and a new exact-cursor reader:

- Claude completed at scope version 8 with one completed root, graph cursor 5,
  and reconnect cursor 8/up-to-date.
- Codex reached the real app-server and settled failed at scope version 5 with
  one failed root, graph cursor 2, and reconnect cursor 5/up-to-date. The
  provider terminal was `usage_limited` with a provider-reported reset at
  05:00 local time. This is a real failure lifecycle trace, not a successful
  inference claim. The earlier named Codex source receipt above already proves
  successful app-server child activity.

The live pass found and fixed three production-only convergence defects:
Pylon allocated sequence 1 while the server correctly expected first sequence
0; the consumer ignored the already-typed exact account target; and Codex
app-server expects sandbox variant `dangerFullAccess`, not the CLI spelling.
Regression oracles now pin all three. The redacted refs-only receipt is
[`2026-07-12-cut11-named-confirmed-reconnect.json`](../../apps/pylon/docs/proofs/2026-07-12-cut11-named-confirmed-reconnect.json).
No prompt, credential, account name, provider payload, or host path is retained.

CUT-11 has no remaining code or trace criterion. Graph presentation remains
CUT-12.
