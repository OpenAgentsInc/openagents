# Everglade II

Everglade II (`/worlds/everglade2`) is the first piece of Everglade v2. Everglade v2 is
rebuilt from scratch, one piece at a time, apart from Everglade v1 (`/worlds/everglade`), which
stays as it is.

This piece is Character Create's glade, one winding path out of it through dense woods, and a
clearing at the path's end. It has no town, gate, walls, people, raid, or HUD. You walk the
character you saved in Character Create, on Everglade's controller and follow camera.

## Walk it

Character Create (`/worlds/character-creator`) is a place in this world: its studio stands in the
glade, and USE IN EVERGLADE runs you out of the glade and down the path, then hands you the keys
(`apps/openagents-web/everglade2-create.coax`, `docs/verse/character-creator.md`).

- Open `/worlds/everglade2`.
- W, A, S, and D run at a jog (4.25 m/s). Hold Shift to walk (1.5 m/s). Space jumps.
- Drag to look. You can look 1.2 radians above or below the horizon. Scroll to zoom. The arrow
  keys turn and tilt the camera.
- On a touch screen, the left thumb walks and the right thumb looks.
- `?at=METRES` starts you that far down the path, for example `/worlds/everglade2?at=300`. The
  clearing starts at about 440 m.
- `?walk` walks you down the path on your own, with the camera behind you, and stops in the
  clearing. Any key takes over. Add it to `?at=`, for example `/worlds/everglade2?at=200&walk`.

## The land

`verse/everglade2/forest.coax` holds the land as plain functions that the CPU and the shaders
share:

- Inside the glade (within 14 m of the middle and north of z = -14), the ground is exactly the
  glade's own (`cg_height`). A test checks this.
- The path leaves the glade on the glade's own road and winds 440 m south through a valley
  (`e2_road_x`). Its grade never climbs steeper than one in five.
- The valley walls rise into wooded ridges away from the path.
- `e2_forest` is how thick the woods stand: thick on both sides of the path and crowding it, none
  in the glade or the clearing. The woods close round the glade on every side, from 17 m out of
  its middle (`e2_wooded`), so the only open ground is the glade and the clearing.
- The clearing is a wide bowl, about 68 m across, at the path's end (`e2_clearing`).
- `e2_autumn` colours the tree crowns. `e2_gold` colours the ground and grass: the glade's own
  gold inside it, and a gentle share outside it, so the woods' floor and the clearing stay
  green.

## The planting

`verse/everglade2/plant.coax` places every kit piece as a pure function of the land, cell by
16 m cell. The page plants all of them at load, in a fraction of a second:

- Trees stand on a jittered 2.6 m grid where the woods are thick, never on the path or in the
  clearing. The first trunks stand 2.6 m from the path's middle, and trees grow larger away
  from the path, so their crowns make a continuous canopy behind the near trunks. There are
  oaks, birches, and pines, with autumn crowns in the autumn patches.
- The understory has ferns, bushes, flowers, mushrooms, clover, and rocks. It is thickest on the
  verges and between the trunks within 35 m of the path.
- Now and then a boulder stands in the woods.

That comes to about 100,000 pieces. Tests check that none stand on the path, that no tree stands
in the clearing, and that trees crowd the path.

## The pack

`verse/tools/assets/everglade2_pack.coax` writes `realms/everglade2/assets/everglade2.pack`
(43 MB) in about 10 seconds. The pack holds:

- Character Create's people pack (the wardrobe and the rig).
- The glade's own composition, placed on Everglade II's land.
- The ground: the glade's grid, a 1 m ribbon along the path that widens round the clearing,
  and coarse 4 m ground out to the valley's edges.
- The 36 kit pieces, each at the origin. Each tree has three levels of detail: whole; two crown
  cards in three with bark on the lower half; and one crown card in three with bark on the
  trunk only.

Build it from the repository root:

```sh
coax/web/tool.sh verse/tools/assets/everglade2_pack.coax \
  "$(coax/web/tool.sh verse/tools/assets/registry.coax source nature "$HOME/.openagents/assets")" \
  apps/openagents-web/realms/character-creator/assets/fantasy.pack \
  apps/openagents-web/realms/everglade2/assets/everglade2.pack
```

## The page

`apps/openagents-web/everglade2.coax` and `everglade2-face.c` run the page:

- The face steps Everglade's controller (`everglade-step.coax`). The page lifts your feet onto
  the land, and keeps you in the glade's 26 m circle, on the path's corridor, or in the
  clearing. Thick woods stand wherever you may not go, so no edge of the world shows open ground.
