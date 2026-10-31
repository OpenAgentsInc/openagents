# 3. State starts from its defaults

> **Status: unimplemented — design draft.** Native Coax applies a struct's
> field defaults when it declares `s: [1] T`; the browser host hands a realm
> zeroed memory, and `coax/web/defaults.py` now writes a
> `coaxial_state_init` export so the browser applies the same defaults. This
> chapter makes that a property of every realm by construction. Engine-core
> task: `ec-state` (#12609); the engine-core agent fills this chapter in as
> the module lands. Its check landed on 2026-10-31:
> `coax/web/state-defaults.test.ts` boots 16 realm states without
> `coaxial_state_init` and requires each to differ from native's defaults;
> it has not joined the chapter yet.

On 2026-10-31 Orbit League ran without gravity for a day: its state declared
`spin_g: f64 = 1.0`, the native build applied it, and the browser read `0`
because WebAssembly memory starts zeroed. The fix generated an initializer.
The lesson is broader. A realm's state is a value the engine constructs, and
the same construction must happen on every host, before the first step, with
nothing left to chance.

## What this chapter teaches

- What a declared default is, and where Coax applies it.
- Why zeroed memory is not a constructed value.
- One construction path for native and browser hosts.
- Comparing initial states byte for byte.

## The method

A realm's state type `T` has, for each field, either a declared default or
the type's zero. Construction is a pure function of the type:

```jai
init(T) = T.{ f₁ = default₁ or zero(f₁), …, fₙ = defaultₙ or zero(fₙ) }
state   = init(T)          // before any step, on every host
digest  = sha256(bytes(state))
```

The engine never hands a realm raw memory. The host allocates the state's
bytes (the browser's arena, native storage) and immediately calls the
realm's constructor, the same code on both. Nested structs and fixed arrays
of structs are constructed recursively; padding bytes are zeroed so that the
byte comparison is meaningful.

A realm may also declare a `setup(state, seed)` step that runs after
construction and before the first fixed step (placing the spawn, building a
heightfield). Setup is deterministic in its inputs and is recorded as part of
a session (chapter 5). Construction plus setup is the initial state.

## What exists today

- [coax/web/defaults.py](../../coax/web/defaults.py): reads the compiler's
  `static T coax_init_t_T(void)` functions, exports `coaxial_state_init(state)`
  for the face's state and `coaxial_state_default(i)` for checks, and runs a
  constructor that defaults the face's Coax globals.
- [coax/web/host.js](../../coax/web/host.js) calls
  `x.coaxial_state_init?.(arena.state)` on a fresh state; the `?.` means a
  realm without the export still starts zeroed.
- [coax/web/state-defaults.test.ts](../../coax/web/state-defaults.test.ts):
  for each engine `apps/openagents-web/build.sh` makes, runs the built
  module's `coaxial_state_init` on a zeroed state and compares every
  defaulted part, byte for byte, with the native `coax_init` of that type.
- Realms such as [verse/player.coax](../../verse/player.coax) rely on
  defaults heavily (`on_ground: bool = true`, `theta: f32 = PI`,
  `dist: f32 = CAM_MAX`, `pace: f32 = 1`).

## What it builds

- `aethyr/state.coax`: the realm state contract: the state type, its
  constructor, an optional deterministic `setup`, and `state_digest`.
- Host changes: the browser and native hosts call the constructor
  unconditionally; a realm without one fails to load with a visible error
  rather than starting zeroed.
- A per-realm check that lists every declared default and its value after
  construction on each host.

## Acceptance checks and oracles

- A check fails if any realm's state starts zeroed where a default is
  declared: for each realm, every field with a declared default reads that
  value after construction, on native and in the browser.
- Native and browser initial states are byte-identical: the digest of the
  constructed state (and of the state after `setup` with a fixed seed) is the
  same on both hosts.
- Oracle: the compiler's own `coax_init_t_T` output for each type, compiled
  natively, independent of the browser path.
- Negative check: a deliberately zeroed state for a realm with a default is
  reported, naming the field.

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md) (the first step runs after
construction). From Coax: struct defaults
([Coax chapter 5](../../coax/book/ch05-structs-enums-unions.md)), memory
([Coax chapter 6](../../coax/book/ch06-memory.md)), targets
([Coax chapter 21](../../coax/book/ch21-targets.md)), and R16 snapshots
([Coax roadmap](../../coax/ROADMAP.md)), which reuse the same construction
when restoring.
