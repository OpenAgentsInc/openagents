# The Grid robot

The Grid robot is an original black-and-white robot that patrols the Grid's
plaza as an NPC. It's built by script in Blender on the Universal rig, so
the Universal Animation Library's clips play on it, and the Grid draws it in
its own flat look: near-black faceted armor, gray lines on its panel edges,
and white glow.

## Design

- Near-black armor panels, darker graphite joints, and two glow materials
  (white and pale) for the accents.
- Its own identity: a wrap-around chevron visor, a hollow-diamond chest
  emblem, a crest fin on the helmet, ear pods, angular pauldrons with a pale
  rim, a pale belt light, chevrons on the shin guards, and glowing bands at
  the elbows and knees.
- Rigid parts, each bound at full weight to one Universal joint, with ball
  joints where limbs meet.

GarderX's "Low Poly Sci-fi Robot" (Fab, standard license) was studied for
proportions, part breakdown, and design language only; nothing was copied.
See [`PROVENANCE.md`](../../assets/verse/characters/original/grid-robot/PROVENANCE.md).

| Level | Pack model | Triangles | Edge lines | Drawn |
| --- | --- | ---: | ---: | --- |
| `lod0` | `grid/robot` | 3,120 | 2,158 | Within 30 m of the eye |
| `lod1` | `grid/robot-far` | 1,300 | 909 | Beyond 30 m |

## Build and admit

```sh
B=/Applications/Blender.app/Contents/MacOS/Blender
for v in lod0 lod1; do $B -b --factory-startup --python scripts/blender/grid_robot.py -- $v; done
python3 scripts/blender/grid_robot_admit.py
$B -b --factory-startup --python scripts/blender/grid_robot_views.py
cargo run -p verse --example grid_pack
```

`grid_robot_views.py` renders a turnaround and the clip poses, and writes
`motion.json`: each foot's lowest point per clip and every pair of parts that
intersect in motion but not at rest. In idle and walk only the curled
fingers touch the palm and the toe touches the boot as the foot rolls;
kneeling presses the knee ball into the floor.

## In the Grid

The Grid pack (`assets/verse/grid/pack.json`) carries both levels, compiled
by `crates/verse/src/grid_robot.rs`: armor in four shades by facet
direction, edges sharper than 30 degrees as lines, and the clips `Idle_Loop`,
`Walk_Loop`, `Jog_Fwd_Loop`, `Jump_Loop`, `Interact`, and `Fixing_Kneeling`.

One robot patrols the plaza between (5.5, -6) and (5.5, 3), to the side of
the spawn and clear of the arches: it walks to the Everglade arch's side,
kneels to tend it, walks back, works a control facing the plaza, and turns
around, about 29 seconds a round. Its patrol clock is local to each client.
The player can't walk through it.

`cargo run --release -p verse --features capture --example grid_robot_capture -- OUT_DIR`
renders it in the Grid: the spawn view, close views at moments of the
patrol beside a line figure, an orbit, and the far level.
