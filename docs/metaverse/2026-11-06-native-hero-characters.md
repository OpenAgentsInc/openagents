# Native hero characters in Metaverse Engine worlds

Echo now renders natively in Everglade through the shared character pass:
one renderer, one frame and the world's depth buffer. The local recording
milestone (#12984, R1–R4 below) has retained 1920×1080 recordings and historical
60 FPS qualification. A fresh November 7 audit fixed a commons-spawn regression,
but current full-raster timing misses the 60 FPS gate on both the current build
and a controlled R4 baseline. The issue remains open. The
[rendering follow-up](../verification/everglade-echo-12984-2026-11-07/performance.md)
passes deterministic image and compatibility checks, but retains the live FPS gate. The
[audit record](../verification/everglade-echo-12984-2026-11-07/README.md)
separates historical evidence from the fresh results.

The earlier [composite](../verification/valley-lab-slay-2026-11-06/README.md#echo-in-everglade-n-stageecho)
is historical. The broader H-series plan below covers the remaining engine,
production delivery and native-host work beyond this local recording milestone.

## What native means

Echo is native in a world when all of these hold in the world's own page,
with no second render and no compositing:

- She stands in the world's depth buffer. Grass, posts, players and rain pass
  in front of her and behind her correctly.
- The world's lights light her: the sun or moon, the lamps and torches, the
  sky's ambient, fog and wet weather.
- She casts shadows into the world's shadow map and receives them from it.
- She is an entity with a collision capsule. Players can walk up to her,
  around her and into her, and the camera can't pass through her.
- Her body and face animation run on the world's one clock, and the room
  replicates her state like any other character's.
- The camera move is an ordinary staged camera in the world.
- The same code works in a second world without changes to the character.

## Original design baseline and foundations

This table records the design's starting constraints. The R1–R5 sections below
describe the delivered implementation and its measured limits.

| Piece | Where it lives | State |
| --- | --- | --- |
| Rig, body pose, face playback | `metaverse-engine/character/` (on `metaverse-engine/anim/pose.coax` and `face.coax`); Echo's declarations in `metaverse/valley/echo.coax` | Moved out of Valley Lab in R2 (#12986): a `Character` record over a world's pack table, sizes under declared maximums, an idle face. Valley Lab runs on it with its TF0290 frames unchanged |
| Skinning, morphs, groom, materials (GPU) | `apps/openagents-web/gpu/valleycanyon.coax` with `metaverse-engine/render/shade/hair-fiber.coax`, `metaverse-engine/anim/morph-gpu.coax`, `metaverse-engine/render/shade/pbr-env.coax` | Works in the Valley page only. Character shading is branches inside that page's `scene_fs` |
| Lights | `metaverse-engine/render/shade/slay-lights.coax` | The TF0290 studio rig, in the Valley page's own uniform slots |
| Post pass (depth of field, skin blur) | `vc_post_fs`, run by `coax/web/host.js` | Generic host feature; any page with a `post_fs` gets it |
| Pack | `~/.openagents/verse/private-build/web/valley-slay.pack`, pinned by `metaverse/valley/canyon-pack-policy.coax` | 1,023,634,576 bytes, owner-local. Its 35 mipmapped 4K RGBA8 layers reserve about 3.13 GB of GPU memory |
| Groom | Valley pack | 118,339 curves, drawn as strands at a share set by the head's size on screen |
| Everglade's characters | `apps/openagents-web/gpu/everglade-v1/gpu.coax` (3,597 lines), `metaverse-engine/anim/skin.coax` | CC0 Quaternius people, skinned from a storage buffer with four joints each, painterly shading |
| Everglade's budgets | `apps/openagents-web/wasm-budgets.txt` | 2,000,446 of 2,150,000 wasm bytes used; 60 FPS rule from `metaverse-engine/core/frame-report.coax` |

Three constraints shaped the original plan:

1. **The host originally gave a page one scene shader.** `host.js` built one
   `scene_vs`/`scene_fs` pipeline per page (plus blend variants). The Valley
   page shades Echo by branching inside its own `scene_fs`. Copying those
   branches into Everglade's shader would make a second copy to maintain,
   and every further world would make another.
2. **Echo's data is far too large for a game.** About 3.13 GB of textures
   and a 1 GB download fit a lab on this Mac, not a multiplayer world on a
   player's laptop or phone.
3. **Echo's source is licensed Epic sample content.** Slay assets stay in
   owner-local packs ([private assets](private-assets.md)). The owner has
   settled that MetaHumans may be shown on openagents.com through the
   broker ([MetaHuman support](metahuman-support.md)), and separately approved
   showing Slay content on 2026-11-06 (owner decisions below). Production
   delivery remains a separate H7 milestone.

## Design

### 1. One character runtime in the Metaverse Engine

Move the CPU side out of `metaverse/valley/` into `metaverse-engine/character/` and make
it work for any asset source:

- `rig.coax`: joints, bind matrices, rest pose and capsules, sized from the
  pack (with declared maximums), not from Echo's constants.
- `pose.coax`: body clips, cross-fades and the root track, on the world's
  fixed step (`metaverse-engine/core/loop.coax`).
- `face.coax`: face clips, curves to morph weights and face bones. This
  absorbs `face-ambient.coax` (idle blinks and saccades), so a character
  standing in a world is never frozen.
- `palette.coax`: the skinning palette and morph weights uploaded each
  frame.
- `character.coax`: one `Character` record (pack, rig, pose, face, tier,
  world transform) with spawn, despawn and step functions. An entity owns
  it (`metaverse-engine/core/entity.coax`).

The public tests run on the CC0 Quaternius people that Everglade already
uses, as [MetaHuman support](metahuman-support.md) recommends. Echo is then
one private pack that the same runtime loads.

### 2. Character pack v1, in tiers

One pack format for Quaternius people, MetaHumans and Echo. Each pack
carries tiers, and the runtime streams the tier the camera needs:

| Tier | Used when | Mesh | Textures | Hair |
| --- | --- | --- | --- | --- |
| Hero | Head over about 300 px on screen | Full source mesh and face morphs | 2K per sheet, compressed where the device supports it | Strands, full share |
| Near | Body on screen, face small | Decimated body, face morphs kept | 1K | Strands at a low share, or cards |
| Far | Crowd distance | Low-poly body, no morphs | 512 | Cards |

- **Textures.** Use BC7 where the device offers `texture-compression-bc`
  (desktop) and ASTC where it offers `texture-compression-astc` (Apple and
  phones), with RGBA8 at a lower size as the fallback. The packer writes
  every variant, and the host picks one per device.
- **Targets:** a hero character uses at most 300 MB of GPU memory on desktop
  and 80 MB on a phone. The first download is the far tier, a few MB; the
  hero tier streams in when the camera comes close.
- **Groom levels.** Strand LODs already exist (the share rule). Add a cards
  tier, built by the packer from the same curves, for far views and phones.
- The packer is a Coax tool that reads the existing Valley exports
  (`metaverse/tools/valley-echo-bundle.coax` and `slay-shot-pack.coax` are the
  starting point). Rules from [private assets](private-assets.md) apply:
  packs stay out of git, and manifests in the repository name no private
  digest or placement.

### 3. A character pass in the host

Add a character pass to `coax/web/host.js` (and later its native hosts)
instead of merging character shading into each world's shader:

- **Its own shader module**, `metaverse-engine/render/shade/character.coax`, compiled by
  `coax wgsl` like other GPU modules: `character_vs` and `character_fs`
  (skin, eyes, teeth, cloth, metal), `strand_vs` and `strand_fs` (hair),
  and `character_caster_vs` for shadows. It is built from the Valley
  shading (`cv_scene_shade`, `cv_skin_light`, `cv_hair_vertex_light`), with
  the TF0290 constants moved into material data.
- **Its own bind group**: character storage (vertices, skin weights, morph
  deltas, strands), its texture array and the per-character palettes.
- **Shared with the world**: the depth buffer, the colour targets (including
  the `light` and `aux` targets the post pass reads), the shadow map and a
  lighting block (below). The pass draws after the world's opaque geometry
  and before its blended geometry, so depth tests and fog work both ways.
- **Shadows.** The character's caster draws into the world's shadow map in
  the world's shadow pass, and the character shader samples that map.

### 4. A lighting contract for worlds

A detailed character needs more than a sun direction. Each world fills one
`SceneLights` block every frame, which the character shader reads:

- the key light (sun or moon): direction, colour and intensity;
- sky and ground ambient colours, and a small environment term for
  reflections;
- up to 8 nearby point lights, picked by distance to the character
  (Everglade already chooses the lamps nearest the eye);
- fog colour and density, and wetness (rain darkens skin and leather and
  sharpens their highlights);
- exposure and the world's tone curve, so the character goes through the
  same grade as the world.

The Valley page fills the same block from `slay-lights.coax`, so the Slay
shots keep rendering the way they do today.

### 5. Matching the world's look

Everglade is painterly and Echo is photoreal. When both go through the same
lights, fog and tone curve, most of the mismatch goes away; the night
composite showed that. Each character pack can also carry an optional
painterly variant (softer specular, flatter albedo) that a world can ask for.
Which look a world uses is an art decision for the owner. The runtime
supports both.

### 6. Placement, collision and the network

- A world places a character as an entity with a position, a heading and a
  capsule from the pack, through `metaverse-engine/physics/query.coax`, so walking and the
  follow camera treat her like any body.
- The room protocol gets a character look id (a pack reference) and
  animation state (body clip, face clip, time). Clients without the pack,
  or not permitted to load it, draw the far tier or the capsule fallback
  that [MetaHuman support](metahuman-support.md) already defines.
- Echo's spot in Everglade, and the orbit camera, become an ordinary staged
  shot: `?stage=echo` spawns her and runs `metaverse/valley/echo-orbit.coax`
  directly. The Valley page's N mode and the composite script then retire.

### 7. Native hosts

The character module is Coax GPU code, so it goes through the same paths as
Everglade's shaders: `coax wgsl` for the web, `coax spirv` for Vulkan and
`coax metal` for the Mac and iPhone ([native GPU](native-gpu.md)). There is
no hand-written WGSL to port.

## Phases

Each phase ends on a check that runs on our own machines. The headless
capture tools (`coax/web/valley-capture.coax`, `coax/web/tool.sh shot`) take
the frames.

| Phase | Work | Exit check |
| --- | --- | --- |
| **H0** | `SceneLights` block and the character pass in `host.js`, drawing one CC0 Quaternius person through `metaverse-engine/render/shade/character.coax` in Everglade | That person stands in Everglade with depth, lamp light, fog, rain and two-way shadows; Everglade holds 60 FPS on the 4080 and this Mac; Everglade's golden frames are unchanged when no character is placed |
| **H1** | Move Echo's CPU runtime into `metaverse-engine/character/`; the Valley page uses it | The Valley page's TF0290 frames (1125, 1132, 1137, 1142, 1147) are byte-identical before and after; all Valley checks pass |
| **H2** | Move Echo's shading into the character module; the Valley page draws her through the character pass | The same five frames are within a stated tolerance of H1's (mean and 99th-percentile channel difference recorded) |
| **H3** | Character pack v1 with tiers and compressed textures; repack Echo | Echo's hero tier at or under 300 MB of GPU memory on this Mac; the far tier under 10 MB; the hero tier's frame 1125 within tolerance of the 4K render |
| **H4** | Echo in Everglade on local dev hosts: `?stage=echo` spawns her beside the player with an idle face and runs the orbit natively | The orbit clip recorded straight from Everglade with no composite; she passes behind posts and in front of grass; lamps light her; Everglade holds 60 FPS on this Mac with her on screen |
| **H5** | A second world places a character through the same API (Fairy Forest or Valley World) | Same checks as H4 in that world, with no changes to `metaverse-engine/character/` |
| **H6** | Room replication of character look and animation state | Two headless room pages see her in the same pose on the same tick |
| **H7** | Production delivery through the broker, plus the phone tier | Only after the owner settles the Slay licence question below. On staging, a phone-size page holds the frame budget with her in view |
| **H8** | Metal and Vulkan hosts | The H4 clip on the Mac host matches the web build within the native GPU tolerances |

H0 is the most important phase. It decides the pass and the lighting
contract, and it needs no private data, so it can run in public with CC0
assets.

## Owner decisions (2026-11-06)

1. **Slay content may go on openagents.com.** Echo goes through the broker
   like a MetaHuman pack ([MetaHuman support](metahuman-support.md)).
2. **Look:** as close to the current Slay version as reasonable graphics
   limits allow, in Everglade too. The painterly variant is not needed.
3. **Phones:** close-ups are not a phone goal. Phones get a low tier.

## First: Echo in the raid scene for recording

The owner's first priority is recording Echo in the Cultist Raid scene.
That needs only part of the plan, done directly on Echo on this Mac, with
her current pack through the local `/private/` route:

| Step | From | Work |
| --- | --- | --- |
| **R1** | H0, H2 | The character pass and the `SceneLights` block, drawing Echo with the Valley shading straight away (no Quaternius stand-in first) |
| **R2** | H1 | Echo's CPU runtime in `metaverse-engine/character/`, with the idle face, used by both pages |
| **R3** | H4 | `?stage=echo` spawns her beside the player with a capsule and runs the orbit natively; the clip records straight from Everglade |
| **R4** | H3 (part) | Groom levels and a frame-rate check while she is in a wide shot of the raid |

H3's texture tiers, H5 (a second world), H6 (replication), H7 (production)
and H8 (native hosts) come after the recording works.

### R1: the character pass (done, #12985)

`?stage=echo` in Everglade draws Echo through the host's character pass, in
the town's own depth buffer, light and aux targets and shadow map. The parts:

- **Shading.** `metaverse-engine/render/shade/character.coax` holds Echo's shading (her
  materials, skinning, morphs, strands and the skin and fibre light), moved
  out of `apps/openagents-web/gpu/valleycanyon.coax`, which now loads it.
  Its pass entries are `ch_character_vs/fs` (body and strands) and
  `ch_caster_vs/fs`. `apps/openagents-web/character.wgsl` is written from it
  (`gpu/character.module`, `gpu/character.bindings`).
- **SceneLights.** `metaverse-engine/render/shade/scene-lights.coax` lays out the block: the
  world's clip rows and camera, the shadow map's rows, the key light, sky and
  ground ambient, the haze, up to 8 point lights (the 8 nearest her that
  reach her), wetness and the aux distance scale. Everglade fills it each
  frame in `apps/openagents-web/old/v1/everglade-character.coax`, from the same
  uniforms its own shaders read. The pass writes linear light, and the
  town's post pass tones it as it tones the town.
- **Runtime.** `apps/openagents-web/character-host.coax` (`build/character-host.wasm`,
  41,715 bytes) runs Echo on `metaverse-engine/character/` beside the world: pack
  admission (SHA-256, catalog, part sizes), standing on the locomotion graph,
  her idle face cycle, the hair's share by her head's size on screen, her
  palette. Everglade's wasm grows by 10 KB (the block and four exports), and
  no Everglade visitor loads anything else until a stage places her.
- **Host.** `coax/web/character-host.js` loads the runtime, `character.wgsl`
  and her pack (`/private/valley-slay.pack` locally, the broker on
  openagents.com) only when the world's `coaxial_character_v1` returns a
  block. It draws her caster after the world's casters in the shadow pass,
  and her body, then her strands, after the world's opaque geometry and
  before its water and particles. A world that may place her asks the device
  for her 187,564,308-byte vertex binding (`coaxial_character_storage_v1`),
  as far as the adapter allows.

Measured on this Mac (headless Chrome, 1280×720 at DPR 1): the 1 GB pack
loads, is checked and uploaded in 10 to 13 s; the runtime's step takes under
0.1 ms; the close-up frame holds 60 FPS (the display's rate) with half the
strands drawn. `coax/web/everglade-echo-frames.coax` records the orbit's
frames, and the TF0290 frames stay byte-identical. Eyes take 0.35 of the
light (`CH_EYE_SHADE`): the town's map, about 6 cm a texel, can't shadow them
under her lids as Valley Lab's close map does.

Left for R3 and later: placing her by the stage rather than a fixed spot,
her capsule, the production render path (the render packages draw through
`render.js`, not this pass), a close shadow map for the face, and the Valley
page on the pass (H2).

### R3: Echo spawned natively, and the orbit recorded (done, #12987)

`/worlds/everglade?stage=echo` stages Echo in the town and records her
orbit with no second page:

- **Placement.** The stage places her: `rp_facts`
  (`everglade-raid-preview.coax`) puts the player at `RP_YOU`, facing the
  South Gate, and Echo a metre to his right, facing it too
  (`rp.echo_at`, `rp.echo_heading`). The block carries her feet and heading,
  and the runtime places her with `character_place` each frame.
- **Idle life.** Her body loops her rig's idle clip (`CHARACTER_IDLE`,
  clip 0), her face plays its idle segments, and
  `metaverse-engine/character/idle.coax` adds blinks, glances and breath on the
  joints and targets `ECHO_IDLE_RIG` names (all five found in her pack).
  The morphs switch on while a blink shows.
- **A body.** Once the host draws her (`coaxial_character_ready_v1`), she
  is one capsule: among the bodies you walk into (`everglade_bodies_near`,
  which `metaverse-engine/move/walk.coax` resolves through `metaverse-engine/physics/query.coax`), and in the
  follow camera's way (`camera_frame`, by `query_ray`). A page that never
  gets her pack has nothing unseen in its way.
- **The orbit.** N runs `metaverse/valley/echo-orbit.coax` as a staged camera
  in her own frame. It holds its last frame until you move, then your own
  camera comes back.
- **No room.** A staged page opens no room socket and no local host
  (`net_facts`): the local host's seat put the player at the room's spawn,
  outside the town, instead of beside her.
- **Retired.** Valley Lab's N (her orbit through Everglade's lens) and K
  (her matte, `u[131]`), and Everglade's J (a plate without the player),
  existed only for the composite and are gone.

