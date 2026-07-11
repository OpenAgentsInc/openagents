# CUT-11 canonical live-agent graph receipt

- Date: 2026-07-11
- Issue: [#8691](https://github.com/OpenAgentsInc/openagents/issues/8691)
- Status: shared schema/replay tranche complete; provider adapters, durable
  projection, Runtime Gateway emission, and live traces pending
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

## Verification

- `@openagentsinc/agent-runtime-schema`: 30 pass, 0 fail, 220 expectations;
  typecheck passes.
- CUT-11 focused graph corpus: 6 pass, 72 expectations.
- A deterministic property corpus applies 50 independently shuffled batches
  of 64 child nodes and parent edges; every result is byte-identical.
- Negative cases cover malformed/private-shaped refs, missing explicit facts,
  exact-replay conflict, gap, stale generation, cursor/timestamp regression,
  terminal mismatch/reopen, missing/mismatched parent, orphan tool, and cycle.

## Residual

This receipt does not claim that Codex or Claude currently emits the graph.
Next CUT-11 tranches must map both providers into equivalent node/edge/delta
semantics, persist/project the graph through the canonical Sync authority,
emit it through Runtime Gateway, prove replay identity at scale, and attach
redacted named-account traces for both providers. Graph presentation remains
CUT-12.
