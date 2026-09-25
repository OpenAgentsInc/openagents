# Literal artifact confirmation protocol

Status: amended before candidate generation for [#9648](https://github.com/OpenAgentsInc/openagents/issues/9648). The
[machine-readable protocol](protocol.json) binds the task list, public image
preflight, executable digests, orchestration sources, and decision rule. This is
another prospective study after earlier negative studies; it does not erase
them or make repeated experimentation a single untouched hypothesis test.

## Population and execution

Use all 15 groups in [selection.txt](selection.txt), at upstream Terminal-Bench
commit `3b5caaa4863d64dda7f0957bf4fc2d4f019202d4`. The
[selection rationale](selection.md) records the public-only selection and
exclusions. All 15 exact public Dockerfiles built successfully. Initial `/app`
size ranges from 4 KiB to 87,052 KiB, below the unchanged 128 MiB restoration
bound; no selected task was excluded after preflight. Public tool and input
inventories, images, build logs, and hashes are retained. No official grader,
solution, candidate, or outcome informed this selection or rule.

Generate **90 candidates: three Luna and three Astra attempts per task**. Keep
the same previously frozen v13 executor artifact and both original prospective
policies. The executor SHA-256 is
`7df7cde47d0f16c8f9200cbd21899254588c0b587f3bbe6f2272fd2eedfb0f5d`.
Record final scenario checks and the original report-based combined verdict;
enable no new repair, escalation, selection, or stopping policy. Preserve each
task's original time and resource allowances. Pin the prebuilt image identity
as the only task-file change, and hash every staged file against upstream.

Run at most two trials per executor concurrently, four total. Use the isolated
truth-confirmation worktree and target directory. Do not update the previous
cohort's frozen checkout or collide with the other agent's family run. The new
jobs end in `9584-literal-artifact-v2`; no existing job can be reused or replaced.
Check for at least 20 GiB of free disk before launch.

Retain every scheduled attempt, including setup, transport, snapshot, and grading
failures. No failing attempt is replaced or omitted. No retry, regrade, or early
stopping based on outcomes is part of this protocol. Unknown official grades
remain unknown and are reported separately from candidate precision and recall.
Do not count infrastructure refusals as agent failures. If an infrastructure
problem prevents execution, preserve it and report the incomplete study.

## Frozen rule

The metadata checker comes from Rust source `24e7864537`, SHA-256
`99f21894b2ed16343545d1eb4a0e76f8ddbcd9af6ff8caab2d8ab0654a4eb9aa`.
The reproduced reviewer remains the original `ca3c91b581` artifact, SHA-256
`68c86604d69910b36bcdd6246524530f2024f607e9fb003197bb06ff011edd53`.
The four checker libraries have individually pinned digests in `protocol.json`.

1. Before generation, extract the unchanged original public-file plan and the
   new literal artifact plan from every original public instruction. The old
   plan includes only instruction-derived normalized `/app` path/format items,
   without commands or `not_executable` reasons. The new plan includes only its
   supported required paths and byte ceilings. Neither uses Jev. Retain empty
   plans and extraction limitations rather than adapting the grammar to a task.
2. After the sole executor finishes, require a complete attributable snapshot
   matching its collected final artifacts. Run both plans in the pinned public
   image with a read-only candidate and root, no network, dropped capabilities,
   and the existing memory/process bounds. A demonstrated difference produces
   a cheap failure. Unreadable evidence and setup errors remain unknown; a match
   never certifies completion.
3. If neither cheap arm establishes failure, use the original Astra high
   reproduced-defect reviewer: `literal-v2`, exact contiguous citations, at most
   seven commands, 30 seconds per command, 300 seconds per review, three findings,
   and the unchanged two Jev questions with `min(reproduced, required) >= 0.8`.
   Use the independently tested `owner-exec` profile: the restored candidate's
   UID/GID and executable 128 MiB scratch. Keep the candidate and root read-only,
   network disabled, 2 GiB memory, two CPUs, and 128 processes. This is a declared
   change from the previous cohort's original review profile.
4. Return **fail** if an original file check differs, a literal artifact differs,
   or an eligible reproduced review calls fail. Otherwise return **unknown**.
   Skip the costly review once a cheap failure decides the OR, retaining the
   reason. Do not infer a standalone negative reviewer prediction from a skip.

No new question, threshold, generated expected constant, source-only review,
report fit, citation recovery, or post-label rejudgment contributes to the rule.
Outside-snapshot evidence is permitted only when byte-identical to the same
paths in the immutable original public image. The review environment does not
recreate packages installed during the attempt. Missing runtime support,
permission limitations, and resource limits alone cannot prove a task defect.
The task's declared necessary conditions and reproduced evidence still do not
cover every hidden grading requirement.

## Seal and evaluate

Do not read official outcomes until all 90 predictions are sealed in a commit on
`origin/main`. It is permitted to check completed attempts while later attempts
run; only the existence of `result.json` establishes completion at this stage,
not its contents. Each completed review record is immutable on rerun.

The seal must include every declared `(task, executor, attempt)` and bind baseline
calls, component calls, candidate digests, plans, all check records, and trial
configuration. The join verifies the published seal and all retained evidence
before opening even the first outcome. Unknown reviews stay in real-failure
recall whenever an official failed grade exists. Refuse duplicates, missing
attempts, wrong-task joins, altered candidates, or unsupported failure calls.

The **primary comparator is the same candidates' `checks.final`**. Also report
the original `verdict.combined`, the cheap arms, and the incremental review
detections. Do not switch the primary comparator if another comparison is more
favorable. Report:

- Failure precision and failure recall with exact counts and Wilson 95% intervals.
- Paired task-cluster bootstrap differences, 10,000 resamples with seed 9584,
  including undefined resamples rather than assigning them zero.
- Per-executor and per-task results, unknown coverage, every false alarm, and
  the concrete evidence behind each detected or missed failure.
- Within-task pass/fail concordance, both pooled and per executor, with equal-task
  means. Candidate pairs are dependent descriptions, not independent trials.
- Full executor/reviewer/Jev usage, unknown prices, all failed-attempt costs,
  trial time, agent time, review time, and cost per official pass.

For #9584's completion claim, both primary precision and recall point estimates
must improve, and uncertainty must support an honest improvement claim. A tie,
undefined primary precision, or negative/inconclusive comparison does not close
the issue. No broad transfer claim follows from 15 selected archived CPU tasks.
This is not TB4, a Fable result, a same-model Coder ablation, or a runtime-policy
promotion. The three previously audited circuit label disagreements remain
separate evidence; no official labels in either cohort are revised here.

## Cost and completed prerequisites

The unchanged soft native limits are $0.09 per Luna attempt and $3 per Astra
attempt, $139.05 over all 90 candidates. Ninety reviews at their $2 soft limit
would add at most $180 before in-flight overshoot; most earlier reviews spent
much less. Jev is counted separately. These are planning limits, not invoices or
hard bounds. Report actual list-price usage and unknown pricing honestly.

The literal component passed nine Rust controls, five live CLI controls, all five
scoped Coder One gate phases, and two complete 72-candidate development replays.
The original 12 contract plans remain byte-identical. The owner-exec profile has
retained positive execution/ownership controls and negative write controls.
Twenty existing Python regressions and three new sealing/population controls
pass on coderos before this freeze. See the
[development report](../../../../docs/terminal-bench/2026-09-25-literal-artifact-checks.md).

This follows the assessment's ladder: controlled fixtures, retained replay, then
new task groups. It measures the signal before giving it authority over a live
Coder loop. Every negative study and unavailable result remains part of the
published evidence.

## Pre-generation amendment for artifact lifecycles

The first freeze at `c36d0e4489` is superseded before generation. Its exact
[machine-readable protocol](protocol-artifact-v1.json), original preparation,
plans, and stopped-launcher record remain retained. Zero `artifact-v1` jobs or
candidates were created. Synthetic controls exposed requirements for temporary
files that should be deleted and for files that a requested program would only
create later. These are extraction errors, independent of any reserved task
outcome.

The corrected checker abstains on those contexts and retires earlier output
obligations after cleanup or relocation. It preserves separate final outputs
when cleanup concerns an unrelated path. See the
[correction, controls, replay, and evidence](../../../../docs/terminal-bench/2026-09-25-literal-lifecycle.md).

Only the metadata checker artifact, protocol identity, and fresh job suffix
change. The 15 tasks, 90 attempts, executor policies and artifact, reviewer and
thresholds, review profile, budgets, orchestration hashes, and primary evaluation
criterion remain fixed. The amended protocol and regenerated public plans must
be published before launch. Wait for the separately owned v18 family run to
finish, then use a new preparation directory; never reuse the superseded plans.


The amended public plans are now retained before launch. The unchanged original
file arm yields zero items on all 15 groups. The literal arm retains one
obligation each for `extract-elf`, `large-scale-text-editing`, and
`schemelike-metacircular-eval`, and abstains on the other 12 groups. Conservative
lifecycle handling removes the earlier obligations for `filter-js-from-html`
and `regex-chess`; their public instructions describe removal/replacement
behavior, which this small grammar does not safely distinguish from cleanup.
This is lost coverage, not evidence that those outputs are optional. Do not
retune the grammar to recover these selected tasks. The remaining rule can
still use the unchanged reproduced reviewer.

Both contamination receipts are clean. All original and amended plans,
configuration, task-file provenance, zero-candidate inventory, and the stopped
original launcher are retained in the [preparation bundle](records/literal-preparation.tar.gz),
SHA-256 `b6b00bdc98db2a8c3736f016709d034289752cd113148a93439485a893a82935`.
Its [manifest](records/literal-preparation-files.json) binds 260 credential-scanned,
separately restored, hash-verified files. Public environment inputs remain in the
previously retained preflight bundle.
