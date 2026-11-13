# The realm-script API

Generated from `metaverse-engine/script/api-table.coax` by
`coax/web/tool.sh metaverse-engine/tools/script-api.coax --write`. Do not edit: change the table.

A realm script calls the engine only through these. A read asks this tick's frame. A write changes
running state of the thing the handler was called for, and is refused for any other thing. An engine
call asks the engine to do something it caps itself. No call writes the world document, a character or
the ledger. See [realm-script-v1.md](realm-script-v1.md) for the module, the limits and the diagnostics.

| Call | Effect | Cost | What it does |
| --- | --- | ---: | --- |
| `sa_trap(line: s32)` | trap | 0 | the module's own failed check: a bounds, arithmetic or cast trap at this source line (the runtime calls it; api.coax does not declare it) |
| `sa_count() -> s32` | read | 2 | how many things this frame holds |
| `sa_at(index: s32) -> u64` | read | 2 | the id of thing INDEX in the frame's order, 0 past the end |
| `sa_alive(thing: u64) -> bool` | read | 2 | whether the thing is in this frame and alive |
| `sa_kind(thing: u64) -> s32` | read | 2 | the thing's kind number, the realm's (-1 when it is not in the frame) |
| `sa_team(thing: u64) -> s32` | read | 2 | the thing's team number, the realm's |
| `sa_x(thing: u64) -> f32` | read | 2 | the thing's x, in metres |
| `sa_y(thing: u64) -> f32` | read | 2 | the thing's height, in metres |
| `sa_z(thing: u64) -> f32` | read | 2 | the thing's z, in metres |
| `sa_facing(thing: u64) -> f32` | read | 2 | the thing's yaw, in radians |
| `sa_speed(thing: u64) -> f32` | read | 2 | the fastest the thing may move, in metres a second |
| `sa_health(thing: u64) -> f32` | read | 2 | the thing's health |
| `sa_var(thing: u64, slot: s32) -> f32` | read | 2 | one of the thing's four running numbers (slot 0 to 3) |
| `sa_point_x(thing: u64, point: s32) -> f32` | read | 2 | x of one of the thing's four authored points (its plan's anchors) |
| `sa_point_z(thing: u64, point: s32) -> f32` | read | 2 | z of one of the thing's four authored points |
| `sa_goal_x(thing: u64) -> f32` | read | 2 | x of where the thing's plan sends it this tick (its own x when it has no plan) |
| `sa_goal_z(thing: u64) -> f32` | read | 2 | z of where the thing's plan sends it this tick |
| `sa_goal_speed(thing: u64) -> f32` | read | 2 | how fast the thing's plan walks it there, in metres a second |
| `sa_notice(thing: u64) -> f32` | read | 2 | how far the thing's plan lets it see, in metres (0: never) |
| `sa_plan_on(thing: u64) -> s32` | read | 2 | what the thing's plan does on seeing someone: 0 chase, 1 call, 2 flee, 3 ignore (-1 with no plan) |
| `sa_distance(a: u64, b: u64) -> f32` | read | 4 | the distance between two things on the ground, a large number when either is missing |
| `sa_near(thing: u64, kind: s32, radius: f32) -> u64` | read | 16 | the nearest other living thing of a kind (any kind when negative) within a radius, 0 when none |
| `sa_time() -> f32` | read | 2 | the world's running time in seconds, the same on the page and in a room |
| `sa_day() -> f32` | read | 2 | the time of day, 0 to 1 |
| `sa_world(slot: s32) -> f32` | read | 2 | one of the realm's eight running numbers (slot 0 to 7) |
| `sa_random(thing: u64) -> f32` | read | 4 | the handler's next seeded draw in [0, 1): the same on the page and in a room |
| `sa_move(thing: u64, x: f32, z: f32, speed: f32)` | write | 8 | walk toward a point at a speed; the engine caps the speed at sa_speed |
| `sa_stop(thing: u64)` | write | 8 | stand still |
| `sa_face(thing: u64, yaw: f32)` | write | 8 | turn to a yaw |
| `sa_set_var(thing: u64, slot: s32, value: f32)` | write | 8 | set one of the thing's four running numbers |
| `sa_timer(thing: u64, timer: s32, seconds: f32)` | write | 8 | call on_timer with this timer number after so many seconds (timers 0 to 3; 0 seconds or less cancels) |
| `sa_strike(thing: u64, target: u64)` | engine | 16 | ask the engine for the thing's strike on a target; the engine checks reach and its own cooldown |
| `sa_say(thing: u64, line: s32)` | engine | 16 | show line number LINE of the thing's authored lines |
| `sa_emit(thing: u64, event: s32, value: f32)` | engine | 16 | raise a realm event with a number; the realm decides what it means |

Cost is in instructions of the call's budget, on top of the instructions that made the call.

## Limits

| Limit | Value |
| --- | ---: |
| A module's bytes | 65536 |
| A script's memory, in 64 KiB pages | 4 |
| The things one script drives (state slots) | 256 |
| A state record's bytes | 256 |
| Instructions one handler call may spend | 20000 |
| Instructions a world's scripts may spend in one tick | 400000 |
| Failed calls in a row before a script is switched off | 3 |
| Scripts one world loads | 8 |
| Things in a frame | 256 |
| Events in a frame | 64 |
| Commands a tick's scripts may queue | 512 |
| A thing's running numbers | 4 |
| A thing's authored points | 4 |
| A realm's running numbers | 8 |
| A thing's timers | 4 |
| Things a trigger tracks inside itself | 8 |
