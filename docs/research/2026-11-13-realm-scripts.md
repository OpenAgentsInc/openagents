# Realm scripts: what Epic's Verse split means for the Metaverse Engine

Status: approved 2026-11-13 and being built (#13467). [What is built](#what-is-built)
at the end says where each stage stands and where the build differs from
the proposal below.
"Epic's Verse" means the language for Unreal Engine 6 and UEFN.
[coax/from-epic-verse.md](../../coax/from-epic-verse.md) already covers what
the Coax language takes from it. This document asks a narrower question:
should the Metaverse Engine have a scripting tier that someone other than us
can write, and that a room can run without trusting the author?

## The short version

Yes, and it should be a **restricted profile of Coax compiled to its own
small WebAssembly module**, not a new language and not a superset. Call it
a realm script.

- A script can only compute, read and write the World through a fixed
  engine API. It has no I/O, no clock, no native calls and no allocation.
- Every call into a script has a time budget, a memory limit and a
  rollback: a script that traps or runs long loses that one call, never the
  room.
- The page runs the script module in the browser. A room runs the same
  bytes in a small interpreter. The server never trusts the compiler: it
  checks the module it is handed.

It fits what we are building this week. Foe plans (#13466) are data, and
data runs anywhere, but data can't express a new kind of behaviour. Code
edits can, but only on a dev server. Scripts are the missing middle:
behaviour an agent or a maker writes that works on a deployed page and in a
room, with no deploy.

## What Epic's split really is

The split is not client against server. Creator code runs on Epic's
servers. The split is **creator code against engine code**:

| | Creator code (Verse) | Engine code (C++) |
|---|---|---|
| Who writes it | Anyone | Epic |
| Memory | Managed: no pointers, collected | Native |
| What it can reach | Only the engine's API | Everything |
| Effects | In every signature, checked through calls | Trusted summaries |
| A failed update | Rolled back | Epic's job |
| Runs on a client | Only what is marked `<predicts>` | Epic's choice |
| A new version | May do less, never more | Epic's choice |

The server can run a stranger's Verse because of five properties, not
because of where the code runs:

1. It can't name anything outside the API.
2. Its effects are known before it runs.
3. A failure undoes its own writes.
4. Its scheduler can stop it.
5. A published version can't quietly gain powers.

## What we have, and what is missing

Coax already has most of the language half, built for Everglade
(`coax/checked-state.md`):

- **Effects** (`#effects(reads, writes, traps)`) checked through calls, with
  the call path named when a clock or I/O sneaks in.
- **Transactions** (`#transaction`) that restore state when a block
  refuses.
- **Handles** whose stale lookups give `none`.
- **Compatibility checks** on exported shapes and saved state.
- **Authority units** (`#authority`) and the durable gate: authoring code
  can't load the modules that hold a player's record or ledger.

None of that was built against a hostile author. Five things are missing:

| Missing | Today |
|---|---|
| A time limit | A loop that never ends hangs the room |
| A memory limit | Bounded by convention only |
| Trap containment | A trap is fatal to the process; one bad index would end a shard |
| A named API surface | A unit can load any module its effects allow |
| A safe way to compile a stranger's source | The compiler says of itself that it is not a sandbox |

This is also why #13432 (a separate origin for realms) was the wrong shape.
It tried to contain a whole realm page, which has the host's full access. A
script module that is handed only the script API needs no origin of its own:
a WebAssembly module can touch nothing it is not given.

## The proposal

### A script is a Coax unit with a profile

```coax
#script "patrol-in-pairs";          // the unit is a realm script

Mind :: struct { leader: Handle(Foe)?; turn_at: f32; }   // fixed size, no allocation

on_tick :: (w: []ScriptWorld, me: Handle(Foe), mind: []Mind, dt: f32) #effects(reads, writes, traps) {
  ...
}
```

The compiler enforces, for a `#script` unit:

- Effects are within `reads`, `writes`, `traps`. No `io`, `clock`,
  `foreign`, `unsafe`, `allocates` or `suspends`, directly or through a
  call.
- It loads only modules marked as the script API, plus pure library code
  that passes the same rules.
- Its state is declared structs of fixed size. The engine owns the memory,
  and a code reload keeps it field by field, as `state-keep` does today.
- No recursion, so the stack has a known bound.
- Its entry points are a fixed set of typed handlers (tick, enter, leave,
  notice, hit, use, timer).

Randomness and time come from the engine as arguments (a seeded draw, `dt`),
so a script is deterministic: the page can run it to predict and the room's
answer will match. That is Epic's `<predicts>` for free.

### The script API is the product

The API is one small Coax module over the World: query (`near`, `ray`,
`path`), read a thing, move or turn a thing, set a foe's plan, start a
timer, say a line, raise an event. Each call is a row with an effect kind,
as realm commands are (#13435), so a script's manifest says what it can do
before it runs.

A script writes **running** state only. It can't write authored state (the
world document) or durable state (a character, the ledger). Anything with
value goes through an engine call that we cap.

### It ships as its own WebAssembly module

The script profile compiles to a separate module whose only imports are the
script API. This gives a second wall that does not depend on our compiler
being right:

- **The server checks the module, not the source.** It validates the
  bytes, and refuses any import outside the API, any memory above the
  limit, and any size above the budget. A compiler bug can't grant a
  script a power, because the module can only call what it imports.
- **The page** instantiates it beside the engine, as it already does the
  editor shell and the chat module.
- **A room** runs the same bytes in a small interpreter written in Coax.
  An interpreter makes the three missing limits natural: it counts
  instructions (time), owns the linear memory (memory), and a trap is a
  return value (containment). Rule scripts are small, so interpreting them
  is affordable; a budget per realm per tick is stated and shown.
- **Every call is a transaction.** Over budget or trapped: the call's
  writes are undone, a typed diagnostic says which handler and why, and a
  script that keeps failing is switched off until its next revision.

Compiling a stranger's source happens in a job with no secrets, a time
limit and a memory limit (the build pool). Its output is only bytes that
the server then checks.

### A script is content

The world document names a script by hash, in dev and live revisions like
any edit. Publishing follows the compatibility rule: a new version may drop
handlers or powers, and adding one is a visible change that the publisher
confirms.

## What this is not

- **Not a change to Coax for engine code.** Engine and game code stay
  native, with no VM. The interpreter is only for scripts.
- **Not Verse's failure contexts, logic variables or garbage collector.**
- **Not client-run authority.** A script runs in the room; the page only
  predicts.
- **Not open to everyone on day one.** Today's rule stands: realm code is
  ours or a reviewed PR. Stages 1 to 4 keep that rule, with scripts written
  by us and our agents. Stage 5 is a separate decision.

## Stages

1. **The profile.** `#script` in the compiler (effect bound, load
   allowlist, no recursion, fixed state) and a first script API for foes.
   Checks only; scripts still link into the page. Splice's raider and
   robot behaviours move into scripts as the proof.
2. **The module and the limits, on the page.** A script compiles to its
   own module with API imports only. The page runs each call with an
   instruction budget, a rollback and typed diagnostics. An agent in the
   editor writes and swaps a script without touching the realm's code.
3. **Scripts in the world document.** By hash, with the server's module
   check, dev and live revisions, and the publish rule. Authors are still
   only us.
4. **Scripts in rooms.** The interpreter in Coax, the per-tick budget, and
   a check that page and room agree step for step.
5. **Other authors.** The compile job, quotas, review and reporting. This
   needs the owner's decision to change the "ours or a PR" rule for this
   tier only.

## Costs and risks

- **The interpreter** is the largest new piece: a few thousand lines of
  Coax, with a validator, fuzzed. Until stage 4, scripts are page-only and
  rooms run plans as data.
- **The API decides whether scripts are worth writing.** Too small and
  makers go back to asking us for code; too large and it is a second
  engine. Start with foes and triggers and grow it by request.
- **Speed.** Interpreted scripts are slow compared with native rules. The
  budget must be visible in the editor, per script.
- **Two ways to write behaviour.** Plan data stays the first choice; a
  script is for what data can't say. The editor should offer data first.

## What would change this

- If plan data and content tables turn out to cover what makers and agents
  ask for, stop after stage 1: the profile is cheap and still tightens our
  own rules code.
- If we decide rooms should never run code we did not review, stop after
  stage 3 and keep scripts as page-side prediction and presentation only.

## What is built

Kept current as stages land (#13467). The contract as built is
[docs/protocol/realm-script-v1.md](../protocol/realm-script-v1.md),
[docs/protocol/realm-script-api.md](../protocol/realm-script-api.md) and
[coax/script-profile.md](../../coax/script-profile.md).

| Piece | State | Where |
| --- | --- | --- |
| `#script` profile in the compiler | built | `coax/lib/compiler/script-profile.coax`, `coax/src/effects.cpp` |
| The script API as one table | built | `metaverse-engine/script/api-table.coax` |
| The build: source to a checked module | built | `coax/web/script.sh`, `metaverse-engine/tools/script-build.coax` |
| The module check | built | `metaverse-engine/script/wasm.coax` |
| The interpreter: budget, owned memory, traps as values, rollback | built | `metaverse-engine/script/interp.coax` |
| The host: a tick of a world's scripts, diagnostics, switch-off | built | `metaverse-engine/script/host.coax` |
| The manifest and the frame and result bytes | built | `metaverse-engine/script/manifest.coax`, `frame.coax` |

Where the build differs from the proposal:

- **The page runs the interpreter too.** The proposal had the page
  instantiate a script's module with the browser's own WebAssembly engine
  and a room interpret it. The browser has no instruction budget and no
  rollback for a module it runs natively, and two engines would count
  differently. So the page runs the same interpreter the room does, in a
  module of its own beside the engine. Page and room then agree instruction
  for instruction by construction, and the budget shown in the editor is the
  budget a room enforces.
- **Writes are commands.** A script does not write the World directly. A
  write queues a command, and the realm applies a kept call's commands to
  its own running state. A failed call's commands are dropped, which is the
  rollback. A handler does not see its own writes until the next tick.
- **A script writes only its own thing.** The host refuses a write that
  names any thing but the one the handler was called for.
- **The module check does not type-check operands.** It checks structure,
  imports, limits and every index. The interpreter checks the stack and the
  memory on every step, so a type-confused module computes nonsense inside
  its own memory.
- **Plan data reaches a script as a goal.** A thing's plan (#13466) still
  decides where it walks when calm: `sa_goal_x`, `sa_goal_z` and
  `sa_goal_speed` give a script the plan's goal for the tick, so a script
  adds a reaction to a plan and does not replace it.
