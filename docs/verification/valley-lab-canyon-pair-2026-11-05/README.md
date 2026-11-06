# Valley Lab: the original Canyon Sandstone pair

Valley Lab now has an isolated section of the installed Ancient World scene:
the two Canyon Sandstone instances in
`AncientWorld_MainGrid_L0_X0_Y0_DL44BCB661`, component
`InstancedStaticMeshComponent_7376`. The complete actor path, source mesh,
material, transforms and independent SDK samples are in `source-placement.json`.

Open `/worlds/valley-canyon?pack=valley-canyon`, or press **P** in the parent
lab. The scan station also has a touch button. **1** returns to the parent;
**V** cycles PBR, albedo, normals, roughness and source vertex paint;
**I** isolates either instance or shows both; drag/wheel orbit/zoom and **R**
resets the view. It is an unlisted Valley Lab inspection view with its own
small GPU-facing module, so the original primitive stations keep their
independent frame/mesh limits.

## What is exact

| Source property | Result |
| --- | --- |
| Source geometry | 999,995 triangles per instance; 1,999,990 for the pair |
| Source triangle corners | All 2,999,985 indexed corner records bit-exact after reindexing |
| Retained attributes | Every position, normal, UV and packed vertex-color word |
| Deduplication | Exact 36-byte attribute equality; 504,367 GPU vertices, no simplification |
| Original Nanite statistics | 999,995 triangles / 504,361 vertices reported by Unreal |
| Difference in vertex counts | Attribute-preserving source export/reindexing; triangle records stay exact |
| Albedo | Original `T_CanyonSandstone_A`, byte-exact exported 4096 x 4096 PNG |
| Roughness | Original `T_CanyonSandstone_R`, byte-exact exported 4096 x 4096 PNG |
| Albedo material multiplier | Original `Albedo Color` override, RGB 1.5 |
| Both poses | Original world translation, quaternion and nonuniform scale retained |
| SDK sample error | At most 0.000001113 metres against direct Unreal SDK transforms |
| All source vertex pose error | At most 0.000000926 metres against an independent double-precision matrix oracle |
| Transformed normal error | At most 0.000000129 per component |

The one common scene origin is the first instance's original translation,
`(-2341.011254092498, -453.31197018573005, -1130.1458705213108)` Unreal
centimetres. Coax swaps Y/Z and converts to metres. Both assets retain their
original pivots and relative height. The second instance is about 5.93 m
across, 4.45 m below and 4.01 m back from that origin. Neither instance is
individually grounded or arranged by eye.

`oracle.json` checks every original indexed triangle corner against the GPU
pack, preserving winding and every retained attribute bit. It also compares
every packed vertex under both Coax matrices with an independent matrix built
from the SDK-exported double-precision quaternion/scale/translation. Direct
SDK transform samples cross-check that oracle's coordinate interpretation.
The browser compares all 48 native/Wasm pose words before drawing.

The actual original source geometry is drawn in ordinary indexed GPU passes.
This does not implement Nanite's runtime cluster selection or page streaming.
Unreal's separately exported 7,606-triangle fallback is recorded as provenance;
it is not used by this view.

## Appearance scope

The source asset geometry, material maps and placements are reproduced.
The view provides Aethyr studio lighting, a GGX dielectric inspection material,
self/cast shadows through the existing host shadow-map path, and filmic tone
mapping. Texture filtering uses the shared Aethyr sampling functions. The
roughness layer restores the original linear scalar after the host's sRGB
array-texture decode. Source inverse-transpose normal transforms retain the
original slightly nonuniform scales.

**A perfect Unreal image match is not qualified.** No matching Unreal render
was captured: the commandlet ran with `-nullrhi` for data export. Terrain,
neighboring components, original lighting/sky, the complete original material
graph, Lumen, exposure and Unreal post-processing are outside this isolated
pair. The screenshots show this Aethyr inspection setup. The export and oracle
receipts support the exact source-data claims above; they are not evidence of
full-scene pixel parity.

## Data extraction and private cache

The installed Unreal 5.8.3 SDK reads the 5.7 project data in a separate clean
commandlet project under
`~/.openagents/aethyrforge/valley-lab/unreal-export/`. That project has no
AncientGame module or sample game plugins. It mounts the Megascans data,
AncientWorld map and external actor packages. Missing unrelated blueprint and
data-layer classes produce warnings; the selected native HLOD component loads
and exports both instance transforms directly. Source packages are never saved.
No launcher or visible editor window is opened; audio is disabled.

The Python SDK adapter (`aethyr/tools/valley-unreal-export.py`, firm reason 6)
performs Unreal API effects: source-model glTF export, native texture export,
material-property reads and world-space instance-transform reads. Every pack,
coordinate conversion, camera, material equation, policy and rendering decision
is Coax. The independent stdlib/Pillow oracle is tooling only (firm reason 2).

The source and GPU payloads live outside git:

- `~/.openagents/aethyrforge/valley-lab/unreal-export/Saved/section-source/`:
  original source export, full maps, fallback and SDK placement receipt.
