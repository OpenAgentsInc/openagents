# Valley World: spatial clusters for original source geometry

This continues `1c651f1315769839a77d81b214a2648d1bc2eb42`. The original
Q16/F32 VDP1 pages, source world pack and Echo are unchanged. The full
1,997,616-triangle sandstone mesh now has spatial index companions and a
bounded conservative hierarchy, rather than only a complete-mesh draw.

## Geometry and artifact identity

A source-order control produced72,300 leaves /144,599 nodes /6,940,860 bytes.
Its median leaf AABB occupied49.197% of the root AABB, so those clusters had
poor locality. The Coax builder now stably sorts triangle centroids by a
30-bit Morton key, then partitions the resulting order into leaves with at
most64 distinct vertices /124 triangles. It neither moves vertices nor
simplifies geometry. Each triangle retains its original three corners,
winding, normals and UVs.

The result has **27,004 leaves,54,007 nodes and height15**. Measured maximum
leaf occupancy is64 vertices /100 triangles; median/root AABB volume is
0.000003 and p90 is0.000013. The companion tree uses the existing VWH1 v1
format, with one model and one identity instance. Bounds include the Q16
page's independently checked local position-error allowance. Runtime scratch
replaces only that instance's affine with the original placement from the
world catalogue; node/index bytes and retained transport artifacts stay intact.

All new licensed payloads remain private under
`~/.openagents/aethyrforge/valley-lab/world-export/Saved/world-detail/`, staged
through `~/.openagents/verse/private-build/web/`.

| Companion | Bytes | SHA-256 |
| --- | ---: | --- |
| `valley-detail-spatial.page` | 23,971,520 | `54fd393bd76d1236eb87ac7bf615375edec70113be6b1e777d33f8c6ef710dba` |
| `valley-detail-spatial-reference.page` | 23,971,520 | `0e12d9ca2c4e1802b65e7d2848ac488b15136f2a4adc670399ab3ee85601bfbd` |
| `valley-detail.clusters` | 2,592,444 | `17b39d202d36ba5d4197bd62f380f65e4a515bd5352a1589e3c1d00ff36abec4` |

VDI1 version1 is a new128-byte index-page header. Its eight little-endian U32
words are magic/version/profile/logical vertex base/index count/payload offset
128/whole byte count/original triangle count. At32 is the corresponding VDP1
SHA-256, at64 the original glTF binary SHA-256, and at96 the offline triangle
permutation SHA-256. The remaining payload is U32 indices. Q16 IDs start at
8,000,000; F32 IDs start at12,000,000. The offline `valley-detail.order` audit
file records each original triangle ordinal and is not loaded by the realm.

The independent oracle checks every original triangle exactly once, every
corner against both original index arrays, each leaf's vertex/triangle limits,
all Q16/F32 positions against its bounds, every ancestor, and unique ownership
of every nonroot node. The unchanged source-page hashes join this result to
the full source-corner oracle at `1c651f1315` without rerunning unchanged gates.

## Rendering, work limits and references

N cycles **original order(default)**, complete spatial reference, and spatial
cluster cut. K retains Q16/F32/fallback selection. G retains the existing
world visibility modes. With N in cut mode, the ordinary G1 path stops at
depth8, visiting at most511 tree nodes per pass. G2 permits the deeper walk
to the actual leaves. Every emitted parent still contains exact original
triangles; this is visibility selection, not simplified LOD.

Color and shadow independently transform their planes by the same original
placement affine. Wholly included subtrees emit one complete range. Adjacent
surviving ranges merge. Output pressure discards the partial fine cut and
uses the complete source mesh; complete scene fallback also retains the fine
source mesh. A red/green fixture catches an initially missing instance count
on this pressure path.

The optional scalar index ABI remains v1. Spatial draw records use a separate
logical first-index tag,0x20000000; Coax maps it to GPU asset6/7 and its physical
word offset. Original fine records keep0x40000000 and GPU asset4/5. The shader
and original vertex pages are unchanged. Metadata is bounded to4MiB of native
Wasm carrier memory, within the unchanged64MiB Wasm maximum.

Coax binds exact artifact identities, stream extents, complete metadata and
profile readiness. Missing, corrupt or held companions preserve the complete
original source draw. The host's optional-failure path now also handles index
and data companions without blocking the root scene. Device ownership fences
are the existing engine/GPU fences from the previous phase.

