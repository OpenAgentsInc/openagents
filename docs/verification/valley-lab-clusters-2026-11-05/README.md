# Valley Lab: bounded cluster leaves

Story: the unlisted Valley Lab geometry station reconstructs the original
rock from independently remapped leaves. `#clusters` opens their ID colors;
`C` switches between the decoded and original paths, `V` changes shading,
and `L` changes source tessellation. Phone VIEW also reaches cluster IDs.

The reusable Coax authority `aethyr/render/geometry/clusters.coax` partitions
an already admitted source in triangle order. Leaves carry up to 64 source
vertex IDs, 124 triangles of local u8 indices, an original triangle range and
position bounds. A private complete candidate replaces the output only on
success. Bad structural input or insufficient capacity preserves the old
result. Mesh quality/provenance admission stays with `indexed.coax`.
This is a source-order correctness baseline: no simplification, spatial
optimization, hierarchy, culling, LOD, paging or triangle-count savings.

| Source divisions | Leaves | Remapped vertices | Triangles | Maximum vertices / triangles per leaf |
| --- | --- | --- | --- | --- |
| 8 | 9 | 556 | 768 | 64 / 97 |
| 16 | 36 | 2,286 | 3,072 | 64 / 90 |
| 32 | 204 | 13,056 | 12,288 | 64 / 62 |

The renderer uploads per-leaf vertices and uses their actual local triangle
indices. It retains the original path and original asset identities. Vertex
positions/normals and triangle order are unchanged. Boundary duplication
increases vertex traffic, especially for the highest source tessellation;
this stage claims correctness and visibility rather than speed savings.
Cluster inspection runs outside the walking scene so its 13,056-vertex high
profile fits the existing 16,384-vertex mesh buffer with the inspector floor.

Evidence:

- `realm-checks.log`: 441/441 checks, including every decoded source corner,
  every leaf bound and all three tessellations' submitted buffers. After
  moving admission scratch into measured state to fix a browser stack
  overflow, `realm-scratch-checks.log` checks the changed Valley units again.
  Unchanged dependency passes remain recorded in the full run.
- `cluster-mutation.log`: 124/124 authority root mutants killed, zero
  survivors and no equivalent annotations. Hand meshes test exact 64/65
  vertex and 124/125 triangle boundaries, shared/repeated corners, malformed
  source data, the 512-leaf physical ceiling and atomic overflow refusal.
- `native-clusters.log`: the native Coax builder/report's counts and all
  three unchanged source hashes. Baseline source bytes and their independent
  native SHA-256 proof remain at commit `73b60ebdf5` in the preceding receipt.
- `shader-checks.log`: permuted index pulling, view depth and cluster color
  preservation; `shader-emission.log` compares committed generated WGSL.
- `browser.log`, `browser-rebased.log` and PNGs: actual muted headless hardware WebGPU, keyboard and
  phone touch, source SHA matches, decoded path counts, cluster colors,
  exact lit reference region (middle 20% of width, 25–55% of height),
  source/decoded toggle, deep link, controller
  regression, unlisted route metadata and no console errors.
  The rebased run checks the latest merged browser host.
- `release-build.log` and `artifacts.sha256`: the 81,715-byte release artifact and
  source identities. Realm generation, visual-source, whitespace and the
  repository tools gate also pass.

The browser initially exposed stack overflow when both admission candidates
were local. The asset candidate now lives in a bounded state scratch buffer,
counted by the existing 6 MB arena check. The generic builder keeps its private
atomic candidate. No global Wasm stack increase or host behavior change is
needed. The initial scene/HUD renderer remains the explicit legacy adoption;
this does not qualify the shared GPU ledger, native host or book chapters.

Toolchain: the same current-source Coax 0.50.0 qualified for the prior slice,
fingerprint `c82201e2cfff7c937fc0a29bf7010ae1ec75e7ab746ad29548bac7c2db61409e`.
`--library` permits testing a module without a program entry point.
The geometry authority's mutation gate ran separately from the compiler's
previous release bootstrap. Browser timing is static frame submission through
GPU queue completion, not GPU timestamps or a route performance qualification.

Reproduce:

```shell
coax/bin/coax test apps/openagents-web/valleylab.coax --library
coax/bin/coax mutate aethyr/render/geometry/clusters.coax --library --root-only
coax/web/tool.sh aethyr/tools/valley-clusters.coax
coax/web/tool.sh valley-lab-check http://127.0.0.1:PORT
```

The [single roadmap](../../verse/2026-11-05-valley-lab.md) now advances to tiny
hierarchy/simplification/error fixtures before attempting screen-error cuts
or paging. Saved walkthroughs, source import and shared renderer qualification
remain open.
