# Prospective model and briefing experiment

This is a plan, not an experiment result. It tests whether a deterministic
brief lets a lower-cost executor complete an issue reliably, and separates
that effect from changing the executor alone. Finish the registered round-2
panel before running these scored tasks. Do not pool its Opus results with
this experiment.

## Four conditions

| Arm | Executor | Optional prepared evidence |
| --- | --- | --- |
| A | `claude-opus-5-5` | None |
| B | `claude-opus-5-5` | Frozen deterministic brief |
| C | `claude-sonnet-5-5` | None |
| D | `claude-sonnet-5-5` | The same frozen brief as B |

Choose Sonnet before observing task outcomes. Haiku is outside this panel.
Local availability probes with CLI 2.1.287 returned the exact model IDs above
for Sonnet and `claude-haiku-4-5-20251001` for Haiku. Each probe requested
medium effort and returned a successful terminal result without tool calls.
Their combined CLI list-price estimate was $0.017074. These tiny probes
establish availability; they do not compare coding quality, effective effort,
or task cost. Sonnet returned exactly `READY`; Haiku did not return that exact
string. No scored source, task, brief, or checker was supplied to either probe.
The selected usage counters are retained in [availability.json](availability.json).

Use medium effort in every arm. This fixes the requested effort label; it does
not establish equal internal computation across models. Pin full model IDs,
check initialization and final served-model accounting, and disable fallback.
Record any mismatch and its cost. Do not silently replace a model mid-panel.
Pin Claude CLI to `2.1.287` in the registration. Require that exact value in
the session initialization's `claude_code_version` for every scored run and
shared warmup. A missing or different version blocks the comparison; retain
the run, candidate checks, and paid cost. The coordinator stops after saving
a scored run with a version mismatch.

## Independent task and frozen inputs

An independent evaluator selects one clean historical reserve task with a
pre-fix snapshot, recoverable original requirements, and independently
calibrated acceptance checks. Keep its identity, source, original fix, and
checker unavailable to the packer implementer until the new packer is frozen.
Publish a commitment before scored calls; reveal the task and evidence with
the results. A task already used to tune this packer is development data.

The packer may use known development tasks before freezing. On the reserve,
run it once by its frozen rules. Both briefing arms receive those exact bytes;
both controls receive the identical common task without the optional suffix.
Do not revise the brief or checker after an arm runs. Keep original solutions,
other-arm candidates, and checker source outside executor roots.

Freeze the task, complete common instructions, brief, source and checker,
packer, runner copy, instruction guard, coordinator, model IDs, CLI version,
preparation measurement, thresholds, and order. Required instructions and
experimental overrides are byte-identical across all four arms. The existing
instruction guard applies before execution and around each verification.

## Four balanced blocks

Run every arm once in each block, on fresh exports of the same reserve task:

1. A, B, D, C
2. B, C, A, D
3. C, D, B, A
4. D, A, C, B

This Williams order places each arm once in every position and balances each
immediate predecessor once. It does not eliminate provider cache or load
variation. Four repeated blocks on one task support a narrow engineering
result; they do not establish transfer across tasks or statistical superiority.

Each arm gets the same five file tools, 600-second agent-time limit, $10 CLI
list-price estimate cap, deterministic formatting service, external checks,
and at most one repair turn in its original persistent CLI process. Stop
successful first attempts. Preserve failed candidates, raw and normalized
patches, check logs, charges, and incomplete accounting. Do not rerun ordinary
model or candidate failures. A confirmed infrastructure defect requires a
recorded reason and a new whole-block registration before any replacement;
keep the original block visible.

The scored cap is 16 runs, at most $160 in CLI estimates. Use one bounded
instruction-prefix warmup per model for the task and record those two costs
separately. Both models get the same warmup policy. Cache state remains
uncontrolled; retain creation/read counters and served-model usage. Record
cold indexing and engineering/machine costs separately. The coordinator
performs no warmup implicitly.

## Predeclared comparisons

The primary comparison is D versus A: the combined change against a fresh
Opus control. A clear cost win on this panel requires all of the following:

- A and D each accept 4/4 runs under the same checks and repair rule.
- D's median total cost is at least 20% lower than A's.
- D is cheaper in at least 3/4 corresponding blocks.
- D's median recorded endpoint wall time is at most 1.10 times A's.
- Source, instructions, model binding, and cost accounting remain valid.

Use each run's last cumulative CLI list-price estimate once, including
repairs. Add any paid briefing cost to briefing arms. Add measured warm
briefing preparation to their recorded endpoint wall time, along with source
export, instruction checks, prompt setup, the model session, external checks,
and executor-process shutdown. The formula remains `wall_s +
preparation_wall_s + warm_brief_wall_s`, with warm briefing preparation added
only for B and D. The frozen runner records `wall_s` before final
candidate/artifact capture, scratch-workspace deletion, and final result
serialization. Those later steps are excluded; this metric does not measure
the entire harness elapsed time. Shared warmups use the same timing boundary.
This clarification changes neither the formula nor the gate. Do not call a
failed run a cost through acceptance. These estimates are not verified
subscription charges.

Apply the same cost-win rule separately to C versus A to measure routing,
and D versus C to measure the brief's contribution on Sonnet. Report B versus
A as the brief's contribution on Opus, and D versus B as routing with a brief.
Report acceptance and first-attempt acceptance for every arm, including cases
where the cost-win rule cannot apply because acceptance differs.

A D-versus-A win alone supports the combined policy. If C already wins against
A and D does not improve on C, attribute the demonstrated saving to model
selection. Do not call it a briefing benefit. If C fails while D accepts,
report the acceptance difference with both fixed-endpoint costs; the primary
cost gate can still establish a combined-policy win, but do not invent a
post hoc correctness threshold or claim a statistically established
interaction. Report the paired changes D/C and B/A to examine whether the
brief has different effects across models.

Finish the registered 16 runs without changing the treatment or increasing
the sample until a threshold passes. If the primary gate fails, report that
result. A revised hypothesis requires a new registration and an independent
reserve. Keep previous rounds and cumulative experiment costs visible.

## Minimal coordinator

`bench/briefing-replay/run_factorial.py` copies the hash-bound round-2 runner
and guard into a new study directory. It loads that copy with one registered
full model ID for each arm; medium effort and all other runner behavior stay
unchanged. It checks that round 2 has all 16 final records before execution,
validates input hashes before every arm, refuses existing run directories,
and records the requested and final served models separately. The plan and
input paths remain private until the evaluator releases the reserve.

The runner copy still grants only the existing file tools. The coordinator
runs no model by default; `--execute` is an explicit launch. No scored task
has been run for this prospective panel.
