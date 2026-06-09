# Model Lab Evidence Graph

Status: implemented for issue #382 / `OPENAGENTS-LAB-003`.

## Purpose

The Model Lab evidence graph links retained failures, signature/module
candidates, training runs, model artifacts, eval reruns, adapter validations,
and promotion gates into one inspected graph. It helps agents and operators see
why a candidate exists and what evidence supports it without allowing automatic
runtime promotion.

Implementation:

- `workers/api/src/omni-model-lab-evidence-graph.ts`
- `workers/api/src/omni-model-lab-evidence-graph.test.ts`

## Graph Records

The graph record carries:

- graph and loop refs;
- typed nodes for retained failures, candidates, training runs, model
  artifacts, eval reruns, adapter validations, and promotion gates;
- typed edges such as `derived_from`, `produced`, `evaluated_by`,
  `validated_by`, `gated_by`, and `supersedes`;
- stale evidence refs and caveats;
- blocker refs;
- rollback posture, rollback refs, and prior node refs; and
- read-only authority.

Projection timestamps use friendly labels and do not expose raw ISO strings.

## Validation Rules

The graph requires:

- at least one node of each core kind;
- edges that reference nodes in the same graph;
- every node to link to the graph loop ref;
- no duplicate node or edge refs;
- no self-loop or directed cycle;
- one connected graph;
- stale caveats when graph-level stale evidence exists;
- stale evidence refs for stale nodes;
- caveat refs for blocked nodes;
- edge and node evidence refs; and
- ready or verified rollback posture when the graph has promotion-gate edges.

## Authority Boundaries

Evidence graphs cannot:

- execute evals;
- launch training;
- call providers;
- install adapters;
- spend money;
- promote runtime behavior;
- mutate routing;
- mutate payouts;
- mutate settlement; or
- upgrade public claims.

Any execution, promotion, payment, or settlement action must happen through a
separate server-authoritative workflow with explicit receipts.

## Projection Audiences

Supported audiences are:

- `public`;
- `agent`;
- `customer`;
- `team`; and
- `operator`.

Public/customer/agent projections redact private node, edge, graph, loop,
rollback, source, stale, and evidence refs as appropriate. Operator and team
projections can retain the safe ref set, but all projections reject private
prompts, source archives, datasets, provider payloads, model weights, secrets,
payment or wallet material, private repositories, raw logs, raw traces, and raw
timestamps.

## Tests

Coverage includes:

- connected graph projection;
- same-loop node validation;
- missing-node, duplicate, cycle, and disconnected graph rejection;
- stale evidence caveats;
- rollback posture requirements;
- public redaction; and
- hard false eval, training, provider, adapter, payment, runtime-promotion,
  routing, payout, settlement, and public-claim mutation authority.
