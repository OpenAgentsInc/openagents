# The cheap baseline

Frozen sentence embeddings plus multinomial logistic regression, scored on the
same suite and the same metric panel as every door in this repository.

This is a measurement harness, not a door. It answers one question: on
`support-v2`, does the cheapest supervised method beat the decision models we
built? The answer, and what it implies, live in
[`docs/decision-models/2026-09-19-frozen-embedding-baseline.md`](../../docs/decision-models/2026-09-19-frozen-embedding-baseline.md).

## Why this is Python

`../README.md` allows a non-Rust tree when there is a reason rather than a
convenience. The reason here is that the frozen encoders this baseline exists
to measure are distributed as `sentence-transformers` packages with no Rust
loader, and the point of a baseline is to run the thing everybody else would
actually reach for. Reimplementing the encoder in Rust would measure our
reimplementation.

Nothing in this directory is a runtime dependency of any crate. It produces
JSON records and a document; that is the whole contract.

## Running it

```sh
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python numpy scikit-learn sentence-transformers
./run.sh
.venv/bin/python compare.py
```

`run.sh` produces every row in the record: two suites, four featurizers, both
strength-selection rules, with the encoder revisions pinned in the script so a
rerun is the same run. Records land in `runs/`, which is tracked, so a reader
can check the document's numbers without owning the encoders.

## The files

| File | Holds |
| --- | --- |
| `baseline.py` | the measurement: fit, score, refuse, and write the record |
| `panel.py` | accuracy, ECE, Brier, log loss, and confident errors, ported from `crates/lev/src/calibrate.rs` |
| `check_panel.py` | proves that port scores the way the Rust does, on the Rust tests' own fixtures |
| `compare.py` | the baseline against the doors, judged against the measured noise floor |
| `run.sh` | every run in the record, with revisions pinned |

## The rules it keeps

**Fit and score never touch the same items.** The head fits on the
`calibration` partition. The L2 strength is chosen by cross-validation inside
that same partition, so the scoring partition never votes on anything. The
`locked` partition of `support-v2-three-way` is not read at all.

**Choice only, and the other two primitives are refused in the record.** A Noul
is a probability that a statement holds, and a Score is a weighted position on
an ordered rubric. A multinomial classifier returns neither, so it says so,
with a typed code and a reason, counted separately from harness failures. A
door that satisfies the type and violates the meaning is worse than no door.

**Both selection rules are reported.** On forty items the cross-validated loss
is nearly flat, and `argmin` and the one-standard-error rule disagree by enough
to change the calibration panel. Choosing between them after seeing the numbers
would be the finding, so the harness runs both every time.

**Every encoder carries its revision.** `--revision` is required for a
`sentence-transformers` encoder, because an unpinned encoder is an
unreproducible run, and the record shows the encoder choice moving accuracy by
more than the suite's own noise floor.