`coax/web/everglade-echo-frames.coax URL OUTDIR clip` records the clip at
1920x1080, 24 fps, from `coaxialClipStep`. The player, signposts, grass and
rain pass in front of and behind her in it. `character-host.wasm` is 47,788
bytes (budget 52,000), and `everglade5e.wasm` 2,007,737.

Left for R4 and later: the groom's levels and a frame-rate check in a wide
raid shot, a close shadow map for her face, her locomotion when a stage
moves her, the production render path and the Valley page on the pass (H2).

### R4: groom detail and the raid frame budget (done, #12988)

Echo uses a native character pass in the live raid. Wide and medium shots hold
60 FPS on this Mac at both 1920×1080 and 1280×720. The frame timer records the
actual raster size as well as the viewport: these results render every requested
pixel. The existing graphics-quality ladder, gameplay and two-frame submission
limit are unchanged.

**Groom and face detail.** `metaverse-engine/character/detail.coax` is the shared CPU/GPU
rule. The original projected-head quadratic share remains above 96 pixels, with
a cheaper tier below 64 pixels and a smooth join between them. The minimum
effective share is 0.004; the close-up maximum remains 0.5. The source curves,
materials and fibre/dual-scattering lighting remain intact.

At load, `character-host.js` orders the original segment indices in 251 curve
groups, with stride 97. A draw uses the first `ceil(share * 251)` groups. The
shader recovers a group's rank using the inverse stride 44 and gives its final
group opacity `clamp(share * 251 - rank, 0, 1)`. Ribbons widen by the inverse
effective share. A new group therefore enters with zero coverage rather than
suddenly halving the existing ribbons' widths. Shadows use the same rule with
their share capped at 0.25. The wide run draws 18,486 segments; the boundary
group is mostly transparent at the minimum share.

