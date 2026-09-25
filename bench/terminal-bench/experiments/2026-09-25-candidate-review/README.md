# Candidate review validation records

See the [assessment](../../../../docs/terminal-bench/2026-09-25-candidate-review-validation.md).
This study preserves rejected alternatives. No new rule is a runtime default.
All 317 historical rows are now development data. All 16 fresh predictions were
sealed before outcomes were opened; later rule changes must treat those outcomes
as development evidence too. Luna passed 0/8 and Astra 4/8. Executable review
detected 2/12 failures with 2/2 correct calls, insufficient to beat existing checks.

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
the compressed bundle too. The published bundle contains 7,161 files, excludes
the separately stored trace archives, and passed an exact local credential scan.
Its SHA-256 is
`7127e21810bfdab1e6629ff9154c14522a3d30f7ef65cd56716fe7ff85873b10`.

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

Repeat the command with `records/prospective-astra-trace-files.json` and
`records/prospective-astra-traces.tar.gz` for all eight Astra controls: 1,159 files
in 551 blobs. The archive SHA-256 is
`9e5c09eebf6d7f4d87d88585d65e42434968a03818099b48c10b416bb93d7f44`.
The original infrastructure-failed VPP attempt is retained in the research
records; the trace bundle contains the successful prespecified retry.

## Reproduce the fresh measurement

Extract `records/retained.tar.gz` into a new directory and verify it as above.
The measurement reads sealed calls and grades only, without inference:

```sh
python3 measure_prospective.py \
  --predictions records/prospective-all-sealed.json \
  --labels records/prospective-official-labels.json \
  --regrades "$RECORDS/cad-regrade-v2" --out /tmp/prospective-measurement.json
```

Omit `--regrades` to reproduce the initial 12 graded rows and four unknowns.
The recovered grades retain their original unknown result identities. The
`cad-regrade` directory retains the first unsuccessful dependency repair, while
`cad-regrade-v2` contains the successful setup repair, unchanged candidate hashes,
unchanged verifier assertions, dependency-file diffs, and complete grader details.
Do not rerun candidates to reproduce these measurements.

`records/prospective-executor-costs.json` and
`records/prospective-astra-executor-costs.json` retain executor accounting.
`records/costs.json` covers the check-research sequence separately, including
unknown usage. Do not add a copied native response more than once.


## Later protocols and fitted rules

- `agreement-protocol.md` and `frozen-agreement-rule.json`: source/readiness agreement.
- `observed-union-protocol.md` and `frozen-observed-rule.json`: selected execution evidence.
- `feature-fit-protocol.md` and `frozen-fusion-model.json`: original calibration fit.
- `feature-fit-protocol.md` (optional-review addendum): exact bounds that skip irrelevant readiness calls.
- `pooled-development-protocol.md` and `frozen-pooled-model.json`: historical data
  retired as validation, with fresh outcomes reserved.
- `reproduced-review-protocol.md`: bounded execution against the actual candidate.
- `strong-controls-protocol.md`: Astra controls and the one infrastructure retry.

`pooled_fit.py --manifest MANIFEST --records RECORDS --partition prospective
--model frozen-pooled-model.json --out PREDICTIONS` applies the frozen model
without labels. It requires NumPy (the retained fit used 2.0.2). Only an explicit
labeled development invocation fits weights. `compare_predictions.py` joins
sealed calls to official outcomes later; unknown rewards stay out of labeled
counts and remain listed. `fusion_bounds.py` applies the earlier full model using
exact ranges for unrequested readiness answers.

`reproduce.py --manifest MANIFEST --jobs JOBS --out RECORDS --binary CODER_ONE`
restores attributable snapshots, builds only public task environments, runs the
review with two workers, and removes its containers. It reads no official grade.
Its inputs and image identities, native replies, commands, judgment answers,
costs, and cleanup results remain under each trial's `reproduced/` directory.
Run `python3 -m unittest test_reproduce test_fusion_bounds` for the evidence
restoration and inference-bounds regression checks.
