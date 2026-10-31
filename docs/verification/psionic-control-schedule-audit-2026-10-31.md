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
