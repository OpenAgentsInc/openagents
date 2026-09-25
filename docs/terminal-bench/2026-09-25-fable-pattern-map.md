# Fable 5.1's winning runs, mapped to components

Status: analysis, 2026-09-25. It applies the fourth change in
[patterns as components](../coder/design/pattern-components.md): map Fable
5.1's winning runs on several tasks to System One components and Microluna
sessions, and build only the patterns that recur in at least three tasks.
No runs were made; this reads the public trajectories cached under
`~/.openagents/terminal-bench/public-replays/`.

## What was read

Ten tasks where Fable 5.1 low passes 4 or 5 of 5, chosen for spread: a
proof, forensics, black-box replication, a checkpoint format, a streaming
service, entity resolution, a GPU kernel, a Spark job, a procedure-driven
filing, and a routing optimization. For each, at least two passing
low-effort trajectories were read step by step, and some higher-effort or
failing runs for contrast. `embedding-drift-monitor` comes from the earlier
mapping in the design note.

| Task | Runs read (file prefix, steps) | Where the generative work was |
| --- | --- | --- |
| `coq-block-bound` | `8c98154d` (11), `4996d3e7` (15), max `cc20acbb` (7) | Writing the proof |
| `shadow-relay` | `56402d9b` (14), `46307711` (13) | Reading execution traces, writing an emulator, listing decryption variants |
| `risk-scorer-replay` | `51a3ac4b` (16), `4b9d6f36` (20) | Reading disassembly, writing the scorer |
| `mp-checkpoint-consolidation` | `c6997bc8` (18), `de3dfda3` (33), high `b031c043` (19) | Inferring an undocumented layout, proposing option sets |
| `payments-pipeline-fix` | `137808c3` (35), `f11cccd6` (37) | Designing the fix and the fault harness |
| `telecom-entity-resolution` | `5cf8a342` (31), `3308d3bc` (36) | Normalization, scoring, and clustering code |
| `fp8-rmsnorm-gemm` | `8a101e1c` (20), `54390d20` (63), `12c21389` (69) | Writing and tuning the kernel |
| `distributed-dedup` | `d3c4a1e6` (15), `ada4bade` (5), `4e5b1fbe` (19) | Writing the pipeline |
| `intrastat-meldung` | `3844c97b` (21), `acc5a2bb` (25) | Deciding each correction from the procedure; mostly choices between sources |
| `photonic-waveguide-routing` | `09c8b8e3` (11), `077e970e` (12) | Designing the routes |

## The patterns that recur

Counted over the 11 tasks. A pattern counts in a task when a passing run
does it and it matters to the result.

