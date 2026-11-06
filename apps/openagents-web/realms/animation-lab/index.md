# Animation Lab

Animation Lab is an unlisted Verse realm at `/worlds/animation-lab`.
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

Drag the model to orbit. Scroll to zoom. On a phone, the same controls are
at the top and bottom of the stage. M selects the mannequin, E selects Echo;
J/K select joints, U/I shapes, V switches joint/curve plots, S toggles the
skeleton. Space or P toggles playback; comma and period step. Escape returns
to Worlds.

A deep link selects the station, rig and clip, for example
`#clip?rig=echo&clip=jog_fwd`. Unknown names stay visibly refused.

## Private source

No licensed data ships with this page. Export the A0 documents, then run:

```sh
coax/web/tool.sh aethyr/tools/anim-lab-bundle.coax DOCUMENT_DIR PRIVATE_OUTPUT.bundle
```

Expose the private output as
`~/.openagents/verse/private-build/web/animation-lab.bundle` for the local
dev server. It is admitted by the same document readers used by the tools.
Without it, selecting Echo names the missing bundle. Keep all private
payloads and captures outside git.

## Rendering and limits

Aethyr samples admitted tracks and applies eight-influence GPU skinning and
sparse morphs. This inspection stage uses neutral colours and weight heat,
with a 64 MiB GPU mesh budget and a 64 MiB private document bundle budget.
It does not reproduce a source material or source lighting. Ghosts are
skeletons so inspection remains usable on smaller devices. The frame budget
is 33.3 ms on desktop and phone; physical phone qualification is recorded
separately from a headless phone viewport.