Each ribbon's two triangles reuse their identical corners: four vertex
invocations instead of six, with the same endpoints, sides, UVs and material.
The body blends its eight palette influences once and reuses those rows for
position, normal and tangent. Face morph detail fades continuously from zero
at a 32-pixel head to full detail at 64 pixels. Source body animation, face
bones and idle life continue; skinning never narrows the original influences.

**The remaining cost was in the town's rain.** A diagnostic suppressing only
Echo's draw calls, while retaining her loaded resources, animation, room state,
spells and lights, still measured about 49 FPS at 1080p. The rain shader queried
up to 32 building footprints for each of four layers at every shaded pixel,
before knowing whether that pixel contained a visible drop. It now rejects
faded, empty and zero-coverage candidates first. A frozen pre-change oracle
checks exact float colour bits over 384 ray fixtures, including visible rain.
Outside-shadow-map pixels also return their existing fully lit result before
performing unused PCF comparisons.

Fire and smoke reject provably empty billow regions and fully depth-hidden
pixels before their noise/lighting work. The conservative outer radius is
`(1 + raggedness / 2) * clamp(cover, 0, 1)`, derived from the existing noise's
maximum. Another 400 fixtures check that rejected points have no visible
original coverage. These changes preserve the original rain, roof occlusion,
shadow bounds, particle colours and spell simulation.

