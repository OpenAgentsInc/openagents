# 46. Resident compilation and state migration

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R27.

`coax0` is not incremental: every run starts from nothing. R27 adds a
resident compiler with a bounded cache built from content and declaration
dependencies, and a resident Mac host that can change a running program's
code and, when a layout changes, migrate its state. A migration either
publishes atomically or rolls back and keeps the old state. This follows the
body-replacement experiment in [chapter 45](ch45-body-replacement.md), and
only if that experiment showed value.

## What this chapter will cover

- A resident cache and how it is invalidated.
- The resident host's versioned C interface.
- Quiescing workers and callbacks, and replacing procedure tables by stable
  ID.
- Versioned migrations that publish or roll back.
- When old code and GPU resources are retired.

## Outline

1. **The resident cache.** Built from content and declaration dependencies.
   Invalidation is tested across targets, modules, descriptors, plugins,
   generics and build options, with the full build's output as the oracle.
   Safe analysis mode stays available.
2. **The host interface.** A versioned C interface. Persistent data, platform
   services and GPU resources stay owned by the host.
3. **Quiescing.** R15 workers and callbacks stop; no old frame runs.
   Procedure tables are replaced by stable ID, never by stored code address.
4. **Migration.** A layout change needs an explicit versioned migration:
   1. Allocate the new state.
   2. Migrate and validate it, reusing R16's snapshot inventory where it
      applies ([chapter 33](ch33-replay-and-snapshots.md)).
   3. Publish atomically, or roll back and keep the old state.
   Compatibility is never inferred from matching field names.
5. **Retiring resources.** Old code and GPU resources go only after their
   users and queues finish.
6. **Limits.** This is Mac development reload. Signed iOS device updates keep
   their signing and deployment rules. No stack migration or device
   code-loading bypass is implied.

## What exists, and what a resident check server would need

`coax0` has no resident process yet. Since #12647 it does have the first
piece of the cache: a compile reuses each `#authority` check whose inputs
didn't change, keyed by the tokens of the declarations the check reaches by
name, the procedures it ran, the compiler, the options and the storage it
starts with (`ReuseIndex` in `coax/src/verify.cpp`; INVARIANTS.md, "A reused
check is one whose every input is unchanged"). After a one-constant edit to
an Everglade realm file, `coax check apps/openagents-web/everglade.coax`
runs 31 of its 302 checks instead of all of them. What is left of a warm
check is the front end: about 0.6 s to load, 0.5 s to lex, 0.25 s to parse
and 0.75 s to type-check, every time, for every unit.

A resident server for agents ([chapter 22](ch22-agents.md)) would keep that
front end's results in memory between requests. It would need:

1. **A protocol.** A local socket, one request per edit (`check FILE`,
   `query ...`), answers in the same `coax.diagnostic.v1` lines and exit
   statuses as the command line, and a compiler identity in every answer so a
   stale server is never used after a rebuild.
2. **File identity.** Each request names the units' revisions, with the same
   bound-revision rule as `fmt --write --revision`. Watching the file system
   can only tell the server to look again, never stand in for that check.
3. **Per-unit lexing and parsing kept warm.** Tokens and parse trees per
   unit, keyed by its SHA-256. The parser already works per unit; the
   checker doesn't, because it resolves names across the whole program. So
   the server reparses only edited units and re-runs the checker on the
   whole graph (about 0.75 s for Everglade today) until the checker can take
   a previous typed graph and the set of changed declarations.
4. **The same keys as the check cache.** The per-check keys above are the
   invalidation rule. The server would keep the item index in memory instead
   of rebuilding it (about 1 s for Everglade's 302 keys today).
5. **Bounded memory and a way back.** A cap on retained graphs, eviction by
   last use, and `coax check` with no server as the reference: any answer the
   server gives must be byte-identical to a cold run's, checked the way
   `tests/check-reuse.coax` checks the cache.

When an edit leaves every check's inputs alone, Everglade's check takes
about 3.2 s. A server holding tokens, parse trees and the index would save
about 2.3 s of that and still pay the checker's 0.75 s. Going below that
needs the checker's incremental input first.

## The consumer and the gate

The consumer is a real world or controller and its shader. The gate:

- An edit preserves state while the code and shader change.
- Incompatible migrations are exercised, and rollback leaves the old state.
- The edit-to-observable-frame latency is measured and published.

## Done when

- R27 has shipped with the gate above.
- The chapter shows a migration and a rollback as exact transcripts.
- The chapter's examples run under `check-examples.sh`.
- Chapter 19's "Hot reload is an engine design" section links here and
  states what shipped.

## Dependencies

- R1 and R8 cache inputs, R3 ownership, R5 stable state identities and R15
  quiescence when workers exist.
- [Chapter 45](ch45-body-replacement.md), [chapter 19](ch19-builds-hot-reload.md)
  and [chapter 33](ch33-replay-and-snapshots.md).