- `~/.openagents/aethyrforge/valley-lab/canyon-pair/`: deduplicated source
  vertices, indices and the 71,028,884-byte VPK1 pack.
- `~/.openagents/verse/private-build/web/valley-canyon.pack`: a symlink to
  that canonical pack, exposed through the existing private local server route.

Only hashes, placement metadata, code and rendered verification images are
committed. `realms.json` uses the established `private:valley-canyon` asset
channel, so a clean public build does not require or upload licensed art.
Local access is explicit through `?pack=valley-canyon`. Without the private
pack, the view reports the missing/refused source; the parent lab remains
available. The private local route needs the normal watching server, because
`--static` intentionally does not expose `/private/`. The persistent preview
uses prebuilt Wasm with a no-op build command.

Reproduce in an implementation worktree with the clean data-only project set
up as above (enable PythonScriptPlugin, EditorScriptingUtilities and
GeometryScripting and GLTFExporter):

```sh
'/Users/Shared/Epic Games/UE_5.8/Engine/Binaries/Mac/UnrealEditor-Cmd' PRIVATE_PROJECT/ValleyData.uproject -run=pythonscript -script=WORKTREE/aethyr/tools/valley-unreal-export.py -nullrhi -unattended -nosplash -nosound -NoAudio -NoCrashDialog -corelimit=4 -stdout
coax/web/tool.sh aethyr/tools/valley-canyon-pack.coax SOURCE_DIR PRIVATE_PACK_DIR
coax/web/tool.sh aethyr/tools/valley-canyon-receipt.coax apps/openagents-web/assets/valley/canyon-poses.json
python3 aethyr/tools/valley_canyon_oracle.py SOURCE_DIR PRIVATE_PACK_DIR apps/openagents-web/assets/valley/canyon-poses.json
```

`source-identities.json` pins the original mesh/material/texture/actor packages
and source exports. The full private pack's SHA-256 is
`9087df269134827845b90554bbd23d6b63ec790a49cdf570fa95300d56fda96c`.
A new export requires a deliberate identity/policy update; the running view
will refuse other bytes.

## Pack integrity and device recovery

The optional [pack digest gate v1](../../../coax/web/PACK-GATE.md) makes the
browser hash the complete received pack before unpacking or allocating its GPU
resources. Its 32-byte digest and byte count go to Coax;
`verse/valley/canyon-pack-policy.coax` pins both. The browser performs the hash
effect, and the authority module decides acceptance. A same-length byte edit
inside the vertex payload is refused before any upload. The entire geometry,
index and both texture payloads are covered by this one content identity.

The device-loss journey exposed a legacy pack bug: recovery reset ordinary
asset rows while packed asset rows still held old-device resources. Shader
binding could use those old buffers before their replacement upload. Recovery
now clears the pack rows as well. The actual source view's state digest and
GPU crop are identical after replacement; the expected device-loss diagnostic
is the only console error in that journey.

The source view declares a separate 640 MB resource budget and 12.1 million
submitted vertices, including both source instances in the scene and shadow
passes. The accounted arena, source pack, geometry/index and complete 4K
mipmap layers total 287,772,840 bytes. This counter is the declared-resource
accounting scope, not a browser-process peak-memory measurement. No FPS or
mobile performance target is claimed. The original lab keeps its existing
80,000 submitted-vertex cap and 9.5 MB byte budget.

## Verification

- Source pose/model and packer helper checks pass.
- Source view native suite: 75/75; six current Canyon checks rerun after the
  private-loader diagnostic counter change.
- Source shaders' scalar helpers pass and their committed WGSL matches Coax.
- Pack authority mutation: 13 mutants killed, zero survivors.
- Renderer adoption authority mutation: 47 mutants killed, zero survivors.
- Independent oracle: every source triangle corner/map/pose/normal checked as
  described above, including the full VPK table and source PNG bytes.
- Actual muted headless WebGPU at desktop and phone viewports: full source counts, native/
  Wasm matrices, five distinct diagnostic crops, both isolated poses, exact
  reset, same-length source corruption refusal, and identical state/crops after
  GPU replacement.
- Actual parent P-key and phone touch navigation opens the private source
  view; its `1` action returns to the parent.
- Existing desktop/phone scan station journeys pass through the updated host.
- Missing private source displays an explicit message and keeps scene readiness
  false; it does not claim an empty successful source frame.
- Viewport tests run on this Mac's Apple Metal adapter; real phone hardware and
  cross-platform performance have not been qualified.
- Realm page generation, new Wasm budgets, tools policy and whitespace checks
  pass. See the adjacent logs and artifact identities.

After rebasing onto `53a44629f5`, the updated host's source-view and parent
navigation journeys passed again. Their receipts are `rebased-host-browser.txt`
and `rebased-host-navigation.txt`. The source view's runtime/shader dependencies did not change. The parent
walker changed only a test step-budget annotation. The new host pack/action
paths were the reason for that repeat qualification.
