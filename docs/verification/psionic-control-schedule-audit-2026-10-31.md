# SDK schedule audit for the future typed-clock consumer

Read the existing `panda-session.coax:ps_control` and the native provider loop;
no simulator/model gate reran. `pc_step` executes on every 2ms physics step;
its goal-update flag is true at substep 0 only. Action goals therefore update
at 20Hz while the torque servo and physics run at 500Hz. Eight accepted rows
per chunk give nominal 2.5Hz model requests, with synchronous inference pausing
simulation. After 25 steps, the loop commits the row, handles a delayed reply
and samples task success. Per-physics-step time/trace checks remain unchanged.

The own-backend note now names all four schedules explicitly. This is source
evidence for what R13 must preserve, not a new typed-clock or throughput proof.
The GR00T target note also records the already-qualified source-framework
comparison and numerical/checkpoint precursors; all old receipts are reused.


## Request demand and observation cache follow-up

Source-only follow-up distinguishes an eight-consumed-row request from a
fixed modulo-200 trigger. The existing refusal/hold path commits one 25-step
row, then the next `pl_request` reanchors its origin at action tick 1. The
controller computes gripper target increments at every physics step, even
though the action command is held for a complete row. The provider's
inference capture/render precedes the next explicit forward/sample. Sample
reads cached site/body fields and current qpos; render does not add a forward
call. Reset explicitly forwards. No physical cache timestamp is inferred
without checking the pinned integrator. These details are requirements for
the new runtime clock precursor, not evidence of its implementation.

Reviewed source identities:

- `psionic/lib/robotics/panda-loop.coax`: `9a35fbd0fefaa34f23ed0bb6026dc7821d7dfa9f6ba9eafedb4fc8e583a4fcd8`

- `psionic/lib/robotics/panda-control.coax`: `6a0ef9ad54cd0ea9d12a1ce77a1e73a48c957bbba173281e5a1de0cf6020bdbc`

- `psionic/tests/robotics/panda_loop_effects.h`: `27da2477e2e6e359ce00a9813607d05eaa253ad3a4609822b28ea9fe2917d687`

- `psionic/platform/panda-env.c`: `dfbc195dacdfe1c1fb6011c0cd1e44aa9fd0d9d53075c8d9eb991c00b0903cb0`

No SDK/model episode, native arithmetic or previous passing mutation gate reran.