**Measurements.** Apple M5 Max, 128 GB, AC power; competing 3D tabs closed,
muted headless Chrome with hardware Metal WebGPU, DPR 1, normal AUTO graphics,
`clock=0.95&weather=rain`. Wide starts at Echo's first cast; medium holds the
orbit's last frame. Each wide/medium condition passed three five-second windows (12 windows,
3,600 drawn frames in total), with zero held frames in every window. The close-up is
the orbit's first 1.2 seconds and is recorded rather than required to hold 60.

| Shot | Actual raster | FPS | Frame p50 / p95 / max, ms | Held frames |
| --- | --- | ---: | --- | ---: |
| Wide raid | 1920×1080 | 60.0 | 16.665 / 16.670 / 16.670 | 0 |
| Wide raid | 1280×720 | 60.0 | 16.665 / 16.670 / 16.670 | 0 |
| Medium | 1920×1080 | 60.0 | 16.665 / 16.670 / 16.670 | 0 |
| Medium | 1280×720 | 60.0 | 16.665 / 16.670 / 16.670 | 0 |
| Close | 1920×1080 | 59.2 | 16.665 / 16.670 / 33.330 | 1 |
| Close | 1280×720 | 60.0 | 16.665 / 16.670 / 16.670 | 0 |

Median p50 / p95 across the three windows (milliseconds):

