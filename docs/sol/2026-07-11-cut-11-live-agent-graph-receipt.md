# CUT-11 canonical live-agent graph receipt

- Date: 2026-07-11
- Issue: [#8691](https://github.com/OpenAgentsInc/openagents/issues/8691)
- Status: shared schema/replay, provider normalization, and Khala Sync entity/
  projection tranches complete; live producer/changelog bindings, Runtime
  Gateway emission, and live traces pending
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
post-image per `scope.thread.<sessionRef>`, keyed by stable `graphRef`. Creation,
advance, JSON encoding, and JSON decoding all re-run the shared graph laws.
Advancement accepts only the shared exact next-cursor reducer, so bootstrap or
reconnect needs no provider history and cannot silently accept a stale/gapped
provider patch.

The post-image test reaches the schema maximum of 2,000 nodes, round-trips it
through the exact stored JSON bytes, and rejects node 2,001. This is the durable
entity/projection boundary; no server changelog producer is claimed yet.

## Verification

- `@openagentsinc/agent-runtime-schema`: 35 pass, 0 fail, 256 expectations;
  typecheck passes.
- CUT-11 focused graph corpus: 11 pass, 108 expectations.
- `@openagentsinc/khala-sync`: 178 pass, 0 fail, 2,601 expectations;
  typecheck passes.
- CUT-11 focused Sync post-image corpus: 3 pass, 11 expectations.
- A deterministic property corpus applies 50 independently shuffled batches
  of 64 child nodes and parent edges; every result is byte-identical.
- Negative cases cover malformed/private-shaped refs, missing explicit facts,
  exact-replay conflict, gap, stale generation, cursor/timestamp regression,
  terminal mismatch/reopen, missing/mismatched parent, orphan tool, and cycle.

## Residual

This receipt does not claim that live Codex or Claude producers currently emit
the graph or write the Sync changelog. Next CUT-11 tranches must bind the typed
adapters to those producers, commit the registered post-image through the
server authority, emit it through Runtime Gateway, prove live reconnect, and
attach redacted named-account traces for both providers. Graph presentation
remains CUT-12.
