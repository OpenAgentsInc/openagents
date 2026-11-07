# Whole-container transport costs and prepaid effects — #12990

This checkpoint starts at `a997216824` in the reused worktree. #12990 remains
open and unpushed. It adds HTP1 container costs and actual shared-loader
prepayment. The corridor's packed-resource adapter still needs HTP1 binding,
per-owner transport histories, remaining transient planning and the final
consumer/residual/teaching audit. These are not replaced by the component tests.

## Source-derived companion

`transport-cost.coax` derives source locations from complete admitted VPK1/VPK2
or exact raw bytes, then calls the original resource matcher/codec. It checks
the complete container, entry-start/subspan bounds and actual source identity.
`transport-manifest.coax` serializes HTP1, with original scene and RCM1 hashes,
a pinned model digest and each resource's full container/content identities,
container extent and source span. Readers authenticate before parsing and
preserve their prior output on refusal. Wire extents narrow before signed
projection. Unknown models, malformed metadata and invalid sources refuse.

The Coax builder reads the unchanged `corridor-v2`: scene SHA
`ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810`.
Four 6,256-byte companions cover both backends/tiers, with 64 rows each over
37 complete containers and 64 distinct payloads. Each has 40 packed and 24
raw rows. Original scene, source, container and RCM1 bytes stay unchanged.
The new index hash is
`441efca80b117cd7b8a1a94475c168a85b79665c0c8775e3054b6ec6fee4e3b9`.
The independent Python oracle reconstructs every companion byte from those
original files, plus 1,024 hypothetical layout/page observations. These
layout vectors do not claim actual allocation of their largest inputs.

The format/model and complete API are in
[the host note](../../../aethyr/host/transport-manifest.md). The fixture builder
has a named 64 KiB input span; the shared policy retains the 64 MiB source
bound. An HTP1 reader is not evidence that future fetched bytes were admitted.

## Actual pre-allocation boundary

The real reader reactor observes its linker heap base. Coax computes aligned
manual arena pages under the existing pack limit, retaining the actual 8 MiB
initial memory. Raw transfers additionally retain an exact byte array whose
reservation rounds to 64 KiB pages. `hpt_reserve` obtains both original inbox
leases atomically before any SDK effect. A failed second reservation publishes
neither lease; generation exhaustion still preserves the terminal retired flag.

`pack-host.js` now uses that boundary for every `loadBytes`, including the
corridor's scene/manifest/collision/body sources. A public `load({url,bytes})`
also prepays its known complete extent, then requires the stream/header to
match. Failure to instantiate returns both leases. Once effects start,
cancellation and final immutable borrowers keep the original reservations
until actual completion and zeroing/disposal. Existing 64-ticket/256 MiB pack
storage and identity/epoch semantics remain the authority; no allocator is added.

The ordinary unknown-size pack path and private broker/crypto/gzip path retain
their existing bounded observed-extents model. A private request cannot use
the public known-length plan. Complete HTP1 binding for the corridor's packed
adapter is still an exit. Opaque SDK/network/GC storage and physical RSS are
outside the explicit engine allocation model and are not measured here.

## Checks and observed effects

The final transport/cost/reservation authority kills **103/103 mutants**,
the companion codec **146/146**, and the browser conformance gate **10/10**,
all with zero survivors and no equivalents waived. The first cost/codec
gates left survivors; boundary, raw/whole-span, layout and terminal-state
checks were sharpened before final gates. Their intermediate logs are retained.

The native/Wasm example compares **64 exact words**: actual small-pack source
derivation, raw/packed footprints, growth decisions, HTP1 bytes/hash/model,
typed refusals, two original page leases and final release. Native debug,
release and ASan/UBSan/LSan outputs match the independent oracle; muted
headless Chrome executes the same Wasm words. The final vector Wasm is
47,956 bytes. Its literal codec roots are deliberately separate from the
real corridor derivation. The book's new example has a checked transcript.

The new actual browser fixture observes **58 independently checked facts**:

- A known 512-byte public pack prepays its 8 MiB reader before construction
  and fetch, then reads the original five admitted entries.
- A four-byte raw transfer prepays the reader and one external page, before
  constructing its reactor. Those two tickets remain held across an immutable
  borrow after retirement starts; the byte array is zeroed only after join.