## Physical observations

Twelve complete2560×1600 Q16/F32 frame comparisons cover all four waypoints,
a paid camera turn and the deeper west cut. **Every byte of each cut frame
equals the corresponding complete spatial reference.** Reordering versus
the original draw changes0–9 pixels per frame (opaque depth/order ties), so
it is not bit-identical to the original draw. Maximum observed mean RGB error
is0.0000161947 levels/channel, and maximum fraction differing by more than8
levels is0.0000009765625. The pre-existing0.25/0.001 image bounds were retained.

| West source mode | Fine color triangles | Fine shadow triangles | Nodes visited, both passes |
| --- | ---: | ---: | ---: |
| Complete original/spatial reference | 1,997,616 | 1,997,616 | 0 |
| Bounded depth8 cut | 1,923,678 | 1,997,616 | 260 |
| Deeper leaf cut | 1,633,596 | 1,997,616 | 3,796 |

The deeper cut removes364,020 fine color triangles(18.2%); the bounded cut
removes73,938(3.7%). The whole chosen rock lies inside the tested shadow
volume, so that pass correctly retains it. The north selected instance
remains occluded; it is not new visible source-detail coverage.

The timing record includes20 serial whole-world frames per plan and view.
Fewer triangles did not establish an overall speedup: CPU traversal and extra
draws still cost time, and concurrent browser activity affects the timings.
G2 also enables the older fallback hierarchy, so its timings do not isolate
the new source traversal.
The original draw therefore remains default. The cut is an inspectable,
qualified geometry path, not a new frame-rate or production Nanite claim.

Missing/corrupt physical HTTP responses, held requests across GPU loss and
retired-owner corrupt completions preserve the original full-source fallback.
Only current complete companions enable the cut. Loaded Q16 and F32 cuts
recover with identical state digests and every complete frame byte identical.
The queue remains bounded to two unfinished GPU submissions.

The new authority kills102/102 mutants; the new cut/append unit kills54/54,
with no equivalents. The previous page-policy/runtime gates remain those at
`1c651f1315`. Focused World checks pass24/24; the Morton/radix fixture passes.
The merged release Wasm is129,823 bytes within the136,000-byte budget,
including main's shared Slay replay/face-lighting updates and unchanged Echo
rig reader extracted into Aethyr at `42920e800b`. Live accounted
residency is7,661,317,120 bytes in the explicit8,000,000,000-byte legacy profile:
seven64MiB reservations cover combined immutable pages, stream chunks and GPU
index/vertex uploads; the metadata carrier adds4MiB. This is not measured
driver/process memory or qualification of the shared renderer's1GiB ledger.

## Remaining work

One mesh type and one nearby instance still use full source geometry. The
other129 types remain fallbacks. The new tree has no simplified parents,
screen-error-qualified LOD, Hi-Z occlusion, GPU-driven traversal, demand page
residency or eviction. Both source profiles and spatial index companions stay
pinned. Native/shared renderer adoption, original lighting/material/hair
parity and exact collision remain separate. GPU traversal and adaptive source
LOD are the next work needed to turn these clusters into a faster large world.

Rebuild with `coax/web/tool.sh aethyr/tools/valley-detail-clusters.coax Q16_PAGE
F32_PAGE OUTPUT_DIRECTORY`; verify with
`python3 aethyr/tools/valley_detail_clusters_oracle.py OUTPUT_DIRECTORY`.
`coax/web/valley-source-cut-check.coax` owns the headless, muted actual-GPU
comparisons, faults, recovery, inspector and timing observations.

The full image/recovery receipts include main's Slay-lighting merge. Its
subsequent Echo-reader extraction keeps the reader logic unchanged; the final
build passes24 current World checks, and the retained local deep-cut PNG is
byte-identical before/after that extraction(SHA-256
`dd033d884dbe432c27b288943a56d3e829fa7a21b786610c81396c3129b6160e`).
`preview.log` verifies the final runtime at
`http://127.0.0.1:8895/worlds/valley-world?pack=valley-world`, outside the
implementation worktree. The original source-cluster implementation and
shader used by the full receipts are unchanged by the extraction.
