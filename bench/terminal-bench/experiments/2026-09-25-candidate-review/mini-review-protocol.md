# Exercise reproduced review on the existing mini-tasks

After fixtures and retained replay, exercise the existing four mini-task families
before another benchmark cohort. Use the checked-in scripted good and bad
candidates for `log-severity`, `interactive-terminal`, `cancel-cleanup`, and
`git-recovery`: eight candidates total. These are development controls with known
labels, not held-out benchmark evidence and not new agent successes.

The ordinary mini-task runner creates and grades each candidate with Jev off and
no model generation. Give the reproduced-defect reviewer only the public task and
the complete, unchanged candidate in a read-only `/app` mount. Keep variant names,
the scripted executor, the hidden mini-task grader, and all labels out of the
model input. Use a pinned Python image with bash, git, and coreutils; no network,
GPU, host credentials, or Docker socket inside the review container. Keep all
existing memory, process, time, command-count, and spend limits.

Freeze the unchanged original reviewer and 0.8 semantic cutoff. Retain the strict
result, then optionally the already defined citation recovery as a separately
named result. Never turn an absence of findings into a pass. Record correct and
incorrect failure calls, failure recall, unknowns, per-family separation, cost,
time, and every request, response, command, and candidate identity. Hash the
candidate before and after review. Do not tune on a claimed held-out result here.

## Second development pass after the original controls

The original strict validator rejected every mini-task finding because the
reviewer added quotation marks or prose to citation fields. Preserve those eight
results and the separately rejudged `quoted-passages-v2` results. A new opt-in
`literal-v2` request adds field descriptions that require one contiguous exact
quotation without wrappers, commentary, or ellipses. It does not loosen the
validator, change the semantic questions, or change the 0.8 cutoff.

The review also reproduced a real defect in the scripted `cancel-cleanup` good
candidate: after one interrupt, faster cleanup finishing triggers another cancel
that interrupts slower cleanup. The mini-grader used equal cleanup delays and
missed this. Issue #9641 fixes the fixture and strengthens that grader. Preserve
the old passing label and report the discrepancy; do not revise it in place.

Run all eight mini controls again with the new prompt and the corrected fixture
and grader. Keep this second pass in a new directory with its exact binary and
request identities. It is another development exercise. It is not an independent
confirmation sample, a TB4 pass, or grounds to close #9584.
