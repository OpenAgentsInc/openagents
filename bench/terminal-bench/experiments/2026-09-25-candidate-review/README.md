# Candidate review validation records

See the [assessment](../../../../docs/terminal-bench/2026-09-25-candidate-review-validation.md).
This study preserves rejected alternatives. None of the source-review rules is a
runtime default. The completion-assessment experiment is separate.

- `protocol.md`: source-review development, frozen V1, and adaptive V2.
- `frozen-rule.json`, `frozen-rule-v2.json`: exact union rules and selection records.
- `public-program-protocol.md`: public-only program generation and bounded execution.
- `readiness-protocol.md`: whole-task completion assessment before comparison.
- `prospective-policy.json`: frozen executor configuration for eight fresh tasks.
- `records/*.json`: predictions, counts, intervals, and rejected alternatives.

## Reproduce a measured feature

Use an extracted retained-record directory as `RECORDS`. The manifest's historical
host paths are provenance; `--records` explicitly relocates the inputs.

```sh
python3 measure.py --manifest "$RECORDS/manifest.json" \
  --records "$RECORDS" --rows ../../../../crates/coder-one/fixtures/truth/rows.jsonl \
  --baseline ../2026-09-25-truthful-checks/records/historical/summary.json \
  --partition held-out --threshold 0.9 --record astra --out /tmp/source-review.json
python3 combine.py --reviews /tmp/source-review.json \
  --audits "$RECORDS/report-audit-held-out" --rule frozen-rule.json \
  --out /tmp/source-union.json
python3 evaluate_rule.py --scores /tmp/source-union.json \
  --rule frozen-rule-v2.json --out /tmp/source-union-v2.json
```

The full public-program calibration is reproduced with `measure_feature.py` using
`--feature public-program` or `--feature public-program-bounded`. Readiness uses
`--feature readiness`. A comparison run must supply its frozen `--threshold`.
The command reads retained observations; it makes no model calls.

`replay_component.py --manifest "$RECORDS/manifest.json" --records "$RECORDS"
--binary /path/to/coder-one --out /tmp/component-replay` reproduces V1 through
Rust with Jev off and no credentials. `costs.py RECORDS OUTPUT` deduplicates native
responses by provider reply ID, while charging each recorded live Jev request.
The ledger reports unknown usage explicitly. Benchmark executor costs are separate.

`archive_records.py verify --root RECORDS --manifest records/retained-files.json`
verifies every extracted file. Add `--archive records/retained.tar.gz` to verify
the compressed bundle too. These files are added when the run is sealed.

## Run new inference

`prepare.py` reads only public instructions and final artifact manifests.
`run.py` performs source review; `readiness.py` combines those inputs with selected
reports from `report_audit`. Each record retains the exact native request and reply.
Use new output directories when changing a prompt or input. Cached outputs are
observations, not permission to relabel an earlier run.

`prepare_public.py` reads public environment files before candidates.
`public_programs.py generate` writes programs, `admit` rejudges the same program
with bounded state, and `run` executes programs in networkless Docker containers.
The generated program is never imported on the host. For a changed admission,
identical program/input executions are reused and the record says so.

## Restore the fresh transcripts and candidates

The separate `records/prospective-traces.tar.gz` contains content-addressed blobs
for every fresh trial's agent and artifact files, plus its configurations.
`records/prospective-trace-files.json` maps the original relative paths to each
blob's size and SHA-256. It contains no official grading files. Restore into a
new directory:

```sh
python3 restore_traces.py --manifest records/prospective-trace-files.json \
  --archive records/prospective-traces.tar.gz --out /tmp/truthful-check-traces
```

The reader verifies all blobs and paths before writing files. It never overwrites
an existing output directory. This restores the complete native transcripts,
ATIF logs, reports, candidate snapshots, and collected deliverables. The initial
bundle has 1,077 files in 533 unique blobs and was restored and hash-checked in
full. `collect_fresh.py` reproduces collection from the frozen prospective
manifest, scanning the current host's credential values before publication.