| Shot / raster | CPU | Submit-to-done observation |
| --- | --- | --- |
| Wide 1080 | 2.965 / 3.335 | 13.695 / 19.360 |
| Wide 720 | 2.895 / 3.450 | 10.545 / 11.915 |
| Medium 1080 | 2.105 / 2.515 | 10.935 / 11.550 |
| Medium 720 | 1.015 / 1.090 | 9.440 / 9.925 |

The rule is unchanged: at least 31 drawn frames, p95 at most 17.5 ms and no
frame over 33 ms. A held callback counts in the next drawn frame's elapsed
time. GPU submit-to-done observations include queued work and callback latency;
they are not isolated GPU execution timestamps. Before the rain reorder, an
otherwise identical character configuration missed the wide-shot rule at both
sizes; after it, the wide/medium windows have no held frames. The timer's
`no-character` diagnostic is labelled and cannot return a hero qualification.

After rebasing onto `267ffbdc2b`, the renderer, room simulation and pack
companion were rebuilt. All four wide/medium cases passed again at their full
requested raster: 300 drawn frames each, 60.0 FPS, p95/max 16.670 ms and zero
held callbacks. This additional run was on battery (32–28%), not AC; the
three-run AC table above is kept separate. CPU p50/p95 and submit-to-done
p50/p95 for that final-source check were:

| Shot / raster | CPU, ms | Submit-to-done, ms |
| --- | --- | --- |
| Wide 1080 | 1.470 / 1.650 | 11.995 / 21.965 |
| Wide 720 | 1.695 / 1.905 | 10.385 / 13.780 |
| Medium 1080 | 0.990 / 1.085 | 10.005 / 14.200 |
| Medium 720 | 0.985 / 1.090 | 9.510 / 12.590 |

**Frame review and regression checks.** New captures contain 217 orbit frames
and 246 raid frames at 1920×1080/24 fps. Every projected head was recentered into
review sheets, not just a sparse set of crops. The orbit contains 57 groom-count
transitions; no abrupt density change or holes were observed at those transitions.
The raid retains a stable far tier, including running, casts and foreground
occlusion. The largest image changes align with occluders and spell flashes;
image differences alone are not treated as proof of no popping. Partial group
weights and face fades have independent continuity checks.

`valley-slay-frames` passes its five source frames, return to walking and isolated
face check with no console errors. `valleycanyon.wgsl` remains byte-identical;
Valley's `cv_*` rendering paths are unchanged. Focused checks pass: character
runtime 126, character renderer 99, effects 95, frame timer 141, and the rain
oracle. The character runtime is 62,513 bytes within its unchanged 64,000-byte
budget. The whole-site shader checker reports zero stale modules and the
pre-existing unrelated `valleyworld.coax` return-type failure.

