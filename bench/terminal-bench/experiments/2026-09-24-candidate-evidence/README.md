# Microluna candidate-preservation experiment

This experiment tests the assessment in
[Microluna's two targets](../../../../docs/terminal-bench/2026-09-24-microluna-two-targets.md).
The target tasks are selected development tasks, not held-out evaluations.
The experiment ID on coderos is `candidate-evidence-9607`.
All 12 attempts are complete. See the [result analysis](../../../../docs/terminal-bench/2026-09-24-microluna-candidate-evidence.md),
[machine-readable results](records/results.json), [scheduler status](records/status.json),
[interrupted-attempt inventory](records/interruptions.json), and
[implementation verification](records/verification/README.md).

## Fixed comparison

[Protocol](protocol.json) and [pins](records/pins.json) record the exact
binary, task revision, policy digests, and planned attempts. Both arms use
`coder-one` at `a45cb547a2bb43886b63052dc9917ffe17b2e442`, Luna high,
Microluna's native tools, and identical session and spending bounds.
`microluna-evidence-v1` adds retained candidate selection and an observational
review to v12. Common score-validation fixes are present in both arms.
The later v13/v14 merges do not change this binary or either arm.

There are three attempts per arm on each task, 12 planned attempts total.
Two trials can run at once. The planned total model-spend ceiling is $3;
individual policy bounds can overshoot by one in-flight call, so the total
is checked before further admission. All costs are token-usage valuations,
including Jev, rather than measured subscription invoices.

The [Fable reference](records/fable-reference.json) retains the 50 relevant
public attempt records, their source links, and the source manifest digest.
Fable failed all 25 `session-window-debug` attempts and passed all 25
`embedding-drift-monitor` attempts. Its five low-effort embedding attempts
averaged $0.8691 and 186.5 seconds of trial wall time. Cross-host timing and
different harnesses limit any claim about speed.

## Reproduce the analysis

Run these scripts with the pinned checkout's `bench/terminal-bench` Python
environment, which supplies `tbench` and Harbor 0.22.0:

```sh
python /path/to/collect.py --output /tmp/candidate-results.json
python /path/to/grade_candidates.py --output /tmp/candidate-grades
```

Both default to `~/.openagents/terminal-bench` and accept `--state-root`.
Collection preserves pending rows and unknown costs. Candidate grading runs
only after a trial finishes, uses the retained candidate directory as the
recorded workspace, and saves the verifier output beside each `grade.json`.
It never sends that result to a model. An existing grade is not overwritten.

Use `tbench retain JOB --traces-dir DIR` to retain complete trial evidence,
including the native Microluna streams, briefings, usage, generated evaluator,
candidate snapshots, final selection, and official verifier result. The
retention manifest records file digests, missing references, bounds, and the
credential scan. A scalar local score does not substitute for that evidence.

The v12 control preserves its original policy behavior, which deletes its
temporary evaluator and intermediate candidate copies. Its full native
traces, usage, final artifacts, and official result are retained, but its
intermediate candidate oracle is unavailable. The post-run candidate grades
therefore measure oracle headroom for the treatment only. A retention record
with no missing referenced files does not establish that v12 recorded those
unreferenced temporary directories.

## Startup incidents

The first launch at 21:08 UTC on September 24 encountered Harbor's Docker
network-capability failure. The required allowlist was not relaxed. The
experiment was stopped, the actual kernel probe was rerun successfully, and
the unchanged experiment resumed at 21:16 UTC with the egress-control sidecar
running. Four archived startup directories contain only a lock, network
record, and trial log. Two other archived attempts reached the agent before
cancellation and record $0.001733508 of Jev usage in total. Cancelled
in-flight Luna requests might not return usage, so that recorded interrupted
cost is a lower bound. These interruptions are not completed verifier trials
and are reported separately, with their traces and scheduler logs, rather
than silently counted as free retries.
