# Semantic world queries — #12989

Work starts from `96f8af027103bec45115b3b9b7f1146dddfc8a17`, in
`/home/christopherdavid/oa-open-issues`. The source tree is bound by
`inputs.json` and the commit linked from the issue's closing comment. This
qualifies the bounded shared query profile, with the pre-existing companion
failures below retained explicitly.

## Implemented boundary

`aethyr/query-filter.coax` owns category/role selection, exact ignored
collider/entity identities and physical surface records. Nonzero friction
and restitution policy references resolve against an admitted world-owned
response table; policy zero means friction 1 and restitution 0. `query-scene.coax`
admits a complete canonical geometry/metadata table, compacts eligible shapes
in source order, maps results to durable identity and publishes a replacement
epoch only after complete validation at a later owner tick. Entity generations
remain checked on each retained semantic selection. `query-fluid.coax` stages
whole trigger/fluid results and samples explicit box volumes with flat or
heightfield surfaces and rectangular holes. Resistance is independent of
buoyancy. Geometry still comes from the original query/ground/contact kernels.

The default filter includes every declared category and both solids and
sensors. Collision-pair policy and render material interpretation are separate.
The only change to legacy `Colliders` is an optional `ground_enabled = true`;
selected empty terrain cannot invent a flat fallback. Existing C facades,
protocol/save identities and replay profiles are unchanged.

The first supported profile is 256 colliders, 64 physical surfaces, 16 terrain
records, and 65,535 height words per record. Floors retain ground-only geometry
queries. Trigger containment supports boxes/capsules; unsupported volume shapes
refuse. Fluid gameplay surfaces use the existing ground sampler's constant-edge
continuation inside their explicit box. Dynamic waves and physical buoyancy
force integration belong to chapter 70's remaining implementation.

Snapshot arrays and heights are retained immutable caller-owned banks.
`QueryScratch` belongs to one borrowed selection at a time. Publication and
entity flush run on the owner's paid step; the API's integer tick is not a
replacement scheduler. A publication failure preserves the old epoch/world.

## Shared consumer

`aethyr/examples/query-world.coax` uses the real capsule resolver,
follow-camera body-clearance code and AI sight code on independently selected
views of one admitted world. Camera sight uses semantic clear. Ground surface
identity selects footstep/reaction classes. The fluid sample drives resistance.
Despawning the NPC invalidates a retained selection. The next tick publishes
an inactive bank without that collider and changes the ground's footstep
class from 4 to 12. The old movement selection refuses its previous epoch;
the newly selected ground produces the replacement class.

| Durable ID | Content |
| --- | --- |
| 100 | Ground, physical surface 10, footstep 4, reaction 9 |
| 200 | Camera-only barrier, physical surface 20 |
| 300 | Ordinary wall, physical surface 20 |
| 400 | High-resistance non-buoyant fluid, physical surface 30 |
| 500 | NPC capsule with a live entity generation |

These IDs are retained for #12990–#12995. The later destruction/save/unload
journey is not claimed by this query-only consumer.

## Evidence so far

Host: coderos-4080, Linux 6.18.49 x86_64, Coax 0.52.0; GCC 15.2.0.
Large generated C/binaries and raw logs are retained below
`~/.openagents/aethyrforge/query-12989/2026-11-06/`.

| Gate | Observation |
| --- | --- |
| `query.coax --root-only` mutation | 473/473 killed, zero survivors |
| `query-filter.coax --root-only` mutation | 103/103 killed, zero survivors |
| `query-scene.coax --root-only` mutation | 325/325 killed, zero survivors, including response-policy and publication-clock admission |
| `query-fluid.coax --root-only` mutation | 155/155 killed, zero survivors |
| Shared native/Wasm journey | All 14 words equal; actual muted headless Chrome 152; final Wasm 60,595 bytes |
| Shared ASan/UBSan journey | Exact fixture transcript; no sanitizer report |
| Recorded walk | Native/Wasm digests match |
| Recorded Grid | Native/Wasm digest matches every step |
| Tools check | 1,848 tracked tools, zero new without a reason |

Mask tests enumerate every four-category mask against hand-worked box,
capsule, terrain and floor outcomes. The coordinate-grid fluid oracle covers
all box/hole boundary combinations. Separate checks cover included/excluded
near hits, ignored exact generations, role selection, capacity endpoints,
malformed sources, exhausted terrain traversal, stale epochs/entities and
atomic output. Earlier failed mutation observations remain retained; no
equivalence exemptions were used. The unchanged query/filter/fluid procedure
gates are reused; the new response declaration adds no filter procedure
mutation sites. The final journey's ordinary compiler admission executes the
current loaded authority checks, including the fluid boundary cases.

The native/Wasm output is the checked example's fixture. Words encode accepted,
walker z, wall-stop z, camera clearance, sight states, footstep/reaction,
fluid depth/velocity, stale refusal, tick-boundary publication and the new
footstep class. Floats are compared by their exact
f32 bit patterns, not text formatting or a widened tolerance.

The executed Wasm SHA-256 is
`3bc12a9488db4d6e8e19699701635139214366679b2b7e184742454fce825061`.
`hosts6.log` retains the actual browser user agent and every observed word.
The final example also matches its fixture under the release profile and
ASan/UBSan with leak detection. Earlier 12-word journeys remain historical
observations; the final 14-word journey supersedes them.

## Companion failures reproduced on remote main

