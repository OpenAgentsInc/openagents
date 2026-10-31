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
