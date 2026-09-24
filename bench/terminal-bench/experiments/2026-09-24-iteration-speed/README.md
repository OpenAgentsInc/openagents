# Shorten the Microluna measurement loop

This selected development experiment addresses issues #9592, #9618, and #9619.
It does not change the ongoing v15/v16 experiments or the issue-to-PR agent.

Before launching, freeze a binary and record its source, artifact hash, policy
hash, and TB4 task revision in `records/pins.json`. Run the published v13 policy
with `retain_candidates: true` on `embedding-drift-monitor` and
`session-window-debug`, three attempts each. Run at most two trials concurrently,
with an admission limit of 8 CPUs, 32 GiB RAM, and 60 GiB free disk. Keep the
existing per-dispatch $0.09 bound. Inspect accumulated usage before admitting
another attempt if total spend reaches $1; an in-flight call can overshoot.

The treatment only records sequential candidates. It preserves v13's editing
review, tied-score replacement, stopping rules, and evaluator calls. Compare to
historical v12 and v13 results descriptively; this is not a fresh randomized
policy comparison. Fable's public attempts are external references with different
models, hosts, and harnesses. Include failures in cost per accepted output.

After all agent attempts finish, grade every retained candidate with the official
verifier. Do not feed these grades into any live session. Report first-candidate,
final-candidate, and oracle outcomes separately. A missing or inconsistent
snapshot is unknown, not a failure or a pass. Record snapshot-copy milliseconds.

Measure grader throughput using the previous experiment's six evidence-v1
trials (12 candidates): first one worker without reuse, then two workers with
explicit within-batch deduplication. Retain both outputs, invocation counts,
wall times, and infrastructure failures. The fixed order and host contention
limit the timing comparison; a reused deterministic grade is not an independent
repeat. No persistent grade cache is introduced.

For reasoning summaries, use a small cache-review prompt unrelated to TB4.
Measure `auto`, `concise`, and `detailed` in three balanced orders at high effort,
then confirm acceptance at the provider's default effort. Retain readable
summaries and exact usage, never login headers or encrypted reasoning. The
initial parser missed streamed output items; preserve that invalid measurement
and its spend separately. Do not interpret its zero-character counts as missing
provider summaries.

## Retained measurements

- [Assessment and implementation](../../../../docs/terminal-bench/2026-09-24-microluna-iteration-speed.md).
- [Summary-setting ledger](records/summary-comparison.json), including invalid
  probe costs and the corrected streamed-output parser.
- [Grading throughput comparison](records/grading-comparison.json): 12 official
  grades per condition, identical outcomes, 19.4% less wall time with two workers.
- [Binary and policy pins](records/pins.json). The six fresh trials run through
  the single-arm suite `tb4--coder-one-microluna-v13-retained`. The initial
  experiment command refused one arm before launching any trial; its
  [refusal record](records/initial-scheduler-refusal.log) remains retained.

Use `collect.py --output /tmp/results.json` with the pinned tbench environment
on coderos to collect all planned attempts, including pending rows and unknown
costs. `run.py` records credentials only in process memory and pins the original
artifact before invoking the suite. Run it from the named frozen checkout;
a later source revision requires a new experiment and artifact identity.

The six attempts are complete: [results](records/results.json),
[analysis](records/analysis.json), [final scheduler status](records/status.json),
and [retention inventory](records/retention-summary.json). The
[fresh candidate batch](records/grading-fresh/batch.json) covers all 12 snapshots
with eight verifier executions and four attributed reuses. Its credential scan
and each trial's retention scan report no known credential matches.

`finish.py` waits for this exact six-attempt suite, retains every trial, and
starts post-run grading only after all attempts finish. `analyze.py RECORDS`
checks the complete grades and produces `analysis.json`. Neither script starts
new model sessions. The full trial bundles live under `../../traces/` by
job and trial name; each result table links its retention manifest.

The [publication check](records/publication-check.json) validates the staged Git
blobs against the retention manifests for this batch and the prior candidate
evidence batch. It includes retained Python bytecode, which Git normally ignores.
The [verification records](records/verification/README.md) retain failed and
corrected gates, the final merged integration gate, and the Python test output.
