# Ocarina Lab

An unlisted owner-local source scene and character lab at `/worlds/ocarina-lab?pack`. It reuses
Aethyr's paid fixed steps, walk trace, follow camera, collision and touch sticks,
and Animation Lab's inspection conventions. No Nintendo payload is in Git.

The native source import, texture layout/channel conversion, material alias
resolution, pack construction, admission, animation and scene logic are Coax.
There is no Python or Blender dependency in this build/playback path. Standard
ASTC blocks are decoded by ARM astcenc 5.7.1; standard BC blocks by the pinned
MIT/Unlicense bcdec header and its small reference runner. These SDKs do not own
BFRES/BNTX structure, Switch tiling, material selection or gameplay.

## Source profile

The source model has 110,515 vertices, 72,778 triangles, 309 bones, eight skin
influences, UV0/UV1, tangents and colors. Eight original clips retain translation,
quaternion rotation, nonuniform scale and per-clip Maya SSC flags. Sampling uses
an explicit 30 source frames/second assumption, not a measured game clock.
See `aethyr/tools/ocarina-source-format.txt` for the binary schema and source
references. The earlier character-only private pack was 49,448,496 bytes with SHA-256
`fad738db36516efd185a81b11ce00538096d4cbb4a7b824fc8d2d02d3fb585c7`.

`ExternalBinaryString.bfres` resolves material name hashes. `UnifyTex.Product.100.byml`
resolves ten missing references to nine shared texture payloads, including the
adult hair *textures* used by the child model. Original child geometry is kept.
The 69 decoded PNGs supply 60 unique runtime layers and 25 material records.

Hair uses source albedo/normal/roughness/AO on UV0 and coverage
`albedo.alpha(UV0) * Gn3.red(UV1) >= 0.5`. Gn0 is a specular LUT, not a replacement
base color. BNTX block-height and component selectors are preserved; normal XY
comes from semantic RG after selection. The old flat white hair and scrambled
coverage came from omitted aliases, UV1, and vertical GOB block height.

The lighting is an Aethyr material translation. Skin scattering, anisotropic hair
response and layered eyes are approximations; the outer corneal/eye-shadow meshes
are suppressed in this opaque inspection renderer. This is not a reproduction of
the game's full shader graph, physics or hair/cloth simulation. The current
forest uses original source scenery, placement and ground; its collision and
admission boundaries are described below. The old procedural forest is removed.

## Rebuild locally

Keep the decompressed original files in an owner-private directory, called `$src`
below. The local prepared copy is under
`~/.openagents/verse/private-build/ocarina/source`; generated streams and textures
are siblings. No NUP download is required to rebuild from that prepared source.

```sh
src="$HOME/.openagents/verse/private-build/ocarina/source"
out="$HOME/.openagents/verse/private-build/ocarina/streams"
tex="$HOME/.openagents/verse/private-build/ocarina/textures"
coax/web/tool.sh aethyr/tools/ocarina-source-pack.coax \
  "$src/Player_Child.Player_Child.bfres" \
  "$src/Player_Child_01_Animation.anim.bfres" \
  "$src/Player_Child_02_Animation.anim.bfres" \
  "$src/Player_Child_03_Animation.anim.bfres" \
  "$src/Player_Child_04_Animation.anim.bfres" \
  "$src/Player_Child.anim.bfres" "$out"
cc -O2 aethyr/tools/ocarina-bc-reference.c -lm -o /tmp/ocarina-bc-reference
coax/web/tool.sh aethyr/tools/ocarina-texture.coax \
  "$src" "$tex" /path/to/astcenc /tmp/ocarina-bc-reference
coax/web/tool.sh aethyr/tools/ocarina-material-pack.coax \
  "$src/Player_Child.Player_Child.bfres" "$src/ExternalBinaryString.bfres" \
  "$src/UnifyTex.Product.100.byml" "$tex" "$out"
k="$HOME/.openagents/verse/private-build/kokiri"
# Prepared Kokiri inputs: see OpenAgentsInc/ore/docs/ocarina-of-time/kokiri-extraction.md.
coax/web/tool.sh aethyr/tools/kokiri-bundle.coax "$out" "$k/scene-v4" \
  "$k/scene-model" "$k/atlas-final" "$tex" "$k/terrain-decoded/world-collision.kcg" \
  "$HOME/.openagents/verse/private-build/web/ocarina-lab.pack" "$k/atlas-final"
coax/web/wasm.sh apps/openagents-web/ocarinalab.coax \
  apps/openagents-web/ocarinalab-face.c apps/openagents-web/build/ocarinalab.wasm
coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/ocarinalab.coax \
  apps/openagents-web/ocarinalab.wgsl apps/openagents-web/gpu/ocarinalab.module
```

