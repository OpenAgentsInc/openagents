# Retrieval Trace And Graph Context

Status: implemented for issue #370 / `OPENAGENTS-LATE-010`.

## Purpose

Retrieval and context selection need to be inspectable. This contract records
which sources were selected, which were excluded, why context is missing, and
how graph-curated facts relate back to source and span refs.

Implementation:

- `workers/api/src/omni-retrieval-trace-context.ts`
- `workers/api/src/omni-retrieval-trace-context.test.ts`

## Retrieval Trace

A trace records:

- workroom ref;
- query intent ref;
- query digest ref;
- selector kind;
- selector model ref;
- source bundle refs;
- selected source hits;
- excluded source hits;
- missing context items;
- graph nodes and edges;
- human-confirmed facts;
- generated summary refs;
- provenance refs;
- caveat refs; and
- redaction policy refs.

The selector kind is structured. Supported values include embedding similarity,
semantic selector, structured query plan, graph expansion, hybrid
semantic-graph, and manual pin. The contract rejects ad hoc keyword-only
selector refs.

## Source Hits

Selected and excluded hits are separate arrays. Each hit includes:

- source ref;
- source bundle ref;
- span refs;
- rank;
- score in basis points;
- freshness;
- provenance refs;
- rights ref;
- caveat refs; and
- optional exclusion reason kind and reason ref.

Selected hits cannot carry exclusion reasons. Excluded hits must carry both an
exclusion reason kind and reason ref.

Freshness labels make stale memory visible instead of silently mixing stale
context into current work.

## Missing Context

Missing context items represent work the agent or operator still needs before a
claim can be trusted:

- needed source;
- stale memory;
- rights blocked;
- private source;
- unclear query; and
- contradiction.

Each item includes a label ref, reason ref, and required-for ref.

## Graph Context

Graph nodes can represent agents, claims, customers, facts, repos, Sites,
sources, spans, and workrooms. Graph edges can support, contradict, derive
from, mention, require, supersede, or human-confirm a relationship.

Nodes and edges must be backed by source refs, span refs, or explicit human
confirmation refs. Fact nodes must be connected by at least one edge.

Human-confirmed facts must include human confirmation refs. Candidate facts can
remain source-backed without being promoted.

## Projection Audiences

The first projections are:

- `public`;
- `team`; and
- `operator`.

Public projections redact private source, span, graph, fact, confirmation,
summary, rights, reason, intent, and workroom refs. Team projections can retain
more internal context but still remove private source/span/fact/confirmation
refs. Operator projections can see the full safe ref set.

Counts reflect visible projected records after redaction.

## Authority Boundaries

Retrieval traces are read-only. They cannot:

- autonomously fetch sources;
- promote facts into workroom objects;
- mutate generated summaries;
- mutate graph state; or
- upgrade public claims.

Those actions require later approval-gated write paths.

## Tests

Coverage includes:

- selected/excluded source projection;
- stale selected source counts and missing context labels;
- graph node/edge source support requirements;
- human-confirmed fact confirmation requirements;
- public redaction of private refs; and
- unsafe rank, score, selector, source, timestamp, and authority rejection.
