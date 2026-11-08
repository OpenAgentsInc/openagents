# Experimental rest controls and admitted activity

Pinned SRD 5.2.1: Long Rest p.185, Short Rest p.187, Unconscious p.191.
PDF SHA-256: `8974902d109d6e63672d7c490bde9ccf052410503d9cfa768237154fbc5e3d87`.
Scope: shared control/activity policy and the actual Experimental consumer under
#13040/#13046. Neither issue is complete. Everglade II's room path is separate.

## Behavior

The core control policy requires scheduled, validated actor ownership and
settled downtime. It supports Short Rest, Sleep, Watch, chosen Hit Dice and
ending rest. Sleep/watch explicitly start, change or resume Long Rest; stopping
movement never manufactures sleep. End preserves completion/16-hour history and
refuses unconsumed earned boundaries rather than discarding them. Short Rest
cannot start over active/pending Long Rest. Hit Dice remain sequential choices.

An admitted walking/jumping observation selects exertion before the frame's
time is credited. Grounded stopping returns to watch; a blocked held key does
not keep crediting walking. Unsettled airborne activity retains exertion until
settled. Actual motion also closes Short Rest/choice windows. Passive translation
and refused movement do not invent voluntary exertion.

Experimental now enables battle scheduling at initialization, reports actual
player and accepted monster movement and exposes five pointer controls below
the level selector. Starting requires an observed grounded/still body, released
movement input, no airborne/automatic-walk state, pending strike or active dash.
The rest state and refusal are shown. Watch explicitly wakes natural sleep;
Sleep uses the shared Unconscious/locomotion permissions. Shared completion and
Hit Dice flow back through the existing HP display. World time is not skipped.

A browser check exposed an existing startup trap on main commit `606d0b0a19`,
reproduced with that prior Wasm at the canonical Experimental URL. Generated C
showed panel helpers passing the complete combat state by value just to read
layout fields. Passing borrowed views for class height, column and slot/rest
boxes removes those copies and the trap. No stack/memory/size limit was raised.

## Verification

- The new real-controller Long Rest motion regression failed before the hook.
- Core composed checks: 344/344. Changed rest-control authority: 83/83 mutants
  killed, zero survivors/exemptions. Earlier unchanged core gates are retained.
- Three final selected Experimental checks cover actual Long Rest movement,
  busy/settled control admission and pointer Short Rest/Hit Die/Sleep actions in
  expanded/collapsed columns. Two level-selector/multirow checks also pass after
  the borrowed-view change. The two existing Short Rest activity checks pass.
  These are focused selections, not a claim that every world acceptance passed.
- Final native compiled witness passes all 72 class/level control/recovery flows,
  pointer Hit Dice and admitted-motion accounting. It explicitly advances the
  shared scheduler rather than rendering eight hours of world/AI frames; the
  earlier frame-clock witness covers 1,843,200 actual clock steps.
- Final shipping Lab Wasm passes in 39.6 seconds: 2,553,473 bytes against the
  unchanged 2,560,000-byte budget. SHA-256:
  `f238fc401d0121f01edd42ab8cf12bd9b23e786d59348c15a1251613c91fd95e`.
  Existing compiler source-version/value-use advisories remain.
- Muted headless Chrome, Apple Metal WebGPU: canonical Experimental loads with
  `ready=true`, `sceneReady=true`; actual pointer Sleep clicks display
  `REST: SLEEPING` at 1280×900 and 390×844. Both screenshots were visually
  inspected; class/level/rest controls and hotbar remain visible.
- Tools policy: 1897 tracked tools, 35 vendored, zero missing reasons.

Local observed screenshots (not shipping assets):

| Image | SHA-256 |
| --- | --- |
| `/tmp/srd-rest-world-sleep.png` | `08cc9133fd05b83fa1c4bb9fa091afbdbb6eab02ae211871f53ab9c72b7a2fc8` |
| `/tmp/srd-rest-world-phone.png` | `9a486f4da70fe923623450fe3aa4ce67fa636c0c76362335aa7cf7bf1580c070` |

```sh
coax/bin/coax test aethyr/rules/srd/long-rest-resources-checks.coax --library
coax/bin/coax mutate aethyr/rules/srd/long-rest-resources-checks.coax --library --unit aethyr/rules/srd/rest-control.coax --jobs 2
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'Experimental'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'experimental combat level'
coax/bin/coax test apps/openagents-web/fivelab.coax --library --only 'rest activity'
coax/web/tool.sh apps/openagents-web/experimental-rest-native.coax
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c apps/openagents-web/build/fivelab.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
coax/web/tool.sh tools-check
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:8894 --static
coax/web/tool.sh shot http://127.0.0.1:8894/worlds/everglade-experimental/ /tmp/srd-rest-world-controls.png 1280x900 3000
```

The Sleep captures additionally dispatch pointerdown/up at CSS (155,465) on the
canvas after scene readiness and await two animation frames. The phone capture
uses touch pointer events. The local server is stopped after verification.

## Remaining acceptance

Everglade II/room-authoritative controls and activity, persistence/rejoin epoch
binding, full optional Arcane/Natural/Sorcerous Recovery choices and simultaneous
ordering, source loss/nutrition producers, inventory dropping and sleep/Prone
presentation remain required. Short Rest still uses the legacy f32 deadline;
its precise frame-duration/late-clock migration remains to qualify. No offline
rest, room protocol, full eight-hour hostile-world run or issue closure is claimed.
