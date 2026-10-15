# verse-lagrange2

The physics behind Verse's [Lagrange 2](../../docs/verse/lagrange-2.md)
zone, a spaceship factory at Earth–Moon L2. It does no rendering,
networking, or I/O. The orbit itself is `verse_lagrange::earth_moon`.

- `ship`: the `spacecraft.v1` document. It holds parts with mass, shape,
  and latches, and derives mass properties. Every edit must pass its
  checks: mass budget, thrust line, test-burn Δv, latch loads, and the
  assembly tree.
- `arm`: the factory arm, Robot Lab's SO-101 at 100 times its size, with
  forward and inverse kinematics.
- `factory`: the deck, four arms, racks, and cradle in one `physics`
  world, with CMG attitude control.
- `assembly`: the `assembly-plan.v1` plan and the stage-checked sequencer.
  It simulates each stage before committing it, checks it after, and steps
  back or makes safe on failure. It survives a lost link, a stuck latch,
  and a controller reset.
- `burn`: the finished ship's test burn, momentum-exact against its
  exhaust, with the keel's axial mode rung by the engine.
- `cad_feed`: CAD Lab's mass feed, read without the CAD kernel. The sample
  ship's parts come from `cad/lagrange-tug.json` and its feed.
- `oracle`: the arm as MuJoCo XML and the recorded MuJoCo 3.3.7 reference
  move in `oracle/arm-reference.json`.

To run the tests:

```sh
cargo test -p verse-lagrange2
```

The tests cover:
- the document's checks and the hand-computed mass properties;
- the arm against the SO-101 URDF;
- momentum conservation with the CMGs counted;
- the free-floating deck's reaction to an arm's swing, against the
  momentum relation;
- nonholonomy, and mirrored arms cancelling each other;
- named failures for collisions, motor limits, and CMG saturation;
- the full four-arm assembly;
- stuck latches, link loss, controller resets, solar-event and
  collision-avoidance holds;
- bit-for-bit replay and frame-rate independence;
- the arm against MuJoCo 3.3.7;
- the parts' mass properties against the CAD feed;
- the rocket equation, the gimbal, the keel's ringing, and the departure
  from L2.
