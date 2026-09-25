# Executed contract checks: complete the eight-task measurement

Issue [#9628](https://github.com/OpenAgentsInc/openagents/issues/9628).
Written before this supplemental replay on 2026-09-25. This is a post-label
measurement: the 16 candidates' outcomes were published by #9584 before
this work began. It does not replace the original protocol, change its
split, or claim a new blinded confirmation.

## Population and isolation

Use exactly the eight Luna and eight Astra candidates in
`../2026-09-25-candidate-review/records/prospective-measurement.json`.
The eight tasks are `distributed-dedup`, `formal-crypto`,
`freecad-impeller`, `freecad-spring-clip`, `math-eval-grader`,
`pretrain-shard-corruption`, `shadow-relay`, and `vpp-loss-divergence`.
Use the published final labels, including the four recorded CAD regrades.
Retain the original missing grades and their regrade provenance.

Verify the retained inputs against both published prospective trace
manifests. Expose only the 16 named trial directories to the offline
reader through a separate jobs directory. No broad job inventory, new
agent attempt, official verifier run, or repair is permitted. The running
artifact-v2 cohort and the v18 family are outside this population.

Run one replay worker on coderos. Use the already-built public environment
images by immutable Docker image ID. Do not alter task images, candidates,
active worktrees, or live processes. The existing offline reader chooses
the retained snapshot when present, otherwise the collected final
workspace; record the kind and whether the snapshot is the graded one.
Restoration or execution errors remain visible as unknowns.

## Fixed component

Use the existing binary built from `3a25a0ff1f`, SHA-256
`cdbf781be1c00814ba61bfddb6d69a581f6e3c345215b97afa4bba42b656bcd7`.
Its contract extractor and comparator retain the rules frozen in
`e37c9a5097`. Later source changes expose helpers and add separate component
modules; the Docker-backed contract path retains the original behavior.
Retain the source diff for inspection.

Run `checks contract offline` with `--kinds snapshot,final`,
`--workers 1`, and an explicit image ID for each task. Keep the original
timeout, outcome, candidate-call, score, and Jev rules. The existing narrow
Jev questions may resolve instruction ambiguity, with answers recorded and
no candidate content in their input. Cap additional Jev spend at $0.05;
no Luna or Astra generation is authorized by this replay. Reproduction
reuses the saved plans and recorded Jev answers.

## Analysis

Use the original measurement definitions and Wilson intervals. Report all
16 identities, per-task and per-executor calls, per-kind outcomes, coverage,
unknowns, elapsed time, and Jev usage. Report the subset with verified
graded-workspace identity separately if restoration cannot establish all
16. Unknowns remain in the population and failure-recall denominator;
also retain the original complete-case summary for comparability.

Measure within-task concordance only on tasks with both a pass and a
failure; undefined scores tie, as in the original protocol. Keep the
original task-bootstrap summaries, with their small-sample limitations.
Do not pool this supplement into the original held-out claim. A negative
measurement completes the issue's component-and-measurement checklist;
it does not admit the component into a runtime policy.