Run the existing watching dev server and open `/worlds/ocarina-lab?pack`. The
`?pack` opt-in is required by the existing local private-pack host. A static
public build has no source assets. A changed source pack requires an intentional
SHA/length update in the Coax admission policy, never silent fallback.

## Earlier character-only verification

The importer passes 70 checks. Rig admission kills all 250 root mutants; pack
identity kills all 22; renderer adoption kills all 58. Texture and material tools
pass their 63 and 69 loaded checks. Nonuniform scale and zero-scale SSC regression
checks pass, including a case which failed with reciprocal parent-scale chaining.
That character-only runtime budget covered desktop, phone and high-DPR frames under 1.2 GiB,
including all 60 mipmapped 1536-square layers and retained source bytes.

`coax/web/ocarina-check.coax` runs muted, headless Chrome against the actual
private source pack. `--verify` checks ordinary lowercase walking, running,
jumping, backflip, pause, all eight clips, frame stepping, all four material views,
blur and touch movement. It captures actual GPU readback as PNG; macOS headless
Chrome's compositor screenshot was black despite valid WebGPU output.

The final 390×844 browser run passed all 32 checks, including forced GPU loss,
recovery in 720 ms, identical complete state digest, and zero private-pack network
requests during recovery. The normal render had no shader errors. Its declared
resident-byte measurement was 1,052,146,064 bytes, below the 1.2 GiB ceiling.
The initial target Wasm was 68,563 bytes. Reassembling from the durable private streams
reproduced the complete pack SHA above. These are local validation results, not
an original-game visual parity or frame-rate qualification.

```sh
coax/web/tool.sh coax/web/ocarina-check.coax \
  'http://127.0.0.1:8837/worlds/ocarina-lab?pack' /tmp/ocarina.png --verify --recover
```

## Forward-loop correction

The original run carrier moves 1.9500002 m over 26 source-frame intervals. The
initial lab added that animated displacement to controller travel, then wrapped
it to zero every 0.8666667 s. This caused the reported recurring backward chop.
A regression against a translating carrier failed before the fix. A second
regression covers returning from a raw moving inspector pose during a crossfade.

Play now removes only carrier X/Z transport after blending. It keeps original
vertical motion and rotations, all child poses and the unmodified inspection
tracks. Controller speeds are derived from the real carrier endpoints: walking
0.64 m/s, running 2.25 m/s and the dash clip 2.655 m/s, at the explicitly assumed
30-fps source clock. Walk/run limits use those measured clip speeds. Gait time
scales with actual collision-resolved distance each paid step; landing plays the
original 0.2-second landing clip before returning to locomotion.

