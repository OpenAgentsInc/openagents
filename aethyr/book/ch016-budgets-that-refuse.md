# 16. Budgets that refuse

> **Status: unimplemented — design draft.** Today each realm declares its own
> vertex cap and per-kind slot counts, and asset packs have byte budgets, but
> a full slot list stops emitting silently. This chapter plans Aethyr's
> resource identity and declared budgets: content-hashed assets, limits per
> realm, and refusal with a visible error. Engine-core task: `ec-budget`
> (#12609); the engine-core agent fills this chapter in as the module lands.
> Nothing here runs yet.

A realm that draws too much has two bad options: clip quietly, so a lamp or
a person vanishes without anyone knowing, or overrun a buffer. Aethyr takes
a third: every resource a realm uses has a declared limit, the limit is
checked where the resource is written, and exceeding it is an error the
realm and its tests can see.

## What this chapter teaches

- Asset identity by content hash, not by file name.
- Budgets as declared data: vertices, draws, instances, texture memory,
  pack bytes, particles, and GPU time per frame.
- Refusal at the write site, reported once per frame with what overflowed.
- Measuring the budgets that cannot be checked statically (frame time).

## The method

An asset's identity is its digest, `id = sha256(bytes)`; a realm's asset
list is a set of digests, so two realms using one mesh share it and a
changed texture is a new asset. A budget is a set of counters with limits:

```jai
Budget :: struct { name: string; limit: s64; used: s64; peak: s64; }
take(b, n) -> bool {
  if b.used + n > b.limit { report(b, n); return false; }   // refuse whole
  b.used += n; b.peak = max(b.peak, b.used); return true;
}
```

`take` refuses the whole request, never part of it, so a mesh is drawn
entirely or not at all and the report names it. Static limits (vertex
capacity, slot counts) are checked in `#test`s over each realm's heaviest
recorded frame; dynamic ones (GPU milliseconds) are measured.

## What exists today

- Vertex caps per page: `MAX_VERTICES` in
  [everglade.coax](../../apps/openagents-web/everglade.coax) (30,000),
  [fairy.coax](../../apps/openagents-web/fairy.coax) (24,000),
  [belt.coax](../../apps/openagents-web/belt.coax) (90,000) and others,
  exported to the host as `coaxial_*_max_vertices`.
- Everglade's per-kind slots (`SLOTS`) with a check that a frame of the
  whole city fits; but its emitters return early when a kind is full
  (`if n >= SLOTS[kind] { return; }`), which is the silent clipping this
  chapter removes.
- Asset packs ([docs/verse/realm-host.md](../../docs/verse/realm-host.md)):
  a `budget` in bytes per manifest in `verse/packs/`, refused by
  `verse/tools/pack_realm.py` when exceeded, with licences per asset.
- Frame time: [coax/web/frametime.ts](../../coax/web/frametime.ts) (GPU time
  per pass by timestamp query) and [coax/web/perf.ts](../../coax/web/perf.ts)
  (Everglade's walk fails when the 95th percentile frame exceeds 17.5 ms or
  any frame 33 ms).
- [verse/home.coax](../../verse/home.coax): bounded resource admission for
  the iOS home (schema-1 configuration admitted into bounded scratch).

## The design in Aethyr

Each realm declares a budget record next to its state: vertices, draws,
instances per mesh kind, texture layers and texture bytes, pack bytes,
particles, local lights, and a GPU-time target per tier (desktop, phone).
The renderer's write functions (chapter 11) call `take`; on refusal the
frame continues without that item and the realm's HUD shows a development
error, and the frame is marked over budget. The asset registry stores
digests and sizes; a realm references assets by digest and the pack
compiler writes the table.

## What it builds

- `aethyr/budget.coax`: budget records, `take`, reports and the per-frame
  over-budget flag.
- `aethyr/assets.coax`: content-hashed asset identity and the registry a
  pack loads into.

## Acceptance checks and oracles

From the `ec-budget` task:

- Every realm declares budgets.
- An over-budget frame is a test failure, not silent clipping.

And:

- A planted overflow (one more instance than a slot allows) is refused
  whole, reported with its kind and count, and fails the realm's check.
- The same bytes give the same asset ID on every host; a one-byte change
  gives a new one.
- The 60 FPS walk (`perf.ts`) and the per-pass GPU times stay within each
  realm's declared target on the reference machines.
- Mutation: 0 survivors in `take` and the registry.

## Dependencies

Chapters 3 (declared defaults, where the budget record lives), 6 (entities
whose spawns take from budgets) and 11–15 (the writers that take). From
Coax: R22 compile and startup budgets for the build-side counterpart, R0
evidence for the measured ones.
