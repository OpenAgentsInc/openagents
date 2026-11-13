# Animation Lab

Animation Lab is an unlisted Metaverse realm at `/worlds/animation-lab`.
It opens on an original block mannequin on the existing 65-joint skeleton.
Echo and the Slay face clips load from the owner's private animation bundle.

## Stations

- `#rig`: select a joint on the skeleton or with JOINT arrows. Its name,
  parent and weights appear. SHAPE arrows and the slider drive a face shape
  from zero to one. The count line reports joints, influence slots, shapes,
  facial bones and materials.
- `#clip`: play, pause, step or drag the timeline. Brackets select a clip.
  The plot shows the selected joint's quaternion components, or a selected
  face curve. Faded skeletons show the poses eight frames before and after;
  G cycles the spacing through 4, 8, 16 and off. Event ticks mark clip events.

- `#retarget`: show source and target beside each other on the same clock.
  Green joints are paired anchors; purple joints are unpaired. The target
  keeps unpaired joints at local rest. T or RETARGET selects this station.

- `#layers`: toggle **BASE**, **OVER**, **ADD**, **FACE** and **FIX**, or use
  keys 1–5. L or LAYERS selects this station. The contact meter shows the
  cycle peak in centimetres with fixes off and on; it turns red above 1 cm.
  Seeking resets the meter. The built-in studio wave has stationary feet.

- `#scene`: play an admitted scene through its first authored camera.
  **N** selects Scene. Drag its timeline to seek; tap **PLAY** to run.
- `#cut`: play the edit's cameras and scene ranges. **X** selects Cut.
  The shot label follows the edit. Space pauses; comma/period step.
  These stations require a private scene project, described below.

In the inspection stations, drag the model to orbit. Scroll to zoom. On a phone, the same controls are
at the top and bottom of the stage. M selects the mannequin, E selects the loaded private rig;
J/K select joints, U/I shapes, V switches joint/curve plots, S toggles the
skeleton. Space or P toggles playback; comma and period step. Escape returns
to Worlds.

A deep link selects the station, rig and clip, for example
`#clip?rig=echo&clip=jog_fwd`. Unknown names stay visibly refused. Retarget links also select the target,
for example `#retarget?rig=echo&clip=jog_fwd&to=mannequin`.

## Imports

Run `openagents labs anim import SOURCE --rig NAME` to import a glTF/FBX,
BVH, ARKit CSV or supported Unreal body source. See the
[import profiles and limits](/docs/verse/anim-imports-v1.md). Expose the output
`library.bundle` through the private alias below, tap **PRIVATE**, then
**CLIP**. A deep link such as `#clip?rig=NAME` selects the imported rig.

## Private source

No licensed data ships with this page. Export the A0 documents, then run:

```sh
coax/web/tool.sh metaverse-engine/tools/anim-lab-bundle.coax DOCUMENT_DIR PRIVATE_OUTPUT.bundle
```

To author the private walk/face study and its measured contact annotations:

```sh
coax/web/tool.sh metaverse-engine/tools/anim-layer-demo.coax PRIVATE_INPUT.bundle NEW_PRIVATE_DIR
```

The command writes `performance.json`, `contact.clip.json`, `report.json` and
`receipt.json`. It returns failure when a quality check exceeds the stated limits. Keep the
private report and documents for review.
Expose the first two files as `animation-lab.performance.json` and
`animation-lab.contact.clip.json` in the same owner-local private web folder.
Open `#layers?rig=echo`. The original source bundle and its public clip count
stay intact. Keep the report and receipt with the private source.

Expose the private output as
`~/.openagents/verse/private-build/web/animation-lab.bundle` for the local
dev server. It is admitted by the same document readers used by the tools.
Without it, selecting Echo names the missing bundle. Keep all private
payloads and captures outside git.

## Rendering and limits

Metaverse Engine samples admitted tracks and applies eight-influence GPU skinning and
sparse morphs. This inspection stage uses neutral colours and weight heat,
with a 64 MiB GPU mesh budget and a 64 MiB private document bundle budget.
It does not reproduce a source material or source lighting. Ghosts are
skeletons so inspection remains usable on smaller devices. The inspection frame budget
is 33.3 ms on desktop and phone; physical phone qualification is recorded
separately from a headless phone viewport.

In `#layers`, the numbered row selects individual layers. Tap **<** or **>**,
then its **ON/OFF** label. Keyboard: **-**, **=**, then **Enter**. The five
group buttons remain shortcuts for all layers of the same kind.


## Scenes and films

`openagents labs anim play FILE PRIVATE_DIR/film.mp4` renders a performance,
scene or cut with headless muted Chrome and muxes audio after the Metaverse
frames. `openagents labs anim check FILE` reports continuity, face ranges,
joint/capsule screening, planted-foot travel and determinism as JSON.
JSON inputs use an adjacent `project.json`; an ALP1 bundle carries the whole
admitted project. See the [A5 scene profile](/docs/verse/anim-scenes-v1.md)
and the original Guardian staging recipe in `metaverse/animlab/guardian/`.

For local playback, expose an ALP1 file as `TOKEN.bundle` in the private web
folder, then open `?project=TOKEN#scene` or `?project=TOKEN#cut`.
Tokens contain 1–64 lowercase letters, digits or hyphens. The CLI uses the
full project SHA-256 as its token. It preserves the Echo inspection alias.
Add `?card=1` within the hash for captures, for example
`?project=TOKEN#cut?card=1`.

This is a blocking renderer with keyed lights and procedural effects.
Studio and courtyard, cuts and incoming fades are supported. Canyon and
image crossfades are refused. Finished render styles and depth of field
belong to A6. Licensed study films and their frames stay owner-local.
