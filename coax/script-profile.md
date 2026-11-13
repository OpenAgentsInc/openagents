# The realm-script profile

`#script "name";` marks a Coax unit a realm script: code that someone other
than us may write for a world in the OpenAgents Metaverse, compiled to its
own WebAssembly module. This page is the compiler's side of the contract.
The design is
[docs/research/2026-11-13-realm-scripts.md](../docs/research/2026-11-13-realm-scripts.md);
the module, the limits and the diagnostics are
[docs/protocol/realm-script-v1.md](../docs/protocol/realm-script-v1.md).

The profile is a restriction of Coax, not a second language. A script is
ordinary Coax that passes more rules. Nothing changes for a unit without the
directive, and an ordinary program may load the same library and API units a
script loads.

## What the compiler enforces

```coax
#script "patrol";
#load "../api.coax";                 // the script API: a #script_api unit

Mind :: struct #persisted { turn_at: f32; steps: s32; }

on_tick :: (me: u64, mind: [] Mind, dt: f32) #export_c #effects(reads, writes, traps) {
  mind[0].turn_at += dt;
  if mind[0].turn_at > 2.0 { mind[0].turn_at = 0; sa_move(me, sa_x(me) + 1.0, sa_z(me), 1.5); }
}
```

| Rule | Code |
| --- | --- |
| The directive is `#script "name";` or `#script_api;`, once a unit, at file scope. The name is 1 to 48 lowercase letters, digits and hyphens and starts with a letter. A script is the entry file of its own compilation: no unit loads it. | E0980 |
| Every procedure the script declares may only read, write and trap, through its whole call graph. A declaration, if written, is within `reads, writes, traps`. The refusal names the effect and the call path. | E0981 |
| Native code is reached only through `#foreign` procedures in a `#script_api;` unit, each declaring effects within `foreign, reads, writes, traps`. A script that calls one is charged what it declares, without `foreign`. A `#foreign` anywhere else in the compilation is refused. | E0982 |
| No procedure the script declares reaches recursion, so its stack has a known bound. | E0983 |
| The only `#export_c` procedures are the script's handlers, in its own unit, with exact signatures, and there is at least one. There is no `main`. | E0984 |
| Every handler takes its state as a writable view of the same `#persisted` struct the script declares. | E0985 |

A script cannot print, read a clock, call native code outside the API or
read uninitialised storage: each is an effect outside the bound. Coax has no
allocation, so there is none to refuse. Time and chance come from the engine
as arguments and API calls.

### Handlers

| Handler | Signature |
| --- | --- |
| `on_tick` | `(me: u64, mind: [] State, dt: f32)` |
| `on_enter` | `(me: u64, mind: [] State, other: u64)` |
| `on_leave` | `(me: u64, mind: [] State, other: u64)` |
| `on_notice` | `(me: u64, mind: [] State, other: u64)` |
| `on_hit` | `(me: u64, mind: [] State, other: u64, amount: f32)` |
| `on_use` | `(me: u64, mind: [] State, other: u64)` |
| `on_timer` | `(me: u64, mind: [] State, timer: s32)` |

`me` is the thing the handler is called for. `mind` is a one-element view of
that thing's state record.

### Libraries

A script may `#load` any unit. What it calls from that unit is held to the
script's bound, because the bound follows calls. A unit with a `#foreign`
that is not marked `#script_api;` cannot be loaded at all. The same unit
loaded by an ordinary program is under no rule of this page.

## `coax script`

```sh
coax/bin/coax script FILE            # the manifest, coax.script.v1
coax/bin/coax script FILE --face     # the C that gives the module flat exports
```

The manifest names the script, its handlers, its state record (type, size,
alignment and each field's name, type, offset and size) and the API imports
some handler reaches, with their effects. The face declares the state of
every thing the script drives as one static array and exports `rs_abi`,
`rs_state`, `rs_state_size`, `rs_slots` and `rs_<handler>` for each handler.

`coax/web/script.sh FILE` runs the whole build: this command, `coax c`,
clang for wasm32 without a libc, the linker and then the module check.

## What the compiler does not do

The compiler is not a sandbox, and this profile does not make it one. The
wall is the module: a WebAssembly module can call only what it imports, and
the module check
([metaverse-engine/script/wasm.coax](../metaverse-engine/script/wasm.coax))
refuses any import outside the script API, any memory over the limit and any
size over the budget, whatever produced the bytes. A compiler bug can refuse
a good script or pass a bad one into C; it cannot grant a module a power.
Whoever compiles source they did not write runs the build in a job with no
secrets and hard limits.

## Where it is implemented

- Policy: [`lib/compiler/script-profile.coax`](lib/compiler/script-profile.coax),
  an `#authority` unit with a zero-survivor mutation gate in the bootstrap:
  the directive's syntax, the effect bound, the handler table and the
  recursion search.
- Native bridge: `src/sources.cpp` finds and blanks the directive;
  `src/effects.cpp` (`script_profile`, `effect_contracts`, `script_report`)
  hands resolved procedures, calls and types to the policy.
- Checks: `coax/tests/run.sh --only script_profile` runs the policy's checks
  and mutants and [`tests/script-profile.coax`](tests/script-profile.coax),
  which compiles every refusal in [`tests/script/`](tests/script/) and the
  admitted script's manifest and face.
- The bootstrap compiler (stage 0) has no profile: there `#script` is an
  unsupported directive.
