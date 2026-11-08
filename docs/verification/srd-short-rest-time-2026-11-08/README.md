# Short Rest uses the owned integer timeline

Source: SRD 5.2.1 Short Rest p.187; Long Rest interruption p.185.
Pinned PDF SHA-256:
`8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
This closes the specific Short Rest clock-precision gap in #13040/#13046, not
those issues' remaining action, world, choice or persistence acceptance.

## Change

Short Rest retains its start as integer microseconds on the same owned clock
as Long Rest. Admission validates that a complete hour fits before seeding or
mutating resources. Completion requires the actual integer deadline; assigning
the legacy f32 spell/display clock cannot finish a rest. Invalid/reversed/
unrepresentable active rest metadata refuses a tick before time or effects
change. Inactive historical metadata does not block time.

Every admitted tick now splits at Short Rest boundaries, whether or not Long
Rest scheduling is enabled. Automatic Long Rest credit/consumption remains
opt-in. Fractional and overdue boundaries progress once without rewinding time
or duplicating recovery. A large tick completing Short Rest at one hour then
continues crediting the later quiet Long Rest segment; a late poll no longer
loses that segment or timestamps ordinary completion at the poll end.

The integer clock exposes one-microsecond simulation quanta with fractional
carry between frames, as in the existing Long Rest clock contract. Spell,
display and event-log timestamps still use their legacy f32 epoch; this change
does not qualify those timers or foreign-epoch rejoin. The internal rest-state
layout changes; old raw f32 rest anchors are not silently reinterpreted.

Resource-only manual fixtures now explicitly advance the owned clock as well
as their display clock. A separate regression proves display-clock assignment
alone cannot complete rest. No compatibility branch retains f32 admission.

## Evidence

The display-clock regression failed before implementation. Final results:

- 353/353 composed core checks and 334/334 hotbar checks pass (overlapping
  suites). Exact integer/deadline, overflow, reversed time, malformed metadata,
  fractional target, overdue poll, manual Long Rest ownership and later quiet
  credit cases are covered.
- Authority scopes: short-rest-time 41/41, rest-scheduler 201/201 and short-rest
  227/227 mutants killed; 469 total, zero survivors/exemptions. Unchanged
  rest-state procedures retain their earlier passing gate; its schema alone
  changes here.
- Native execution passes **1,722,982 actual frame ticks** across 30/60/120/
  144/64 Hz and uneven intervals, including epochs where f32 frame additions
  no longer advance. Recovery occurs only on crossing the owned hour, never
  early, and never twice. All 72 class/level recovery-and-chosen-Hit-Die cases
  pass without automatic HP healing or Reaction renewal.
- The existing Experimental native witness passes again against the new state
  layout: 72 class/level control/recovery flows, pointer Hit Dice and admitted
  motion accounting. Its explicit scheduler advances are not an eight-hour
  rendered world/AI endurance run.
- Shipping Lab Wasm passes in 66.5 seconds, 2,553,577 bytes within the unchanged
  2,560,000-byte budget. SHA-256:
  `9511e4649043adc9c221866518b954a15113c6588bee11337f38879ab26a607f`.
  Existing compiler source-version/value-use advisories remain. No memory,
  stack, traversal or size budget was raised.
- Muted headless Chrome with Apple Metal WebGPU loads canonical Experimental
  at 1280×900, reports `ready=true`/`sceneReady=true`, and an actual Short pointer
  click displays `SHORT REST`. Screenshot `/tmp/srd-short-time-world.png` was
  visually inspected. Earlier unchanged phone layout evidence is retained.
- Tools policy: 1897 tracked tools, 35 vendored, zero missing reasons.

```sh
coax/bin/coax test aethyr/rules/srd/long-rest-resources-checks.coax --library
coax/bin/coax test apps/openagents-web/fivebar-test.coax --library
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/short-rest-time.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/rest-scheduler.coax --jobs 2
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/short-rest.coax --jobs 2
coax/web/tool.sh aethyr/rules/srd/short-rest-time-native.coax
coax/web/tool.sh apps/openagents-web/experimental-rest-native.coax
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c apps/openagents-web/build/fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh tools-check
```

The headless check uses the existing static dev server and `shot` tool against
`/worlds/everglade-experimental/`, then dispatches pointerdown/up at CSS (55,465)
and awaits two animation frames. The local server is stopped after verification.

## Remaining work

Everglade II/room controls and activity, persistence/rejoin epoch binding,
remaining optional recovery choices and controller simultaneous ordering,
source loss/nutrition producers, inventory dropping and sleep/Prone presentation
remain open. Room/protocol integration is not started in this change.