| Pattern | Tasks | Count | Component | Built today? |
| --- | --- | ---: | --- | --- |
| Read every provided file, spec, and procedure before editing | all | 11 | `evidence.select`, `evidence.pack` | Yes |
| Check tools, libraries, versions, and service settings first | coq, shadow, risk, mp, payments, telecom, fp8, dedup, intrastat | 9 | `evidence.environment`, widened to library names and service settings | Partly: tool presence only |
| Write the whole first version in one edit, then fix from the first error | coq, risk, mp, payments, telecom, fp8, dedup, intrastat, photonic, embedding | 10 | `microluna.edit` in a build loop | Yes |
| Rerun an executable check after every edit | every task with edits | 10 | `verify.executed` | Yes |
| **Get an executable acceptance check that doesn't depend on the candidate, before or with the first edit**: the provided checker or example, a reference program, or an oracle written once from the task's own definition (a formula, a brute-force computation, a proxy for a hidden metric) | coq (the build), risk (the reference binary), mp (reference logits), payments (fault harness and expected outputs), telecom (hidden-label proxy), fp8 (formula oracle), dedup (brute-force Jaccard), intrastat (schema and readback), photonic (provided checker), embedding (scenarios and properties) | 10 | `checks.oracle`, new; `checks.contract` covers only the provided-checker case | No |
| **Measure the stated numeric target, then improve it under a keep-if-better rule with a restore** | mp (distance to reference), payments (latency), telecom (threshold table), fp8 (speedup), dedup (runtime), photonic (cost) | 6 | `checks.metric_target` and `control.optimize`, new; `search.option_sweep` for option grids | Keep-best only, on a self-score |
| **Localize a failure before fixing it**: print the source at the error, trace the first mismatched value through intermediate stages, ablate parts, time phases, group errors by feature | coq, mp, payments, telecom, fp8, dedup | 6 | `evidence.error_context`, `evidence.mismatch_trace`, `evidence.phase_timing`, `diagnose.ablation`, `diagnose.error_sample`, new | No |
| Turn every "must not" into a check; remove scratch files; confirm only allowed files changed | coq, risk, payments, fp8, dedup, intrastat | 6 | `checks.prohibitions` and `control.cleanup`, new; `checks.literals` covers output paths | Partly |
| Profile the data the task works on, including a live service's data, grouped by an entity with outliers | shadow, risk, mp, payments, telecom, embedding | 6 | `evidence.data_profile`, widened | Shipped files only |
| Keep a backup or write only on success, and restore after a failed experiment | coq, fp8, dedup, photonic | 4 | Workspace snapshots in `control.optimize` | Keep-best exists |
| Confirm a finding through a second, independent path, or read back what was written | shadow, risk, mp (noise floor), intrastat (readback) | 4 | `verify.readback` and independent confirmation, new | No |
| Bound every run with a timeout; kill or wait on hung work | fp8, dedup, photonic, payments | 4 | `supervise` deadlines, `control.stall` | Yes |
| Never finish while a provided checker or oracle fails | photonic (the failing run finished anyway), dedup (the failing run never measured its budget) | 2 failures that the rule would have stopped | `control.finish` (#9638), extended to oracles and targets | Partly |

Patterns that appeared in only one or two tasks, and stay out of the build
list for now: fitting data to a library of generator families (shadow,
risk), probing a reference program one input at a time (risk), randomized
testing against a reference (risk), comparing sibling files (mp), the
numeric noise floor (mp), converting shipped documents to text and calling
lookup services (intrastat), and a fault-scenario harness (payments).

**None of `embedding-drift-monitor`'s specific patterns recurred.** Defended
comments, method conformance, and profiling shipped arrays didn't matter in
any of the ten other tasks. That agrees with the offline results for
#9652, #9653, and #9654.

## What this says about Coder

1. **The contract is back, in the form winners use.** The pattern that
   recurs most, and that matters most, is an acceptance check that doesn't
   depend on the candidate: the task's provided checker, its reference
   program, or an oracle written once from the task's stated definition.
   That's the thesis's contract. Microluna's version failed because its
   check was Luna's own guess about the output, written alongside the
   candidate, and it checked shape instead of substance. Fable's oracles
   compute the task's definition by an independent route: brute-force
   Jaccard, the fp32 formula, the reference logits, the reference binary.
   That's the "independently supported" tier from #9629.
2. **The loop around the generative step is mechanical.** Build, run the
   oracle, localize the first failure, fix, rerun; then measure the target,
   try an improvement, keep it only if the oracle still passes and the
   metric improves, otherwise restore. Code can own every part of that loop.
   Luna writes the implementation, the oracle once, and each candidate fix.
3. **The generative middle is still large.** Fable spent 10 to 25 steps on
   most tasks writing a hypothesis, running it, and revising. Components can
   run, rank, and localize those hypotheses, but nothing here writes them.
   That's where Luna's capability decides the outcome, and it's what the
   capability-gap log records.
4. **Two of the failing Fable runs broke a rule code can enforce.** One
   finished while the provided checker still failed; one finished without
   measuring its resource budget. `control.finish` should refuse both.

## The build list

In order, each a component with a trigger from the workspace's structure
and the instruction, an output that is evidence or an executed check, and
admission only on tasks it wasn't learned from:

1. **`checks.oracle`.** Code finds a provided checker, example, or reference
   program. When there's none, Jev extracts the task's stated definition,
   its parameter values, and its boundary inputs, and a separate Luna session
   writes an oracle from that definition alone, without seeing the
   implementation. Code runs it on every stated parameter value and records
   it in the "independently supported" authority tier. Checks that the
   oracle isn't trivially passing are part of it.
2. **`checks.metric_target` and `control.optimize`.** Code measures the
   stated numeric goal with a fixed harness and alternated repeats, and runs
   bounded improvement rounds that keep a change only when the oracle still
   passes and the metric improves, restoring the last passing snapshot
   otherwise.
3. **Failure localization.** `evidence.error_context` (the source lines an
   error names), `evidence.mismatch_trace` (the first value that fails the
   oracle, compared stage by stage), and `evidence.phase_timing` (where the
   time goes), each produced by code and handed to the next Luna session as
   evidence. Built and measured offline, and left off:
   [failure localization](2026-09-25-failure-localization.md).
4. **`checks.prohibitions` and `control.cleanup`.** Jev extracts every
   "must not" from the instruction; code checks each one and cleans up
   scratch files before finishing.
5. **`control.finish`, extended.** No finish while a provided checker, the
   oracle, or a stated target fails.
6. **`evidence.environment`, widened** to library names and service
   settings.

Each gets measured the usual way: tier 0 on retained candidates where
possible, then matched mini-task runs, then a pinned family, with the tasks
above that it was learned from excluded from its evidence.