`coax test verse/ocarina/play.coax` passes all 434 loaded checks. The real-asset
`aethyr/tools/ocarina-motion-check.coax` checks eight seconds each of walk, run
and dash: five, nine and ten loop boundaries, with zero planar carrier residual
and strictly forward pelvis motion. The headless browser's `--motion` path holds
forward across multiple run loops and checks the same pelvis placement used by
the renderer, including drawing interpolation and yaw, rather than only checking
the controller position. Use `--verify --motion --recover` for the complete flow.
The final 960×600 motion run crossed six run loops with zero carrier residual;
every rendered pelvis step stayed forward (0.005524–0.092859 m per 30-Hz frame,
including the initial acceleration and clip transition). No shader or console
errors were reported. The updated Wasm is 70,026 bytes. The earlier complete
36-check browser run also passed inspection, touch, landing/jump transitions and
GPU recovery; the final motion capture uses the renderer's exact pelvis placement.

These changes align travel and animation to the recovered source data. The
current source scene retains this correction. Its expanded private pack has a
different identity, pinned in `verse/ocarina/pack-policy.coax`; the historical
character-only identity above is not its current admission pin.

## Original Kokiri scene admission and collision

KOC1 is the original source catalogue and remains accepted as opaque-only input. Its 64-byte header binds the complete
OCL1 rig, complete KCG1 collision index, finite spawn and dynamic GPU byte,
index, texture-layer and material counts. GPU geometry mixes 128-byte character
records and 64-byte static records, so admission bounds total bytes without
pretending that the complete upload has one vertex stride. Materials and atlas
layers have independent counts. The unchanged whole-pack SHA/length gate remains
separate from structural validation.

`scene-policy.coax` validates the entire collision stream before publication:
bounded finite coordinates, exact nonoverlapping sections, contiguous cell
ranges, sorted in-range triangle IDs and the declared maximum candidates per
cell. It delegates rig validation to the existing OCL1 gate. The C browser face
owns one 96 MiB source staging buffer. After successful admission its capacity
closes to further writes: collision borrows that immutable retained buffer,
while the smaller rig is copied into the existing animation state. GPU recovery
retains these owners. Readiness requires the catalogue, both source streams,
whole-pack authentication and all exact declared uploads.

The source collision bundle contains 1,238,100 triangles: 481,558 original dense
ground BFRES triangles plus original child Phive physical and smooth stair
geometry. Two-metre bins contain 2,377,095 references, with at most 1,536 per
cell. Original standalone shape bytes independently identify physical section
11, stair-slope section 12 and detailed stair-step section 13. Original CCT
mask definitions include the first two and exclude detailed stair steps. The
runtime uses physical and smooth stair surfaces. The one unsupported cylinder
is reported rather than replaced with an invented box. The TARB terrain wrapper
can be unpacked, but its inner height descriptors remain unresolved; the dense
original rendered ground is the explicit grounding fallback.

Every paid 1/64-second step runs shared movement, source capsule resolution and
source floor selection before publishing its history or advancing gait from
actual resolved travel. The floor query is capped at the previous feet plus
the recovered 0.35 m step height, preserving raised bridges without selecting
roofs above the player. Reset uses `(-12, 110.052536, 0)`, a tested open village-floor location. This is a lab choice, not a recovered original-game spawn. The raised log floor at `(-45.6, 116.50062, -9.74)` remains part of the source regression. The shop approach base floor is approximately 110.6042 m.
The follow camera queries source walls and ceilings with bounded indexed line
segments. This does not provide swept camera-volume collision.

The original child standing shape is a prism with radius 0.23 m, head height
1.17 m and shoulder height 1.02 m. The lab uses an explicitly approximate
capsule with radius 0.23 m and total height 1.17 m. It keeps three local
depenetration passes, a 55-degree steep-face split and two-sided floor tests.
Original Havok margins, interior/backface filtering, material response,
continuous collision and the original controller's 74.9-degree maximum slope
and slip behavior beginning at 50 degrees are not reproduced. Serialized
contact-skin overrides were not found; omitted compiled defaults are unknown.