Reproduce the owner-local evidence (no private renders or packs are committed):

```sh
base='http://127.0.0.1:8931/worlds/everglade'
out="$HOME/.openagents/verse/private-build/echo-r4-finish"
mkdir -p "$out/raid"
coax/web/tool.sh coax/web/everglade-echo-perf.coax \
  "$base?raid=preview&stage=echo&clock=0.95&weather=rain" wide 1080
coax/web/tool.sh coax/web/everglade-echo-perf.coax \
  "$base?stage=echo&clock=0.95&weather=rain" medium 1080
coax/web/tool.sh coax/web/everglade-echo-frames.coax \
  "$base?raid=preview&stage=echo&clock=0.95&weather=rain" "$out/raid" raid \
  > "$out/raid-capture.log"
coax/web/tool.sh coax/web/character-detail-check.coax \
  "$out/raid" "$out/raid-capture.log" "$out/raid-heads"
```

The private `raid.mp4` and `orbit.mp4`, full frames, per-frame detail observations
and all head-review sheets are retained under that output directory. The helper
reports adjacent-image differences for review; it does not automatically claim
visual parity from a low numeric difference.

### R5: Echo fights and casts in the Cultist Raid (done, #12996)

`/worlds/everglade?raid=preview&stage=echo` puts Echo in the raid on the
watch's side. Everything she does comes from the room, as every raid body's
does:

- **The room.** The local host sends `hero 1` with the raid's control lines
  when the URL has `stage=echo` (`coax/web/room-worker.js`). As the siege
  begins, `ev_siege_begin` (`coax/web/room/room-events.coax`) calls
  `e5rd_hero` (`everglade5e-room-raid.coax`): Everglade's level 9 druid
  (`fb_everglade_hero`) on the town's team, form `E5RR_FORM_HERO`, with the
  watch's ranged at the front. She can be hurt but never drops, and the
  cult's fall pays no XP for her.
