# Candidate review validation protocol

Frozen before live reviews on 2026-09-25 for #9584.

## Inputs and rule

The review reads only the public instruction, bounded retained final output files,
and a statement of coverage limits. It receives no writer report, verifier data,
reward, task outcome, trial name, or prior verdict. Luna high makes one request
with a native `submit_review` function and at most three counterexamples. Each
must quote the public requirement and candidate file exactly. Code rejects
ungrounded quotes. Jev judges the grounded counterexample's correctness and
whether it violates a stated requirement; its score is the lower probability.
A candidate's score is the maximum valid finding score. No valid finding means
unknown, never pass. Missing files, an interrupted request, malformed output,
and speculative concerns cannot establish failure.

The exact prompt, question text, tool schema, limits, requests, replies, token
usage, elapsed time, and findings are retained. Input JSON rejects unknown fields,
including labels. The Rust component executes no commands or candidate code.
Static counterexamples are not described as executed behavior tests.

## Development and comparison

Use the original 317-trial task-group partition: 132 calibration trials on 26
tasks, 185 comparison trials on 32 different tasks. All rows remain in the
coverage and recall denominator, including unavailable outputs. Candidate files
come from Harbor's final output manifest, excluding logs; binary data and overly
large files are explicitly unavailable. Retain up to 160 KB across at most 100
files, ordered by path, and state every omission. Do not substitute the initial
workspace for an unretained final output.

Run the calibration partition first. Choose among thresholds 0.5, 0.7, 0.8, 0.9,
and 0.95: maximize true failure detections while requiring at least five failure
calls and at least 90% observed failure precision. Break ties with the higher
threshold. If none qualifies, retain the negative result and revise only on
calibration data before freezing a new rule. Do not inspect comparison reviews or
join their grades until the prompt, thresholds, input construction, and
combination rule are committed.

The 32-task comparison partition was inspected during earlier report-only
experiments. It is held out from this review's calibration, but is reused
validation, not a never-observed test set. Report that limitation prominently.
Do not use comparison results to tune this version. A separate prospective
Microluna confirmation must freeze task groups and the complete procedure before
running new candidates; previously studied task groups remain development data.

## Measurements and continuation

Compare failure precision and failure recall with the same candidates' final
scenario checks and original report verdict. Report exact counts, Wilson 95%
intervals, per-task results, and paired whole-task bootstrap differences (10,000
resamples, seed 9584). Count unknowns in the failure-recall denominator. Report
false alarms on correct candidates and coverage by available artifact type.
Never infer a precision improvement against a baseline with no failure calls.

Retain failures and unavailable reviews. Include total known costs and disclose
unknown costs. Bound each Luna request to 180 seconds and each Jev request to its
existing 60-second budget. Run at most four reviews concurrently. Preserve other
coderos worktrees and running jobs. This first protocol is an experiment, not a
new runtime default or grounds to close #9584 by itself.

## Calibration adapter correction

The first calibration batch often enclosed an otherwise exact requirement in
curly quotation marks. Before looking at comparison reviews, allow surrounding
quotation marks and whitespace normalization in citation matching, with a
minimum eight-character quote. This changes no requirement text or finding.
Replay the retained Luna replies through the corrected matcher and Jev; do not
regenerate reviews. Keep both versions and account for the repeated Jev calls.

## Prospective Microluna cohort

Freeze eight CPU task groups outside all 58 original task groups:
`distributed-dedup`, `formal-crypto`, `freecad-impeller`,
`freecad-spring-clip`, `math-eval-grader`, `pretrain-shard-corruption`,
`shadow-relay`, and `vpp-loss-divergence`. Choose them by text artifact
availability and CPU execution, not by correctness labels. Other new task groups
require a specific GPU, supply only binary/visual artifacts to this text review,
or depend primarily on service state. This is a selected text-review population,
not an estimate over the whole benchmark.

Run one fresh candidate per task with the retained v13 binary
`5e9aa12daf74` (`sha256:7df7cde47d0f16c8f9200cbd21899254588c0b587f3bbe6f2272fd2eedfb0f5d`)
and `prospective-policy.json`: unchanged Microluna generation and selection,
with final scenario checks, original report verdict, and bounded snapshots added.
No repair, second executor, or response to the official verifier is enabled.
Run at most two trials concurrently, retaining all setup failures separately.
Do not inspect official labels until the complete review rule is frozen and
reviews are retained. The other agent's previously announced shadow-relay failure
is known; this task is new to the label fit but not globally unobserved. Report it
separately from the seven other groups.

## Stronger-reviewer development comparison

Interim calibration results reject static Luna review alone: after 68 completed
reviews, the 0.8 cutoff has seven true and seven false failure calls. False
alarms include demanding a general cipher solver where the task asks to decrypt
one supplied ciphertext, arithmetic/string mistakes, and speculative runtime
inputs. Preserve this negative result.

Before opening comparison outputs, run the identical packet and native review
schema through `gpt-6-astra` at high effort on calibration only. This is an
explicitly more expensive verifier for a cheap executor, not a Luna-only
configuration. Bound each call to 180 seconds and record its list-price usage.
Use the same threshold-selection rule. No model is promoted without reporting
its full cost and comparing its own held-out behavior after calibration freezes.

## Scope checks on calibration

The full Luna calibration has 15 true and seven false failure calls at 0.8;
none of the five thresholds qualifies. Record that result before trying a
second scoring variant. Keep the Luna proposals fixed and add two narrow Jev
questions: whether the counterexample belongs to the task's actual input domain,
and whether the claimed wrong result follows from supplied observations without
an unsupported premise. The score becomes the minimum of all four probabilities.
Missing answers remain unknown. Measure this separately; choose no threshold or
model from comparison results. The Astra development arm uses the original
questions so its model comparison remains interpretable.

## Concrete-report development component

Static-review false alarms also motivate a cheaper signal: distinguish an
explicit, unresolved failure of a mandatory task requirement from uncertainty
about whether a grader might accept it. `report_audit` asks those three conditions
separately over the selected report and public task. Its score is their minimum;
missing answers are unknown. Evaluate the same five cutoffs and the same 90%
precision/minimum-five rule on calibration only. It does not ask the model to
certify success. Keep its results separate from independent source review, and
freeze any combination before comparison. This is a new question set, not a
post-hoc relabeling of the original report verdict.
