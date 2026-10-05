# Tiled navigation and crowd fixture

This fixture drives 40 upright capsules through an original two-level floor,
six stair treads, and a doorway. Construction at tick 90 invalidates affected
routes; a door closes at tick 180 and reopens at tick 210. Capsule queries admit
movement and assert that actors do not penetrate geometry or each other.
The planner distinguishes ready routes, no path, and exhausted work.

`crowd.json` retains every tick, route duration, result, and actor trajectory.
`fixture.vnt1` is the pinned cooked graph. `receipt.json` pins the compiler,
CPU, executable, and sources; `sources.zip` retains those sources. The executable
runs the complete simulation twice and checks identical trajectories, route
outcomes, and content identity. An independent disconnected-wall probe and a
one-unit work probe verify distinct no-path and exhaustion outcomes.

| Measurement | Retained result |
| --- | --- |
| Actors / ticks / fixed interval | 40 / 360 / 1/30 second |
| Tiles / spans | 25 / 1,516 |
| Cooked bytes / cold cook | 119,680 / 91.00 ms |
| Route p99 | 3.42 ms |
| Total navigation tick p99 | 5.60 ms |
| Maximum route starts per tick | 4 |
| Construction invalidations | 31 |
| Actors receiving an admitted route | 40 |
| No-path results / ordinary exhausted results | 264 / 0 |
| Actors reaching goals within 12 seconds | 7 |
| Replay equality | Pass |

The optimized executable runs on CPU 0 of an Intel Core i7-14700K. The CPU is
shared, with no explicit warmup. Timing includes route scheduling, avoidance,
character movement, collider updates, and trace preparation. Retaining and
writing the final JSON is outside the tick timer. These are navigation component
measurements; V18 owns the whole-game frame and failure budgets.

The seven arrivals make the congestion limit visible: fixed velocity sampling
and conservative replanning do not guarantee crowd liveness through a doorway.
Goals have enough space for nonoverlapping capsules. The planner supplies goals;
the motor admits each movement. Scene edits that change walkable surfaces need a
new cooked graph. Route queries must include transient hard obstructions;
Verse adds spell-wall bounds without changing the durable prop life book.

`diagnostic-unpinned.json` retains an earlier congested run without the shortcut
quotas. Its exact source snapshot is unavailable, and its goal spacing and retry
policy differ. It records the investigation and cannot establish a controlled
performance comparison.

To reproduce from the repository, use the pinned toolchain and a warm target
outside the checkout:

```sh
cargo build -p physics --release --example navigation_crowd
taskset -c 0 "$CARGO_TARGET_DIR/release/examples/navigation_crowd" > crowd.json
```

The printed JSON also contains `cooked_graph`; the retained result stores that
field separately in `fixture.vnt1`. Removing that field leaves `crowd.json`.
