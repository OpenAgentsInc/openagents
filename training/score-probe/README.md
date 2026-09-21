# score-probe

Does a Score's ordering mean anything?

A Score answer is `score = Σ i · p_i`, a probability-weighted position on an
ordered rubric. That number carries information only if level 2 really is
further along one axis than level 1. Nothing in the contract enforces it, and
a model trained with independent per-level loss has no mechanism that would.
This probe asks each door the same Score questions and measures three things
that separate an ordered readout from an unordered one.

The record it produced is
[`docs/decision-models/measurements/2026-09-19-score-ordinality.md`](../../docs/decision-models/measurements/2026-09-19-score-ordinality.md).

## Why this is Python

`training/` holds harnesses that are not Rust, one exception per reason. This
one is a probe against running doors over HTTP: it changes no crate, links no
crate, and its outputs are read by a document rather than by a binary. Putting
it in Rust would mean a new workspace member for a measurement that talks
only JSON. If a statistic here earns a place in a gate, it belongs in
`crates/gym`, in Rust, not here.

## What it measures

Monotonicity under a known ramp
: `ramps.json` holds twelve severity ramps authored here. Each ramp fixes a
  subject — the export button, mobile sync, the invoice PDF — and walks the
  true level from 0 to 4, changing only the clause that carries the severity.
  Within a ramp, every pair of levels asks whether the reported score is
  greater at the higher level. `tau` is 1.0 for a perfect walk and 0.0 for a
  coin toss.

Adjacent versus distant confusion
: When the argmax level is wrong, an ordered model errs to a neighbor. The
  probe reports the adjacent fraction and the mean distance beside the null
  that spreads the error uniformly over the other levels, computed from the
  true levels the items actually carry. Beating that null is the evidence;
  sitting on it is the absence of evidence.

Bimodality
: A distribution with mass at both ends and a trough in the middle is one
  where the weighted mean reports a level no part of the distribution
  supports. The probe counts those, and separately counts the stronger case:
  the level the score rounds to carries less probability than some level
  below it and less than some level above it.

The exact thresholds are stated at the top of `analyze.py` and were fixed
before the doors were called.

## Items

| Family | Items | Levels | Source |
| --- | --- | --- | --- |
| `ramp` | 60 | 5 | `ramps.json`, authored here |
| `severity` | 36 | 3 | the `severity` family of `crates/lev/suites/support-v2.json` |

The severity family is included because it is the rubric this repository
already publishes Score-item numbers against. It is not a ramp: its items are
independent judgments, so it measures confusion and bimodality but not
monotonicity.

## Running it

Start the doors you want to reach:

```sh
./target/release/kev-serve --bundle-dir ~/work/kev-artifacts --port 8009
./scripts/build-lev-bridge.sh
cargo +1.95.0 run --release -p lev --features serve --bin lev-serve -- --port 11436
```

Then probe and analyze:

```sh
python3 probe.py --list
python3 probe.py --door kev-0.5b
python3 probe.py --door jev --env-file ~/.secrets/typesafe.env
python3 analyze.py
python3 analyze.py --door kev-0.5b --verbose
```

`probe.py` writes one JSONL row per item to `results/<door>.jsonl`, including
the full distribution, so the analysis can be rerun without calling a door
again. Those files are committed: the record cites them, and a reader should
be able to check a count without paying for the hosted door.

The hosted door is metered. Read its key from `TYPESAFE_API_KEY` or from a
`KEY=value` file outside the repository. No key is read from a tracked file
and none is printed.

Only `python3` and the standard library are needed. There is no dependency to
install.
