# Frame-owned rest clock and sub-second journal boundaries

Base: `42def3bc4d`. Source contract remains pinned SRD 5.2.1 p185;
PDF identity is recorded in `docs/dnd/long-rest-implementation.md`.

## Clock and journal contract

Every admitted `rs_tick` now advances an owned integer microsecond rest clock.
Fractional carry retains frame time below the exposed microsecond simulation
quantum. The f32 spell/display clock remains separate. Invalid/negative or
nonfinite time, poisoned clock metadata and signed-integer overflow refuse
before either clock or ordinary tick effects change. `rs_reset` resets the
owned rest timeline. Direct assignment to the old f32 clock does not retime it.

The journal explicitly supports seconds or microseconds. Existing second-based
callers retain their behavior. A new admitted microsecond journal scales every
source boundary: eight-hour minimum, six-hour sleep, two-hour light activity,
one-hour exertion/extension/Short Rest eligibility and sixteen-hour exclusion.
Its unit is retained across resumption and cannot change after completion
history exists; a unit change cannot reinterpret the exclusion interval.
Completion still finds the first exact tick in bounded binary-lifting work.

## Verification

- Final composed `long-rest-resources-checks.coax --library`: 290 checks pass.
  New checks precede implementation and cover one-tick boundaries, fractional
  starts, rate/history retention, invalid rates and before-mutation refusal.
- Scoped mutation through that entry: timing policy 57/57, journal 283/283,
  frame clock 54/54 killed. Total394, zero survivors/exemptions.
- Actual compiled `coax/web/tool.sh aethyr/rules/srd/rest-clock-native.coax`:
  `PASS REST-CLOCK (1843200 real rest frames, fractional start and 16-hour readmission)`.
  It starts at 15,625 microseconds, executes every 64 Hz battle tick for eight
  hours, refuses completion one frame early, consumes the pending boundary at
  28,800,015,625 microseconds, checks HP/Exhaustion/sleep recovery and refuses
  readmission until sixteen hours after that exact finish. It also executes
  signed-integer overflow refusal with fractional carry. This is a compiled
  engine/scheduler witness, not eight hours of browser wall-clock observation.
- `coax/web/tool.sh tools-check`: 1897 tracked tools, 35 vendored, zero missing
  reasons. `git diff --check` passes.
- Full checked Lab Wasm composition passes in43.3s, 2,539,962 bytes:

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-rest-clock.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

Wasm SHA256:
`af61f633d6b0db71c219fcfa0c3f531a5f6c24059b3b61e321508143dafcc85b`.
Existing compiler-source-version and near-limit compile-time advisories remain
(value slots86%); no work budget was raised. The temporary artifact's absent
shipping size budget does not introduce a product budget.

Mutation uses `coax/bin/coax mutate
 aethyr/rules/srd/long-rest-resources-checks.coax --library --unit UNIT --jobs 2`.
Source SHA256:

- timing: `d1a09730b1d34f90aeffab58d5318f6904dd13ce04eeb5027d35b25f350a5b52`
- journal: `f1c9437539d44d7e33c915d9df3c15ddb1c6f83424568038d48ce9c985bb9162`
- clock: `7a6eec3fe781d3280b165aec9ea76b987035350c11d5017074b99b331e9d212c`
- native entry: `c41c2a6123285c9e90ce12ae86c7cdc845b46dfbb7f4ddd14fb01bb508f947e5`

## Required remaining integration

The real battle clock now advances; it does not yet automatically advance
actor journals or expose Long Rest controls. The native witness explicitly
selects sleep, synchronizes its journal and consumes completion. World
admission, mode transitions and before-event interruption synchronization are
next. In particular, a late mode report after the light budget must not invent
sleep in the intervening gap. Actual event ordering must expire the appropriate
prior effects and apply rest completion before later damage/casting/Initiative.

World startup/rejoin owners must bind the retained journal to this timeline;
legacy manual f32 clock assignments are not a binding or history migration.
Persistent clock/session ownership, controls, source reduction/nutrition
producers and full inventory dropping remain acceptance items. No issue is
closed by this increment.
