# Aribeth: four original NWN animations

The private Experimental scene now cycles Aribeth through four original inherited
Neverwinter Nights clips: `pause1` (idle, 2 s), `walk` (1 s), `bow` (2 s) and
`castout` (0.5 s). The 5.5-second sequence repeats automatically. A label above
her shows the current native clip. She remains near spawn at `(1.8,0,-2)`.

Open http://127.0.0.1:8949/worlds/everglade-experimental?aribeth=1 for a close view.
The position hook runs only on the existing local development path. The current
private pack target is `~/ore-nwn-private/aribeth/animated-scene-v2/`; forest,
farmland, desert, the standing trees and the ten comparison pairs remain present.

## Native reconstruction

The ore tools `nwn_animation.py` and `nwn_animation_pack.py` read the installed
MDLs and bake 66 poses at 12 Hz. Idle, walk and bow resolve to `a_fa`; casting
resolves to `a_ba`. Position tracks interpolate linearly and XYZW quaternion
tracks use shortest-arc SLERP in the original rigid-node hierarchy. The native
inherited position scale is 0.894. Animated ground uses native Y0, preserving
root translation and bob rather than grounding every frame separately.

Every pose retains all 18 parts, 1,969 vertices, 1,242 triangles, the original
128² texture and the same UVs/materials. The adapter verifies every index and
packed vertex of the old static specimen before removing its 1,242 triangles.
All prior vertex bytes, 116 image/provider entries and the other draw payloads
are preserved. The old static vertices remain unused, so earlier buffer origins
stay valid. No second frozen Aribeth is drawn beneath the moving specimen.

## Playback

`verse/experimental-aribeth.coax` validates the private catalog's frame rows and
clip schedule. `C glade/aribethN` identifies each pose; `A aribeth NAME FIRST
COUNT DURATION_MS` gives the original clip timing. Missing, duplicate, out-of-range
or incomplete frame banks stay inactive; catalog replacement resets the state.

`Lab` owns bounded playback storage. `Creator` borrows its view so skeletal
helpers do not copy a large frame bank. One existing flag16 static draw selects
the current pose; there is no shader change or per-frame vertex upload. The
Coax HUD uses the same sampled clock for the clip label. The ordinary public
pack has no animation bank and produces no extra draw or label.

This initial preview switches clips at their duration boundaries. Native
transition blending, animation events, spell effects, cloth dynamics, collision,
NPC behavior and gameplay locomotion are outside this four-clip reconstruction.

## Validation

The original native track sources, hashes, per-pose hashes, bind inverse check,
scale references and reproduction commands live in ore at
`docs/neverwinter-nights/aribeth-model/animations.md` and its JSON companions.
Six native-decoder checks and six frame-bank adapter checks pass. Playback and
catalog/draw checks pass, including the existing dense two-handed pose checks.
The release Wasm is 2,600,860 bytes, below the existing 2,602,000-byte limit.
The actual in-world motion capture and final source/build hashes are recorded in
ore's `aribeth-model/animation-validation.json`.
