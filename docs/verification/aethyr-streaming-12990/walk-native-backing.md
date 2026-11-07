# Native backing-plan adoption — #12990

Starting at `eb093b67a0`, the reused worktree adopts the earlier admitted SDK
factories in the actual bounded native corridor. The issue remains open and
this checkpoint is not pushed. Browser transport/container/transient planning
and the final consumer/residual/teaching audit remain required.

## What runs

`aethyr/host/native-backing.coax` binds the original authenticated native RCM1
and device observations to exact pre-load allocation plans. The C face queries
private unbound objects without allocating memory, then destroys them. Actual
factories must reproduce those requirements and reserve bytes before
`vkAllocateMemory`. This covers initial staging, streamed destinations, fixed
renderer buffers/targets, replacement staging and readback. Every successful
allocation and physical free is observed independently of reservation/release.
Failure retains the reservation through actual SDK unwind; pending work keeps
its source, staging and destination until the existing terminal observation.

The profile uses the existing 64-ticket inbox authority, 2 MiB aggregate,
1 MiB per allocation and eight named category caps. The fixture has 63 valid
plans per device out of 135 possible roles. Its immutable source manifest and
logical costs are unchanged. The new v7 envelope explicitly carries the
original v2–v6 observation contract and a mandatory backing companion.
[The host note](../../../aethyr/host/native-backing.md) specifies every field.

The independent oracle derives descriptors from the original manifest,
checks recorded requirements and reconstructs leases, categories, aggregate
and actual allocation totals. Existing source, movement, ownership, image,
loss and SDK-prefix oracles still check their original contracts. Sanitized
CPU replay validates the backing history through the same Coax authority,
then performs the accepted world history with actual CPU codecs and malloc/free.
Recorded GPU requirements and wall times are observations, not CPU GPU emulation.

## Evidence

The matrix contains **136 SDK fault histories**, both tiers on ordinary RTX
4080 and ASan/UBSan/LSan Lavapipe: buffer/image × staging/destination phases
1–4, submission phases 1–6 and poll phases 1–3. The same four configurations
also complete full automatic-recovery out-and-back routes and physical-cap
pressure probes. All use the installed Vulkan validation layer and report
zero errors/warnings. Every accepted paid step replays exactly.

Build v4 qualifies that 144-case matrix. Altered-receipt testing then exposed
a missing clone-generation binding: a refused private clone could name a
different generation without changing admission counters. The final guard
requires the live original index, destination part, generation and world epoch.
Build v5 qualifies all four pressure configurations with that guard; CPU v6
validates them. The SDK/route histories never use part 3, so the new branch is
unreachable there; their passing v4 results are retained, not rerun. Generated
C/headers and binaries for both emissions are preserved and pinned.
The final emission additionally qualifies 40 compatibility histories: logical
category pressure, all seven allocation/source/decode failures, and held
buffer/image uploads, each on both tiers/backends. The held-upload companion
is independently checked against the new receipt's actual hash and original
v2 observation contract; its eight early retirement/effect calls still refuse.

Across these **188 histories** there are **74,004 paid steps**, **40,072 backing
events**, **9,844 reservations/releases**, and **9,828 successful allocations
and matching frees**. The difference includes admitted SDK failures before
allocation; reservation peaks must not be presented as successful allocations.
The original CPU verifier rejects **2,256 altered world histories**. Each
final pressure configuration additionally rejects **280 altered backing
receipts through both independent oracle and sanitized CPU replay**: **1,120**
final negative cases. The earlier red run that found the wrong generation is
retained; it is not counted as a pass.
The final CPU binary also replays an original v6 SDK-prefix history and an
original browser callback-retirement history, preserving those input contracts.

The final authority kills **562/562 mutants**, zero survivors. Replay schema
selection kills **19/19**, zero survivors. The earlier unchanged budget
authority's **148/148** result remains at `eb093b67a0`. No equivalents are
waived. The initial policy gate had 42 survivors; boundary, source/device
binding, lease and partial-history tests were strengthened before final gates.

## Pressure and observed sizes

The pressure profile sets the texture cap to the measured row-zero destination
cost. It asks for a second private copy of the same immutable source while
the original remains live. The category refuses before allocation, returns
`VK_ERROR_OUT_OF_POOL_MEMORY`, and leaves tick, source SHA, canonical state,
reserved/allocated bytes and the complete 230,400-channel RGBA8 image unchanged.
Controlled replacement then preserves the original content. It is declared
budget pressure, not physical OOM.

| Device / tier | Original texture requirement and cap | Reservation/allocation peak | Fixed renderer peak |
| --- | ---: | ---: | ---: |
| RTX 4080 / 0 | 512 B | 1,648,592 B | 1,413,376 B |
| RTX 4080 / 1 | 1,024 B | 1,650,640 B | 1,413,376 B |
| Lavapipe / 0 | 256 B | 1,258,440 B | 1,024,256 B |
| Lavapipe / 1 | 512 B | 1,259,464 B | 1,024,256 B |

These are Vulkan required allocation extents. Opaque driver allocations,
pipeline/sampler internals and physical residency are outside this ledger.
The bounded observer adds 803,432 static bytes, the Coax backing authority
25,272, and the effects memory/lease map plus active pointer 2,056. All are
included in the 854,086 reported SDK/carrier static bytes; core statics remain
874,009. The observer is explicit tooling cost, not hidden GPU payload.
Timing quantiles remain in the individual oracle records; concurrent runs
are correctness qualification, not quiet-machine performance comparisons.

## Reproduction and retained artifacts

Use `coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_DIR [--sanitize]`
with the installed Vulkan pkg-config path. Run its `stream-native` over
`corridor-v2`, scene SHA
`ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810`, the emitted
shader, a new output directory and tier 0/1. Modes include `--auto-route`,
`--backing-pressure` and the existing `--sdk-{buffer,image}-{stage,create,submit,poll}-N`.
Select the explicit NVIDIA or Lavapipe ICD, validation-layer path, and use
leak/abort/halt sanitizer options on software. The previously documented
NVIDIA sanitizer-loader limitation is reused, not retried.

`aethyr/tools/stream-walk-native.coax` builds the sanitized CPU replay; the
same replay binary can consume each receipt into its own `replay.json`.
The existing CPU oracle expects a sibling `replay` executable; the case
directories retain a symlink to the pinned shared executable. Run
`streaming_backing_oracle.py SCENE_DIR CASE_DIR SHADER [REPLAY_EXE]`; the optional
fourth argument performs the altered-backing suite as well as the original
world-history verification. All commands have explicit 90–240-second bounds.

Artifacts live under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/physical-*`. Compact matrix
and healthy/pressure oracle records, final policy/schema gates, tool/registry
checks and source/artifact hashes are committed beside this record. Earlier
component, browser-callback and SDK-failure evidence remains separately scoped.
All six chapter registries pass. The broader Coax book structure command
also ran and reports three stale checked excerpts (chapters 11, 15 and 25)
and a broken chapter-6 link; those files are unchanged from the starting
commit. Its failed log is retained, and it is not reported as a passing gate.
No physical GPU reset, Metal/mobile qualification or native persistent-rescue
claim is added. This checkpoint does not close #12990 or promote the chapter.