The frozen Dead Gears v1 native program names the former `Store` type. It
does not compile against main's `DgStore`. A separately versioned v2 program
changes only `[1] Store;` to `[1] DgStore;`. The independent runner verifies
every body against frozen v1 bytes with exactly that substitution; original
assertions and other body bytes remain intact. V1 programs/manifests retain
their identity. V2 uses a distinct execution receipt schema.

V2 debug, release and ASan/UBSan each pass their first four programs and fail
the cover scenario's distance assertion at line 61, after the no-penetration
assertion passes. The identical v2 adapter on untouched remote main fails
the same assertion. This is a pre-existing companion failure, not a passing
whole-game qualification. The original v1 compile refusal is also retained.

Everglade debug and ASan/UBSan fail scenario 0 at line 75's rebuild assertion.
The debug C emission took 662.19 seconds, within its 810-second limit; the
sanitized run reuses that qualified emission. The first debug execution also
observed the runner's version-adapter edit during its lifetime, so it is
retained separately; a new execution with the unchanged emitted C and current
runner reproduces the assertion without a source change. Untouched remote
main reproduces the same line-75 assertion (662.82 seconds for its complete
debug companion, including normal compiler admission). These are not passing
whole-Everglade qualification receipts. No gameplay threshold, drift tolerance,
production rule or frozen original assertion was changed to obtain a pass.

The full book structure check reports an existing chapter-53 `format` listing
drift against `aethyr/forge/pack.coax` on both current and untouched main.
Chapter 7's updated source-linked listings match exactly. This does not claim
a passing whole-book check; #12610 retains the wider book reconciliation.

## Storage and work scope

The compiler's LP64 layout reports 38,784 bytes per `QueryScratch` and 56
bytes per metadata record. The shared journey owns four selection scratch
records (155,136 bytes total). Fluid result staging is 256 records of 72
bytes (18,432 bytes); overlap stages 256 geometric IDs. These are bounded
caller/stack storage, not SDK or driver allocation estimates. The response
table adds at most 64 records. Publication and queries allocate no heap or
GPU resources in this profile.

Admission bounds duplicate checks to 32,640 collider pairs and 2,016 pairs
per 64-entry surface/response table, and terrain validation to 16 × 65,535
height words. Selected shape queries retain the declared heightfield traversal
budget; exhaustion refuses the complete result. This is a bounded-work
inventory, not an assertion that maximum-sized admission fits a frame budget.

A separate single-process observation of the final native journey took
1.16 ms wall time and reported 17,992 KiB child peak RSS. RSS includes process
startup/launcher/loader storage and is not an engine allocation counter or a
statistical frame-time qualification. The native image SHA-256 is
`cf23033dde663858c5d62f4b7f51ac20c135478cb556e953134c2ac245dd2f95`;
its exact transcript SHA-256 is
`2722da47f1fe4bc16a293680c16daf7fce194b9e6ba0ce522e3f5bee6bc92331`.

## Acceptance audit

| Requirement | Evidence |
| --- | --- |
| Camera-only obstruction, walker passage and independent sight | Shared journey uses `collide_move`, `follow_room_checked`, `semantic_clear` and `sense_sight` on separate selections of the same content |
| Fluid entry, holes, resistance without buoyancy | Fluid coordinate-grid oracle and shared journey; density, buoyancy and drag remain independent admitted fields |
| Self/ignored generations, roles, mixed layers and stable ties | Filter checks, exact source-order compaction, mixed-shape mask enumeration and original kernel tie/terrain oracles |
| Complete bounded query results | Atomic overlap/volume staging, exact capacity endpoints, malformed source checks and exhausted heightfield refusal |
| Default regressions | 473 original/extended root kernel mutants killed; unchanged recorded walk/Grid native/Wasm receipts; native companion failures reproduced on untouched main |
| Bounded native/Wasm consumer | 14 exact words, including replacement publication and the changed surface consumer; final native sanitizer and release fixture matches |

This does not qualify full terrain rendering, every possible native/Wasm
scene, all legacy realms, full water simulation or maximum-capacity frame
performance. Those are separate profiles/workstreams rather than implicit
claims from this small shared fixture.

## Remaining legacy adapters

Ordinary realms still supply their existing trusted `Colliders` through
`walk.coax`/`player.coax`; their collision profiles, follow cameras and replay
profiles are unchanged. `ai/stimulus.coax` retains its unfiltered public
entry. Existing Everglade `verse/water*.coax`/`verse/everglade/river.coax`
retain their own complete wave/render/force integration. The new shared
fixture consumes selected geometry at those existing engine boundaries;
this does not claim a whole-realm semantic metadata migration.

## Reproduce

```sh
coax/bin/coax test aethyr/query-fluid.coax --library
coax/bin/coax mutate aethyr/query.coax --library --root-only
coax/bin/coax mutate aethyr/query-filter.coax --library --root-only
coax/bin/coax mutate aethyr/query-scene.coax --library --root-only
coax/bin/coax mutate aethyr/query-fluid.coax --library --root-only
coax/web/tool.sh aethyr/tools/query-world-hosts.coax
coax/web/tool.sh replay-hosts walk
coax/web/tool.sh replay-hosts grid
python3 aethyr/tests/check_native_gameplay.py --realm deadgears --mode debug
```

Commands stay below 15 minutes. Reuse the recorded passing results for
unchanged files; the commands above are reproduction instructions, not a
request to repeat those checks during this work.
