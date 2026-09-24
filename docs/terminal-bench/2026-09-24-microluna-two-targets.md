# Microluna: preserve good work, then measure two kinds of win

Status: implementation and measurement in progress, September 24, 2026.
Issues [#9606](https://github.com/OpenAgentsInc/openagents/issues/9606),
[#9607](https://github.com/OpenAgentsInc/openagents/issues/9607), and
[#9608](https://github.com/OpenAgentsInc/openagents/issues/9608).

The next useful result is a repeated verifier pass, with all of the cost
counted. There are two distinct targets: a task whose retained Fable attempts
all fail, and a task Fable solves where Microluna delivers accepted work more
cheaply or quickly. Neither is established by an internal green score.

My chosen first change is to preserve candidates and make the final review
observational. The traces already show the agent finding correct code and
the harness losing it. This is a concrete loss to remove before adding
another acceptance writer or more parallel candidates. It will not supply
reasoning that Luna lacks or make a weak test complete.

## What episode 288 actually supports

The retained [transcript](../transcripts/288.md) describes the intended
system: code owns execution and stopping, Jev supplies narrow judgments,
Luna generates code, and the Gym exposes the evidence. It explicitly asks
for both kinds of win, accepts slower work if it is much cheaper at the same
quality, and rejects injecting benchmark findings into runtime instructions.
It also observes the cost of a ten-minute suite-writing phase and asks for
parallelism only where it shortens the measured critical path.

The transcript is machine generated. Its claims about an ongoing run need
the retained results, rather than transcription alone, to establish them.

| Commit | Change | Meaning for this assessment |
| --- | --- | --- |
| `dd5ffdc11e` | Added the incomplete transcript | Original recording, not a finished benchmark report |
| `974e096115`, `4250a72472` | Added later recording text | Established the Gym inspection and learning goals |
| `7015f73c77` | Added clips d and e | Recorded the Luna pivot and both benchmark targets |
| `d3a8c6a495` | Added clip f | Recorded the v6 runner failure, incomplete checks, and requested v7 improvements |
| `f7445b83f2` | Added clip g | Announced the first v7 pass and claimed the thesis worked as designed |
| `cd421c5dcb` | Removed clip g | The operator cut the announcement after trace analysis showed an in-sample result that did not reproduce |

The removed text said that v7 passed all 11 verifier tests, cost about three
cents in eight minutes, was roughly 27 times cheaper than Fable low, and
confirmed that the suite writers found the deciding fact and the loop ended
honestly green. The verifier pass was real. The causal account and implied
reliability were wrong. The exact removed text remains in
[the removal commit](https://github.com/OpenAgentsInc/openagents/commit/cd421c5dcbeb1fcdb91ef274d51be36ae6c20f6a).
This assessment does not restore the cut recording or erase the pass.

The [v7 trace reconstruction](2026-09-24-microluna-v7-embedding-definitive.md)
and [v6–v8 report](2026-09-24-microluna-v6-v8-report.md) establish the sequence:

1. The first v7 edit session reached an 11/11 workspace at 3:04.
2. A generated guard encoded the original defect. Making it pass restored
   the wrong implementation.
3. A later repair, triggered by a check-budget artifact, restored the fix.
   The frozen suite was not rerun on that final workspace.
4. A repeat of the same build failed. v8 also reached a passing workspace
   before an editing audit lost it.

The all-in v7 cost was about $0.036; Harbor's smaller figure omitted a
writer. The speed comparison to Fable's all-effort average also obscured the
much stronger low-effort reference: five passes in five, about $0.87 and
3.1 minutes. A selected in-sample pass cannot establish cost per reliable
completion, and reducing cost per failed attempt does not close that gap.

There is a separate false-positive problem in the later arc: the literal
scan flagged a legitimate word list as memorized answers. v11 changed that
scan to whole input-and-answer records. That correction is distinct from
the withdrawn v7 success interpretation.

## What is missing now

The [v9-and-later report](2026-09-24-microluna-iterations.md) documents 0/3
dev passes for solo and v9. v10's embedding attempt reached 10/11; its
self-score was already full. v11 bounded commands and corrected the word-list
scan, but the published sound-change and cipher attempts still failed.
v12 added worked-example and standard-form practices and removed the
search-program practice that hurt the sound-change attempt.

The active Claude work on coderos owns that v12 iteration and issue #9597's
issue-to-PR retries. This work uses its own worktree, target directories,
policy name, and experiment ID. It does not stop those trials or modify
their checkout. The comparison baseline advances from the initially proposed
v11 to v12 **before any trial in this experiment**, so the two arms share the
same current guidance. Historical v11 results retain their identities.

The other agent subsequently added v13's Jev-ranked source-comment
suspects. That change is merged alongside this implementation, but it does
not change either experimental arm or the pinned binary. Comparing that
generation change with candidate preservation needs its own experiment.

Three problems should remain separate:

- **Generation:** a candidate never implements a required rule. More
  trustworthy selection cannot recover a candidate that does not exist.
- **Evaluation:** a self-test passes on both a correct and an incorrect
  candidate, or asserts a wrong expected value. Writing it first, freezing
  it, or getting a confident judgment does not establish validity.
- **Selection:** correct work exists but later editing or tie-breaking
  discards it. Candidate retention and independent post-run grading expose
  this loss directly.

Code should validate observable facts: command exit, timeout, complete
output, well-formed counts, stable evaluator contents, and retained candidate
identity. Jev can judge whether cited evidence supports a narrow claim, but
that judgment needs calibration before it becomes authority to reverse code.
This follows the distinction in TypeSafe's
[citation-checking example](https://docs.typesafe.ai/cookbooks/citation_check):
finding a quoted span and judging what it supports are separate operations.

## The implementation

The experimental `microluna-evidence-v1` policy derives from v12. It keeps
the same Luna model, effort, work-session count, command limits, and spend
bound. Its changes are explicit:

- Every score must come from successful execution with complete output and
  valid counts. Missing scores, malformed counts, a changed denominator,
  timeouts, and failed commands leave evidence unknown. Retries remain
  bounded; an unknown runner result is not an instruction to change code.
- The first evaluator is frozen and its file digests are checked around
  execution. A later change invalidates its evidence.
- Candidate snapshots, evaluator files, selection records, and submitted
  file digests remain in the run's artifacts. A workspace above the existing
  snapshot bound is explicitly unavailable; it is not silently treated as a
  retained candidate.
- Equal scores preserve the earlier candidate. A tie is not evidence that
  a later rewrite is better. This rule is conservative and can also reject
  an improvement that the score misses; the experiment must count both.
- The selected candidate receives a fresh review with file reads and
  `finish` only. Shell commands and edits are refused in code, including
  inside task containers. The reviewer sees host-recorded test results and
  can report gaps; it cannot repair them or overwrite the submission.
- The host reruns the evaluator on the submission. The record distinguishes
  local checks, the review's own status, and the external benchmark outcome.
  Local green remains a limited measurement, not a benchmark pass.

Common score-validation bug fixes apply to both arms in the matched build.
The treatment comparison isolates retained-candidate selection and the
observational review as a package; it does not isolate each component's
individual effect.

The issue-to-PR fix is separate: carry the review's failure and unfinished
state into publication and the returned report. An execution that answered
does not override a later review that failed. It helps Coder implement
future iterations honestly, but contributes no claimed TB4 accuracy gain.

## Two targets and the experiment

| Target | Primary task | Retained comparison | Reason to start here |
| --- | --- | --- | --- |
| Pass where Fable failed | `session-window-debug` | Fable 0/25, all five effort settings; earlier Opus-based Coder passed | Small local debugging problem; tests are inexpensive and intermediate workspaces can be retained |
| Accepted work at lower cost or time | `embedding-drift-monitor` | Fable 25/25; low 5/5 at about $0.87 and 3.1 minutes; cheapest public pass about $0.74 | Correct intermediate Microluna work already existed; directly tests whether preserving it helps |

These are selected development tasks. The task-anatomy report was read for
selection and diagnosis, so they are not unseen tests. Neither its decisive
facts nor public solutions enter the runtime briefing. A lexical contamination
check cannot prove independence from prior research; the report must state
the provenance as well.

The first matched experiment uses three attempts per arm on each task, with
arms interleaved, for 12 planned attempts. Both use the same clean binary,
task revision, host, model, effort, tools, and outer limits. The planned total
model-spend ceiling is $3, with at most two concurrent trials and existing
disk, memory, credential, and network admission checks. The policy bounds
individual runs; the experiment stops admitting new trials if its total
ceiling or an integrity condition is reached. No benchmark result changes
a frozen arm mid-run.

For each attempt retain the official result, full traces and briefings,
per-component usage, phase times, local score and review, candidate identity,
and selection reason. Grade intermediate candidates only after execution;
never feed those hidden results back into the agent. Report every attempt,
including failed or invalid attempts and their costs. Compare oracle headroom
(any retained candidate passes) with the selected candidate's result.

A lane earns a preliminary repeatability claim only with at least two fresh
passes in its three treatment attempts. That is still a small selected
sample. Report total cost divided by accepted outputs, and leave the ratio
undefined when there are no passes. Show both agent time and full trial time;
public Fable timing and host differences limit a speed claim. Treat all price
figures as usage valuations, not measured subscription invoices.

If both lanes pass that bar, pin one additional task per lane before
inspecting its new outcomes and run the unchanged policy. Otherwise use the
recorded generation, evaluation, and selection losses to choose one next
general change. An incomplete result keeps #9607 open. It does not justify
expanding to the full suite.

## How the existing issues fit

- **#9584 and #9588:** still the evaluation research. Use retained candidates
  to measure false acceptance, false rejection, and requirement coverage.
  Do not restore generated-suite authority before it discriminates.
- **#9587:** best-of-N is useful when candidate selection works. The earlier
  passing candidate that cost-based selection discarded is evidence to
  evaluate the selector, not a reason to buy more candidates immediately.
- **#9585:** the executor exists. Reconcile the original delivery/comparison
  checklist separately from ongoing algorithm efficacy; stale pending rows
  should not masquerade as current queue state.
- **#9597:** active work on coderos, so this task does not duplicate its
  issue-to-PR retries. #9608 supplies the review-outcome correction.
- **#9594 and #9592:** useful inspection features, but neither blocks this
  candidate experiment or supplies evidence of higher completion rates.
- **#9577:** needs real human labels. Agent-generated labels must not be
  presented as the operator's judgments.
- **#9598 and #9558:** keep the archive roadmap parked and the full-suite
  run paused until the targeted evidence justifies them.

The next claim should name the task, pinned policy, repeated verifier results,
and total cost. It should not repeat clip g's leap from one favorable result
to a causal success story.
