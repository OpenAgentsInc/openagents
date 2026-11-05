# Valley Lab: first procedural rock

Story: `/worlds/valley-lab#assets` opens an independently inspectable original
asset. The same indexed source appears in the walkable basin and geometry
station. Coax generates, measures, validates, hashes, retains and draws it;
the C face only exposes scalar browser ABI and buffer pointers.

`verse/valley/rock.coax` generates a seeded six-face cube-sphere rock with
continuous noise and finite-difference normals. Face seams duplicate vertices
but evaluate the same surface. The canonical little-endian content includes
generator version, seed, resolution, positions/normals and original indices.
This is an internal generated artifact, not an untrusted file importer or a
change to VPK1. Source and generated art are original Apache-2.0 work.

The default has 1,734 vertices, 3,072 triangles and 78,504 content bytes.
The 8/16/32 subdivision profiles have 768/3,072/12,288 triangles. `L` changes
the source tessellation; it is not automatic LOD or a simplification hierarchy.
The shader pulls original indices from its second storage buffer, retaining
shared positions instead of expanding triangles on the CPU. It still uses
the initial scene/HUD renderer and ordinary triangle draw calls. The walkable
rock has an explicit conservative AABB collision proxy.

`1`/`2`/`3`, station tabs and URL hashes select WALK/ASSETS/MESH; transitions
release held input and pause the walker. The inspectors orbit by drag/arrows
and zoom by wheel; V cycles lit, albedo, normals, edges/bounds and 0–20 m view
depth. Phone buttons change view/detail. Dimensions, counts, bytes and SHA-256
come from the actual admitted source, not display constants.

Evidence:

- `realm-checks.log`: all 434 checks pass, including indexed-source coverage
  at every detail, station navigation, holding/releasing one asset identity,
  byte bounds and the existing character/controller checks.
- `indexed-mutation.log`: all 50 root mutants killed, no survivors and no
  equivalent annotations. Hand-worked triangles cover source/licence,
  shape, normal boundaries, byte boundaries, invalid indices, zero areas and
  extrema. `shader-checks.log` pins a permuted-index vertex and depth value.
- `native-assets.txt` and `native-sha256.txt`: the native writer's three
  generated assets and independent system SHA-256 results.
- `browser.log`: actual muted headless browser input, all three native/Wasm
  hash matches, all five GPU readback modes (distinct central patch hashes),
  gray depth pixels, station selection, detail changes, one retained asset,
  controller regression, noindex and no console errors. A phone viewport on
  this Mac is not qualification of a physical phone.
- PNGs retain actual hardware WebGPU views: lit, normal, edges/bounds and
  view depth, plus the phone layout. No image editing or mockup substitution.
- `release-build.log`: measured release Wasm under the existing 160,000-byte
  cap. `artifacts.sha256` identifies the source, shader and built artifact.
  Shader generation comparison, realm generation, visual-source and
  whitespace checks pass. `tools-check.log` records the repository tool gate.

Browser timing rows are 30 static inspection frames measured from submission
through queue completion, with each row's actual viewport and triangle count.
They are neither GPU timestamp measurements nor the plan's 60-second route
qualification. The 6 MB declared CPU arena includes mesh/index storage and
the enlarged HUD capacity; the frame budget counts submitted index traffic.
It is not a measured process/GPU residency total or the future shared GPU
ledger profile.

Toolchain: current-source Coax 0.50.0, built by the repository's release
bootstrap path with `COAX_MUTATION=0` for the compiler bootstrap. The geometry
authority's 50-mutant gate ran separately. The 434 checks ran on that 0.50
bootstrap; the final compiler emitted WGSL and the 77,221-byte release Wasm.
Its checked source fingerprint is
`c82201e2cfff7c937fc0a29bf7010ae1ec75e7ab746ad29548bac7c2db61409e`.
The artifact manifest includes final compiler and provenance-record hashes.

Reproduce native content:

```shell
coax/web/tool.sh aethyr/tools/valley-rock.coax /tmp
shasum -a 256 /tmp/rock-8.vro /tmp/rock-16.vro /tmp/rock-32.vro
coax/web/tool.sh valley-lab-check http://127.0.0.1:PORT
```

The [roadmap](../../verse/2026-11-05-valley-lab.md) records this procedural
slice and the next bounded cluster partitioning task. Source import, saved
replay, shared-renderer/native qualification, error-based LOD, page residency
and streaming remain open. This work does not complete book chapters 38/53
or claim a Nanite implementation.