The exact Ocarina route and producer now have their own immutable collision
profile. The Wasm declaration is 4,096 pages (256 MiB), leaving Animation Lab's
existing declaration intact. The browser exports a 256 MiB storage binding
ceiling for the expanded GPU stream. Resident accounting derives geometry and
all mipmapped texture bytes from the catalogue, includes the retained encoded
pack, and uses a four-GiB frame budget. Guard probes return JavaScript numbers
without truncating resident counts to signed 32-bit integers.

Initial KOC1 verification: all 455 loaded runtime checks pass; scene admission kills
all 258 root mutants, collision-profile binding all 39 and renderer adoption
all 59, with zero survivors. The actual final foliage catalogue admits 65,766,052
source bytes, 196,137,856 GPU vertex bytes, 10,234,311 indices, 163 physical texture layers and 758 materials. Native verification through the production
`ocl_step` checks eight paid steps against eight frames, jumping and landing,
128 bridge/shop movement steps and 360 samples spanning three loops each of
original Wait, Run and Facial tracks. The jump rises 0.59765625 m; the maximum
observed candidate work is 8,774 tests per frame. Original planar carrier
transport remains removed in play and preserved in inspection.

```sh
coax/web/tool.sh aethyr/tools/kokiri-lab-runtime-check.coax \
  "$HOME/.openagents/verse/private-build/kokiri/atlas-final/scene.catalog"
```

See `aethyr/tools/kokiri-collision-format.txt` for source layouts, exact geometry
matches, index schema and regeneration commands. These native checks do not
claim original-game physics or final browser visual parity; browser verification
belongs to the assembled current pack and renderer.

## Selected private scene and browser receipt

The selected `scene-v4` bake contains 2,346,143 vertices, 2,691,473 triangles,
724 scene-local materials and 3,202 source instances. All 24 previously unsupported
meshlet shapes across 18 models now decode from the original shader-proven format.
The separate dense source ground and 4,253 source-placed impostor plants join Link
in the runtime pack. The model journal retains 43 unavailable model paths; cubemap
and hiding-hole helpers are explicitly excluded. This is a scenery reconstruction,
not the game's story/NPC/effect systems.

Current private pack: **598,882,100 bytes**, SHA-256
`56f795206044619cc9abf614f3bf1193117ed27251f4d4caa7c523ab0ccc43ba`.
The KOC2 catalogue is 65,766,068 bytes; declared high-DPR residency is
3,925,600,224 bytes against a 4,294,967,296-byte budget.

Muted headless Chrome verifies keyboard/run/jump/backflip, all eight clips,
inspection, pause/frame stepping, material views, blur release and touch movement
at desktop and phone sizes. Device loss recovers from retained private bytes with
no refetch and an identical source/player digest. Actual forward movement crosses
five original run loops: zero planar carrier residual, visible steps between
-0.001793 and 0.092859 m. These are observed controller/renderer results, not a
claim of original Havok or pixel parity.

