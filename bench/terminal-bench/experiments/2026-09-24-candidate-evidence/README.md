# Microluna candidate-preservation experiment

This experiment tests the assessment in
[Microluna's two targets](../../../../docs/terminal-bench/2026-09-24-microluna-two-targets.md).
The target tasks are selected development tasks, not held-out evaluations.
The experiment ID on coderos is `candidate-evidence-9607`.

## Fixed comparison

[Protocol](protocol.json) and [pins](records/pins.json) record the exact
binary, task revision, policy digests, and planned attempts. Both arms use
`coder-one` at `a45cb547a2bb43886b63052dc9917ffe17b2e442`, Luna high,
Microluna's native tools, and identical session and spending bounds.
`microluna-evidence-v1` adds retained candidate selection and an observational
review to v12. Common score-validation fixes are present in both arms.
The later v13 merge does not change this binary or either arm.

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

## Startup incidents

The first launch at 21:08 UTC on September 24 failed during Harbor's Docker
network-capability check. The required allowlist was not relaxed. The
experiment was stopped, the actual kernel probe was rerun successfully, and
the unchanged experiment resumed at 21:16 UTC with the egress-control sidecar
running. Startup failures and interrupted setup attempts are infrastructure
incidents, not verifier failures. The scheduler log is retained with the
final measurement records.
