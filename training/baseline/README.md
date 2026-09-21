# The cheap baseline

Frozen sentence embeddings plus multinomial logistic regression, scored on the
same suite and the same metric panel as every door in this repository.

This started as a measurement harness and is now also a door. `baseline.py`
answers one question offline: on `support-v2`, does the cheapest supervised
method beat the decision models we built? The answer, and what it implies,
live in
[`docs/decision-models/measurements/2026-09-19-frozen-embedding-baseline.md`](../../docs/decision-models/measurements/2026-09-19-frozen-embedding-baseline.md).
`door.py` serves the same head as a TypeSafe-compatible `POST /v1/systemone`
server so the Gym scores it through the store like every other door; that run
is
[`docs/decision-models/measurements/2026-09-20-frozen-embedding-door.md`](../../docs/decision-models/measurements/2026-09-20-frozen-embedding-door.md).

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
uv pip install --python .venv/bin/python -r requirements.lock
./run.sh
.venv/bin/python compare.py
```

`requirements.lock` pins every package, with the CPU-only `torch` wheel. Nothing
here needs a GPU.

## Serving it as a door

```sh
.venv/bin/python door.py \
  --suite ../../crates/gym/suites/support-v2-three-way.json \
  --encoder BAAI/bge-base-en-v1.5 \
  --revision a5beb1e3e68b9ab74eb54cfd186867f64f240e1a \
  --port 8020
```

The door fits one head per Choice family on the suite's `calibration`
partition at startup, then serves `GET /v1/models`, `GET /health`, and
`POST /v1/systemone`. A Noul or a Score request, and a Choice whose options
are not the fitted set, get HTTP 422 with the typed refusal envelope the Rust
client reads (`error.code`), so the Gym records them as door refusals rather
than harness failures. Score it with the unchanged Gym:

```sh
cargo run -p gym --bin gym -- eval --door baseline-bge=http://127.0.0.1:8020 \
  --suite crates/gym/suites/support-v2-three-way.json --partition development \
  --record crates/gym/results/support-v2-three-way.jsonl --timeout 120
```

`run.sh` produces every row in the record: two suites, four featurizers, both
strength-selection rules, with the encoder revisions pinned in the script so a
rerun is the same run. Records land in `runs/`, which is tracked, so a reader
can check the document's numbers without owning the encoders.

## The files

| File | Holds |
| --- | --- |
| `baseline.py` | the measurement: fit, score, refuse, and write the record |
| `door.py` | the same head behind `POST /v1/systemone`, for the Gym to score through the store |
| `requirements.lock` | every Python package pinned, CPU-only |
| `panel.py` | accuracy, ECE, Brier, log loss, and confident errors, ported from `crates/gym/src/calibrate.rs` |
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
