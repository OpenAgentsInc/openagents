# Semantic Retrieval And Search Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #19 from the Bun/Effect terminal-agent systems list. It defines
how a terminal coding agent should search files, code, transcripts, commands,
memory, tools, and project knowledge without relying on brittle keyword-only
routing.

## Target

Build one retrieval layer that can combine exact search, structured parsing,
semantic ranking, and model-assisted selection behind typed contracts.

Search should be fast enough for interactive use and disciplined enough that it
does not leak private data, overfill context, or route user intent through ad
hoc string checks.

## User-Visible Capability

The user should be able to:

- Search the current repo by name, content, symbol, or intent.
- Find relevant prior conversation context.
- Retrieve project memory without manually naming a file.
- Discover commands and tools by capability.
- See why a result was selected or skipped.
- Use exact search when they ask for exact text.
- Use semantic search when they ask for concepts.
- Avoid repeated irrelevant context injection.

The agent should search deliberately and explainably.

## Retrieval Modes

Use a closed set of retrieval modes:

- `exactFile`: path, glob, and filename search.
- `exactContent`: regex or literal content search.
- `structured`: parsed ids, emails, dates, symbols, command names, and enums.
- `semantic`: embedding-ranked documents or chunks.
- `modelSelected`: bounded model selection over prefiltered candidates.
- `hybrid`: exact candidate generation plus semantic or model ranking.

The runtime should choose a mode through a typed planner, not scattered string
conditions.

## Core Design

Define a `RetrievalService` that owns search planning, execution, ranking, and
context projection.

Suggested service boundary:

```ts
interface RetrievalService {
  plan(request: RetrievalPlanRequest): Effect.Effect<RetrievalPlan, RetrievalError>
  search(plan: RetrievalPlan): Stream.Stream<RetrievalCandidate, RetrievalError>
  rank(request: RetrievalRankRequest): Effect.Effect<RetrievalResultSet, RetrievalError>
  project(request: RetrievalProjectionRequest): Effect.Effect<ContextSlice, RetrievalError>
}
```

Tool discovery, memory retrieval, transcript search, and file search can use
different adapters, but should return the same candidate shape.

## Candidate Shape

Every candidate should include:

- Candidate id.
- Source kind.
- Source ref.
- Title or path.
- Snippet or summary.
- Rank score.
- Match mode.
- Provenance.
- Freshness.
- Visibility.
- Size estimate.
- Redaction class.
- Skip or inclusion reason.

The context assembler should consume candidates through `ContextSlice`, not raw
search output.

## Source Adapters

Initial adapters:

- Filesystem exact path and glob search.
- File content search.
- Code-aware symbol search when language services are available.
- Transcript search over user-visible text.
- Command-history search.
- Memory header and body retrieval.
- Tool and command capability search.
- Documentation index search.

Each adapter should declare permissions, size caps, ignored paths, and whether
it can return private content.

## Planning Rules

Planning should follow these rules:

- Use exact search for quoted strings, concrete paths, ids, and explicit regex.
- Use structured parsing for bounded fields after the route is selected.
- Use semantic retrieval for conceptual or capability questions.
- Use hybrid retrieval when user wording is ambiguous.
- Prefer small headers before full bodies.
- Limit model-assisted selection to prefiltered candidates.
- Include source and freshness metadata with every chosen result.
- Return no-result evidence instead of fabricating recall.

Avoid hidden keyword maps for user-facing intent routing.

## Ranking Rules

Ranking should combine:

- Exactness.
- Source authority.
- Recency.
- Workspace proximity.
- User-visible relevance.
- Prior successful use.
- Context size.
- Privacy class.
- Staleness penalty.

The ranker should be configurable but deterministic for tests. Model-assisted
ranking should emit structured output and be bounded by candidate and token
limits.

## Bun/Effect Boundary

Use these primitives:

- `Effect.Service` for retrieval planning and result projection.
- `Schema` for plans, candidates, scores, result sets, and context slices.
- `Stream` for large result sets and incremental search.
- `Layer` for file, memory, transcript, embedding, and language adapters.
- `Cache` for indexes and normalized candidate text.
- `Queue` for background indexing events.
- `Schedule` for index refresh and transient adapter retries.

Embedding or AI-SDK style message shapes can be adapter details. The owned
runtime should persist its own retrieval schema.

## Safety Rules

- Do not search outside allowed workspace roots.
- Do not expose private transcript or memory results in public receipts.
- Do not include raw large files when a snippet or summary is enough.
- Do not run semantic routing with unbounded candidate pools.
- Do not treat model-selected candidates as authoritative without provenance.
- Do not add keyword-only intent routing for tools, memories, or user goals.
- Do not index secrets or ignored files unless policy explicitly allows it.
- Do not hide no-result cases.

## Tests

Minimum regression coverage:

- Plan exact path search for concrete path input.
- Plan exact content search for quoted text.
- Plan semantic retrieval for conceptual queries.
- Parse bounded ids only after route selection.
- Rank exact matches above fuzzy semantic matches for exact requests.
- Cap candidate count and snippet size.
- Skip ignored or private files with an explicit reason.
- Retrieve transcript text without hidden internal messages.
- Select relevant memory headers before reading full memory bodies.
- Produce deterministic rankings with a fixture ranker.
- Fail soft when an optional retrieval adapter is unavailable.

## OpenAgents Translation Notes

When promoted, map retrieval candidates to OpenAgents context slices,
capability refs, artifact refs, memory refs, policy refs, and projection
visibility. Verify live issue state before claiming semantic retrieval is
implemented.

## Decision

Search should be a typed retrieval pipeline. Exact, structured, semantic, and
model-selected retrieval can coexist, but they must emit the same candidate
shape, preserve provenance, and obey workspace and privacy boundaries.
