# The Grid robot

The Grid robot is an original black-and-white robot. Four of them patrol the
Grid's plaza as NPCs, and a fifth dances beside the Everglade arch. It's built by script in Blender on the Universal rig, so
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
the dance's feet stay within 2 cm of the floor, with the same finger
contacts as idle.

## In the Grid

The Grid pack (`assets/verse/grid/pack.json`) carries both levels, compiled
by `crates/verse/src/grid_robot.rs`: armor in four shades by facet
direction, edges sharper than 30 degrees as lines, and the clips `Idle_Loop`,
`Walk_Loop`, `Jog_Fwd_Loop`, `Jump_Loop`, and `Dance_Loop`.

The Grid draws the robot at 1.5 times its rig's size, about 2.8 m tall,
scaled about its feet, and walks it at the stride's speed at that size.

Four robots patrol the plaza, one in each quadrant around the spawn
(`ROUTES` in `crates/verse/src/grid_robot.rs`): two ahead, outside the side
arches, and two behind, off the spawn's back corners. Each walks its route,
stands idle a moment at each end while it turns, and walks back, about 19
seconds a round. Each starts 0.287 of a round after the one before, so no
two move in step. The routes stay clear of the arches and their approaches,
the walks from the spawn to the arches, the Gym, the pillar, the ball, the
blocks, the dancer, and each other.

The fifth robot dances in place on a loop beside the Everglade arch: 3.6 m
out from the arch's middle, past the pillar on the side away from the
spawn's line, and a step toward the spawn, facing it. The arch's opening and
its approach stay clear.

The robots' clock is local to each client. The player can't walk through
any of them: a player's center stays 1.125 m (the robot's body radius plus
the player's) from a robot's.

`cargo run -p verse --features capture --example grid_robot_capture -- OUT_DIR`
renders them in the Grid: the spawn view, the plaza from above, each
patroller beside a line figure, a pause at a route's end, the dancer by the
arch through one loop, and the far level. It also prints the spawn view's
GPU time with and without the robots.
