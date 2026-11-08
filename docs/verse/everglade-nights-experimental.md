# Everglade Nights Experimental

## Contents

1. [Independent runtime](#independent-runtime)
2. [Private source contract](#private-source-contract)
3. [Controls and appearance](#controls-and-appearance)
4. [Native interface](#native-interface)
5. [Rendering](#rendering)
6. [Validation and limits](#validation-and-limits)

## Independent runtime

The unlisted `/worlds/everglade-nights-experimental` route selects `nights.coax` and `nights-face.c`. It imports no `fivelab.coax`, `charactercreator.coax` or `fivelab-bodies.coax` product logic. The source shader imports engine math and lighting from `gpu/charactercreator.coax`; that shader source has no source-art payload dependency. The HUD uses native NWN bitmap font glyphs, panels, buttons and quickbar artwork from an owner-local atlas. There is no procedural stroke font or Quaternius UI art.

The private pack is served at `/realms/everglade-nights-experimental/assets/nights.pack.gz` through an owner-local web root. The payload remains outside Git. The tracked route and catalog contract do not publish or reconstruct the original game distribution.

The tiny C face transports the scalar browser ABI. Coax owns pack metadata admission, appearance selection, movement, animation, camera, drawing and save bytes. It uses the shared fixed-step walk/run/jump controller and touch sticks. Scene coordinates and native frame positions are Y-up metres. Native actor geometry is local to its feet, facing -Z at record yaw zero. The picker keeps native forward toward its camera; exploration adds π to the rendered actor yaw to match the shared controller’s +Z-forward convention, without changing physics or source geometry.

## Private source contract

The ordinary pack retains native static 32-byte vertex and u32 index streams, image layers and a text catalog. All indexed spans use the pack's actual per-row vertex word origin. These additional catalog rows are product metadata, not claims of renderer parity:

```text
C nights/environment FIRST COUNT SOURCE WORD NVERTS 0
C nights/heroH/frameF FIRST COUNT SOURCE WORD NVERTS 0
H nights H Display_Label
P nights H GENDER HEAD OUTFIT
A nights H CLIP FIRST_FRAME FRAME_COUNT DURATION_MS
V nights PALETTE_COUNT LAYER_STRIDE
Q nights SKIN_COUNT HAIR_COUNT CLOTH_COUNT
Z nights REGION MIN_X MIN_Z MAX_X MAX_Z FLOOR_Y SPAWN_X SPAWN_Z
B nights MIN_X MIN_Z MAX_X MAX_Z
R nights ground GRASS_LAYER EARTH_LAYER REPEAT_METRES
```

`H` is a contiguous zero-based actor bank (at most 32); `F` is a contiguous zero-based frame (at most 128 per bank). Each actor has at most eight ordered clips whose frame ranges exactly partition its frames. Missing or duplicate frames and malformed bounded actor metadata refuse scene readiness. Clip names are native, with `pause1`, `walk` and `run` driving locomotion. Other clips are available for inspection. Names are copied from catalog storage, never retained as borrowed strings.

Without a valid saved choice, the initial bank prefers an available male head-zero armored outfit (outfit one), otherwise the first actual bank.

`P` binds actual source reconstructions to gender/head/outfit values; the picker only selects an available combination. `V` describes congruent native texture atlas variants. `Q` describes independent native skin/hair/cloth palette dimensions, flattened as `(skin * HAIR_COUNT + hair) * CLOTH_COUNT + cloth`. Eighteen variants may encode 2 × 3 × 3 colors with stride two when the canonical actor atlas occupies two layers. Palette selection changes only the native actor's layer number, without duplicating geometry or recoloring area textures.

Exactly three `Z` rows admit flat native forest, rural and city regions. The capsule checks the union of raw bounds, permitting walking across shared edges between regions. `B` rows (at most 1024) block source prop/building footprints and shorten the follow-camera boom; the controller receives conservative 0–8 m collider boxes for those footprints. They are bounded conservative footprints, not native NWN walkmesh recovery. More complex native stairs, sloped walkmeshes and interiors require a later navigation adapter.

## Controls and appearance

See the route's `index.md` for desktop and touch controls. The shared controller runs by default; Shift selects walking. The top Menu control opens native buttons for character creation, regions, animation and automatic motion. The appearance panel selects gender/head/outfit and skin/hair/cloth, followed by previous/next presets and Enter World. Exploration shows twelve native quickbar slots, with the first six mapped to the three regions, animation inspection, automatic motion and Jump. `C` cycles all decoded presets. Unsupported combinations are never synthesized from stand-in geometry.

The `NWN1` eight-byte save contains a two-digit actor and palette index. Browser action 4 saves it under this route's independent `everglade-nights-experimental` slot. Loading old Character Create bytes is refused. This is a local appearance convenience, not a shared multiplayer identity or canonical game save.

## Native interface

The appearance menu and twelve-slot exploration quickbar use the shipped NWN interface artwork and native glyph metrics. Coax owns their layout, labels and input actions. Slots invoke the available character, region and animation controls; they do not imply combat or inventory systems. UI source identities, atlas placement and original GUI definitions are documented in ore’s Nights UI audit.

The optional `coaxial_fx_scene_v1` and `coaxial_fx_alpha_v1` scalar exports ask the generic browser host to bind the existing scene texture array to the final FX pass and blend straight alpha. Other producers retain their existing binding and blend defaults. The native UI draws after scene postprocessing, with no DOM controls or world depth occlusion.

## Rendering

A static flag-16 environment draw and a flag-18 local animated actor draw use the existing engine's depth, lighting, texture array and postprocessing contracts. Native animation is a sequence of actual source-deformed mesh frames, sampled at catalog duration; there is no invented skeletal animation or crossfade. The new `gpu/nights.coax` wrapper offsets only the actor texture layer by object record word 13, including its shadow slice. Area material layers remain unchanged.

The shared background is an engine-generated sky. It is not recovered NWN sky or native lighting. The world uses the shared daylight renderer; documenting native shaders and lighting inputs does not imply native lighting reconstruction in this page.

## Validation and limits

Focused Coax checks cover catalog reset/incomplete actors, independent palette save/restore, walking, jump, region reset and key release. The source WGSL compiles through the ordinary module generator. Actual muted headless Apple Metal WebGPU checks rendered the picker and all three regions. A fresh page restored actor 16/palette 13 after an IndexedDB save; keyboard movement, a 1.11328125 m jump and touch movement at 430×900 were observed. The pre-UI native-only pack contains 80 tiles, 26 trees, 18 actor banks × 52 frames and 18 palette variants, with 119 texture layers and zero Quaternius payloads. Its raw/gzip sizes are 91,691,840/43,086,026 bytes. The runtime suite passed 351 checks, with ten focused final checks covering later changes. Exact realm adoption killed all 73 root mutants; the unlisted registry check passed. The pre-UI release Wasm was 70,630 bytes. Full native provenance and pack receipt are in ore’s `docs/neverwinter-nights/nights/README.md`.

This is a local asset rebuild: no multiplayer room, authoritative combat, spell bar, quests, inventory, audio, NWN animation blending or complete native area navigation. Existing Everglade Experimental, Character Create and Everglade II retain their own modules, assets and save slots.

After rebasing onto the shared environment-frame interface, Nights writes the resolver’s cached fixed-noon 108-word record at uniform offset 208 in a 316-float block. The corresponding focused check and rebuilt hardware WebGPU page pass; the shader is generated against the current shared delegates.

The native UI revision uses one additional source-bound 1024² atlas at layer 119 (120 layers total), containing twelve NWN sprite crops and all 256 Galahad14 glyphs. Its full-frame slot border comes from `gui_spell_hilite`; the twelve-slot layout follows the native quickbar, while action mappings belong to Nights. All previous scene payload bytes remain unchanged. Final raw/gzip pack sizes are 91,879,916/43,258,896 bytes. The UI release Wasm is 68,480 bytes. Fifteen focused runtime checks and the independent browser FX transport oracle pass. Actual hardware clicks changed gender, entered exploration, selected City, opened the menu and jumped on a 430-pixel phone viewport (observed feet Y=0.72656). Original UI pixels and font RGBA were independently compared with the installed resources and match exactly.

## Forest ground revision

The forest uses dedicated native 512² grass/earth images at layers 120/121, blended across continuous world coordinates at two metres per repeat. The bounded `R` row requires both declared layers below 124 and repeat 0.5–8 metres. Coax writes the admitted fields at uniforms 104–107. Only native flat forest-floor vertices marked with float32 7001.0 in word seven use this material. The new branch removes the extra +1 ground mip bias while retaining derivative-based eight-tap filtering and shared lighting. Other scene textures and actors keep their original rendering.

Sparse original NWN groundcover adds 84 grass tufts and 36 ferns, with two native image layers (124 total). They share the existing environment draw and do not change navigation. The private packer preserves actor vertex records and relocates their index starts exactly around the environment insertion. Four pack checks and two focused runtime/shader checks pass; the release is 68,917 bytes. Native source/placement receipts and visual evidence are documented in ore’s `nights/ground.md`.

The denser green revision replaces the sparse scatter with 2,070 native grass tufts and 40 ferns in deterministic irregular patches. Full conservative plant footprints stay outside the path and creation clearing with 0.35 m additional clearance. The grass sample uses NWN’s `PLC_K09` diffuse color in the linear rendering space, normalized by luminance; earth remains unchanged. Off-trail earth is at most 4.5%, with a smaller 2.0–3.2 m clearing and 0.8–1.6 m path transitions. Five pack checks include deterministic density and footprint clearance. Source material evidence and the updated pack receipt are in ore’s ground documentation.