- Every tree trunk and rock stops you. A trunk is a capsule sized to its tree kind's trunk. A
  rock is a capsule along its long axis, fitted to its model's footprint. Both are scaled and
  turned with the placed piece, the glade's own trees and rocks included. Pebbles don't stop
  you. `everglade2-walk-test.coax` walks into a boulder and a trunk through the real controller,
  and runs the whole path from the glade to the clearing down its middle and a metre to each
  side without being held up.
- Every collider comes from the same item list and transform as the drawn piece. The glade's
  pieces that stand on the path out (`e2_glade_kept`) are left out of both the pack and the
  colliders, so nothing unseen stands in your way.
- The run and the walk are Everglade II's own: the page sets the step's pace, so v1's speeds
  (7.5 and 3.5 m/s) stay as they are. The gait plays by the distance you cover (a 3.4 m running
  stride and a 1.3 m walking one), so the feet keep pace.
- Each frame, the page picks the pieces in view, sorts them into one run for each kit piece and
  level of detail, and draws each run as one instanced draw. About 75 draws make a frame.
- Trees are whole to 36 m and lighter to 70 m. Past 70 m, one tree in three stands for its
  neighbours, a third larger, and past 160 m one in six, half again larger.
- Character Create's glade grass isn't drawn. At about 35 million vertices a frame it cost
  about 10 ms of GPU time; the blade grass covers the glade instead.
- Blade grass covers all of the ground except the bare path (`verse/everglade2/field.coax`). The
  GPU places one tuft in each cell of a grid fixed to the land, seeded by the cell, in four
  nested tiers round the eye: 0.5 m cells to 28 m, 1 m to 40 m, 2 m to 52 m, and 4 m to 140 m.
  Each coarse tuft stands exactly where one of the four finer tufts under it stood. Past 16 m,
  the other three shrink away one by one, and the one that stays grows wider, so the grass only
  thins with distance and never swaps one tuft for another. The grass thins in at the path's
  edges and grows thinner, shorter, and darker under the trees. The page draws the blocks of 16
  by 16 cells in view.
- A jump plays its takeoff over 0.2 s, its loop while you are in the air, and its landing over
  0.35 s. It blends over the run or the idle pose at 0.45: it eases in over 0.12 s after takeoff
  and out over 0.18 s after landing, so the legs keep their stride. These came from Everglade
  Experimental (`charactercreator.coax`).
- The follow camera looks straight down your back at every tilt, with no shoulder offset, and
  keeps in front of the trunks between it and you. When you tilt up far enough that the eye
  would go under the land, the eye stays where it meets the land, 0.2 m above it, and the view
  keeps tilting up. As it does, the aim rises up to 2 m over your head, so you stand in the lower
  middle of the view and the sky fills the rest. The eye moves smoothly onto and off the land.
  Everglade II tilts from 1.2 radians below the horizon to 1.2 above.
- A frame-rate counter in the bottom-right corner shows frames a second, averaged over 0.5 s.
- The graphics governor (`aethyr/quality.coax`) sets how many pixels the page draws. It lowers
  the resolution when frames run slow and raises it again when there's room.
- The key light's shadow map follows you, 68 m across, in 2 m steps.

Read the counter in your own browser for the frame rate. Headless measurements on a busy Mac
put GPU time at 2400 by 1400 at about 16 ms at the glade (31 ms before the glade grass was
dropped) and 17 ms at the clearing.

`gpu/everglade2.coax` holds the passes. It uses Character Create's glade light, sky, fog, and
materials, adds the kit pieces' vertex stage and the grass tufts' shading, and gives the woods'
floor its leaf litter.

## Build and serve

Everglade II builds in seconds:

```sh
cd apps/openagents-web
../../coax/web/wasm.sh everglade2.coax everglade2-face.c build/everglade2.wasm \
  everglade-step.coax ../../aethyr/sticks.coax
cd ../..
coax/web/tool.sh coax/web/wgsl-module.coax apps/openagents-web/gpu/everglade2.coax \
  apps/openagents-web/everglade2.wgsl apps/openagents-web/gpu/everglade2.module
```

To serve it without the whole site's build, serve the app statically:

```sh
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8863 --static --cache 0
```

The page needs the shared host modules in `apps/openagents-web/build/` too
(`host-services`, `host-sockets`, `host-packs`, `latency-client`, and the others from
`build.sh`). A plain `coax serve` without `--static` runs the whole of `build.sh` before it
answers.
