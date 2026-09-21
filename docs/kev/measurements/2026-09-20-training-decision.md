# Coder-specific training decision

Do not train a Coder adapter from the current evaluation data. The pinned
replacement 4B has measurable headroom, but input coverage, question
contracts, and independent labels need work before a training run can
answer a useful question. The compute budget for this decision is zero;
no training job ran and no new adapter was produced.

The [baseline](2026-09-20-candidate-4b.md) measures 461 open requests under
frozen suite and question digests. The candidate answers 447 and gets 381
correct against the retained labels. Fourteen refusals and 66 answered
errors make 80 unsuccessful requests. Locked cases remain unscored.

## Error review

[`error-review.json`](data/training-decision/error-review.json) assigns each
of the 80 requests one primary investigation category. This is one reviewer's
triage, not independent adjudication, causal proof, or permission to change
the labels. Secondary causes can overlap. Counts preserve refusals and the
original scores.

| Primary investigation | Requests | Next step before training |
| --- | ---: | --- |
| Oversized state | 14 | Measure an explicit state budget and coverage policy. |
| Question or option contract | 17 | Reconcile what the question asks with what the label measures. |
| Label uncertainty | 2 | Retain disagreements and obtain an independent ruling. |
| Potential model error | 47 | Obtain independent task labels and a separate confirmation set. |
| Missing state | 0 identified | Do not infer missing evidence from an incorrect answer alone. |
| Exact logic or arithmetic | 0 primary errors | Keep directly observable checks in Rust; this suite does not test day arithmetic. |

All oversized cases are `action` requests rejected at 4,096 packed tokens.
Fine-tuning cannot make a refused request enter the model. Increasing the
bound without measuring memory and concurrency would not settle this issue.

The 17 contract cases are two action errors, five shell errors, and ten
program errors. The action `respond` criterion requires clear intent and no code needed
yet, while these observed-response labels include a broad repository-work
request and a terse request to correct a stale issue.
The nine missed `answer-question` requests expose the distinction between
explicit delegation and ordinary answering. The remaining program case
confuses a supplied list of ad hoc tasks with a stored burn-down list.
These are reasons to define and validate the intended boundary before
training, not claims that the model's selections are correct.

All five shell errors select `pass` where the retained label is `retry`.
A failed command can still return useful information or partially complete
a task. The question about progress and the label derived from command
outcome do not necessarily describe the same boundary. Exit status and
missing output are directly available to Rust; interpreting whether to
recover or continue still needs an explicit application policy. A model
should not learn to conceal failed commands behind a favorable judgment.

The uncertain action example is a one-word user reply, `you`, labeled from
what the previous agent did. The support example, `routing/067`, already
has a retained billing-versus-sales disagreement. The candidate selects
technical, agreeing with neither reader; the dispute does not excuse that
error. The other two retained support disputes (`urgency/043` and
`urgency/058`) are not candidate errors against the stored labels.

The 47 residual cases comprise five program errors, 25 support errors, and
17 external-label errors. They are plausible model weaknesses, but this
review does not establish that a Coder-specific training mixture would
fix them without reducing transfer. The external examples are regression
controls, not a proposed training set.

## Why current headroom is insufficient

Shell accuracy is 39/44, but its five false passes matter differently from
Jev's three false passes and seven false retries. The paired interval does
not separate the candidate from Jev, and these rounds come from a few
correlated sessions. The program suite has only one real positive open
turn; authored positives have a single author's labels. The candidate
fails the predeclared program rule and makes five confident program errors.
Those observations do not supply an independent training and confirmation
corpus.

The support and external controls show both gains and regressions. Upstream's
reported learning-rate and data-mixture results motivate preserving transfer
checks; they do not establish a recipe for this application's labels.
No new generalization claim or calibrated probability map is made here.

## Conditions for a later experiment

First define the action, shell recovery, and program boundaries in terms
an independent reader can label. Freeze any revised question set under a
new digest, then collect independent cases with enough positive programs
and failed-command recoveries to measure the consequential errors. Split
by conversation, source, and authored template so near duplicates do not
cross partitions. Keep existing development and locked examples out of
training, preserve provenance and licenses, and retain disagreements.
Jev outputs can be a comparison, not unquestioned ground truth.

Only then propose a bounded experiment with a separately authorized compute
budget, fixed seeds, immutable recipe/code/data identities, and rejection
rules declared before the first run. Those rules must price false shell
passes and spurious program execution separately, require complete input
coverage, and preserve external transfer, calibration, and deployment
latency. Fit calibration only on its designated partition. Record every
trial, including regressions, and admit only the families supported by
independent held-out evidence.

A future paid experiment requires its own authorization. The present
no-training decision completes
[#9461](https://github.com/OpenAgentsInc/openagents/issues/9461) without
spending a budget or treating the current open examples as fresh holdout.
