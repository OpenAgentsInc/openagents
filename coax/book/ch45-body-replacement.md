# 45. Replacing a procedure body while it runs

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R26.

Nothing in `coax0`, `verse/` or the iOS app reloads code today
([chapter 19](ch19-builds-hot-reload.md)). R26 is the smallest useful
experiment: on a restricted Mac host, replace the bodies of procedures whose
exports, calling conventions, semantic profiles and layout fingerprints have
not changed. It needs R1's interface identities and a defined quiet point in
the host's lifecycle. It doesn't need plugins, self-hosting or owning
containers. It has to show value before general state migration
([chapter 46](ch46-resident-compilation.md)) is built.

## What this chapter will cover

- What may change in a replacement and what must not.
- The safe point at which a replacement is admitted.
- The cases that are refused, and why refusing them is part of the feature.
- Measuring edit-to-visible-result on a real change.

## Outline

1. **The resident host.** Persistent storage and platform resources stay in
   the host, which is never reloaded. The iOS app's `host.c` already owns the
   buffers Coax draws into.
2. **Unchanged interfaces.** A candidate library must match the running one's
   exports, calling conventions, semantic profiles and layout fingerprints.
3. **The safe point.** Replacement happens only at an established quiet
   point, with no old frame on any stack.
4. **Refusals.** A changed layout or export, an escaped callback or code
   pointer, an outstanding old frame and unsupported worker activity are each
   refused with a reason.
5. **Keeping the old table.** The old callable table stays in place until the
   candidate validates and publishes.
6. **Measure it.** Edit a controller or world procedure, and measure the time
   from saving the file to the changed result on screen.
7. **What stays out.** Signed iOS builds don't load code. Replacement runs on a
   Mac.

## The consumer and the gate

The consumer is a real controller or world change in a Verse realm on a Mac.
The gate:

- Each refused case has a fixture that is refused with its reason.
- The old table survives a failed candidate.
- The measured edit-to-visible-result time is published, and it decides
  whether R27 goes ahead.

## Done when

- R26 has shipped with the gate above.
- The chapter shows the refusal fixtures and the latency measurement as
  exact transcripts.
- The chapter's examples run under `check-examples.sh`.

## Dependencies

- R1 interface and layout identities.
- [Chapter 19](ch19-builds-hot-reload.md) for the hot-reload requirements and
  [chapter 18](ch18-protocol.md) for versioned interfaces.
