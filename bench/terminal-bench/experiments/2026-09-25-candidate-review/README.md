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

## Mini controls and grader repairs

`mini-review-protocol.md` preserves the original eight controls, the literal
citation prompt's eight controls, and the two-candidate cancellation follow-up.
The original labels remain unchanged even where the review exposed a real
fixture defect. See the assessment's mini-control section for #9641.

Extract `records/truth9584-mini-records.tar.gz` into a new directory, then verify
and measure it without inference:

```sh
python3 archive_records.py verify --root "$MINI_RECORDS" \
  --manifest records/mini-files.json --archive records/truth9584-mini-records.tar.gz
python3 measure_mini.py --root "$MINI_RECORDS" --out /tmp/mini-measurement.json
python3 costs.py "$MINI_RECORDS" /tmp/mini-costs.json
```

The retained measurement and deduplicated costs are `records/mini-measurement.json`
and `records/mini-costs.json`. Re-running `mini_review.py` makes new paid model
calls and new evidence; it is not how to reproduce this table.

## New archive confirmation

The [frozen protocol](archive-confirmation-protocol.md) declares 72 trials on
12 previously unused archived task groups. The candidate policies and check rule
are fixed before generation. Its population is broader than TB4 and cannot
establish a Fable comparison.

`records/truth9584-archive-preflight.tar.gz` holds the 151-file environment and
plan preflight, including both corrected setup failures, immutable image IDs,
original/staged task hashes, exact job configs, and checker runtime hashes.
`records/archive-preflight-files.json` verifies it. Archive SHA-256:
`ae8f26f624284e8e77ba9113e0c8958d3774d7041801de6c2b1883f57abb13fe`.
All files were restored and verified, with no exact credential matches.

`archive_run.py --prepare-only` stages the jobs without inference; omit that flag
only to start new paid attempts. `archive_checks.py --plans-only` extracts and
executes the public file plans on pristine images. Use `--completed-only` to
inspect finished attempts while later candidates run, without opening outcomes. The latter
requires `--runtime` naming the four-library checker runtime manifest. Keep
outcomes unopened until all check and baseline predictions have been sealed.


The first launch stopped in setup before any agent or verifier ran. The protocol
records a separate `--run-id r2` restart with the same frozen rule. Verify the
959-file setup, guard, replay, and gate archive with `archive_records.py` and
`records/truth9642-files.json`. No original attempt is erased.

`archive_pipeline.py` runs the frozen checks on completed attempts with two
review workers. Once all 72 attempts have a check record, seal their calls:

```sh
python3 seal_archive.py --checks "$COHORT/checks" --jobs "$JOBS" \
  --out records/archive-sealed.json
```

Commit and push that exact file. Then use its commit and repository path to open
official outcomes. The join checks the published seal and every evidence digest:

```sh
python3 join_archive.py --predictions records/archive-sealed.json \
  --seal-commit "$SEAL_COMMIT" \
  --seal-path bench/terminal-bench/experiments/2026-09-25-candidate-review/records/archive-sealed.json \
  --jobs "$JOBS" --checks "$COHORT/checks" --out records/archive-labels.json
python3 measure_archive.py --predictions records/archive-sealed.json \
  --labels records/archive-labels.json --out records/archive-measurement.json
python3 archive_executor_costs.py --manifest "$COHORT/checks/manifest.json" \
  --jobs "$JOBS" --out records/archive-executor-costs.json
python3 costs.py "$COHORT/checks" records/archive-review-costs.json
python3 archive_resources.py --labels records/archive-labels.json \
  --executor-costs records/archive-executor-costs.json --out records/archive-resources.json
```

These measurement and accounting commands make no model calls. `test_archive_seal.py` checks cohort
completeness, duplicate and mismatched joins, unknown-evidence recall, and sealing
without parsing grades. Run it with `test_archive_checks` and `test_reproduce`.