Static maps use explicit 496-square derivatives in padded 1536-square pages;
character layers retain their existing layout. Original 128-square foliage
images use the fixed-card layout below. The original full source maps remain private. Shared terrain-array surfaces
use an explicit source moss adaptation while TerrainArc blending is unresolved.
Second-albedo-only props use their bound source second map; inner/fade meshes use
an explicit dark adaptation and bake-shadow proxy meshes are suppressed. Water and sky use the appearance adaptation described below. Native terrain/material graphs, baked light, full plant
growth/wind/coverage, water/reflections and story-dependent actor visibility remain
limits. The complete extraction report and 3,476-entry verified source manifest
live in [ore](https://github.com/OpenAgentsInc/ore/blob/main/docs/ocarina-of-time/kokiri-extraction.md).

## Water, sky and ground appearance pass

The previous ground fallback projected `Sand_Kokiri_MossGround_A_Alb` across the
world, exposing the stretched padding around its authored UV islands. The new
fallback uses the original tiling `Plant_Kokiri_MossGround_C_*` and
`Sand_Kokiri_HouseSoil_A_Alb`, with restrained world-space soil/moss variation.
It still does not decode native TerrainArc material blending.

The original front/back flow-map meshes have identical positions. The back helper
is suppressed instead of competing for the same depth values. Water now draws
in a separate alpha pass after opaque geometry, with depth testing and no depth
writes, using Aethyr's existing `coaxial_draw_blend_v1` profile 2. A separate shadow
list excludes water. The original indices are stably partitioned into 10,218,582
opaque and 15,729 water indices, preserving their total and all source triangles.

KOC2 is a separate catalogue magic/version with an 80-byte header: it keeps the
KOC1 field positions but sets rig offset 80, adds opaque/water index counts at
64/68, and reserves zero words at 72/76. Both nonempty spans must be aligned to
triangles and cover the complete index buffer. The existing rig/collision
validators are reused. KOC1 retains its 64-byte format and opaque-only behavior.

The sky follows camera rays and includes a blue zenith, warm horizon, cloud veil
and sun glow. Height-sensitive haze and warmer direct light replace the uniform
olive fog. Water normals animate smoothly; view-angle Fresnel controls reflection
and opacity, so submerged geometry remains visible. Reflections sample this sky,
not the forest geometry. Native water flow/refraction, environment reflections,
terrain blend and lighting bakes remain unported.

Verification: 459 runtime checks, 71 material checks, 53 bundle checks and 31
shader checks pass; KOC2 admission kills all 105 root mutants, pack identity all
22. Native original-source movement/jump/clip checks pass. Muted 1920×1080 Chrome
passes controls, all clips/views, and retained device recovery with no source
refetch or unexpected GPU errors. Pond and upward-sky captures are inspected
separately, alongside the phone viewport.

## Foliage stability and camera pass

The camera-facing implementation rotated entire branch silhouettes and switched
source views at camera-yaw boundaries. That made nearby leaves appear to slide,
turn and pop during walking. The replacement bakes two crossed source-view
silhouettes in the original placement basis (one for one/two-view recipes).
Material kind 14 uses ordinary static world vertices in both visible and shadow
passes; no camera-selected view remains. These are fixed source cards, not a
recovery of native three-dimensional plant generation, wind or growth.

All 4,253 admitted placements remain; the same 4,867 unmatched placements remain
omitted. Thirteen exact source recipes use 26 original 128-square images without
resizing. Two 1536-square albedo/normal pages use 256-square cells with 64-pixel
edge-repeated gutters. The PNG round-trip checks preserve their assembled bytes.
Softer two-sided lighting and restrained foliage tint reduce bright yellow-green
clumps. This pass emits 8,506 polygons, 51,036 vertices and 102,072 indices.

The current catalogue contains 183,503,872 GPU vertex bytes, 9,839,499 indices
(9,823,770 opaque; 15,729 water), 751 materials and 149 physical texture layers.
The private pack is 53,765,524 bytes smaller. Regeneration uses `atlas-fixed`
with `kokiri-foliage-pack.coax` and `kokiri-bundle.coax`.

Play camera pitch is constrained to 0.45–1.85 radians to avoid the extreme
below-floor orbit on level ground; inspection orbit is unchanged. Camera-volume
collision is still absent, so nearby foreground rocks can occlude a low orbit.
The sun-shadow origin snaps to a fixed light-space texel lattice to reduce
shadow crawling during movement. Source collision and animation payloads stay
unchanged.

Verification: 461 runtime checks, 162 foliage-assembler checks and 32 shader
checks pass; all 22 pack-identity mutants are killed. The shader checks hold
world positions, normals and shadow coordinates invariant over eight camera
poses. Native original-source movement/jump/clip checks pass. Muted headless
Chrome at 1280×800 passes controls, seven actual GPU walk/orbit readbacks over
2.268 m of lateral travel, and retained device recovery without source refetch.
The walk/orbit images were visually inspected. The 390×844 phone check covers
the same controls; receipts and capture hashes are recorded in ore.
