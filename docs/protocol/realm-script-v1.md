# Realm scripts v1: the module, the manifest, the frame and the result

A realm script is behaviour someone writes for a world in the OpenAgents
Metaverse that runs on a deployed page and in a room with no deploy. It is a
restricted Coax unit compiled to its own WebAssembly module. The design is
[docs/research/2026-11-13-realm-scripts.md](../research/2026-11-13-realm-scripts.md).
The compiler's rules are [coax/script-profile.md](../../coax/script-profile.md).
The calls a script may make are [realm-script-api.md](realm-script-api.md).

The profile [proto/world/script-v1.json](../../proto/world/script-v1.json)
is written from the code and pins every number on this page:

```sh
coax/web/tool.sh metaverse-engine/tools/script-profile.coax proto/world/script-v1.json
```

## The module

A server, a page and a room each run the same check on the bytes they are
handed (`metaverse-engine/script/wasm.coax`, `wm_check`). The check trusts
neither the source nor the compiler. A module passes only when:

- It is at most 65,536 bytes, and every section, count and immediate decodes
  inside it.
- Every import is a row of the script API: module `env`, the row's name and
  the row's signature. A module can call nothing else.
- It has one memory with a stated maximum of at most 4 pages (256 KiB), no
  start function, no element segments and no passive data. The linker's one
  function table is admitted empty: nothing fills it and `call_indirect` is
  refused.
- Its exports are the module face's, each with its exact signature:
  `rs_abi`, `rs_state`, `rs_state_size`, `rs_slots` and `rs_<handler>` for
  at least one handler.
- Every instruction is one of the WebAssembly 1.0 instructions (plus the
  five sign-extension instructions), and every local, global, function and
  label it names exists.

The check does not type-check operands. The interpreter owns the value stack
and the memory and checks both on every step, so a module that confuses
types computes nonsense inside its own memory and nowhere else.

For a module with sound types, every instruction gives the result
WebAssembly 1.0 gives, with two choices the standard allows: a NaN result
is the canonical quiet NaN, and `memory.grow` by one or more pages answers
-1 (growth by none answers the size), so a script's memory never grows.

A refusal names one of: `too-big`, `not-wasm`, `section`, `malformed`,
`limit`, `signature`, `import`, `memory`, `export`, `instruction`, `global`,
`data`, `no-handler`, and the byte it is about.

### The face

| Export | Signature | Answer |
| --- | --- | --- |
| `rs_abi` | `() -> i32` | 1 |
| `rs_state` | `() -> i32` | where the state records start in the memory |
| `rs_state_size` | `() -> i32` | one record's bytes, 1 to 256 |
| `rs_slots` | `() -> i32` | how many things the script can drive, 1 to 256 |
| `rs_on_tick` | `(me: i64, slot: i32, dt: f32)` | |
| `rs_on_enter`, `rs_on_leave`, `rs_on_notice`, `rs_on_use` | `(me: i64, slot: i32, other: i64)` | |
| `rs_on_hit` | `(me: i64, slot: i32, other: i64, amount: f32)` | |
| `rs_on_timer` | `(me: i64, slot: i32, timer: i32)` | |

Each thing a script drives has one state record. The engine owns the
memory: it zeroes a record when a thing first appears, and when a script is
swapped for a new version it keeps every field whose name, type and size
are the same in both.

## Running a script

`metaverse-engine/script/interp.coax` runs the module and
`metaverse-engine/script/host.coax` runs a world's scripts for a tick. The
page and a room run the same two files over the same bytes.

Every handler call is a transaction:

- It may spend 20,000 instructions. All of a world's scripts share 400,000
  a tick. A call to the API costs its row's cost on top.
- Its writes are the commands it queues and its own memory. When it
  returns, both are kept. When it traps, runs out of budget or is refused,
  both are undone: the call never happened.
- A failure is a diagnostic. Three failed calls in a row switch the script
  off until it is loaded again.

A script writes only the thing its handler was called for, and only running
state. No call changes the world document, a character or the ledger.

Time and chance are arguments: `dt`, the frame's time, and `sa_random`, a
draw seeded by the frame's seed, the tick, the thing and the handler. The
same frames give the same commands and spend the same instructions wherever
they run.

The order within a tick is fixed: the realm's events in frame order, then
each thing in frame order: its timers, its trigger's leaves and enters, its
tick.

## Diagnostics

Each is a row of `proto/registry.json` (`engine.diagnostic.script-NAME.v1`).

