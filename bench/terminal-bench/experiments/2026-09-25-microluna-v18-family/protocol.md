# Microluna v18 on the Luna-sized family: pre-registration

Status: pre-registered on 2026-09-25, not run. No Terminal-Bench trial,
Luna session, or Jev call was made to write it. It belongs to issue
[#9640](https://github.com/OpenAgentsInc/openagents/issues/9640), whose
rules call for the Luna-sized family to be rerun "under a new
pre-registration with the environment fix (`38092f57e8`), v18 pinned by
digest, and the win threshold fixed in advance."

This document reuses the
[original pre-registration](../2026-09-25-luna-sized-family/protocol.md)
for everything it doesn't change: the family, the split, the Fable 5.1
figures, the counting rules, and the outcome table. Its
[`tasks.json`](../2026-09-25-luna-sized-family/tasks.json) stays the
machine-readable source for the tasks' scores, resources, split digests,
and Fable figures. This document changes three things: the policy
(`microluna-v18` in place of `microluna-v15`), the harness (with the
environment fix), and a stated exposure of two family tasks.

## Summary

- **Family and split, unchanged.** Development:
  `mp-checkpoint-consolidation`, `live-database-cutover`, and
  `photonic-waveguide-routing`. Confirmation: `payments-pipeline-fix`,
  `cumulative-layout-shift`, and `telecom-entity-resolution`.
- **Hypothesis.** The pinned `microluna-v18` passes at least 2 of 3
  attempts on at least 2 of the 3 confirmation tasks, each at an all-in
  cost per pass under 10% of Fable 5.1 low's cost per pass on that task.
  Trial time per pass is reported beside Fable low's, but isn't part of
  the win.
- **Size.** 3 attempts on each of the 6 tasks: 18 counted trials, 9 on
  confirmation tasks. Spend ceiling: $3.00 of Luna and Jev at list price,
  reruns included.
- **The win threshold is the original's, unchanged.** #9640 doesn't
  change it.

## What changed since the first run

The first run of this family (2026-09-24,
[results](../2026-09-25-luna-sized-family/results.md)) was invalid: under
the trial network allowlist, Harbor 0.22.0 refused to start the three
multi-service tasks, so 4 trials couldn't be graded and the run stopped
after 3 graded trials, none passing. Its verdict was inconclusive, and it
licensed no rerun by itself.

Two changes license this one, and neither was chosen by looking at a
family task's result:

- **The environment fix, `38092f57e8`.** Under the allowlist, only the
  agent's container goes behind the egress sidecar. The task's other
  services keep their own namespaces on the task's networks, made
  internal, so the agent reaches them by name and they have no route off
  the host. An environment-only bring-up started all three multi-service
  family tasks with no agent or verifier.
- **A new policy, `microluna-v18`**, built from #9640's pieces.

The three graded v15 attempts of the first run aren't pooled with this
run's trials.

## The pinned policy

| Field | Value |
| --- | --- |
| Manifest | [`crates/coder-one/policies/microluna-v18.json`](../../../../crates/coder-one/policies/microluna-v18.json), `coder-one-microluna-v18`, added in `918971c137` |
| Harness arm | `coder-one-microluna-v18` in `bench/terminal-bench/profiles/agents.json` |
| File SHA-256 | `4da78507d84a67bbc18886ba9e6975d1f8b0bd15962dde8901f4186f9b1ba912` |
| Manifest digest | `8d3fbaf3e4fb53ede219cf782402ea658e315d12a00edef8fbd7960e813435a5`, with no overrides |
| Resolved policy digest | `05aac15cefa419cfc3dc2db9225ace213bda351c252a9590d5d50de6b717c445`, under `CODER_ONE_EPISODE_DEADLINE=28680`, the 8-hour agent timeout every family task declares less the adapter's margin. This is the `policy_digest` every counted trial must record. |

The resolved digest was computed with `coder-one episode doctor
--contract openagents.coder.episode.v1` built from `918971c137`. The
same command, with the same deadline, resolves `microluna-v15` to
`d3e1396f7b5a…2af2ea1`, the digest the first run pinned, so the procedure
matches the first run's.

### What v18 runs

v18 is `microluna-v13-retained`, the arm that passed
`embedding-drift-monitor` 3 of 3, with the pieces #9640's offline
measurements admitted. #9640's "Unchanged" rule holds: GPT-6 Luna at
`high` effort, one first session, the v13 bounds (4 lean sessions and a
self-check, 60 turns and 900 seconds a session, a 20-minute loop wall
inside a 25-minute executor deadline, 180 seconds a command, $0.09 of Luna
a trial), a frozen self-score with keep-best, Jev-ranked suspects, a
hard-coding scan, and sequential candidate retention, which records
candidates and changes no decision. There's no acceptance suite with power
over the code, no lanes, and no escalation. `verify.checks`,
`verify.support`, and `verify.close` are off.

| Piece | Issue | In v18 | Why |
| --- | --- | --- | --- |
| `evidence.environment` | [#9632](https://github.com/OpenAgentsInc/openagents/issues/9632) | On: `policy.evidence.environment` | Admitted. The line would have prevented 18 of 20 Microluna tool misses before the first edit (90%, 70–97%). |
| `evidence.baseline` | [#9633](https://github.com/OpenAgentsInc/openagents/issues/9633) | On: `lean.baseline` | Admitted behind its switch. It found a runnable entry point on 6 of 18 anatomy tasks; its only decisive new fact was on `embedding-drift-monitor`. It runs once, bounded to 60 s, before session 1, and has no power over the code. This run is its first test on tasks it wasn't developed on. |
| `evidence.departures` | [#9634](https://github.com/OpenAgentsInc/openagents/issues/9634) | Off | Neither new source was admitted; `departures::ADMITTED` is empty. |
| `accept.grade` | [#9635](https://github.com/OpenAgentsInc/openagents/issues/9635) | Off | Not admitted; code refuses the switch. |
| `verify.executed` | [#9636](https://github.com/OpenAgentsInc/openagents/issues/9636) | Off | It caught 0 of 49 failing candidates (0–7%) and changed 0 of 20 keep-best decisions, at 7 to 47 s a candidate. A guard with no measured benefit adds trial time, which this run reports. |
| `control.review` | [#9637](https://github.com/OpenAgentsInc/openagents/issues/9637) | Off | Not admitted as a way to skip reviews. With `unknown_fires: true` it never skips, but it changes the review's guidance, which no measurement covered. Off keeps v13's review exactly. |
| `control.finish` | [#9638](https://github.com/OpenAgentsInc/openagents/issues/9638) | On, score only: `lean.finish_rule: {"max_refusals": 3, "baseline": false}` | Admitted on the score half: 0 of 31 retained Microluna `done` finishes refused (0–11%), and 8 of 12 against 7 of 12 on matched mini-tasks. The baseline half wasn't admitted: with baselines, the rule would have refused 12 of 16 retained finishes (51–90%), 6 of the 10 on passing trials, with no measured benefit. |
| Run card | [#9639](https://github.com/OpenAgentsInc/openagents/issues/9639) | Not a policy switch | Measurement: every trial gets a `gym runs characterize` card. |

## Exposure of two family tasks

The original protocol asked that no signal calibration use the six family
tasks until the family is graded. One did. The executed contract checks
([#9628](https://github.com/OpenAgentsInc/openagents/issues/9628),
[protocol](../2026-09-25-executed-contract-checks/protocol.md)) put
`telecom-entity-resolution`, a confirmation task, and
`mp-checkpoint-consolidation`, a development task, in their development
half. That protocol allowed the extractor's rules to be written while
reading those tasks' instructions, workspace files, and candidates'
rewards. `evidence.baseline` finds entry points with that extractor
(`extract::draft`, unchanged) plus its own kinds, which were developed on
the task anatomy's tasks, none in this family.

No hidden test, reference solution, or verifier output of a family task
was read for v18. The exposure is to the instruction and the shipped
workspace, through one component. It doesn't change the family or the
split, and it's handled by a sensitivity line fixed now:

- The verdict uses the outcome table below, on all three confirmation
  tasks.
- The report also states whether a win holds on `payments-pipeline-fix`
  and `cumulative-layout-shift` alone, that is, whether both are cheap
  wins. A win that needs `telecom-entity-resolution` is reported as a win
  that depends on an exposed task.

## Launch conditions

- **Harness.** `bench/terminal-bench` from a commit at or after this
  pre-registration's, which includes `38092f57e8`; record the commit, and
  run it unchanged for the whole run. Harbor 0.22.0. TB4 at `v4.0.0`
  (`452bf305c6daa62fc59061d22133a7cbc7c1572e`). Profile `tb4`, arm
  `coder-one-microluna-v18`.
- **Artifact.** `scripts/build-coder-one-linux.sh` from this
  pre-registration's commit. Record its SHA-256 and version string before
  launch; the adapter pins it with `artifact_sha256`.
- **Preflight.** Before launch, the artifact's doctor resolves
  `microluna-v18` to the resolved digest above under
  `CODER_ONE_EPISODE_DEADLINE=28680`. A trial whose recorded
  `policy_digest` differs is void, and so is the run if the policy, the
  artifact, or the harness changes during it.
- **Network and credentials.** The trial allowlist (`openagents.com`,
  `api.typesafe.ai`, and `chatgpt.com`) under `38092f57e8`'s overlay; the
  #9599 login protection; no Claude credential. Microluna reads the Codex
  login and never refreshes it, so before launch, check that the login's
  access token stays valid for the expected wall time plus 2 hours.
- **Resources.** Each trial gets its task's declared CPUs and memory
  (`tasks.json`): the host's Docker must offer at least 16 GiB of memory
  and 16 CPUs for `live-database-cutover`, and 8 GiB for
  `payments-pipeline-fix`, `cumulative-layout-shift`, and
  `telecom-entity-resolution`. A host that can't give a task its declared
  resources can't run this protocol. The first run's host,
  `coderos-4080` (Linux, x86_64), meets this.
- **Disk.** Before each image build, check the Docker volume's free space
  and wait while it's under 20 GiB. After a task's last trial, remove only
  the images this run built for it.

## Procedure

As in the original protocol:

1. **Attempts.** 3 on every family task: 18 counted trials.
2. **Order.** Attempt 1 of all six tasks, then attempt 2, then attempt 3.
   Within a round, launch in rank order as the host's CPU budget allows;
   `live-database-cutover` runs alone.
3. **Stopping rule.** Every counted trial runs. The run stops early only
   when the spend ceiling is reached, when the run is invalid, or when the
   operator stops it for the host's safety; each makes the result
   inconclusive.
4. **Infrastructure failures.** A trial that fails before the agent's
   first model request, or whose verifier doesn't run, is rerun once, and
   both records are kept. It counts as neither a pass nor a fail. The run
   is invalid if more than 3 of the 18 trials can't be graded after their
   rerun. A trial the policy's own bounds end is a counted failure.
5. **Spend ceiling.** $3.00 of recorded Luna and Jev spend at list price,
   reruns included. Check it after every trial, and stop when it's
   passed.

## Counting cost and time

Unchanged from the original protocol. A trial's cost is every Luna
request at list price plus every Jev request, from the trial's records.
When a request was still open at a bound, the trial counts at the larger
of its recorded cost and the $0.09 Luna bound plus its recorded Jev cost.
Cost per pass is the sum over a task's 3 counted attempts divided by its
passes. Trial time is the Harbor trial's start to its end; trial time per
pass is the sum over the 3 counted attempts divided by the passes, beside
Fable low's.

## Outcomes, fixed now

A confirmation task is a **cheap win** when at least 2 of its 3 attempts
pass and its cost per pass is under 10% of Fable 5.1 low's cost per pass:
under $0.625 for `payments-pipeline-fix`, $1.438 for
`cumulative-layout-shift`, and $0.605 for `telecom-entity-resolution`.

| Result | Condition |
| --- | --- |
| **Win** | At least 2 of the 3 confirmation tasks are cheap wins. |
| **Loss** | No confirmation task passes 2 of 3, and at most 1 of the 9 confirmation attempts passes. |
| **Inconclusive** | Anything else: one cheap win, passes spread too thin to reach 2 of 3, or an invalid or stopped run. |

An inconclusive result is reported as it stands and doesn't license
rerunning the confirmation tasks until they pass.

## What the report states

- The verdict against the table above, and the sensitivity line on the
  exposed task.
- Per task: passes of 3, verifier test counts per attempt, cost per pass,
  and trial time per pass, beside Fable 5.1 low's cost per pass and trial
  time per pass from `tasks.json`, each labeled cheaper or dearer and
  faster or slower.
- The pooled confirmation pass rate and, separately, the development pass
  rate, each with a 95% Wilson interval.
- Passes on the three Luna-sized tasks against the other three.
- Each failure's kind: a signal gap when the self-score was full on a
  failing trial, a capability gap when no session produced a passing
  candidate, and the bound that ended a time-bounded trial.
- For every trial, a `gym runs characterize` card. For the v18 pieces:
  whether the baseline found an entry point, how many finishes the rule
  refused and whether any ended `unverified`, and how many commands
  failed as not found before the first edit.
- Oracle headroom from the retained candidates (`tbench candidates`),
  now that v18 retains them.
- Confirmation tasks: outcome fields only (verifier result and test
  counts, cost, time, session outcomes, and the run card's counts). Their
  traces aren't read for lessons.

## What this run doesn't test

The same limits as the original protocol: it isn't a matched-model
comparison with Fable, the family isn't a random sample of TB4, and three
attempts a task give wide intervals. It also doesn't separate the v18
pieces from each other or from v13; a win or a loss is v18's as a whole.
