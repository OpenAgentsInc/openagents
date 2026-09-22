# The tenant-training demo

A scratch rehearsal of the whole contract `tenancy::training` enforces —
corpus check, headroom, recipe freeze, the trials ledger, the candidate
seal, and the retention delete — over a small synthetic corpus, without
touching the locked partition and without paid compute.

```sh
cargo build -p tenancy --bin tenant-train
sh training/tenant-demo/run.sh /tmp/tenant-train-demo target/debug/tenant-train
```

## What the demo proves and what it does not

It proves the flow: an eligible corpus registers, a leaked corpus and an
undocumented one are refused at `check`, the baseline's development
failures are bucketed before a recipe may freeze, a frozen recipe's
digest is what every trial pins, a foreign seed is refused, the kept
trial's artifacts are what the candidate seal binds, and `delete`
tombstones the corpus while its digest still resolves.

It does not prove a model improves. `make_artifacts.py` emits
deterministic, honestly-labeled placeholder artifacts so the
identity-and-ledger path can be rehearsed; real adapter training runs
through `training/lev-adapter` (Apple's toolkit) or the kev pipeline,
and a sealed candidate still has to win admission through
`tenancy::admission` before any registry binds it.

## Files

- `corpus-eligible.json` — a four-partition corpus: 8 training, 4
  calibration, 6 development, 4 locked items, each with provenance and
  a per-item label rule. Development items carry the reviewer
  annotations the headroom report reads.
- `corpus-leaked.json` — refused: an exact duplicate across
  training/development inside one group.
- `corpus-undocumented.json` — refused: an item with no license.
- `baseline-scores.json` — the frozen baseline's development scores: 5
  failures, 3 of them model-addressable, so the verdict is `train`.
- `recipe.json` — the bounded recipe: seed policy, trial cap, compute
  budget, winning metric, rejection rules, transfer controls.
- `make_artifacts.py` — the deterministic demo trainer; writes valid
  safetensors from a seed so a reader can byte-for-byte reproduce the
  digests.
- `run.sh` — the end-to-end rehearsal.
