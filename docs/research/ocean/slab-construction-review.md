# SLAB construction film

The construction film is the SLAB world's build view
(`/worlds/slab?view=build`) played from the empty yard to the commissioned
platform. The view draws one run of the build simulation
(`metaverse/slab-build.coax`; the method is in
[slab-construction-method.md](slab-construction-method.md)) day by day. Each
caisson, tow, tug, barge, gantry and line is drawn where the run puts it, at
its own draft, and the upper structure and the city rise bay by bay as the
run casts them. The film plan (`metaverse/slab-build-film.coax`) chooses the
days each second shows and where the camera flies: slow for the first pour,
the first float-out, the first tow and the first joint at sea, fast for the
years of repeated work.

The earlier 92-second film, its frame-by-frame reviewer and its contact
sheet are removed. They showed a closed-form schedule
(`metaverse/slab-construction.coax`, also removed) with a Brownsville freight
port, a 40 m berth and single caissons towed one at a time. None of that
matches the simulation the world now draws.

## Take it again

One command takes the film headless at 1920 × 1080 and 30 frames a second,
with a dev server for the checkout running:

```shell
scripts/slab-build-film.sh docs/research/ocean/slab-construction.mp4
```

Each frame is one fixed step of the page's clock, so the same commit gives
the same film. The film is about 215 seconds long (6,500 frames).

[slab-construction.mp4](slab-construction.mp4) is the film taken from
`f2d0854d20` (the build view has not changed since): 216 seconds at
1920 × 1080. It is encoded small for the
repository (H.264 at CRF 30, about 28 MB, no sound); take it again with the
command above for full quality.

## What the film shows and what it does not

- **Worked out:** each day's weather, each tow's forces, draft and towline
  tension, each joint's wait for a calm, the pours, the lifts and the
  storm stand-downs. Tap a body in the world for its weight, draft, GM and
  towline tension.
- **Placed along paths, not worked out:** moves inside the yard, a pair's
  approach from stand-by to its joint, and tugs and barges coming back.
- **Not modelled:** the free surface of the ballast water, and the small
  roll and pitch of each body (the drawing turns bodies about the vertical
  only).
- **Drawn for reading:** the land round the yard is schematic (an apron, the
  casting cells, their gates and a far bank), and the lines are drawn
  thicker than they are so they show on screen.