- **Her turns.** `rt_hero_aim` (`everglade-raid-tactics.coax`) picks her
  spell and its point: Meteor Swarm first, centred where it reaches most of
  the cult, then by how many stand together (Meteor Swarm for three or more,
  Mega Thunderbolt for two, Water Orb for one close, Thunderbolt else).
  The pages hear the cast begin (`e rc`, look `RT_K_HERO0 + k`). She stands
  still while the spell's bar runs; the town takes its strike as the bar
  ends; the rules roll it with `rs_cast` as it lands (Meteor Swarm 3.6 s
  after it begins), with the players and the watch stepped out of it, as a
  player's harm keeps them.
- **The page.** `net_facts` opens the local host for `stage=echo` only with
  `raid=preview`. The room's Echo is drawn by the character pass, not the
  town rig: `rp_echo_room` places her from the creature, and
  `everglade_character_motion` writes the block's motion slots (116 to 125
  in `metaverse-engine/render/shade/scene-lights.coax`): her pace, her cast, its time, bar
  and point, and the ground's rise under her.
- **Her motion.** `character-host.coax` runs her own ECH1 locomotion graph
  (idle, walk, jog, starts and stops) at the room's pace. The town rig's
  clips aren't retargeted onto her: her own clips are better, and loading
  the 65-joint town rig would not fit the runtime's budget. A cast plays
  TF0290's source frames 1126 to 1152 from her pack's shot as the wind-up,
  ending as the bar ends. Then `metaverse-engine/character/fix.coax` (A3's solvers on
  the character runtime) plants her feet on the town's ground while they
  touch it, lowers her hips for a hold out of reach, turns her head toward
  the spell's point and reaches her orb hand toward it.
- **The shot.** The camera follows close behind her right shoulder as she
  runs. Through a cast it goes over her shoulder on the line from her to the
  spell's point, below the South Gate's beam, so she stands in the
  foreground and the spell lands beyond her (`rp_echo_follow`).
  `coax/web/everglade-echo-frames.coax URL OUTDIR raid` records it at
  1920x1080, 24 fps. In clip mode the local room runs on the clip's clock
  (`coaxialClipRoom`: `roomHost.advance`, and the page's `coaxial_now_ms`),
  and its random numbers come from a fixed seed, so every frame sees the
  room at its moment. `castfps` measures the frame rate in real time
  through her first cast.

`character-host.wasm` is 61,013 bytes (budget 64,000), and `everglade5e.wasm`
2,011,505.

Measured on this Mac (headless Chrome): the recording is 246 frames
(10.25 s): she runs from beside the player to the South Gate, stops, winds
up and casts Meteor Swarm on the cult's line, and it lands. Her planted feet
held to under 0.1 mm of their locks (the farthest a planted foot strayed and
the most one travelled, from her pass's probes 23 and 24) over 34 plants;
the same in real time (27 plants). Through her first cast the page holds
60 FPS (the display's rate) at 1280x720, and her runtime's step takes
0.13 ms. After the cast she moves along the palisade and the camera behind
her goes into the gate's tower, so the recording ends as the swarm lands.

Left for later: a staged camera that keeps clear of the town's walls (the
follow camera's scenery test, on a staged ray), her face's expression during
a cast, the bystander player (the page's seat stands idle, and the raid can
kill it later in the siege), replication to other players' pages (H6), and
the production render path.

## Risks

- **Frame time.** Strand hair is the costliest part: the Valley page runs the
  TF0290 shot at about 36 FPS at device pixel ratio 2 on this Mac with the
  post pass on. The tiers and the cards LOD are the mitigation, and H4's
  exit check measures it.
- **Memory.** Compressed formats aren't available on every device. The
  RGBA8 fallback is the reason the far and near tiers use small textures.
- **Wasm size.** Everglade has about 150 KB of budget left. The character
  runtime belongs in a shared host module (as `host-packs.wasm` does for
  packs), not inside each world's wasm.
- **Two shading models in one frame.** Exposure and tone have to agree, or
  she looks pasted in even when rendered natively. The `SceneLights` block
  carries exposure and the tone curve for this reason.
