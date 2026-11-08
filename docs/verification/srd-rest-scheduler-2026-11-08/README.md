# Automatic, ordered rest boundaries

Initial verification base: `b14d567010`. Source: pinned SRD 5.2.1 p185;
PDF identity is recorded in `docs/dnd/long-rest-implementation.md`.

## Behavior

`rs_rest_schedule_enable` selects battle-owned scheduling before actor journals
start; manual/external owners keep their existing interface. `rs_tick` delegates
to `rs_rest_tick`, which preserves the ordinary tick path for manual owners.
The scheduled path preflights time and all active rest body/bank state before
mutation. It refuses stale historical completion and incompatible clocks or
unowned sleep. `rs_reset` clears scheduler ownership.

A large step is split at each distinct actor boundary. Journal progress is
credited at the boundary, ordinary timed effects expire through that point,
and pending completion or Short Rest fallback is then consumed before later
time. A healing prohibition that expires *after* rest completion therefore
still blocks that recovery; later expiry does not retroactively heal the
creature. Earlier expiry permits recovery. Multiple actors retain their own
finish timestamps. Zero-time pending completion is consumed before future
fractional time. No replay grants another completion or Reaction budget.

Fractional carry is preserved across splits, including independent legacy and
persistent timer epochs. Completion sharing an integer microsecond with frame
end is still processed before a later fractional effect expiry. A retained
light-limit marker prevents the same boundary from stalling fractional time.
Exact-cap watch can select sleep; continued watch abandons the period.

Exertion grants its earned Short Rest once, with class banks and initial Hit
Dice ownership, without automatic HP healing. It opens the existing optional
choice window and ends Rage. Actual exertion closes an old Short Rest choice
window, and cannot admit a new Short Rest while the active journal still
records exertion. Explicit Short Rest completion consumes its quiet segment so
Long Rest interruption cannot grant the same segment twice.

## Verification before merging newer main

- Composed `long-rest-resources-checks.coax --library`: 308 checks pass. New
  checks preceded implementation and were sharpened against surviving mutants.
- Scoped mutation through that entry, zero survivors/exemptions:
  scheduler196/196, owned operations150/150, Short Rest260/260, journal283/283.
  Total889 killed. The unchanged owned-operation result is retained from its
  first passing run; later changes were to scheduler/tests and Short Rest.
- Compiled `coax/web/tool.sh aethyr/rules/srd/rest-scheduler-native.coax`:
  `PASS REST-SCHEDULER (72 class/level transactions, 1843200 automatic frames, exertion and fractional ordering)`.
  All twelve classes at levels1–6 execute two-actor sleep/watch transactions,
  distinct finish times, blocked versus eligible healing, bank recovery and
  preserved Reaction. A separate run executes every 64 Hz frame for eight
  hours with automatic completion, then tests exertion fallback, manual-owner
  Short Rest admission, and compiled sub-microsecond effect ordering.
- `coax/web/tool.sh tools-check`: 1897 tracked tools, 35 vendored, zero missing
  reasons. `git diff --check` passes.
- Checked Lab Wasm composition: 35.5s, 2,545,751 bytes:

```
coax/web/wasm.sh apps/openagents-web/fivelab.coax apps/openagents-web/fivelab-face.c /tmp/srd-rest-scheduler.wasm apps/openagents-web/everglade-step.coax aethyr/sticks.coax
```

Initial Wasm SHA256:
`dc01cfcaf5975cc67476c03a61c8df29693d68268b2ecfbded7e9b74b5057e59`.
Existing compiler-version and near-limit compile-time work/value advisories
remain (value slots87%); no work budget was raised. No shipping size budget was
added for the temporary artifact name.

Mutation uses `coax/bin/coax mutate
 aethyr/rules/srd/long-rest-resources-checks.coax --library --unit UNIT --jobs N`
(N2–3, same mutation semantics). Source SHA256:

- scheduler: `a9a932c1a67173dd5bca0e8c910df2495307d73516b26f70b76c4cd481b84a9e`
- owned operations: `dbecd2720c99340db2920abf91e8604995cc970f8830f8c29942f5a0325d2ebc`
- journal: `90cfde4a40530e62c8833d05b43abbfc08f7782fcad5b7beef489ca2f359536a`
- Short Rest: `683c5cdbfae7f7e0f9bdbeb020ea31f730c95637825c025e0200465f99cf974a`
- final native entry: `f6809180376591a0c20cead4d0a58047018cdc4af63140ab5d27c8025021ca8b`

## Remaining acceptance

The scheduler is an engine ownership mode; current world controls do not enable
it yet. Actual damage, accepted leveled casting and Initiative producers still
need ordered interruption integration. The native damage witness is explicitly
*after* completion and proves replay refusal, not pre-completion interruption.
Queued optional choices must be resolved before later causal events; default
fallback does not automatically spend Hit Dice. Existing movement producers
still need to report actual Long Rest activity, including stopping/waking.

World controls, settled-observation admission, rejoin/timeline binding,
source-specific stat/nutrition producers, derived-stat consequences and full
held-inventory dropping remain required. General legacy numeric deadlines and
other class features remain separate issue requirements. #13040 stays open.

## Final merged-main verification

Rebased onto `0bb972d3d2`, preserving the new four spell executors, Magic Missile
ward, area-origin profile and both invariant additions. The changed authority
units above are byte-identical to their passing mutation results, so those
results were not rerun. The merged dependency graph and battle layout pass:

- 313/313 composed core checks.
- The final native entry passes the same72 class/level transactions, all
  1,843,200 automatic frames, exertion fallback and compiled fractional ordering.
- Final checked Lab Wasm:42.9s, 2,548,777 bytes; SHA256
  `ff7d19136441cfe2506403bbeadb794347e613d038ff62f821b1c025f71fe9f6`.
  Command as above with output `/tmp/srd-rest-scheduler-merged.wasm`.
  Existing value-slot advisory87%; no budget increase.

`git diff --check` passes after resolving the append-only invariant conflict by
retaining both the area-origin/Shield text and the new rest-scheduling text.