| Number | Name | Retry | Means |
| ---: | --- | --- | --- |
| 1 | `over-budget` | no | A handler call spent its instruction budget. The call was undone. |
| 2 | `trap` | no | The module trapped: a failed bounds, arithmetic or cast check, or a bad memory access. The call was undone. The line is the script's source line when it knows it. |
| 3 | `not-yours` | no | A write or an engine call named a thing other than the one the handler was called for. The call was undone. |
| 4 | `commands-full` | yes | The tick's command list was full. The call was undone. |
| 5 | `tick-budget` | yes | The world's instruction budget for this tick was spent. The script's remaining calls this tick were skipped. |
| 6 | `switched-off` | no | The script failed several calls in a row and is off until it is loaded again. |
| 7 | `bad-argument` | no | A number that is not finite, or a slot, timer or point that does not exist. The call was undone. |
| 8 | `refused-module` | no | The module check refused the bytes. The detail is the refusal and the line is the byte. |
| 9 | `no-slot` | no | More things use the script than it has state slots. |
| 10 | `bad-face` | no | The module's face answered out of range. |

A diagnostic carries its code, the script, the handler, the interpreter's
status (`done`, `import`, `over-budget`, `trap`, `memory`, `divide`,
`convert`, `stack`, `malformed`, `idle`), the line, a detail, the thing, the
tick and the instructions spent.

## The manifest, `realm.script.v1`

One line of JSON in one form (`metaverse-engine/script/manifest.coax`): a
reader refuses a text that is not exactly what the writer writes.

```json
{"schema":"realm.script.v1","name":"pacer","hash":"528d…37a5","bytes":954,"pages":2,
 "state":{"size":16,"slots":256,"fields":[{"name":"chasing","type":"u64","at":0,"size":8},{"name":"waited","type":"f32","at":8,"size":4}]},
 "handlers":["on_tick","on_hit"],"powers":["read","write","engine","trap"],
 "calls":["sa_trap","sa_x","sa_z","sa_speed","sa_goal_x","sa_goal_z","sa_goal_speed","sa_notice","sa_distance","sa_near","sa_move","sa_stop","sa_set_var","sa_strike"]}
```

`hash` (SHA-256 of the module), `bytes`, `pages`, `state.size`,
`state.slots`, `handlers`, `powers` and `calls` are facts of the bytes. A
reader that holds the module computes them from it and never takes them
from an author. `name` and `state.fields` are the author's; they decide only
how the script's own state is kept across a swap.

`powers` are the effect kinds of the calls the module imports: `read`,
`write`, `engine`, `trap`. A script's manifest says what it can do before it
runs.

A new version of a script grows when it has a handler, a power or a call
the old one lacks (`sm_growth`). Publishing a version that grows is a
visible change the publisher confirms.

## The frame and the result

A realm fills a frame each tick and reads back a result
(`metaverse-engine/script/frame.coax`). Both are little-endian with fixed
rows.

`script.frame.v1`, at most 32,324 bytes:

| Part | Bytes | Fields |
| --- | ---: | --- |
| Head | 68 | magic `SFR1`, things u16, events u16, tick s64, seed s64, dt f32, time f32, day f32, world 8 × f32 |
| Thing | 120 | id u64, kind s32, team s32, script s32, flags u32 (bit 0: alive), x y z facing speed health reach f32, goal x, goal z, goal speed, notice f32, plan on s32, vars 4 × f32, points x 4 × f32, points z 4 × f32 |
| Event | 24 | handler s32, thing u64, other u64, amount f32 |

`script.result.v1`, at most 19,160 bytes:

| Part | Bytes | Fields |
| --- | ---: | --- |
| Head | 24 | magic `SRS1`, commands u16, diagnostics u16, more diagnostics s32, budget left s32, calls s32, failed s32 |
| Command | 36 | op s32 (the API row), i s32, thing u64, target u64, a b c f32 |
| Diagnostic | 44 | code, script, handler, status, line, detail s32, thing u64, tick s64, used s32 |

A reader refuses bytes that are not exactly one frame or one result.

## The status, `world.script-status.v1`

A world reports its scripts as one line of JSON
(`metaverse-engine/script/world.coax`, `sw_status`), for the editor's script
pane and `coax drive`:

```json
{"schema":"world.script-status.v1","ticks":1,"call_fuel":20000,"tick_fuel":400000,"tick_left":399983,
 "scripts":[{"at":0,"name":"fixture","hash":"…","off":false,"things":1,"calls":1,"failures":0,
             "spent":17,"peak":17,"last":"","handler":"","line":0,"status":""}]}
```

`spent` is the instructions the script used this tick and `peak` the most
one call has used since it was loaded, both against `call_fuel` and
`tick_fuel`. `last` is the id of its last diagnostic, with the handler, the
source line and the interpreter's status.

## Building a script

```sh
coax/web/script.sh metaverse-engine/script/examples/pacer.coax --out /tmp/scripts
```

writes `pacer.wasm` and `pacer.json` and prints the manifest. The last step
of the build is the module check above. Whoever compiles source they did not
write runs the build in a job with no secrets and hard limits.