- Two simultaneous raw readers hold 25,296,896 bytes including the persistent
  master. Retiring one behind a held cancellation retains all tickets until
  that actual cancel resolves; the other owner remains charged.
- Constructor failure releases its private reservations. With 30 existing
  public readers plus the master (260,046,848 bytes), a raw request's second
  ticket refuses atomically. No new Wasm instance or fetch begins and current
  owners remain intact.
- Invalid extents refuse before either effect; final cleanup leaves only the
  original master (one ticket, 8,388,608 bytes).

The unchanged public/private/crypto/gzip/borrow/full-pool suite and physical
split/cut/TCP-cancel suite also pass with the new reactor and shared JS. Their
final authoritative runs are isolated/sequential (`transport-pack-*-v2`).
Earlier concurrent runs used the same fixed test port and are retained as
precursors. Later fixture changes extend only its planned-operation export;
the legacy/physical exports and the executed shared implementation are unchanged.
A first expanded borrower fixture retained an already-resolved callback in
its test release queue; its owned browser was stopped and that fixture fixed.
That interrupted run is not counted as a pass. Final `planned-v3` passes.

## Actual corridor and replay

The unchanged corridor reactor/shaders now execute with the updated shared
pack loader and reader reactor. Both visual tiers complete the full 768 m
out-and-back route: 3,206 frames and 114 attempts each. Independent checks
reconstruct physical payload lifetimes, category/owner peaks and all expected
content. Sanitized CPU replay reproduces **25,618 accepted paid steps**, with
zero final owners, and rejects **12 altered histories**. Both tiers match
3,206 gameplay frames, all twelve final actor cells and source identities.
Different live completion schedules are not promised to match.

| Tier | Frame p50 / p95 / p99 (ms) | Actual payload CPU peak | Replay CPU peak |
| --- | --- | ---: | ---: |
| 0 | 2.240 / 5.400 / 8.025 | 14,132 B | 14,819 B |
| 1 | 2.165 / 5.550 / 8.300 | 14,212 B | 14,883 B |

These are manual fixed-history correctness runs, not quiet-machine live-FPS
benchmarks. Resource allocation/stage observations and complete owner/category
peaks remain in the individual records. Transport reservations are separate
from the 32 KiB dynamic payload cap. The settled page retains master, reserved
reactor arena and its two-MiB rescue snapshot (18,874,368 reserved bytes);
closing returns to the master alone. Full per-owner transport peaks still need
the later HTP1 carrier history.

Both tiers also replace the actual WebGPU device without source requests.
State/cells and complete captured pixels match, with continued paid movement;
replacement takes about 93.41 / 91.75 ms. Both before/after image hashes are
`6e9c077f552b9beaad81e412f95b7561b1949ed228412ac5b3d71f6aad21e317`.
The owned server is stopped after browser checks. The existing world Wasm
SHA remains `2bf4397d355718ded0b79f38afa7ffb377afd0f7b16cc9b123d6ae5ad7e8c73a`.

## Reproduce and remaining work

Use `stream-transport-manifest.coax SCENE_DIR NEW_DIR` and
`transport-cost-hosts.coax NEW_DIR` through `coax/web/tool.sh`. The independent
oracle takes `SCENE_DIR TRANSPORT_DIR HOST_VECTOR_DIR [PLANNED_RECEIPT]`.
Build the actual reader with `coax/web/build-packs.coax ROOT OUT.wasm` and run
`check-browser-packs.coax ROOT REACTOR NEW_DIR` with `--planned`, `--physical`
or no fourth argument, one at a time because the qualifier owns a fixed port.
All commands are bounded at 60–240 seconds; the owned world server had an
850-second ceiling and was stopped explicitly within it.

Artifacts under `~/.openagents/aethyrforge/streaming-12990/2026-11-07/transport-*`
retain metadata, native/Wasm/sanitized binaries, the served site, receipts,
readbacks and logs. Compact oracles/gates and source/artifact hashes accompany
this record. Six book registries and the new example's debug/release transcript
pass. The broader Aethyr book check finds an unchanged pre-existing chapter-87
excerpt drift (`shared-step`); that failed log is retained, not reported as a
passing gate. No chapter is promoted and #12990 is not closed here.
