# Shared rendering and spatial audio in Aethyr

Date: 2026-11-06. Baseline: `da62755e33`.

## Review findings

The book's registry distinguishes implementation, qualified targets and
adoption. Its architecture already supplies fixed-step time, typed space,
queries, kinematic contacts, animation, resource ownership and replay. This
pass reviewed that inventory and the rendering, materials, lighting, asset,
audio, terrain/scatter, environment and streaming chapters in more detail.
It does not claim that every design draft is implemented.

Three lessons guide these changes:

1. Reusable arithmetic belongs to engine modules; realms provide style,
   content and scheduling. A compatibility layer can route existing consumers
   to shared code without migrating every renderer at once.
2. Validated inputs must also produce finite outputs. A nonnegative check
   alone admits positive infinity; spatial sound needs to validate its room,
   listener and source before computing pan or attenuation.
3. Similar formulas are not integration. The world and the audio workbench
   previously maintained separate distance laws. They now call the same
   function, with separate camera and flat-field pan adapters.

## What is integrated

| Shared engine owner | Systems included | Current consumers |
| --- | --- | --- |
| `aethyr/render/sky.coax` | Horizon/zenith response, halo, portable value noise, cloud octaves, coverage, light and horizon fade | Through `verse/style.coax`: the fixture, Everglade renderer family, authored realms, Anime Lab, 5E Lab and the one-shot glade |
| `aethyr/render/vegetation.coax` | World-space gust advection, wind phase, root-anchored bending, blade height/tilt, density and range fades, path masks, mover parting, canopy normals, sway and gaps | Fixture and Everglade canopy/grass rendering, through the same style bridge |
| `aethyr/render/shadow-filter.coax` | Desktop/phone Poisson sample offsets and filter radius; tap count remains in `light.coax` | Fixture and the Everglade renderer family, including 5E MMO |
| `aethyr/render/post.coax` | Exposure, saturation, black lift, bloom extraction/composition, chroma preservation, light-shaft source/fade/weights/normalization | Fixture, Everglade family, authored realms, Anime Lab, MetaHuman Lab, 5E Lab and the one-shot glade; each consumes the functions it needs |
| `aethyr/render/material.coax` | Typed material/table admission and finite emission channels | Existing material API and book example; live realm material-table adoption remains unfinished |
| `aethyr/audio/attenuation.coax` | Checked inverse-distance gain, final-fifth range fade, propagation delay, air-absorption lowpass and common constants | `audio/space.coax` for camera-relative world effects; `audio/everglade-mix.coax` for Audio Lab's flat field |
| `aethyr/audio/space.coax` | Listener/source validation and camera-space pan, composed with shared attenuation | Everglade's spell, combat, destruction, footstep and other positional cues |

The style bridge retains the established `verse_style_*` function names and
exports. Source ownership changes; style presets, uniforms, palettes, scene
content and GPU pass scheduling remain with their existing owners. The new
render modules may also be loaded directly by other engine consumers.

## Behavioral changes

- Material emission rejects NaN and both infinities in every channel, while
  admitting every finite nonnegative f32, including the largest value.
- Malformed spatial-audio inputs produce a silent record with finite zero
  diagnostics. A nonpositive sample rate gives a zero lowpass update
  coefficient; a zero corner at a valid rate still means no filter.
- Audio attenuation uses a normalized last-fifth fade, avoiding division by
  a rounded-zero distance interval for a very small positive cutoff. Air
  absorption multiplies by the bounded distance ratio before multiplying
  brightness. Ordinary float rounding can differ from the former duplicated
  formulas; this is not a claim of sample-identical host output.
- The shadow filter drops redundant central-tap writes and a redundant zero
  term in its radius. Valid filter positions and radius remain the same.
- The near-unity light-shaft sum threshold is exactly 2^-20, rather than a
  decimal threshold that cannot be reached by the subtraction being tested.
  A 1,000-tap iterative reference checks the exact boundary. Current normal
  shaft presets stay away from this boundary.

The four regenerated WGSL files change only that shaft threshold and the
redundant shadow writes. Sky, clouds, vegetation, tone and bloom arithmetic
retain their previous generated shader text.

## Verification

- Style closure: 89 checks pass. Material admission: 18 checks pass.
- Spatial-audio closure: 21 checks pass. Mixer closure: 39 checks pass.
- Product integration: 12 sound checks in the complete world source and
  four Audio Lab interaction/raid checks pass. These filtered runs do not
  replace unrelated authority coverage or complete product qualification.
- Root mutation gates: sky 54, vegetation 92, post 40, shadow filter 50,
  material 149, attenuation 38 and spatial audio 23. All **446** mutants are
  killed, with no equivalent annotations or excluded survivors.
- The material and light book examples match their committed transcripts
  in native debug and release modes.
- Book structure passes: 52 programs, 216 source-linked listings and local
  links. The registry gains source owners without promoting chapter status
  or claiming new backend qualifications.
- Fixture and Audio Lab Wasm rebuilds succeed. Both pages load and draw in
  muted headless Chrome on hardware Apple Metal-3; local image receipts are
  `/tmp/aethyr-shared-fixture.png` and `/tmp/aethyr-shared-audiolab.png`.

Reproduction commands, from the repository root:

```sh
coax/bin/coax test verse/style.coax
coax/bin/coax test aethyr/audio/space.coax --library
coax/bin/coax test aethyr/audio/everglade-mix.coax --library
coax/bin/coax test apps/openagents-web/everglade5e.coax --only 'sound:'
coax/bin/coax test apps/openagents-web/audiolab.coax --only 'lab:'
python3 psionic/tests/book.py --book aethyr
```

Run each changed primitive's gate with
`coax/bin/coax mutate PATH --library --root-only`. The complete site shader
checker encounters an existing return-type mismatch in untouched
`gpu/valleyworld.coax`: `vw_bg_fs` declares `V4` but its `vc_bg_fs` call returns
`VcLit`. The affected four shaders emit successfully. That separate failure
is retained rather than reporting a passing whole-site build.

## Boundaries that remain

This pass does not implement a unified weather/day clock, streamed-cell
ownership, a terrain material splat pipeline, GPU plant generation, a new
physics solver, or full material-table adoption. It does not replace the
renderer graph, resource ledger, audio interpreter, device clock or canonical
host audio path. Those systems retain their existing contracts and evidence.

The book chapters and renderer/module READMEs now name the shared owners and
their current adoption. Future consumers should compose these engine
primitives rather than copy their arithmetic into another realm.
